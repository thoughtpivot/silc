use std::env;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    match args.next() {
        None => {
            println!("silc {}", env!("CARGO_PKG_VERSION"));
            println!("usage: silc <program.silc|program.raku>");
        }
        Some(path) => {
            if let Err(err) = compile(Path::new(&path)) {
                eprintln!("silc: {err}");
                process::exit(1);
            }
        }
    }
}

fn compile(entry: &Path) -> Result<(), String> {
    if !entry.exists() {
        return Err(format!("file not found: {}", entry.display()));
    }
    let supported_extension = entry
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext, "silc" | "raku"));
    if !supported_extension {
        return Err(format!(
            "expected a .silc or .raku entry file (rename .sil to .silc), got {}",
            entry.display()
        ));
    }

    let workdir = entry
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = entry
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| format!("invalid entry filename: {}", entry.display()))?;
    let runtime_root = workdir.join(".runtime").join(stem);

    let source = std::fs::read_to_string(entry)
        .map_err(|error| format!("failed to read {}: {error}", entry.display()))?;
    let program = sil_parser::parse(&source).map_err(|error| error.to_string())?;
    program.validate()?;
    let decisions = sil_router::route_program(&program);
    let output = sil_codegen::emit(
        &program,
        &decisions,
        entry,
        &runtime_root,
        env!("CARGO_PKG_VERSION"),
    )?;

    println!("silc {}", env!("CARGO_PKG_VERSION"));
    println!("entry:   {}", entry.display());
    println!("workdir: {}", workdir.display());
    println!(
        "parsed:  {} contract(s), {} module(s)",
        program.contracts.len(),
        program.modules.len()
    );
    println!("routes:");
    for decision in &decisions {
        println!(
            "  {:<24} -> {:<6} ({})",
            decision.module,
            decision.target.as_str(),
            decision.provenance
        );
    }
    println!("runtime: {}", output.root.display());
    println!("manifest: {}", output.manifest.display());
    println!("generated: {} file(s)", output.generated.len());
    println!();
    println!("Gate B scaffold complete: parse -> validate -> route -> stub emit");
    println!("worker execution and IPC are not implemented yet.");
    Ok(())
}
