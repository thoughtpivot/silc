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
fn feedback_portal_http_sqlite_e2e() {
    free_port(18080);
    free_port(18023);
    let temp = std::env::temp_dir().join(format!(
        "silc-e2e-home-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&temp).unwrap();

    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/feedback_portal.silc");

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
        .join("../../examples/.runtime/feedback_portal/typescript");
    assert!(
        runtime.join("dist/index.html").is_file(),
        "ui::web index missing"
    );
    assert!(
        runtime.join("dist/app.js").is_file(),
        "ui::web app.js missing"
    );
    assert!(
        runtime.join("dist/theme.css").is_file(),
        "ui::web theme missing"
    );
    let app_js = std::fs::read_to_string(runtime.join("dist/app.js")).unwrap_or_default();
    assert!(
        app_js.contains("Silc") || app_js.contains("submit") || app_js.len() > 100,
        "bundled app.js looks empty"
    );

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
        if let Ok(resp) = ureq::get("http://127.0.0.1:18080/health").call() {
            let body = resp.into_string().unwrap_or_default();
            if body.contains("\"ok\":true") || body.contains("\"ok\": true") {
                assert!(
                    body.contains("react") || body.contains("web"),
                    "health should advertise ui substrate: {body}"
                );
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

    let page = ureq::get("http://127.0.0.1:18080/")
        .call()
        .expect("get ui")
        .into_string()
        .unwrap();
    assert!(
        page.contains("id=\"app\"") || page.contains("silc"),
        "expected React shell HTML, got: {page}"
    );
    assert!(
        page.contains("/assets/app.js") || page.contains("app.js"),
        "expected bundled app asset link"
    );

    let asset = ureq::get("http://127.0.0.1:18080/assets/app.js")
        .call()
        .expect("get app.js");
    assert_eq!(asset.status(), 200);

    let mut terminal = TcpStream::connect("127.0.0.1:18023").expect("connect terminal UI");
    let banner = read_until(&mut terminal, "Author:", Duration::from_secs(5));
    assert!(
        banner.contains("THOUGHTPIVOT") && banner.contains("Feedback Portal"),
        "missing terminal banner: {banner:?}"
    );
    terminal
        .write_all(b"terminal-e2e\r\nfeedback through telnet\r\n")
        .expect("write terminal feedback");
    let result = read_until(&mut terminal, "\"ok\"", Duration::from_secs(10));
    assert!(
        result.contains("\"ok\"") && result.contains("true"),
        "terminal submission failed: {result:?}"
    );
    terminal.write_all(b"/quit\r\n").ok();

    let resp = ureq::post("http://127.0.0.1:18080/submit")
        .set("content-type", "application/json")
        .send_string(r#"{"author":"e2e","text":"silc feedback portal works"}"#);
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
        .join("../../examples/.runtime/feedback_portal/data/feedback.db");
    assert!(db.is_file(), "sqlite db missing at {}", db.display());
    let _ = std::fs::remove_dir_all(temp);
}
