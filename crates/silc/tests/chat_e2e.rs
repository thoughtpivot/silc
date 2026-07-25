use std::path::PathBuf;
use std::process::Command;

fn silc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_silc"))
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
    let manifest = std::fs::read_to_string(root.join("manifest.json")).unwrap();
    assert!(!manifest.contains("portal_kind"));
    assert!(manifest.contains("llm") || manifest.contains("llm.complete"));
    assert!(manifest.contains("terminal"));
}

#[test]
#[ignore = "requires local GGUF model download"]
fn chat_assistant_real_completion_e2e() {
    // Placeholder for nightly/manual runs with a provisioned model.
}
