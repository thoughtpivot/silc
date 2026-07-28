use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn silc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_silc"))
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

fn read_until(stream: &mut TcpStream, needle: &str, timeout: Duration) -> String {
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    let start = Instant::now();
    let mut out = Vec::new();
    let mut buffer = [0u8; 2048];
    while start.elapsed() < timeout {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&buffer[..n]);
                if String::from_utf8_lossy(&out).contains(needle) {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => panic!("terminal read failed: {error}"),
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn blog_app_seeds_admin_modal_and_surfaces() {
    free_port(18120);
    free_port(18121);
    let temp = std::env::temp_dir().join(format!(
        "silc-blog-e2e-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&temp).unwrap();

    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blog_app.silc");

    let build = Command::new(silc_bin())
        .args(["build", example.to_str().unwrap()])
        .output()
        .expect("build");
    assert!(
        build.status.success(),
        "build failed: {}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let runtime =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/.runtime/blog_app");
    let app = std::fs::read_to_string(runtime.join("typescript/src/App.tsx")).unwrap();
    assert!(
        app.contains("onSelect={(row) => on_select(row)}"),
        "missing table select wiring"
    );
    assert!(app.contains("<Dialog open={dialog_open}"), "missing dialog");
    assert!(
        app.contains("__filterComplete") && app.contains("filter_query"),
        "missing silclm feed filter"
    );
    assert!(
        !app.contains("year_filter") && !app.contains("month_filter"),
        "chip year/month filters should be gone"
    );
    assert!(
        app.contains("__chatComplete") || app.contains("/complete"),
        "missing chat"
    );

    let worker = std::fs::read_to_string(runtime.join("typescript/worker.ts")).unwrap();
    assert!(worker.contains("RESOURCE_SEEDS"), "missing seed metadata");
    assert!(
        worker.contains("INSERT OR IGNORE"),
        "missing idempotent seed insert"
    );
    assert!(worker.contains("article-001") && worker.contains("article-030"));

    let data_table =
        std::fs::read_to_string(runtime.join("typescript/src/components/ui/data-table.tsx"))
            .unwrap();
    assert!(
        data_table.contains("onSelect"),
        "data-table missing onSelect"
    );

    let data_dir = runtime.join("data");
    for name in ["app.db", "app.db-shm", "app.db-wal"] {
        let _ = std::fs::remove_file(data_dir.join(name));
    }

    let log_path = temp.join("run.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut child = Command::new(silc_bin())
        .arg(example.to_str().unwrap())
        .arg("--terminal")
        .env("SILC_HTTP_PORT", "18120")
        .env("SILC_TERMINAL_PORT", "18121")
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("run");

    let ready = Instant::now();
    let mut healthy = false;
    while ready.elapsed() < Duration::from_secs(300) {
        if let Ok(resp) = ureq::get("http://127.0.0.1:18120/health").call() {
            let body = resp.into_string().unwrap_or_default();
            if body.contains("\"ok\":true") || body.contains("\"ok\": true") {
                healthy = true;
                break;
            }
        }
        if let Ok(Some(status)) = child.try_wait() {
            let log = std::fs::read_to_string(&log_path).unwrap_or_default();
            panic!("silc exited early: {status}\n{log}");
        }
        thread::sleep(Duration::from_millis(500));
    }
    assert!(
        healthy,
        "health never became ready; log:\n{}",
        std::fs::read_to_string(&log_path).unwrap_or_default()
    );

    let list = ureq::get("http://127.0.0.1:18120/api/articles")
        .call()
        .expect("list articles")
        .into_string()
        .unwrap();
    let rows: serde_json::Value = serde_json::from_str(&list).expect("articles json");
    let arr = rows.as_array().expect("array");
    assert_eq!(arr.len(), 30, "expected 30 seeded articles, got {list}");
    assert!(
        arr.iter()
            .any(|row| row.get("id").and_then(|v| v.as_str()) == Some("article-001")),
        "missing article-001"
    );

    // Idempotent reseed: restart would INSERT OR IGNORE; mutating one row then listing keeps count.
    let updated = ureq::put("http://127.0.0.1:18120/api/articles/article-001")
        .set("content-type", "application/json")
        .send_string(
            r#"{"title":"Updated Seed Title","body":"Edited by e2e.","author":"E2E","published_at":"2024-01-01","year":"2024","month":"January"}"#,
        )
        .expect("update")
        .into_string()
        .unwrap();
    assert!(updated.contains("Updated Seed Title"), "{updated}");

    let created = ureq::post("http://127.0.0.1:18120/api/articles")
        .set("content-type", "application/json")
        .send_string(
            r#"{"title":"Fresh Post","body":"Created in e2e.","author":"E2E","published_at":"2026-07-27","year":"2026","month":"July"}"#,
        )
        .expect("create")
        .into_string()
        .unwrap();
    assert!(created.contains("Fresh Post"), "{created}");

    let list2 = ureq::get("http://127.0.0.1:18120/api/articles")
        .call()
        .expect("list again")
        .into_string()
        .unwrap();
    let rows2: serde_json::Value = serde_json::from_str(&list2).unwrap();
    assert_eq!(
        rows2.as_array().unwrap().len(),
        31,
        "create failed: {list2}"
    );
    assert!(list2.contains("Updated Seed Title") && list2.contains("Fresh Post"));

    let deleted = ureq::delete("http://127.0.0.1:18120/api/articles/article-002")
        .call()
        .expect("delete")
        .into_string()
        .unwrap();
    assert!(
        deleted.contains("ok") || deleted.contains("true"),
        "{deleted}"
    );

    let list3 = ureq::get("http://127.0.0.1:18120/api/articles")
        .call()
        .expect("list after delete")
        .into_string()
        .unwrap();
    assert!(!list3.contains("\"id\":\"article-002\"") && !list3.contains("article-002"));

    let mut terminal = TcpStream::connect("127.0.0.1:18121").expect("connect terminal");
    let banner = read_until(&mut terminal, ">", Duration::from_secs(5));
    assert!(
        banner.contains("Silc") || banner.contains("/list"),
        "missing terminal help: {banner:?}"
    );
    terminal.write_all(b"/list articles\n").unwrap();
    let listed = read_until(&mut terminal, "Updated Seed", Duration::from_secs(5));
    assert!(
        listed.contains("Updated Seed") || listed.contains("Fresh Post"),
        "terminal list failed: {listed:?}"
    );
    terminal.write_all(b"/quit\n").ok();

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGKILL);
    }
    let _ = child.wait();
    for name in ["app.db", "app.db-shm", "app.db-wal"] {
        let _ = std::fs::remove_file(data_dir.join(name));
    }
    let _ = std::fs::remove_dir_all(temp);
}
