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
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/chatApp/main.silc");
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
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/chatApp/.runtime/main");
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
    assert!(
        worker.contains("json_extract(payload, '$.session_id') = ?"),
        "worker must filter scoped history in SQLite"
    );
    let manifest = std::fs::read_to_string(root.join("manifest.json")).unwrap();
    assert!(!manifest.contains("portal_kind"));
    assert!(manifest.contains("llm") || manifest.contains("llm.complete"));
    assert!(
        manifest.contains("silclm"),
        "default model must resolve to silclm even when source omits :model"
    );
    assert!(!manifest.contains("llama3.2-1b"));
    assert!(manifest.contains("terminal"));
    let source = std::fs::read_to_string(example).unwrap();
    assert!(
        source.contains("llm::complete()") && !source.contains(":model("),
        "chatApp should rely on the implicit silclm default"
    );
}

#[test]
fn multi_session_chat_builds_race_safe_ui() {
    let root = std::env::temp_dir().join(format!("silc-multi-chat-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("multi_chat.silc");
    std::fs::write(
        &source,
        r#"@version("0.4.0")
contract ChatRecord {
    has Str $.prompt;
    has Str $.reply;
    has Str $.session_id;
}
component ChatPage {
    has state Str $.prompt = "";
    has state Str $.active_session = "session-a";
    method render() {
        ui::page(
            when $.active_session {
                ui::text(:text("Selected"))
            },
            ui::chat_history(:title("History")),
            ui::chat(
                :value($.prompt),
                :session($.active_session),
                :on(send(on_send))
            )
        )
    }
    method on_send() { Assistant.complete(); }
}
app ChatApp {
    route "/" => ChatPage;
}
processor Assistant {
    has Str $.model_ref = "silclm";
    method complete(ChatRecord $record) {
        $record.prompt ==> llm::complete(:model("silclm"))
    }
}
"#,
    )
    .unwrap();

    let build = Command::new(silc_bin())
        .args(["build", source.to_str().unwrap()])
        .output()
        .expect("build multi-session chat");
    assert!(
        build.status.success(),
        "build failed: {}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let app =
        std::fs::read_to_string(root.join(".runtime/multi_chat/typescript/src/App.tsx")).unwrap();
    assert!(app.contains("AbortController"));
    assert!(app.contains("pending: true"));
    assert!(app.contains("capturedSession"));
    assert!(app.contains("focusKey={active_session}"));
    assert!(app.contains("historyLoading"));
    assert!(app.contains("historyError || chatError"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[ignore = "requires local GGUF model download"]
fn chat_assistant_real_completion_e2e() {
    free_port(18090);
    free_port(18091);

    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/chatApp/main.silc");
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
        .env("SILC_HTTP_PORT", "18090")
        .env("SILC_TERMINAL_PORT", "18091")
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("run chatApp");

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
        .send_string(r#"{"prompt":"Reply with alpha only.","session_id":"session-alpha"}"#)
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

    let second = ureq::post("http://127.0.0.1:18090/complete")
        .set("content-type", "application/json")
        .send_string(r#"{"prompt":"Reply with beta only.","session_id":"session-beta"}"#)
        .expect("post second /complete");
    let second_status = second.status();
    let second_body = second.into_string().unwrap_or_default();
    assert_eq!(second_status, 200, "second /complete failed: {second_body}");

    let alpha_history = ureq::get("http://127.0.0.1:18090/history?session_id=session-alpha")
        .call()
        .expect("get alpha /history")
        .into_string()
        .unwrap();
    assert!(
        alpha_history.contains("Reply with alpha only.")
            && !alpha_history.contains("Reply with beta only."),
        "alpha history leaked another session: {alpha_history}"
    );

    let beta_history = ureq::get("http://127.0.0.1:18090/history?session_id=session-beta")
        .call()
        .expect("get beta /history")
        .into_string()
        .unwrap();
    assert!(
        beta_history.contains("Reply with beta only.")
            && !beta_history.contains("Reply with alpha only."),
        "beta history leaked another session: {beta_history}"
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
    let first = read_until(&mut terminal, ">", Duration::from_secs(180));
    // The initial prompt can arrive in a separate packet after the banner.
    let reply = if first.trim() == ">" {
        read_until(&mut terminal, ">", Duration::from_secs(180))
    } else {
        first
    };
    assert!(
        !reply.contains("error:") && reply.len() > 2,
        "terminal /chat failed or hung: {reply:?}"
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&temp);
}
