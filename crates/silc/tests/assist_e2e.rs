//! Offline + optional live tests for `silc assist` (ADR-008).

use std::path::PathBuf;
use std::process::Command;

fn silc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_silc"))
}

#[test]
fn assist_usage_mentions_task_flag() {
    let output = Command::new(silc_bin()).output().expect("silc");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("silc assist"),
        "CLI usage should mention assist:\n{text}"
    );
}

#[test]
fn assist_rejects_empty_invocation() {
    let output = Command::new(silc_bin())
        .arg("assist")
        .output()
        .expect("silc assist");
    assert!(!output.status.success());
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(
        err.contains("usage") || err.contains("task"),
        "unexpected stderr: {err}"
    );
}

/// Live silclm assist run — downloads ~2.02 GB model + llama-cpp-python on first use.
#[test]
#[ignore]
fn assist_live_writes_valid_program() {
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("silc-assist-e2e");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = out_dir.join("notes.silc");

    let output = Command::new(silc_bin())
        .args([
            "assist",
            "Write a minimal dual-surface Silc notes app with a text field and submit button.",
            "--out",
            out.to_str().unwrap(),
            "--max-turns",
            "10",
            "--wall-clock-secs",
            "600",
        ])
        .output()
        .expect("silc assist live");

    assert!(
        output.status.success(),
        "assist failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let program = std::fs::read_to_string(&out).expect("read out");
    assert!(
        program.contains("@version"),
        "expected Silc program, got:\n{program}"
    );

    let check = Command::new(silc_bin())
        .args(["build", out.to_str().unwrap()])
        .output()
        .expect("silc build assist output");
    assert!(
        check.status.success(),
        "assist output failed silc build:\n{}\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}
