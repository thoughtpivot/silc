use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn silc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_silc"))
}

/// Downloads the pinned ~808 MB GGUF on first use and performs real inference.
/// Run explicitly with:
/// `cargo test -p silc --test llm_e2e -- --ignored --nocapture`
#[test]
#[ignore = "downloads pinned GGUF and runs real local Llama inference"]
fn llm_portal_real_completion_sqlite_e2e() {
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/llm_portal.silc");
    let build = Command::new(silc_bin())
        .args(["build", example.to_str().unwrap()])
        .output()
        .expect("build llm portal");
    assert!(
        build.status.success(),
        "build failed: {}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let temp = std::env::temp_dir().join(format!("silc-llm-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&temp).unwrap();
    let log_path = temp.join("run.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut child = Command::new(silc_bin())
        .arg(example.to_str().unwrap())
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("run llm portal");

    let started = Instant::now();
    let mut healthy = false;
    while started.elapsed() < Duration::from_secs(300) {
        if let Ok(response) = ureq::get("http://127.0.0.1:18090/health")
            .timeout(Duration::from_secs(2))
            .call()
        {
            if response.status() == 200 {
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
    assert!(healthy, "LLM portal health never became ready");

    let response = ureq::post("http://127.0.0.1:18090/complete")
        .timeout(Duration::from_secs(180))
        .set("content-type", "application/json")
        .send_string(r#"{"prompt":"Reply with exactly the word SILC."}"#)
        .unwrap_or_else(|error| {
            let log = std::fs::read_to_string(&log_path).unwrap_or_default();
            panic!("completion failed: {error}\n{log}")
        });
    let body = response.into_string().unwrap();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["ok"], true, "{body}");
    assert_eq!(json["model"], "llama3.2-1b", "{body}");
    assert!(
        json["reply"]
            .as_str()
            .is_some_and(|reply| !reply.trim().is_empty()),
        "{body}"
    );

    let history = ureq::get("http://127.0.0.1:18090/history")
        .timeout(Duration::from_secs(10))
        .call()
        .expect("load persisted history")
        .into_string()
        .unwrap();
    let history_json: serde_json::Value = serde_json::from_str(&history).unwrap();
    assert_eq!(history_json["ok"], true, "{history}");
    assert!(
        history_json["turns"].as_array().is_some_and(|turns| {
            turns.iter().any(|turn| {
                turn["prompt"] == "Reply with exactly the word SILC."
                    && turn["reply"]
                        .as_str()
                        .is_some_and(|reply| !reply.is_empty())
            })
        }),
        "{history}"
    );

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }
    let _ = child.wait();

    let db = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/.runtime/llm_portal/data/chat.db");
    assert!(db.is_file(), "SQLite db missing at {}", db.display());
    let _ = std::fs::remove_dir_all(temp);
}
