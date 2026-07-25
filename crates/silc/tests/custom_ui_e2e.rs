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

#[test]
fn custom_feedback_view_http_sqlite_e2e() {
    free_port(18088);
    let temp = std::env::temp_dir().join(format!(
        "silc-custom-ui-e2e-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&temp).unwrap();

    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/custom_feedback_ui.silc");

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

    let runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/.runtime/custom_feedback_ui/typescript");
    assert!(runtime.join("dist/app.js").is_file(), "ui bundle missing");

    let app = std::fs::read_to_string(runtime.join("src/App.tsx")).unwrap_or_default();
    assert!(app.contains("AppBar"), "expected AppBar in generated App");
    assert!(app.contains("SidePanel"), "expected SidePanel");
    assert!(app.contains("RadioGroup"), "expected RadioGroup");
    assert!(app.contains("Toolbar"), "expected Toolbar");
    assert!(app.contains("FeedbackView") || app.contains("View: FeedbackView"));
    assert!(app.contains("/submit"));
    assert!(app.contains("Good") && app.contains("Okay") && app.contains("Bad"));

    let manifest = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/.runtime/custom_feedback_ui/manifest.json"),
    )
    .unwrap_or_default();
    assert!(manifest.contains("\"view\": \"FeedbackView\"") || manifest.contains("FeedbackView"));
    assert!(manifest.contains("app-bar.tsx"));

    let log_path = temp.join("run.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut child = Command::new(silc_bin())
        .arg(example.to_str().unwrap())
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("run");

    let ready = Instant::now();
    let mut healthy = false;
    while ready.elapsed() < Duration::from_secs(120) {
        if let Ok(resp) = ureq::get("http://127.0.0.1:18088/health").call() {
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
        thread::sleep(Duration::from_millis(250));
    }
    assert!(healthy, "health never became ready");
    thread::sleep(Duration::from_millis(300));

    let page = ureq::get("http://127.0.0.1:18088/")
        .call()
        .expect("get ui")
        .into_string()
        .unwrap();
    assert!(
        page.contains("id=\"app\"") || page.contains("silc"),
        "expected React shell HTML, got: {page}"
    );

    let resp = ureq::post("http://127.0.0.1:18088/submit")
        .set("content-type", "application/json")
        .send_string(
            r#"{"author":"view-e2e","text":"custom declarative ui works","rating":"Good"}"#,
        );
    let body = match resp {
        Ok(r) => r.into_string().unwrap(),
        Err(e) => {
            let log = std::fs::read_to_string(&log_path).unwrap_or_default();
            panic!("post failed: {e}\n{log}");
        }
    };
    assert!(
        body.contains("\"ok\":true") || body.contains("\"ok\": true"),
        "{body}"
    );

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }
    let _ = child.wait();

    let db = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/.runtime/custom_feedback_ui/data/feedback.db");
    assert!(db.is_file(), "sqlite db missing at {}", db.display());
    let _ = std::fs::remove_dir_all(temp);
}
