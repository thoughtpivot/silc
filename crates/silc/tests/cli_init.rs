use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn silc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_silc"))
}

fn tempdir() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "silc-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn usage_mentions_init() {
    let output = silc().output().expect("run silc");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("silc init"));
    assert!(stdout.contains("silc build"));
}

#[test]
fn init_path_creates_scaffold_and_compiles() {
    let root = tempdir();
    let project = root.join("app");

    let init = silc()
        .arg("init")
        .arg(&project)
        .output()
        .expect("silc init");
    assert!(
        init.status.success(),
        "init stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(project.join("AGENTS.md").is_file());
    assert!(project.join("main.silc").is_file());
    assert!(project.join(".gitignore").is_file());

    let agents = fs::read_to_string(project.join("AGENTS.md")).unwrap();
    assert!(agents.contains("https://github.com/thoughtpivot/silc"));
    assert!(agents.contains("Raku-inspired"));

    let compile = silc()
        .arg(project.join("main.silc"))
        .output()
        .expect("silc main.silc");
    assert!(
        compile.status.success(),
        "compile stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    assert!(project.join(".runtime/main/manifest.json").is_file());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn init_refuses_overwrite_of_main() {
    let root = tempdir();
    let project = root.join("exists");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("main.silc"), "already here\n").unwrap();

    let init = silc()
        .arg("init")
        .arg(&project)
        .output()
        .expect("silc init");
    assert!(!init.status.success());
    let stderr = String::from_utf8_lossy(&init.stderr);
    assert!(stderr.contains("refusing to overwrite"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn init_merges_existing_gitignore() {
    let root = tempdir();
    let project = root.join("merge");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join(".gitignore"), "vendor/\n").unwrap();

    let init = silc()
        .arg("init")
        .arg(&project)
        .output()
        .expect("silc init");
    assert!(init.status.success());
    let gi = fs::read_to_string(project.join(".gitignore")).unwrap();
    assert!(gi.contains("vendor/"));
    assert!(gi.lines().any(|l| l.trim() == ".runtime/"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn init_silc_path_still_compiles_not_subcommand() {
    let root = tempdir();
    // Copy a known-good example into a file named init.silc
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sensor_alert.silc");
    let entry = root.join("init.silc");
    fs::copy(&example, &entry).expect("copy example");

    let compile = silc().arg(&entry).output().expect("silc init.silc");
    assert!(
        compile.status.success(),
        "compile stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    assert!(root.join(".runtime/init/manifest.json").is_file());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn direct_compile_examples_still_works() {
    let example =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/article_pipeline.silc");
    let root = tempdir();
    let entry = root.join("article_pipeline.silc");
    fs::copy(&example, &entry).expect("copy");

    let compile = silc().arg(&entry).output().expect("compile");
    assert!(
        compile.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let _ = fs::remove_dir_all(&root);
}
