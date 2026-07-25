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
fn init_scaffolds_runnable_dual_surface_app() {
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

    let main = fs::read_to_string(root.join("main.silc")).unwrap();
    assert!(main.contains("@version(\"0.2.0\")"));
    assert!(main.contains("is component"));
    assert!(main.contains("is app"));
    assert!(main.contains("ui::web"));
    assert!(main.contains("ui::terminal"));
    assert!(!main.contains("is view"));
    assert!(!main.contains("PortalKind"));

    let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(agents.contains("is component"));
    assert!(agents.contains("is resource") || agents.contains("resource"));
    assert!(agents.contains("ui::web") && agents.contains("ui::terminal"));
    assert!(
        agents.contains("Do not create a `stdlib/`")
            || agents.to_lowercase().contains("no separate"),
        "AGENTS.md must forbid a separate component stdlib"
    );
    assert!(
        agents.contains("Removed in 0.2.0") && agents.contains("Portal profiles"),
        "AGENTS.md must document portal-profile removal"
    );

    let compile = silc()
        .arg("build")
        .arg(root.join("main.silc"))
        .output()
        .expect("compile");
    assert!(
        compile.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&compile.stderr),
        String::from_utf8_lossy(&compile.stdout)
    );
    let stdout = String::from_utf8_lossy(&compile.stdout);
    assert!(
        stdout.contains("mode:  runnable") || stdout.contains("mode: runnable"),
        "expected runnable mode, got:\n{stdout}"
    );

    let runtime = root.join(".runtime");
    assert!(runtime.is_dir(), "expected .runtime after build");
    // Find generated app artifacts under .runtime/<stem>/
    let mut found_web = false;
    let mut found_terminal = false;
    let mut found_manifest = false;
    if let Ok(entries) = fs::read_dir(&runtime) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if dir.join("manifest.json").is_file() {
                found_manifest = true;
                let manifest = fs::read_to_string(dir.join("manifest.json")).unwrap();
                assert!(!manifest.contains("portal_kind"));
                assert!(manifest.contains("web") && manifest.contains("terminal"));
            }
            if dir.join("typescript/src/App.tsx").is_file() {
                found_web = true;
            }
            if dir.join("typescript/terminal.ts").is_file() {
                found_terminal = true;
            }
        }
    }
    assert!(found_manifest, "manifest.json missing after init build");
    assert!(found_web, "web App.tsx missing after init build");
    assert!(found_terminal, "terminal.ts missing after init build");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn direct_compile_examples_still_works() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data_pipeline.silc");
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
