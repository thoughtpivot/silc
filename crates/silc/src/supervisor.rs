//! Compile-and-run supervisor for runnable Silc feedback programs.

use crate::runtimes::RuntimeLock;
use sil_codegen::EmitResult;
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
    fs::create_dir_all(&dist).map_err(|e| format!("create dist: {e}"))?;

    // Compile Tailwind utilities into the published theme asset.
    let css = Command::new(&lock.bun_bin)
        .current_dir(&ts_dir)
        .args([
            "x",
            "--bun",
            "tailwindcss",
            "-i",
            "./src/theme.css",
            "-o",
            "./dist/theme.css",
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
            "--outfile=dist/app.js",
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

    if !dist.join("app.js").is_file() {
        return Err("Silc ui::web bundle did not produce dist/app.js".into());
    }
    if !dist.join("theme.css").is_file() {
        return Err("Silc ui::web Tailwind compile did not produce dist/theme.css".into());
    }
    Ok(())
}

pub fn run_feedback(output: &EmitResult, lock: &RuntimeLock) -> Result<(), String> {
    let graph = output
        .graph
        .as_ref()
        .ok_or_else(|| "program is not executable in Silc v1".to_string())?;
    // Fail before spawning any workers. Previously Bun could report READY over
    // UDS and then fail its HTTP bind, leaving the supervisor claiming success.
    ensure_http_port_available(graph.http_port)?;
    if let Some(port) = graph.terminal_port {
        ensure_terminal_port_available(port)?;
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

    fs::write(
        output.root.join("run.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "socket": socket_path,
            "http_port": graph.http_port,
            "terminal_port": graph.terminal_port,
            "ipc_dir": ipc_dir,
            "db": data_dir.join("feedback.db"),
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
        let listener = listener.try_clone().map_err(|e| e.to_string())?;
        thread::spawn(move || accept_loop(listener, workers, pending, pool, stop))
    };

    // Python scales with CPU; one Go writer avoids SQLITE_BUSY storms.
    const PYTHON_REPLICAS: usize = 16;
    const GO_REPLICAS: usize = 1;
    let mut children = spawn_workers(
        output,
        lock,
        &socket_path,
        &ipc_dir,
        &data_dir,
        graph,
        PYTHON_REPLICAS,
        GO_REPLICAS,
    )?;
    wait_for_pool(&workers, "python", PYTHON_REPLICAS, Duration::from_secs(90))?;
    wait_for_pool(&workers, "go", GO_REPLICAS, Duration::from_secs(90))?;

    let bun_child = Command::new(&lock.bun_bin)
        .arg(output.root.join("typescript/worker.ts"))
        .env("SILC_SOCKET", &socket_path)
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

    println!(
        "silc: ui::web ({}) listening on http://127.0.0.1:{}{}",
        graph.ui_surface.substrate(),
        graph.http_port,
        graph.http_route
    );
    if let Some(port) = graph.terminal_port {
        println!("silc: ui::terminal listening at telnet://127.0.0.1:{port}");
        println!("silc: connect with `telnet 127.0.0.1 {port}`");
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

fn ensure_terminal_port_available(port: u16) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|error| {
        format!(
            "ui::terminal cannot listen on telnet://127.0.0.1:{port}: {error} (choose another :port or stop the existing process)"
        )
    })?;
    drop(listener);
    Ok(())
}

fn accept_loop(
    listener: UnixListener,
    workers: Arc<Mutex<HashMap<String, WorkerPool>>>,
    pending: Arc<Mutex<HashMap<String, Pending>>>,
    pool: Arc<Mutex<SlotPool>>,
    stop: Arc<AtomicBool>,
) {
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        let workers = Arc::clone(&workers);
        let pending = Arc::clone(&pending);
        let pool = Arc::clone(&pool);
        thread::spawn(move || {
            if let Err(err) = handle_client(stream, workers, pending, pool) {
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
            }) if role == "bun" => {
                // Keep the Bun reader hot: slot acquire / notify must not block
                // reading the next INGEST frames off the UDS.
                let workers = Arc::clone(&workers);
                let pending = Arc::clone(&pending);
                let pool = Arc::clone(&pool);
                let response_writer = Arc::clone(&writer);
                thread::spawn(move || {
                    if let Err(err) = start_ingest(
                        workers.as_ref(),
                        pending.as_ref(),
                        pool.as_ref(),
                        request_id.clone(),
                        author,
                        text,
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
                &workers, &pending, &pool, request_id, segment_id, seq, result,
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
            Ok(ControlFrame::Shutdown {}) | Err(_) => break,
            Ok(_) => {}
        }
    }
    Ok(())
}

fn start_ingest(
    workers: &Mutex<HashMap<String, WorkerPool>>,
    pending: &Mutex<HashMap<String, Pending>>,
    pool: &Mutex<SlotPool>,
    request_id: String,
    author: String,
    text: String,
    response_writer: Option<Arc<Mutex<BufWriter<UnixStream>>>>,
) -> Result<(), String> {
    let id = Uuid::new_v4().to_string();
    let record = serde_json::json!({
        "id": id,
        "author": author,
        "text": text,
        "summary": "",
        "score": 0.0,
    });
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

fn on_ack(
    workers: &Mutex<HashMap<String, WorkerPool>>,
    pending: &Mutex<HashMap<String, Pending>>,
    pool: &Mutex<SlotPool>,
    request_id: String,
    segment_id: u64,
    seq: u64,
    result: Option<serde_json::Value>,
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

fn spawn_workers(
    output: &EmitResult,
    lock: &RuntimeLock,
    socket: &Path,
    ipc_dir: &Path,
    data_dir: &Path,
    graph: &sil_core::ExecutableGraph,
    python_replicas: usize,
    go_replicas: usize,
) -> Result<Vec<Child>, String> {
    let mut children = Vec::new();
    for _ in 0..python_replicas {
        children.push(
            Command::new(&lock.python_bin)
                .arg(output.root.join("python/worker.py"))
                .env("SILC_SOCKET", socket)
                .env("SILC_IPC_DIR", ipc_dir)
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
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
                .env("SILC_DB_PATH", data_dir.join("feedback.db"))
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
