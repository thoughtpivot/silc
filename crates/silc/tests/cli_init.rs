use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn silc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_silc"))
}

fn tempdir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("silc-init-{}-{}", std::process::id(), nanos));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn init_scaffolds_project_and_compiles() {
    let root = tempdir();
    let init = silc()
        .args(["init", root.to_str().unwrap()])
        .output()
        .expect("init");
    assert!(
        init.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(root.join("main.silc").is_file());
    assert!(root.join("AGENTS.md").is_file());

    let compile = silc()
        .arg("build")
        .arg(root.join("main.silc"))
        .output()
        .expect("compile");
    assert!(
        compile.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    assert!(root.join(".runtime/init/manifest.json").is_file() || root.join(".runtime").is_dir());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn direct_compile_examples_still_works() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/data_pipeline.silc");
    let root = tempdir();
    let entry = root.join("data_pipeline.silc");
    fs::copy(&example, &entry).expect("copy");

    let compile = silc().arg("build").arg(&entry).output().expect("compile");
    assert!(
        compile.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let _ = fs::remove_dir_all(&root);
}
