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
fn scored_form_web_and_terminal_e2e() {
    free_port(18080);
    free_port(18023);
    let temp = std::env::temp_dir().join(format!(
        "silc-e2e-home-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&temp).unwrap();

    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scored_form.silc");

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
        .join("tests/fixtures/.runtime/scored_form/typescript");
    assert!(
        runtime.join("dist/index.html").is_file(),
        "web index missing"
    );
    assert!(
        runtime.join("dist/assets/app.js").is_file(),
        "web app.js missing under dist/assets/"
    );
    assert!(
        runtime.join("dist/assets/theme.css").is_file(),
        "web theme.css missing under dist/assets/"
    );
    assert!(
        runtime.join("terminal.ts").is_file(),
        "terminal surface module missing"
    );
    let manifest = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/.runtime/scored_form/manifest.json"),
    )
    .unwrap();
    assert!(!manifest.contains("portal_kind"));
    assert!(manifest.contains("\"surfaces\""));
    assert!(manifest.contains("web") && manifest.contains("terminal"));

    let log_path = temp.join("run.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut child = Command::new(silc_bin())
        .arg(example.to_str().unwrap())
        .env("SILC_HTTP_PORT", "18080")
        .env("SILC_TERMINAL_PORT", "18023")
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

    let page = ureq::get("http://127.0.0.1:18080/")
        .call()
        .expect("get ui")
        .into_string()
        .unwrap();
    assert!(
        page.contains("id=\"app\"") || page.contains("silc") || page.contains("app.js"),
        "expected React shell HTML, got: {page}"
    );
    assert!(
        page.contains("/assets/app.js") && page.contains("/assets/theme.css"),
        "index.html must reference /assets/ paths, got: {page}"
    );

    let js = ureq::get("http://127.0.0.1:18080/assets/app.js")
        .call()
        .expect("get app.js");
    let js_ct = js.header("content-type").unwrap_or("").to_string();
    let js_body = js.into_string().unwrap();
    assert!(
        js_body.len() > 1_000 && !js_body.contains("<!DOCTYPE html>"),
        "expected real JS at /assets/app.js, got {} bytes content-type={js_ct}: {}",
        js_body.len(),
        &js_body[..js_body.len().min(120)]
    );
    assert!(
        js_ct.contains("javascript") || js_ct.contains("ecmascript") || js_ct.is_empty(),
        "unexpected content-type for /assets/app.js: {js_ct}"
    );

    let css = ureq::get("http://127.0.0.1:18080/assets/theme.css")
        .call()
        .expect("get theme.css")
        .into_string()
        .unwrap();
    assert!(
        css.len() > 20 && !css.contains("<!DOCTYPE html>"),
        "expected real CSS at /assets/theme.css, got {} bytes: {}",
        css.len(),
        &css[..css.len().min(120)]
    );

    let mut terminal = TcpStream::connect("127.0.0.1:18023").expect("connect terminal");
    let banner = read_until(&mut terminal, ">", Duration::from_secs(5));
    assert!(
        banner.contains("Silc") || banner.contains("terminal") || banner.contains("/help"),
        "missing terminal banner: {banner:?}"
    );
    terminal.write_all(b"/help\n").expect("write terminal help");
    let help = read_until(&mut terminal, "submit", Duration::from_secs(5));
    assert!(
        help.contains("/submit") || help.contains("Commands") || banner.contains("/submit"),
        "terminal help missing submit: banner={banner:?} help={help:?}"
    );
    terminal.write_all(b"/quit\n").ok();

    let resp = ureq::post("http://127.0.0.1:18080/submit")
        .timeout(Duration::from_secs(15))
        .set("content-type", "application/json")
        .send_string(r#"{"author":"e2e","text":"silc scored form works"}"#);
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
        libc::kill(child.id() as libc::pid_t, libc::SIGKILL);
    }
    let _ = child.wait();

    let db = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/.runtime/scored_form/data/app.db");
    assert!(db.is_file(), "sqlite db missing at {}", db.display());
    let _ = std::fs::remove_dir_all(temp);
}
