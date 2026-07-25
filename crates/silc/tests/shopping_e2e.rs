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
fn shopping_app_resources_web_and_terminal_e2e() {
    free_port(18094);
    free_port(18095);
    let temp = std::env::temp_dir().join(format!(
        "silc-shop-e2e-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&temp).unwrap();

    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shopping_app.silc");

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

    let manifest = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/.runtime/shopping_app/manifest.json"),
    )
    .unwrap();
    assert!(!manifest.contains("portal_kind"));
    assert!(!manifest.contains("inventory"));
    assert!(manifest.contains("products") || manifest.contains("actions"));
    assert!(manifest.contains("terminal"));

    let data_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/.runtime/shopping_app/data");
    for name in ["app.db", "app.db-shm", "app.db-wal"] {
        let _ = std::fs::remove_file(data_dir.join(name));
    }

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
        if let Ok(resp) = ureq::get("http://127.0.0.1:18094/health").call() {
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

    // Empty catalog initially.
    let list = ureq::get("http://127.0.0.1:18094/api/products")
        .call()
        .expect("list products")
        .into_string()
        .unwrap();
    assert!(
        list.trim() == "[]" || list.contains("[]"),
        "expected empty products: {list}"
    );

    // Admin create via resource API.
    let created = ureq::post("http://127.0.0.1:18094/api/products")
        .set("content-type", "application/json")
        .send_string(r#"{"name":"Gala Apples","price":3.5}"#)
        .expect("create product")
        .into_string()
        .unwrap();
    assert!(created.contains("Gala Apples"), "create failed: {created}");

    let list2 = ureq::get("http://127.0.0.1:18094/api/products")
        .call()
        .expect("list products again")
        .into_string()
        .unwrap();
    assert!(
        list2.contains("Gala Apples"),
        "storefront missing product: {list2}"
    );

    // Cart mutation.
    let cart = ureq::post("http://127.0.0.1:18094/api/cart_items")
        .set("content-type", "application/json")
        .send_string(r#"{"product_id":"1","name":"Gala Apples","price":3.5,"quantity":2}"#)
        .expect("add cart")
        .into_string()
        .unwrap();
    assert!(
        cart.contains("Gala Apples") || cart.contains("\"id\""),
        "{cart}"
    );

    let cart_list = ureq::get("http://127.0.0.1:18094/api/cart_items")
        .call()
        .expect("list cart")
        .into_string()
        .unwrap();
    assert!(cart_list.contains("Gala Apples"), "cart empty: {cart_list}");

    // Terminal surface uses the same resources.
    let mut terminal = TcpStream::connect("127.0.0.1:18095").expect("connect terminal");
    let banner = read_until(&mut terminal, ">", Duration::from_secs(5));
    assert!(
        banner.contains("Silc") || banner.contains("/list"),
        "missing terminal help: {banner:?}"
    );
    terminal.write_all(b"/list products\n").unwrap();
    let listed = read_until(&mut terminal, "Gala", Duration::from_secs(5));
    assert!(listed.contains("Gala"), "terminal list failed: {listed:?}");
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
