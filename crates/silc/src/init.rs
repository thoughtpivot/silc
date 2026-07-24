use std::fs;
use std::path::{Path, PathBuf};

const AGENTS_MD: &str = include_str!("../templates/AGENTS.md");
const MAIN_SILC: &str = include_str!("../templates/main.silc");
const GITIGNORE_ENTRY: &str = ".runtime/";

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

    println!("silc {}", env!("CARGO_PKG_VERSION"));
    println!("initialized: {}", display_root(&root));
    println!("  AGENTS.md");
    println!("  main.silc");
    println!("  .gitignore  (+ {GITIGNORE_ENTRY})");
    println!();
    println!("next: silc main.silc");
    Ok(())
}

fn ensure_gitignore(path: &Path) -> Result<(), String> {
    if path.exists() {
        let existing = fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        if gitignore_has_runtime(&existing) {
            return Ok(());
        }
        let mut updated = existing;
        if !updated.ends_with('\n') && !updated.is_empty() {
            updated.push('\n');
        }
        updated.push_str("# Generated Silc runtime (do not hand-edit)\n");
        updated.push_str(GITIGNORE_ENTRY);
        updated.push('\n');
        fs::write(path, updated)
            .map_err(|e| format!("failed to update {}: {e}", path.display()))?;
    } else {
        let contents = include_str!("../templates/gitignore");
        fs::write(path, contents)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    }
    Ok(())
}

fn gitignore_has_runtime(contents: &str) -> bool {
    contents.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == ".runtime/" || trimmed == ".runtime" || trimmed == "**/.runtime/"
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

    #[test]
    fn init_current_dir_creates_scaffold() {
        let _guard = CWD_LOCK.lock().unwrap();
        let tmp = tempfile();
        let prev = env::current_dir().unwrap();
        env::set_current_dir(&tmp).unwrap();
        run(None).expect("init");
        assert!(tmp.join("AGENTS.md").is_file());
        assert!(tmp.join("main.silc").is_file());
        assert!(tmp.join(".gitignore").is_file());
        let gi = fs::read_to_string(tmp.join(".gitignore")).unwrap();
        assert!(gitignore_has_runtime(&gi));
        env::set_current_dir(prev).unwrap();
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn init_path_creates_directory() {
        let _guard = CWD_LOCK.lock().unwrap();
        let tmp = tempfile();
        let project = tmp.join("myapp");
        run(Some(project.to_str().unwrap())).expect("init path");
        assert!(project.join("main.silc").is_file());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn init_refuses_overwrite() {
        let _guard = CWD_LOCK.lock().unwrap();
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
        let _guard = CWD_LOCK.lock().unwrap();
        let tmp = tempfile();
        let project = tmp.join("merge");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(".gitignore"), "node_modules/\n").unwrap();
        run(Some(project.to_str().unwrap())).expect("init");
        let gi = fs::read_to_string(project.join(".gitignore")).unwrap();
        assert!(gi.contains("node_modules/"));
        assert!(gitignore_has_runtime(&gi));
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
