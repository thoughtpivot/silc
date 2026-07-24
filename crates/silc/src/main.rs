use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    match args.next() {
        None => {
            println!("silc {}", env!("CARGO_PKG_VERSION"));
            println!("usage: silc <program.sil>");
        }
        Some(path) => {
            if let Err(err) = prepare_runtime(Path::new(&path)) {
                eprintln!("silc: {err}");
                process::exit(1);
            }
        }
    }
}

fn prepare_runtime(entry: &Path) -> Result<(), String> {
    if !entry.exists() {
        return Err(format!("file not found: {}", entry.display()));
    }
    if entry
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext != "sil")
        .unwrap_or(true)
    {
        return Err(format!(
            "expected a .sil entry file, got {}",
            entry.display()
        ));
    }

    let workdir = entry
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let runtime_root = workdir.join(".runtime");
    for target in ["go", "python", "typescript"] {
        let dir = runtime_root.join(target);
        fs::create_dir_all(&dir).map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
    }

    println!("silc {}", env!("CARGO_PKG_VERSION"));
    println!("entry:   {}", entry.display());
    println!("workdir: {}", workdir.display());
    println!("runtime: {}", runtime_root.display());
    println!();
    println!("scaffold: created .runtime/{{go,python,typescript}}");
    println!("runtime direction: Go, Python, and Bun-executed TypeScript");
    println!("compile and execute are not implemented yet.");
    Ok(())
}
