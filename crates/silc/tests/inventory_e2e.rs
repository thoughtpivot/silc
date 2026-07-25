use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn silc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_silc"))
}

fn read_until(stream: &mut TcpStream, needle: &str, timeout: Duration) -> String {
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    let started = Instant::now();
    let mut out = Vec::new();
    let mut buffer = [0u8; 4096];
    while started.elapsed() < timeout {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                out.extend_from_slice(&buffer[..count]);
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

fn response_json(response: ureq::Response) -> serde_json::Value {
    serde_json::from_str(&response.into_string().unwrap()).unwrap()
}

/// Uses the cached local GGUF and exercises real AI interpretation and scoped chat.
/// Run explicitly with:
/// `cargo test -p silc --test inventory_e2e -- --ignored --nocapture`
#[test]
#[ignore = "loads the pinned local Llama model for AI search and scoped chat"]
fn grocery_inventory_web_ai_and_terminal_e2e() {
    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/grocery_inventory.silc");
    let build = Command::new(silc_bin())
        .args(["build", example.to_str().unwrap()])
        .output()
        .expect("build grocery inventory");
    assert!(
        build.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let temp = std::env::temp_dir().join(format!("silc-inventory-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&temp).unwrap();
    let log_path = temp.join("run.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut child = Command::new(silc_bin())
        .arg(example.to_str().unwrap())
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("run grocery inventory");

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(180) {
        if ureq::get("http://127.0.0.1:18094/health")
            .timeout(Duration::from_secs(2))
            .call()
            .is_ok()
        {
            break;
        }
        if let Ok(Some(status)) = child.try_wait() {
            panic!(
                "silc exited early: {status}\n{}",
                std::fs::read_to_string(&log_path).unwrap_or_default()
            );
        }
        thread::sleep(Duration::from_millis(300));
    }

    let products = response_json(ureq::get("http://127.0.0.1:18094/products").call().unwrap());
    assert_eq!(products["count"], 24);
    let filtered = response_json(
        ureq::get("http://127.0.0.1:18094/products?category=Snacks&max_price=5")
            .call()
            .unwrap(),
    );
    assert!(filtered["count"].as_u64().is_some_and(|count| count > 0));

    let ai = response_json(
        ureq::post("http://127.0.0.1:18094/ai_search")
            .timeout(Duration::from_secs(180))
            .set("content-type", "application/json")
            .send_string(r#"{"query":"in-stock snacks under $5"}"#)
            .unwrap(),
    );
    assert_eq!(ai["ok"], true);
    assert!(ai["filters"].is_object());

    let scope = filtered["products"].clone();
    let chat_payload = serde_json::json!({
        "prompt": "Name one visible product.",
        "visible_products": scope,
    })
    .to_string();
    let chat = response_json(
        ureq::post("http://127.0.0.1:18094/complete")
            .timeout(Duration::from_secs(180))
            .set("content-type", "application/json")
            .send_string(&chat_payload)
            .unwrap(),
    );
    assert_eq!(chat["ok"], true);
    assert_eq!(chat["visible_count"], filtered["count"]);

    let mut terminal = TcpStream::connect("127.0.0.1:18024").unwrap();
    let banner = read_until(&mut terminal, "inventory>", Duration::from_secs(5));
    assert!(banner.contains("THOUGHTPIVOT"));
    terminal.write_all(b"/list\r\n").unwrap();
    let listing = read_until(&mut terminal, "Gala Apples", Duration::from_secs(5));
    assert!(listing.contains("Gala Apples"));
    terminal.write_all(b"/quit\r\n").ok();

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(temp);
}
