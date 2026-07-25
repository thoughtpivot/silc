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
    let mut buffer = [0u8; 4096];
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

/// Offline codegen/build smoke for the chat example (real LLM completion is ignored).
#[test]
fn chat_assistant_builds_dual_surface() {
    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/chat_assistant.silc");
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
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/.runtime/chat_assistant");
    assert!(root.join("typescript/worker.ts").is_file());
    assert!(root.join("typescript/terminal.ts").is_file());
    assert!(root.join("typescript/src/App.tsx").is_file());
    assert!(root.join("python/requirements.txt").is_file());
    let app = std::fs::read_to_string(root.join("typescript/src/App.tsx")).unwrap();
    assert!(app.contains("ChatThread"));
    assert!(app.contains("fetch(\"/history\")") || app.contains("/history"));
    assert!(app.contains("__chatComplete"));
    assert!(!app.contains("items={[]}"));
    let worker = std::fs::read_to_string(root.join("typescript/worker.ts")).unwrap();
    assert!(
        worker.contains("text: prompt") || worker.contains("author: \"\""),
        "worker must send author/text on INGEST for /complete"
    );
    let manifest = std::fs::read_to_string(root.join("manifest.json")).unwrap();
    assert!(!manifest.contains("portal_kind"));
    assert!(manifest.contains("llm") || manifest.contains("llm.complete"));
    assert!(manifest.contains("terminal"));
}

#[test]
#[ignore = "requires local GGUF model download"]
fn chat_assistant_real_completion_e2e() {
    free_port(18090);
    free_port(18091);

    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/chat_assistant.silc");
    let temp = std::env::temp_dir().join(format!(
        "silc-chat-e2e-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::create_dir_all(&temp);

    let log_path = temp.join("run.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut child = Command::new(silc_bin())
        .arg(example.to_str().unwrap())
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("run chat_assistant");

    let ready = Instant::now();
    let mut healthy = false;
    while ready.elapsed() < Duration::from_secs(300) {
        if let Ok(resp) = ureq::get("http://127.0.0.1:18090/health").call() {
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
    assert!(healthy, "chat health never became ready");

    let page = ureq::get("http://127.0.0.1:18090/")
        .call()
        .expect("get ui")
        .into_string()
        .unwrap();
    assert!(
        page.contains("/assets/app.js"),
        "expected React shell with /assets/app.js, got: {page}"
    );

    let complete = ureq::post("http://127.0.0.1:18090/complete")
        .set("content-type", "application/json")
        .send_string(r#"{"prompt":"Say hi in one short word."}"#)
        .expect("post /complete");
    let status = complete.status();
    let body = complete.into_string().unwrap_or_default();
    assert_eq!(status, 200, "/complete failed: {body}");
    assert!(
        body.contains("\"ok\":true") || body.contains("\"ok\": true"),
        "expected ok complete response: {body}"
    );
    assert!(
        body.contains("\"reply\"")
            && !body.contains("\"reply\":\"\"")
            && !body.contains("\"reply\": \"\""),
        "expected non-empty reply: {body}"
    );

    let history = ureq::get("http://127.0.0.1:18090/history")
        .call()
        .expect("get /history")
        .into_string()
        .unwrap();
    assert!(
        history.contains("Say hi") || history.contains("prompt"),
        "expected chat history row, got: {history}"
    );

    let mut terminal = TcpStream::connect("127.0.0.1:18091").expect("connect terminal");
    let banner = read_until(&mut terminal, ">", Duration::from_secs(5));
    assert!(
        banner.contains("Silc") || banner.contains("terminal") || banner.contains("/help"),
        "missing terminal banner: {banner:?}"
    );
    terminal
        .write_all(b"/chat Hello from terminal\n")
        .expect("write terminal chat");
    let reply = read_until(&mut terminal, ">", Duration::from_secs(180));
    assert!(
        !reply.contains("error:") && reply.len() > 2,
        "terminal /chat failed or hung: {reply:?}"
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&temp);
}
