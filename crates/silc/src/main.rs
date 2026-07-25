mod init;
mod models;
mod runtimes;
mod supervisor;

use sil_core::ExecutionMode;
use std::env;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        None => {
            println!("silc {}", env!("CARGO_PKG_VERSION"));
            println!("usage: silc <program.silc|program.raku>");
            println!("       silc build <program.silc|program.raku>");
            println!("       silc init [path]");
        }
        Some("init") => {
            let path = args.next();
            if let Err(err) = init::run(path.as_deref()) {
                eprintln!("silc: {err}");
                process::exit(1);
            }
        }
        Some("build") => {
            let Some(path) = args.next() else {
                eprintln!("silc: usage: silc build <program.silc|program.raku>");
                process::exit(1);
            };
            if let Err(err) = build_only(Path::new(&path)) {
                eprintln!("silc: {err}");
                process::exit(1);
            }
        }
        Some(path) => {
            if let Err(err) = compile_and_maybe_run(Path::new(path)) {
                eprintln!("silc: {err}");
                process::exit(1);
            }
        }
    }
}

fn build_only(entry: &Path) -> Result<(), String> {
    let (_workdir, output, lock) = compile_common(entry)?;
    println!("silc {}", env!("CARGO_PKG_VERSION"));
    println!("built: {}", output.root.display());
    println!("mode:  {}", output.execution_mode);
    if output.execution_mode == ExecutionMode::Runnable {
        println!("bun:     {}", lock.bun_bin.display());
        println!("cpython: {}", lock.python_bin.display());
        println!("go:      {}", lock.go_bin.display());
        println!("engines locked under .silc/runtimes.lock.json");
    } else {
        println!("stub emit only — this program is not executable in Silc 0.2.0");
    }
    Ok(())
}

fn compile_and_maybe_run(entry: &Path) -> Result<(), String> {
    let (_workdir, output, lock) = compile_common(entry)?;
    println!("silc {}", env!("CARGO_PKG_VERSION"));
    println!("entry:    {}", entry.display());
    println!("runtime:  {}", output.root.display());
    println!("manifest: {}", output.manifest.display());
    println!("mode:     {}", output.execution_mode);

    match output.execution_mode {
        ExecutionMode::Stub => {
            println!();
            println!("stub emit complete — worker execution requires runnable 0.2.0 operations");
            println!(
                "(is app with ui::web + ui::terminal, resources/actions, optional text::score or llm::complete, or service::http)"
            );
            Ok(())
        }
        ExecutionMode::Runnable => {
            let graph = output
                .graph
                .as_ref()
                .ok_or_else(|| "runnable program missing executable graph".to_string())?;
            if graph.is_api_only() {
                supervisor::run_api(&output, &lock)
            } else {
                supervisor::run_app(&output, &lock)
            }
        }
    }
}

fn compile_common(
    entry: &Path,
) -> Result<(PathBuf, sil_codegen::EmitResult, runtimes::RuntimeLock), String> {
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

    let lock = supervisor::ensure_project_runtimes(&workdir)?;

    let output = sil_codegen::emit(
        &program,
        &decisions,
        entry,
        &runtime_root,
        env!("CARGO_PKG_VERSION"),
    )?;

    if output.execution_mode == ExecutionMode::Runnable {
        let graph = output
            .graph
            .as_ref()
            .ok_or_else(|| "runnable program missing executable graph".to_string())?;
        if graph.has_ui() {
            supervisor::build_ui_web(&lock, &output.root)?;
            supervisor::build_go_worker(&lock, &output.root)?;
            if graph.needs_llm() {
                let model_id = graph
                    .model_ref
                    .as_deref()
                    .ok_or_else(|| "llm chat graph missing model_ref".to_string())?;
                models::ensure_model(model_id)?;
                supervisor::build_llm_python(&lock, &output.root)?;
            }
        }
        if graph.has_api() {
            supervisor::build_go_api_worker(&lock, &output.root)?;
        }
    }

    println!("routes:");
    for decision in &decisions {
        println!(
            "  {:<24} -> {:<6} ({})",
            decision.module,
            decision.target.as_str(),
            decision.provenance
        );
    }

    Ok((workdir, output, lock))
}
