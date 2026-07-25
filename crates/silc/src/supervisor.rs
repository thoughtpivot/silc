//! Compile-and-run supervisor for runnable Silc UI portal programs.

use crate::runtimes::RuntimeLock;
use sil_codegen::EmitResult;
use sil_core::ProcessorOp;
use sil_ipc::{
    read_frame, write_frame, ControlFrame, SlotPool, DEFAULT_PAYLOAD_CAPACITY, DEFAULT_SLOT_COUNT,
    HEADER_SIZE,
};
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, BufWriter};
use std::net::TcpListener;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

struct WorkerPool {
    writers: Vec<Arc<Mutex<BufWriter<UnixStream>>>>,
    next: usize,
}

impl WorkerPool {
    fn push(&mut self, writer: Arc<Mutex<BufWriter<UnixStream>>>) {
        self.writers.push(writer);
    }

    fn next_writer(&mut self) -> Option<Arc<Mutex<BufWriter<UnixStream>>>> {
        if self.writers.is_empty() {
            return None;
        }
        let idx = self.next % self.writers.len();
        self.next = self.next.wrapping_add(1);
        Some(Arc::clone(&self.writers[idx]))
    }

    fn len(&self) -> usize {
        self.writers.len()
    }
}

struct Pending {
    stage: Stage,
    segment_id: u64,
    response_writer: Option<Arc<Mutex<BufWriter<UnixStream>>>>,
    id: String,
}

#[derive(Clone, Copy)]
enum Stage {
    Python,
    Go,
}

pub fn ensure_project_runtimes(workdir: &Path) -> Result<RuntimeLock, String> {
    let lock = crate::runtimes::ensure_runtimes()?;
    crate::runtimes::write_lock(workdir, &lock)?;
    Ok(lock)
}

pub fn build_go_worker(lock: &RuntimeLock, runtime_root: &Path) -> Result<PathBuf, String> {
    let go_dir = runtime_root.join("go");
    let out = go_dir.join("worker");
    let _ = fs::remove_file(go_dir.join("feedback_worker.go"));
    let _ = Command::new(&lock.go_bin)
        .current_dir(&go_dir)
        .args(["mod", "tidy"])
        .env("GOTOOLCHAIN", "local")
        .status();
    // `-o` must be relative to current_dir; absolute/relative project paths nest wrongly.
    let status = Command::new(&lock.go_bin)
        .current_dir(&go_dir)
        .args(["build", "-o", "worker", "."])
        .env("GOTOOLCHAIN", "local")
        .status()
        .map_err(|e| format!("failed to build Go worker with Silc Go: {e}"))?;
    if !status.success() || !out.is_file() {
        return Err("Silc Go worker build failed".into());
    }
    Ok(out)
}

/// Build the compiler-owned Gin HTTP API binary for `service::http`.
pub fn build_go_api_worker(lock: &RuntimeLock, runtime_root: &Path) -> Result<PathBuf, String> {
    let api_dir = runtime_root.join("go/api");
    if !api_dir.join("worker.go").is_file() {
        return Err("missing compiler-generated go/api/worker.go for service::http".into());
    }
    if !api_dir.join("go.mod").is_file() {
        return Err("missing compiler-generated go/api/go.mod for service::http".into());
    }
    let out = api_dir.join("worker");
    let tidy = Command::new(&lock.go_bin)
        .current_dir(&api_dir)
        .args(["mod", "tidy"])
        .env("GOTOOLCHAIN", "local")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to tidy Go API module with Silc Go: {e}"))?;
    if !tidy.status.success() {
        return Err(format!(
            "Silc Go API `go mod tidy` failed:\n{}\n{}",
            String::from_utf8_lossy(&tidy.stdout),
            String::from_utf8_lossy(&tidy.stderr)
        ));
    }
    let status = Command::new(&lock.go_bin)
        .current_dir(&api_dir)
        .args(["build", "-o", "worker", "."])
        .env("GOTOOLCHAIN", "local")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to build Go API worker with Silc Go: {e}"))?;
    if !status.status.success() || !out.is_file() {
        return Err(format!(
            "Silc Go API worker build failed:\n{}\n{}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr)
        ));
    }
    Ok(out)
}

/// Install compiler-pinned React/Tailwind deps and bundle ui::web assets with Silc-owned Bun.
pub fn build_ui_web(lock: &RuntimeLock, runtime_root: &Path) -> Result<(), String> {
    let ts_dir = runtime_root.join("typescript");
    if !ts_dir.join("package.json").is_file() {
        return Err("missing compiler-generated typescript/package.json for ui::web".into());
    }
    if !ts_dir.join("src/main.tsx").is_file() {
        return Err("missing compiler-generated typescript/src/main.tsx for ui::web".into());
    }
    if !ts_dir.join("tailwind.config.js").is_file() {
        return Err("missing compiler-generated typescript/tailwind.config.js for ui::web".into());
    }

    let install = Command::new(&lock.bun_bin)
        .current_dir(&ts_dir)
        .args(["install", "--frozen-lockfile"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to install ui::web deps with Silc Bun: {e}"))?;
    if !install.status.success() {
        return Err(format!(
            "Silc Bun install for ui::web failed:\n{}\n{}",
            String::from_utf8_lossy(&install.stdout),
            String::from_utf8_lossy(&install.stderr)
        ));
    }

    let dist = ts_dir.join("dist");
    let assets = dist.join("assets");
    fs::create_dir_all(&assets).map_err(|e| format!("create dist/assets: {e}"))?;

    // Compile Tailwind utilities into the published theme asset.
    // Paths must match ui_web_index.html (/assets/theme.css, /assets/app.js).
    let css = Command::new(&lock.bun_bin)
        .current_dir(&ts_dir)
        .args([
            "x",
            "--bun",
            "tailwindcss",
            "-i",
            "./src/theme.css",
            "-o",
            "./dist/assets/theme.css",
            "--minify",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to compile ui::web Tailwind CSS with Silc Bun: {e}"))?;
    if !css.status.success() {
        return Err(format!(
            "Silc Bun ui::web Tailwind compile failed:\n{}\n{}",
            String::from_utf8_lossy(&css.stdout),
            String::from_utf8_lossy(&css.stderr)
        ));
    }

    // Bundle React SPA. Use a cwd-relative outfile — Bun 1.2.x mishandles absolute --outfile paths.
    let build = Command::new(&lock.bun_bin)
        .current_dir(&ts_dir)
        .args([
            "build",
            "./src/main.tsx",
            "--outfile=dist/assets/app.js",
            "--target=browser",
            "--minify",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to bundle ui::web with Silc Bun: {e}"))?;
    if !build.status.success() {
        return Err(format!(
            "Silc Bun ui::web bundle failed:\n{}\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        ));
    }

    // Always publish the compiler HTML shell under dist/.
    fs::copy(ts_dir.join("index.html"), dist.join("index.html"))
        .map_err(|e| format!("copy ui::web index.html: {e}"))?;

    if !assets.join("app.js").is_file() {
        return Err("Silc ui::web bundle did not produce dist/assets/app.js".into());
    }
    if !assets.join("theme.css").is_file() {
        return Err("Silc ui::web Tailwind compile did not produce dist/assets/theme.css".into());
    }
    Ok(())
}

pub fn build_llm_python(lock: &RuntimeLock, runtime_root: &Path) -> Result<PathBuf, String> {
    let python_dir = runtime_root.join("python");
    let requirements = python_dir.join("requirements.txt");
    if !requirements.is_file() {
        return Err("missing compiler-generated python/requirements.txt for llm::complete".into());
    }
    let python = python_dir.join(".venv/bin/python");
    if !python.is_file() {
        let output = Command::new(&lock.python_bin)
            .args(["-m", "venv"])
            .arg(python_dir.join(".venv"))
            .output()
            .map_err(|e| format!("failed to create llm::complete Python environment: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "Silc Python environment creation failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    let status = Command::new(&python)
        .args(["-m", "pip", "install", "--disable-pip-version-check", "-r"])
        .arg(&requirements)
        .status()
        .map_err(|e| format!("failed to install llm::complete Python dependency: {e}"))?;
    if !status.success() {
        return Err("Silc install of compiler-pinned llama-cpp-python failed".into());
    }
    Ok(python)
}

/// Run an API-only `service::http` program (Go/Gin, no Bun UI).
pub fn run_api(output: &EmitResult, _lock: &RuntimeLock) -> Result<(), String> {
    let graph = output
        .graph
        .as_ref()
        .ok_or_else(|| "program is not executable in Silc 0.2.0".to_string())?;
    if !graph.is_api_only() {
        return Err("run_api requires an API-only service::http program".into());
    }
    let port = graph
        .api_port()
        .ok_or_else(|| "service::http graph missing API port".to_string())?;
    ensure_api_port_available(port)?;

    let api_bin = output.root.join("go/api/worker");
    if !api_bin.is_file() {
        return Err(format!("Go API binary missing at {}", api_bin.display()));
    }

    fs::write(
        output.root.join("run.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "api_port": port,
            "adapter": "gin-v1",
            "routes": graph.api_routes.iter().map(|r| serde_json::json!({
                "method": r.method,
                "path": r.path,
                "contract": r.contract,
            })).collect::<Vec<_>>(),
        }))
        .unwrap(),
    )
    .map_err(|e| e.to_string())?;

    let mut child = Command::new(&api_bin)
        .env("SILC_API_PORT", port.to_string())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("failed to spawn Silc Go API worker: {e}"))?;

    wait_for_http_health(port, Duration::from_secs(30))?;

    println!("silc: service::http (gin-v1) listening on http://127.0.0.1:{port}");
    for route in &graph.api_routes {
        println!("silc:   {} {}", route.method, route.path);
    }
    println!("silc: press Ctrl-C to stop");

    wait_for_ctrl_c();
    let _ = child.kill();
    let _ = child.wait();
    println!("silc: stopped");
    Ok(())
}

pub fn run_app(output: &EmitResult, lock: &RuntimeLock) -> Result<(), String> {
    let graph = output
        .graph
        .as_ref()
        .ok_or_else(|| "program is not executable in Silc 0.2.0".to_string())?;
    if graph.is_api_only() {
        return run_api(output, lock);
    }
    // Fail before spawning any workers. Previously Bun could report READY over
    // UDS and then fail its HTTP bind, leaving the supervisor claiming success.
    ensure_http_port_available(graph.http_port)?;
    if let Some(port) = graph.terminal_port {
        ensure_terminal_port_available(port)?;
    }
    if let Some(port) = graph.api_port() {
        ensure_api_port_available(port)?;
    }
    let ipc_dir = output.root.join("ipc");
    let data_dir = output.root.join("data");
    fs::create_dir_all(&ipc_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    // Keep Spotlight from indexing hundreds of mmap slot files mid-run (macOS).
    let _ = fs::write(ipc_dir.join(".metadata_never_index"), b"");
    let _ = fs::write(data_dir.join(".metadata_never_index"), b"");

    let socket_path = short_socket_path(&output.root)?;
    let _ = fs::remove_file(&socket_path);
    let listener =
        UnixListener::bind(&socket_path).map_err(|e| format!("bind supervisor socket: {e}"))?;

    let db_path = data_dir.join("app.db");
    fs::write(
        output.root.join("run.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "socket": socket_path,
            "http_port": graph.http_port,
            "terminal_port": graph.terminal_port,
            "ipc_dir": ipc_dir,
            "db": &db_path,
            "processor_op": graph.processor_op.as_str(),
            "capabilities": {
                "web": graph.capabilities.web,
                "terminal": graph.capabilities.terminal,
                "score": graph.capabilities.score,
                "llm": graph.capabilities.llm,
                "history": graph.capabilities.history,
                "resources": graph.capabilities.resources,
            },
            "model_ref": graph.model_ref,
        }))
        .unwrap(),
    )
    .map_err(|e| e.to_string())?;

    let pool = Arc::new(Mutex::new(SlotPool::create(
        &ipc_dir,
        output.schema_id,
        DEFAULT_SLOT_COUNT,
        DEFAULT_PAYLOAD_CAPACITY,
    )?));
    let workers: Arc<Mutex<HashMap<String, WorkerPool>>> = Arc::new(Mutex::new(HashMap::new()));
    let pending: Arc<Mutex<HashMap<String, Pending>>> = Arc::new(Mutex::new(HashMap::new()));
    let stop = Arc::new(AtomicBool::new(false));

    let accept_workers = {
        let workers = Arc::clone(&workers);
        let pending = Arc::clone(&pending);
        let pool = Arc::clone(&pool);
        let stop = Arc::clone(&stop);
        let processor_op = graph.processor_op;
        let model_id = graph.model_ref.clone();
        let listener = listener.try_clone().map_err(|e| e.to_string())?;
        thread::spawn(move || {
            accept_loop(
                listener,
                workers,
                pending,
                pool,
                stop,
                processor_op,
                model_id,
            )
        })
    };

    // LLM weights load once; deterministic scoring can scale with CPU.
    let python_replicas = if graph.needs_llm() { 1 } else { 16 };
    const GO_REPLICAS: usize = 1;
    let model_path = if graph.needs_llm() {
        Some(crate::models::ensure_model(
            graph
                .model_ref
                .as_deref()
                .ok_or_else(|| "llm chat graph missing model_ref".to_string())?,
        )?)
    } else {
        None
    };
    let mut children = spawn_workers(
        output,
        lock,
        &socket_path,
        &ipc_dir,
        &data_dir,
        graph,
        python_replicas,
        GO_REPLICAS,
        model_path.as_deref(),
    )?;
    wait_for_pool(
        &workers,
        "python",
        python_replicas,
        if graph.needs_llm() {
            Duration::from_secs(300)
        } else {
            Duration::from_secs(90)
        },
    )?;
    wait_for_pool(&workers, "go", GO_REPLICAS, Duration::from_secs(90))?;

    if graph.has_api() {
        let api_bin = output.root.join("go/api/worker");
        if !api_bin.is_file() {
            return Err(format!("Go API binary missing at {}", api_bin.display()));
        }
        let api_port = graph.api_port().unwrap();
        let api_child = Command::new(&api_bin)
            .env("SILC_API_PORT", api_port.to_string())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("failed to spawn Silc Go API worker: {e}"))?;
        children.push(api_child);
        wait_for_http_health(api_port, Duration::from_secs(30))?;
    }

    let bun_child = Command::new(&lock.bun_bin)
        .arg(output.root.join("typescript/worker.ts"))
        .env("SILC_SOCKET", &socket_path)
        .env("SILC_DB_PATH", &db_path)
        .env("SILC_SQLITE_TABLE", &graph.sqlite_table)
        .env("SILC_HTTP_PORT", graph.http_port.to_string())
        .env("SILC_HTTP_ROUTE", &graph.http_route)
        .env(
            "SILC_TERMINAL_PORT",
            graph.terminal_port.unwrap_or_default().to_string(),
        )
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("failed to spawn Silc Bun worker: {e}"))?;
    children.push(bun_child);
    wait_for_pool(&workers, "bun", 1, Duration::from_secs(30))?;

    if graph.capabilities.web {
        println!(
            "silc: ui::web listening on http://127.0.0.1:{}{}",
            graph.http_port, graph.http_route
        );
    }
    if let Some(port) = graph.terminal_port {
        println!("silc: ui::terminal listening at telnet://127.0.0.1:{port}");
        println!("silc: connect with `telnet 127.0.0.1 {port}`");
    }
    if let Some(port) = graph.api_port() {
        println!("silc: service::http (gin-v1) listening on http://127.0.0.1:{port}");
        for route in &graph.api_routes {
            println!("silc:   {} {}", route.method, route.path);
        }
    }
    println!("silc: press Ctrl-C to stop");

    wait_for_ctrl_c();
    stop.store(true, Ordering::SeqCst);
    let _ = UnixStream::connect(&socket_path);
    let _ = accept_workers.join();

    if let Ok(map) = workers.lock() {
        for pool in map.values() {
            for writer in &pool.writers {
                if let Ok(mut w) = writer.lock() {
                    let _ = write_frame(&mut *w, &ControlFrame::Shutdown {});
                }
            }
        }
    }
    for child in &mut children {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = fs::remove_file(&socket_path);
    println!("silc: stopped");
    Ok(())
}

fn ensure_http_port_available(port: u16) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|error| {
        format!(
            "ui::web cannot listen on http://127.0.0.1:{port}: {error} (choose another :port or stop the existing process)"
        )
    })?;
    drop(listener);
    Ok(())
}

fn ensure_api_port_available(port: u16) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|error| {
        format!(
            "service::http cannot listen on http://127.0.0.1:{port}: {error} (choose another :port or stop the existing process)"
        )
    })?;
    drop(listener);
    Ok(())
}

fn ensure_terminal_port_available(port: u16) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|error| {
        format!(
            "ui::terminal cannot listen on telnet://127.0.0.1:{port}: {error} (choose another :port or stop the existing process)"
        )
    })?;
    drop(listener);
    Ok(())
}

fn wait_for_http_health(port: u16, timeout: Duration) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/health");
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(response) = ureq::get(&url).timeout(Duration::from_secs(1)).call() {
            if (200..300).contains(&response.status()) {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "timed out waiting for service::http health at {url}"
    ))
}

fn accept_loop(
    listener: UnixListener,
    workers: Arc<Mutex<HashMap<String, WorkerPool>>>,
    pending: Arc<Mutex<HashMap<String, Pending>>>,
    pool: Arc<Mutex<SlotPool>>,
    stop: Arc<AtomicBool>,
    processor_op: ProcessorOp,
    model_id: Option<String>,
) {
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        let workers = Arc::clone(&workers);
        let pending = Arc::clone(&pending);
        let pool = Arc::clone(&pool);
        let model_id = model_id.clone();
        thread::spawn(move || {
            if let Err(err) = handle_client(stream, workers, pending, pool, processor_op, model_id)
            {
                eprintln!("silc supervisor: {err}");
            }
        });
    }
}

fn handle_client(
    stream: UnixStream,
    workers: Arc<Mutex<HashMap<String, WorkerPool>>>,
    pending: Arc<Mutex<HashMap<String, Pending>>>,
    pool: Arc<Mutex<SlotPool>>,
    processor_op: ProcessorOp,
    model_id: Option<String>,
) -> Result<(), String> {
    let reader_stream = stream.try_clone().map_err(|e| e.to_string())?;
    let writer_stream = stream.try_clone().map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(reader_stream);
    let writer = Arc::new(Mutex::new(BufWriter::new(writer_stream)));

    let first = read_frame(&mut reader)?;
    let ControlFrame::Hello {
        worker_id, role, ..
    } = first
    else {
        return Err(format!("expected HELLO, got {first:?}"));
    };
    let ready = read_frame(&mut reader)?;
    if !matches!(ready, ControlFrame::Ready { .. }) {
        return Err("expected READY after HELLO".into());
    }
    {
        let mut map = workers.lock().map_err(|_| "workers lock".to_string())?;
        map.entry(role.clone())
            .or_insert_with(|| WorkerPool {
                writers: Vec::new(),
                next: 0,
            })
            .push(Arc::clone(&writer));
        let count = map.get(&role).map(|p| p.len()).unwrap_or(0);
        println!("silc: worker ready role={role} id={worker_id} pool={count}");
    }

    loop {
        match read_frame(&mut reader) {
            Ok(ControlFrame::Ingest {
                request_id,
                author,
                text,
                session_id,
            }) if role == "bun" => {
                // Keep the Bun reader hot: slot acquire / notify must not block
                // reading the next INGEST frames off the UDS.
                let workers = Arc::clone(&workers);
                let pending = Arc::clone(&pending);
                let pool = Arc::clone(&pool);
                let response_writer = Arc::clone(&writer);
                let model_id = model_id.clone();
                thread::spawn(move || {
                    if let Err(err) = start_ingest(
                        workers.as_ref(),
                        pending.as_ref(),
                        pool.as_ref(),
                        request_id.clone(),
                        author,
                        text,
                        session_id,
                        processor_op,
                        model_id,
                        Some(response_writer),
                    ) {
                        let _ = fail_pending(pending.as_ref(), &request_id, err);
                    }
                });
            }
            Ok(ControlFrame::Ack {
                request_id,
                segment_id,
                seq,
                result,
                ..
            }) => on_ack(
                &workers,
                &pending,
                &pool,
                request_id,
                segment_id,
                seq,
                result,
                processor_op,
            )?,
            Ok(ControlFrame::Error {
                request_id,
                message,
                ..
            }) => {
                if let Some(id) = request_id {
                    fail_pending(&pending, &id, message)?;
                }
            }
            Ok(ControlFrame::Shutdown {}) => break,
            Err(err) => {
                eprintln!("silc: worker role={role} frame decode error: {err}");
                break;
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn start_ingest(
    workers: &Mutex<HashMap<String, WorkerPool>>,
    pending: &Mutex<HashMap<String, Pending>>,
    pool: &Mutex<SlotPool>,
    request_id: String,
    author: String,
    text: String,
    session_id: String,
    processor_op: ProcessorOp,
    model_id: Option<String>,
    response_writer: Option<Arc<Mutex<BufWriter<UnixStream>>>>,
) -> Result<(), String> {
    let id = Uuid::new_v4().to_string();
    let mut record = match processor_op {
        ProcessorOp::LlmComplete => serde_json::json!({
            "id": id,
            "prompt": text,
            "reply": "",
            "model": model_id.unwrap_or_else(|| sil_core::DEFAULT_MODEL_ID.to_string()),
        }),
        ProcessorOp::Score => serde_json::json!({
            "id": id,
            "author": author,
            "text": text,
            "summary": "",
            "score": 0.0,
        }),
        ProcessorOp::None => serde_json::json!({
            "id": id,
            "author": author,
            "text": text,
        }),
    };
    if !session_id.is_empty() {
        if let Some(obj) = record.as_object_mut() {
            obj.insert("session_id".into(), serde_json::Value::String(session_id));
        }
    }
    let bytes = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
    let (segment_id, header) = acquire_slot(pool, &bytes)?;
    {
        let mut pending = pending.lock().map_err(|_| "pending lock".to_string())?;
        pending.insert(
            request_id.clone(),
            Pending {
                stage: Stage::Python,
                segment_id,
                response_writer,
                id,
            },
        );
    }
    notify_role(
        workers,
        "python",
        ControlFrame::Notify {
            request_id,
            segment_id,
            offset: HEADER_SIZE as u32,
            len: header.payload_len,
            schema_id: header.schema_id,
            seq: header.seq,
            stage: "python".into(),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn on_ack(
    workers: &Mutex<HashMap<String, WorkerPool>>,
    pending: &Mutex<HashMap<String, Pending>>,
    pool: &Mutex<SlotPool>,
    request_id: String,
    segment_id: u64,
    seq: u64,
    result: Option<serde_json::Value>,
    processor_op: ProcessorOp,
) -> Result<(), String> {
    let mut pending_map = pending.lock().map_err(|_| "pending lock".to_string())?;
    let Some(entry) = pending_map.get_mut(&request_id) else {
        return Ok(());
    };
    match entry.stage {
        Stage::Python => {
            entry.stage = Stage::Go;
            let schema_id = {
                let pool = pool.lock().map_err(|_| "pool lock".to_string())?;
                pool.schema_id
            };
            drop(pending_map);
            notify_role(
                workers,
                "go",
                ControlFrame::Notify {
                    request_id,
                    segment_id,
                    offset: HEADER_SIZE as u32,
                    len: 0,
                    schema_id,
                    seq,
                    stage: "go".into(),
                },
            )
        }
        Stage::Go => {
            let response = ControlFrame::Response {
                request_id: request_id.clone(),
                ok: true,
                id: Some(entry.id.clone()),
                score: result
                    .as_ref()
                    .and_then(|v| v.get("score"))
                    .and_then(|v| v.as_f64()),
                summary: result
                    .as_ref()
                    .and_then(|v| v.get("summary"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                reply: result
                    .as_ref()
                    .and_then(|v| v.get("reply"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                model: if processor_op.needs_llm() {
                    result
                        .as_ref()
                        .and_then(|v| v.get("model"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                } else {
                    None
                },
                error: None,
            };
            if let Some(writer) = entry.response_writer.clone() {
                if let Ok(mut w) = writer.lock() {
                    let _ = write_frame(&mut *w, &response);
                }
            }
            let seg = entry.segment_id as usize;
            pending_map.remove(&request_id);
            drop(pending_map);
            let mut pool = pool.lock().map_err(|_| "pool lock".to_string())?;
            pool.release(seg)
        }
    }
}

fn fail_pending(
    pending: &Mutex<HashMap<String, Pending>>,
    request_id: &str,
    message: String,
) -> Result<(), String> {
    let mut pending = pending.lock().map_err(|_| "pending lock".to_string())?;
    if let Some(entry) = pending.remove(request_id) {
        if let Some(writer) = entry.response_writer {
            if let Ok(mut w) = writer.lock() {
                let _ = write_frame(
                    &mut *w,
                    &ControlFrame::Response {
                        request_id: request_id.to_string(),
                        ok: false,
                        id: None,
                        score: None,
                        summary: None,
                        reply: None,
                        model: None,
                        error: Some(message),
                    },
                );
            }
        }
    }
    Ok(())
}

fn acquire_slot(pool: &Mutex<SlotPool>, bytes: &[u8]) -> Result<(u64, sil_ipc::Header), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        {
            let mut pool = pool.lock().map_err(|_| "pool lock".to_string())?;
            match pool.acquire_write(bytes) {
                Ok((idx, header)) => return Ok((idx as u64, header)),
                Err(err) if err.contains("exhausted") => {}
                Err(err) => return Err(err),
            }
        }
        if Instant::now() >= deadline {
            return Err("slot pool exhausted".into());
        }
        thread::sleep(Duration::from_micros(200));
    }
}

fn notify_role(
    workers: &Mutex<HashMap<String, WorkerPool>>,
    role: &str,
    frame: ControlFrame,
) -> Result<(), String> {
    let writer = {
        let mut map = workers.lock().map_err(|_| "workers lock".to_string())?;
        let pool = map
            .get_mut(role)
            .ok_or_else(|| format!("worker role `{role}` not ready"))?;
        pool.next_writer()
            .ok_or_else(|| format!("worker role `{role}` pool empty"))?
    };
    let mut writer = writer.lock().map_err(|_| "writer lock".to_string())?;
    write_frame(&mut *writer, &frame)
}

#[allow(clippy::too_many_arguments)]
fn spawn_workers(
    output: &EmitResult,
    lock: &RuntimeLock,
    socket: &Path,
    ipc_dir: &Path,
    data_dir: &Path,
    graph: &sil_core::ExecutableGraph,
    python_replicas: usize,
    go_replicas: usize,
    model_path: Option<&Path>,
) -> Result<Vec<Child>, String> {
    let mut children = Vec::new();
    let python_bin = if graph.needs_llm() {
        let path = output.root.join("python/.venv/bin/python");
        if !path.is_file() {
            return Err(format!(
                "llm::complete Python environment missing at {} (run `silc build`)",
                path.display()
            ));
        }
        path
    } else {
        lock.python_bin.clone()
    };
    for _ in 0..python_replicas {
        let mut command = Command::new(&python_bin);
        command
            .arg(output.root.join("python/worker.py"))
            .env("SILC_SOCKET", socket)
            .env("SILC_IPC_DIR", ipc_dir)
            .stdout(if graph.needs_llm() {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::inherit());
        if let Some(path) = model_path {
            command.env("SILC_MODEL_PATH", path);
        }
        if let Some(id) = graph.model_ref.as_deref() {
            command.env("SILC_MODEL_ID", id);
        }
        children.push(
            command
                .spawn()
                .map_err(|e| format!("failed to spawn Silc CPython worker: {e}"))?,
        );
    }
    let go_bin = output.root.join("go/worker");
    if !go_bin.is_file() {
        return Err(format!("Go worker binary missing at {}", go_bin.display()));
    }
    for _ in 0..go_replicas {
        children.push(
            Command::new(&go_bin)
                .env("SILC_SOCKET", socket)
                .env("SILC_IPC_DIR", ipc_dir)
                .env("SILC_DB_PATH", data_dir.join("app.db"))
                .env("SILC_SQLITE_TABLE", &graph.sqlite_table)
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| format!("failed to spawn Silc Go worker: {e}"))?,
        );
    }
    Ok(children)
}

fn wait_for_pool(
    workers: &Mutex<HashMap<String, WorkerPool>>,
    role: &str,
    min: usize,
    timeout: Duration,
) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        {
            let map = workers.lock().map_err(|_| "workers lock".to_string())?;
            if map.get(role).map(|p| p.len()).unwrap_or(0) >= min {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "timed out waiting for Silc worker pool `{role}` (>= {min})"
    ))
}

fn short_socket_path(runtime_root: &Path) -> Result<PathBuf, String> {
    let candidate = runtime_root.join("supervisor.sock");
    if candidate.to_string_lossy().len() < 100 {
        return Ok(candidate);
    }
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(runtime_root.to_string_lossy().as_bytes());
    let digest = hex::encode(hasher.finalize())[..16].to_string();
    Ok(std::env::temp_dir().join(format!("silc-{digest}.sock")))
}

fn wait_for_ctrl_c() {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    set_handler(tx);
    let _ = rx.recv();
}

fn set_handler(tx: std::sync::mpsc::Sender<()>) {
    static HANDLER: OnceLock<Mutex<Option<std::sync::mpsc::Sender<()>>>> = OnceLock::new();
    let cell = HANDLER.get_or_init(|| Mutex::new(None));
    *cell.lock().unwrap() = Some(tx);
    unsafe extern "C" fn on_signal(_: libc::c_int) {
        if let Some(cell) = HANDLER.get() {
            if let Ok(mut guard) = cell.lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(());
                }
            }
        }
    }
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as usize);
        libc::signal(libc::SIGTERM, on_signal as *const () as usize);
    }
}
