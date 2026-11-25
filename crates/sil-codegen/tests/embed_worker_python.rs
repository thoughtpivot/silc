//! Runs tests/embed_worker/test_embed_worker.py against the embed worker and
//! embedding models emitted for tests/fixtures/embedding_app.silc. Needs a
//! `python3` on PATH (or `SILC_TEST_PYTHON`); the sqlite-vec half runs only
//! where `sqlite_vec` imports.

use std::path::{Path, PathBuf};
use std::process::Command;

fn python() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SILC_TEST_PYTHON") {
        return Some(PathBuf::from(path));
    }
    let ok = Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then(|| PathBuf::from("python3"))
}

#[test]
fn embed_worker_python() {
    let Some(python) = python() else {
        eprintln!("skipping embed worker checks: no python3");
        return;
    };
    let source = include_str!("../../silc/tests/fixtures/embedding_app.silc");
    let program = sil_parser::parse(source).expect("parse");
    program.validate().expect("validate");
    let decisions = sil_router::route_program(&program);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = std::env::temp_dir().join(format!("silc-embed-worker-{nonce}"));
    sil_codegen::emit(
        &program,
        &decisions,
        Path::new("embedding_app.silc"),
        &out,
        "test",
    )
    .expect("emit");
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/embed_worker/test_embed_worker.py");
    let run = Command::new(&python)
        .arg(&script)
        .arg(out.join("python"))
        .output()
        .expect("run python");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "embed worker checks failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(stdout.contains("ok openai-compatible"), "{stdout}");
    if stdout.contains("skip nearest") {
        eprintln!("embed worker: sqlite_vec is not installed; nearest ranking not checked");
    } else {
        assert!(stdout.contains("ok nearest"), "{stdout}");
    }
    let _ = std::fs::remove_dir_all(&out);
}
