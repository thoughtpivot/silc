use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn tempdir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("silc-pipeline-e2e-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn silc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_silc"))
}

#[test]
#[ignore = "downloads pinned runtimes, MiniLM ONNX artifacts, and tensor wheels"]
fn local_http_to_normalized_embedding_to_sqlite() {
    let root = tempdir();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data_pipeline_runnable.silc");
    let entry = root.join("main.silc");
    fs::copy(fixture, &entry).unwrap();

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        let body =
            "<html><body><h1>Silc deterministic fixture</h1><p>Pipeline embedding text.</p></body></html>";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });

    let input = format!(r#"{{"url":"http://127.0.0.1:{port}/article"}}"#);
    let run = silc()
        .arg("run")
        .arg(&entry)
        .arg("--input-json")
        .arg(input)
        .output()
        .expect("run pipeline");
    assert!(
        run.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    server.join().unwrap();
    assert!(String::from_utf8_lossy(&run.stdout).contains("\"ok\":true"));

    let db = root.join(".runtime/main/data/app.db");
    assert!(db.is_file(), "missing generated SQLite database");
    let lock: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(".silc/runtimes.lock.json")).unwrap())
            .unwrap();
    let python = lock["python_bin"].as_str().unwrap();
    // Synthesized sink table for contract ArticlePayload is `article_payloads`
    // (see sil_core::sink_table_for_contract / infer_graph.sqlite_table).
    let query = Command::new(python)
        .arg("-c")
        .arg(
            "import sqlite3,sys; print(sqlite3.connect(sys.argv[1]).execute('select data from article_payloads limit 1').fetchone()[0])",
        )
        .arg(&db)
        .output()
        .unwrap();
    assert!(
        query.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&query.stdout),
        String::from_utf8_lossy(&query.stderr)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(String::from_utf8_lossy(&query.stdout).trim().as_bytes()).unwrap();
    assert!(payload["raw_content"]
        .as_str()
        .unwrap()
        .contains("Silc deterministic fixture"));
    let embedding = payload["vector_embedding"].as_array().unwrap();
    assert_eq!(embedding.len(), 384);
    let norm = embedding
        .iter()
        .map(|value| value.as_f64().unwrap().powi(2))
        .sum::<f64>()
        .sqrt();
    assert!((norm - 1.0).abs() < 1e-4, "embedding norm was {norm}");

    let _ = fs::remove_dir_all(root);
}

const SITE_PAGE: &str = "<html><head><title>httpx 2.4.0</title></head><body><nav>menu</nav>\
<article><h2>2.4.0</h2><p>TLS: the default certificate bundle changed; clients that pin \
the old chain fail the handshake.</p></article></body></html>";

/// Serves [`SITE_PAGE`] for every request until the test process exits.
fn serve_site_page() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                SITE_PAGE.len(),
                SITE_PAGE
            );
        }
    });
    port
}

fn free_port(port: u16) {
    let _ = Command::new("bash")
        .args([
            "-c",
            &format!("lsof -ti:{port} | xargs kill -9 2>/dev/null || true"),
        ])
        .status();
    thread::sleep(Duration::from_millis(200));
}

struct SiteLoop {
    root: PathBuf,
    port: u16,
    child: Child,
    log: PathBuf,
}

impl Drop for SiteLoop {
    fn drop(&mut self) {
        unsafe {
            libc::kill(self.child.id() as libc::pid_t, libc::SIGTERM);
        }
        let _ = self.child.wait();
        free_port(self.port);
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl SiteLoop {
    /// Builds a site fixture, checks its cost report, and serves it.
    fn start(fixture: &str, port: u16, report: &[&str]) -> SiteLoop {
        free_port(port);
        let root = tempdir();
        let entry = root.join("main.silc");
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(fixture),
            &entry,
        )
        .unwrap();
        let build = silc()
            .arg("build")
            .arg(&entry)
            .output()
            .expect("silc build");
        let stdout = String::from_utf8_lossy(&build.stdout);
        assert!(
            build.status.success(),
            "stdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&build.stderr)
        );
        for line in report {
            assert!(stdout.contains(line), "missing `{line}` in:\n{stdout}");
        }

        let log = root.join("run.log");
        let out = fs::File::create(&log).unwrap();
        let mut child = silc()
            .arg(&entry)
            .env("SILC_HTTP_PORT", port.to_string())
            .env("SILC_TERMINAL_PORT", "0")
            .env("SILC_MCP_TOKEN", "site-loop-token")
            .stdout(Stdio::from(out.try_clone().unwrap()))
            .stderr(Stdio::from(out))
            .spawn()
            .expect("silc run");
        let ready = Instant::now();
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                panic!("silc exited early: {status}\n{}", read_log(&log));
            }
            let up = ureq::get(&format!("http://127.0.0.1:{port}/health"))
                .call()
                .ok()
                .and_then(|r| r.into_string().ok())
                .is_some_and(|body| body.contains("\"ok\":true"));
            if up && read_log(&log).contains("kernel ready") {
                break;
            }
            assert!(
                ready.elapsed() < Duration::from_secs(600),
                "{fixture} never became ready\n{}",
                read_log(&log)
            );
            thread::sleep(Duration::from_millis(500));
        }
        SiteLoop {
            root,
            port,
            child,
            log,
        }
    }

    fn post(&self, path: &str, body: serde_json::Value) -> serde_json::Value {
        ureq::post(&format!("http://127.0.0.1:{}{path}", self.port))
            .set("content-type", "application/json")
            .send_string(&body.to_string())
            .unwrap_or_else(|e| panic!("POST {path}: {e}"))
            .into_string()
            .map(|text| serde_json::from_str(&text).unwrap())
            .unwrap()
    }

    fn get(&self, path: &str) -> serde_json::Value {
        ureq::get(&format!("http://127.0.0.1:{}{path}", self.port))
            .call()
            .unwrap_or_else(|e| panic!("GET {path}: {e}"))
            .into_string()
            .map(|text| serde_json::from_str(&text).unwrap())
            .unwrap()
    }

    fn mcp(&self, tool: &str) -> serde_json::Value {
        let reply: serde_json::Value = ureq::post(&format!("http://127.0.0.1:{}/mcp", self.port))
            .set("authorization", "Bearer site-loop-token")
            .set("accept", "application/json, text/event-stream")
            .set("content-type", "application/json")
            .send_string(
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools/call",
                    "params": {"name": tool, "arguments": {"requested_by": "ci"}},
                })
                .to_string(),
            )
            .unwrap_or_else(|e| panic!("MCP {tool}: {e}"))
            .into_string()
            .map(|text| serde_json::from_str(&text).unwrap())
            .unwrap();
        reply["result"]["structuredContent"].clone()
    }

    /// Starts the loop once through its MCP run tool and waits for the run.
    fn run_once(&self, tool: &str, timeout: Duration) -> serde_json::Value {
        let request = self.mcp(&format!("{tool}_run"));
        assert_eq!(request["status"], "pending", "{request}");
        let started = Instant::now();
        loop {
            let recent = self.mcp(&format!("{tool}_recent"));
            if let Some(run) = recent["runs"]
                .as_array()
                .and_then(|runs| runs.iter().find(|r| r["status"] == "finished"))
            {
                return serde_json::json!({"run": run, "notices": recent["notices"]});
            }
            assert!(
                started.elapsed() < timeout,
                "{tool} did not finish\n{}",
                read_log(&self.log)
            );
            thread::sleep(Duration::from_secs(2));
        }
    }

    /// The loop event log, one `path kind value` line per event.
    fn events(&self) -> String {
        let lock: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(self.root.join(".silc/runtimes.lock.json")).unwrap(),
        )
        .unwrap();
        let out = Command::new(lock["python_bin"].as_str().unwrap())
            .arg("-c")
            .arg("import sqlite3,sys; [print(*r) for r in sqlite3.connect(sys.argv[1]).execute('select path, kind, value from loop_events order by run_id, seq')]")
            .arg(self.root.join(".runtime/main/data/app.db"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
}

fn read_log(path: &Path) -> String {
    String::from_utf8_lossy(&fs::read(path).unwrap_or_default()).into_owned()
}

/// The site's ArticleIngest loop, run once on the CPython embed worker: a
/// local page is scraped, embedded with minilm-l6-v2, and stored.
#[test]
#[ignore = "downloads pinned runtimes, MiniLM ONNX artifacts, and tensor wheels"]
fn site_article_ingest_runs_on_cpython() {
    let page = serve_site_page();
    let app = SiteLoop::start(
        "article_ingest.silc",
        18641,
        &[
            "ArticleIngest  schedule \"0 7 * * 1-5\" America/Detroit, catch up 4h",
            "embeds 40 (minilm-l6-v2, CPython embed worker; not model calls)",
        ],
    );
    let url = format!("http://127.0.0.1:{page}/news/httpx");
    app.post("/api/sources", serde_json::json!({ "url": url }));

    let done = app.run_once("article_ingest", Duration::from_secs(300));
    assert_eq!(
        done["run"]["outcome"],
        "succeeded",
        "{done}\n{}",
        app.events()
    );
    assert_eq!(
        done["notices"][0]["text"],
        "ArticleIngest embedded 1 sources"
    );

    let articles = app.get("/api/articles");
    let rows = articles.as_array().unwrap();
    assert_eq!(rows.len(), 1, "{articles}");
    assert_eq!(rows[0]["url"], url);
    assert!(rows[0]["raw_content"]
        .as_str()
        .unwrap()
        .contains("default certificate bundle changed"));
    assert_eq!(rows[0]["vector_embedding"].as_array().unwrap().len(), 384);
}

/// The site's UpstreamWatch loop, run once with silclm on CPython: the
/// changelog is read and embedded, the nearest incident is found with
/// sqlite-vec, the model rates the release, and the gate decides.
#[test]
#[ignore = "downloads pinned runtimes, silclm, MiniLM ONNX artifacts, and tensor wheels"]
fn site_upstream_watch_runs_on_cpython() {
    let page = serve_site_page();
    let app = SiteLoop::start(
        "upstream_watch.silc",
        18642,
        &[
            "UpstreamWatch  schedule \"0 7 * * 1-5\" America/Chicago",
            "model calls 120 · effects 41 (writes 40, notices 1) · approvals 0 · reads 120 · scores 0 · rows scanned 240",
            "embeds 40 (minilm-l6-v2, CPython embed worker; not model calls)",
        ],
    );
    app.post(
        "/api/dependencies",
        serde_json::json!({
            "name": "httpx",
            "changelog": format!("http://127.0.0.1:{page}/httpx/CHANGELOG"),
        }),
    );
    app.post(
        "/api/incidents",
        serde_json::json!({
            "summary": "TLS handshake failures after certificate chain change",
            "vector_embedding": vec![0.05f32; 384],
        }),
    );

    let done = app.run_once("upstream_watch", Duration::from_secs(1200));
    let events = app.events();
    assert_eq!(done["run"]["outcome"], "succeeded", "{done}\n{events}");
    assert!(
        done["notices"][0]["text"]
            .as_str()
            .is_some_and(|text| text.starts_with("Watch ran ")),
        "{done}"
    );
    assert!(events.contains("\"version\":\"2.4.0\""), "{events}");
    assert!(
        events.contains("TLS handshake failures after certificate chain change"),
        "nearest find must return the seeded incident:\n{events}"
    );
    assert!(
        events.contains(" ask ") || events.contains(" trace:ask_failed "),
        "{events}"
    );
    // The model decides the item: a ticket is written once, or the item is
    // skipped by the gate ("no risk") or by the ask ("no answer that fits
    // Impact").
    let tickets = app.get("/api/tickets");
    let tickets = tickets.as_array().unwrap();
    if tickets.is_empty() {
        assert!(
            events.contains("trace:skipped \"no risk\"")
                || events.contains("trace:skipped \"no answer that fits Impact\""),
            "{events}"
        );
    } else {
        assert_eq!(tickets.len(), 1, "{tickets:?}");
        assert!(events.contains("upgrade:httpx:2.4.0"), "{events}");
    }
}
