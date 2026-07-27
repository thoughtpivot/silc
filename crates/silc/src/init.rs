use std::fs;
use std::path::{Path, PathBuf};

const AGENTS_MD: &str = include_str!("../templates/AGENTS.md");
const MAIN_SILC: &str = include_str!("../templates/main.silc");
const GITIGNORE_ENTRY: &str = ".runtime/";
const SILC_META_ENTRY: &str = ".silc/";

pub fn run(path: Option<&str>) -> Result<(), String> {
    let root = match path {
        None => PathBuf::from("."),
        Some(p) => PathBuf::from(p),
    };

    if root.exists() {
        if !root.is_dir() {
            return Err(format!(
                "init path exists and is not a directory: {}",
                root.display()
            ));
        }
    } else {
        fs::create_dir_all(&root)
            .map_err(|e| format!("failed to create directory {}: {e}", root.display()))?;
    }

    let agents = root.join("AGENTS.md");
    let main = root.join("main.silc");
    let gitignore = root.join(".gitignore");

    if agents.exists() {
        return Err(format!(
            "refusing to overwrite existing file: {}",
            agents.display()
        ));
    }
    if main.exists() {
        return Err(format!(
            "refusing to overwrite existing file: {}",
            main.display()
        ));
    }

    fs::write(&agents, AGENTS_MD)
        .map_err(|e| format!("failed to write {}: {e}", agents.display()))?;
    fs::write(&main, MAIN_SILC).map_err(|e| format!("failed to write {}: {e}", main.display()))?;

    ensure_gitignore(&gitignore)?;

    println!("silc: provisioning owned Bun / CPython / Go engines…");
    let lock = crate::runtimes::ensure_runtimes()?;
    crate::runtimes::write_lock(&root, &lock)?;

    println!("silc {}", env!("CARGO_PKG_VERSION"));
    println!("initialized: {}", display_root(&root));
    println!("  AGENTS.md");
    println!("  main.silc");
    println!("  .gitignore  (+ {GITIGNORE_ENTRY} / {SILC_META_ENTRY})");
    println!("  .silc/runtimes.lock.json");
    println!(
        "  engines: bun {} / cpython {} / go {}",
        lock.bun_version, lock.python_version, lock.go_version
    );
    println!();
    println!("next: silc main.silc");
    Ok(())
}

fn ensure_gitignore(path: &Path) -> Result<(), String> {
    if path.exists() {
        let existing = fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        let mut updated = existing;
        let mut changed = false;
        if !gitignore_has_line(&updated, GITIGNORE_ENTRY) {
            if !updated.ends_with('\n') && !updated.is_empty() {
                updated.push('\n');
            }
            updated.push_str("# Generated Silc runtime (do not hand-edit)\n");
            updated.push_str(GITIGNORE_ENTRY);
            updated.push('\n');
            changed = true;
        }
        if !gitignore_has_line(&updated, SILC_META_ENTRY) {
            if !updated.ends_with('\n') && !updated.is_empty() {
                updated.push('\n');
            }
            updated.push_str("# Compiler-owned runtime lock/state (do not hand-edit)\n");
            updated.push_str(SILC_META_ENTRY);
            updated.push('\n');
            changed = true;
        }
        if changed {
            fs::write(path, updated)
                .map_err(|e| format!("failed to update {}: {e}", path.display()))?;
        }
    } else {
        let contents = include_str!("../templates/gitignore");
        fs::write(path, contents)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    }
    Ok(())
}

fn gitignore_has_line(contents: &str, entry: &str) -> bool {
    let bare = entry.trim_end_matches('/');
    contents.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == entry || trimmed == bare || trimmed == format!("**/{entry}")
    })
}

fn display_root(root: &Path) -> String {
    if root == Path::new(".") {
        ".".to_string()
    } else {
        root.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    // Serialize tests that chdir.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn cwd_guard() -> std::sync::MutexGuard<'static, ()> {
        CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn init_current_dir_creates_scaffold() {
        let _guard = cwd_guard();
        let tmp = tempfile();
        let prev = env::current_dir().unwrap();
        env::set_current_dir(&tmp).unwrap();
        run(None).expect("init");
        assert!(tmp.join("AGENTS.md").is_file());
        assert!(tmp.join("main.silc").is_file());
        assert!(tmp.join(".gitignore").is_file());
        let main = fs::read_to_string(tmp.join("main.silc")).unwrap();
        assert!(main.contains("component HomePage"));
        assert!(main.contains("app MyApp"));
        assert!(main.contains("@version(\"0.4.0\")"));
        assert!(!main.contains("method serve()"));
        assert!(!main.contains("ui::web"));
        assert!(!main.contains("sink "));
        assert!(!main.contains("is view"));
        let agents = fs::read_to_string(tmp.join("AGENTS.md")).unwrap();
        assert!(agents.contains("component X"));
        assert!(agents.contains("dual-surface") || agents.contains("ui::terminal"));
        assert!(agents.contains("Removed in 0.2.0") || agents.contains("portal profiles"));
        let gi = fs::read_to_string(tmp.join(".gitignore")).unwrap();
        assert!(gitignore_has_line(&gi, GITIGNORE_ENTRY));
        assert!(gitignore_has_line(&gi, SILC_META_ENTRY));
        env::set_current_dir(prev).unwrap();
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn init_path_creates_directory() {
        let _guard = cwd_guard();
        let tmp = tempfile();
        let project = tmp.join("myapp");
        run(Some(project.to_str().unwrap())).expect("init path");
        assert!(project.join("main.silc").is_file());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn init_refuses_overwrite() {
        let _guard = cwd_guard();
        let tmp = tempfile();
        let project = tmp.join("exist");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("main.silc"), "x").unwrap();
        let err = run(Some(project.to_str().unwrap())).unwrap_err();
        assert!(err.contains("refusing to overwrite"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn init_merges_gitignore() {
        let _guard = cwd_guard();
        let tmp = tempfile();
        let project = tmp.join("merge");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(".gitignore"), "node_modules/\n").unwrap();
        run(Some(project.to_str().unwrap())).expect("init");
        let gi = fs::read_to_string(project.join(".gitignore")).unwrap();
        assert!(gi.contains("node_modules/"));
        assert!(gitignore_has_line(&gi, GITIGNORE_ENTRY));
        assert!(gitignore_has_line(&gi, SILC_META_ENTRY));
        let _ = fs::remove_dir_all(&tmp);
    }

    fn tempfile() -> PathBuf {
        let mut path = env::temp_dir();
        path.push(format!(
            "silc-init-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
