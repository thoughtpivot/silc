mod assist;
mod init;
mod models;
mod runtimes;
mod supervisor;

use clap::{Parser, Subcommand};
use sil_core::ExecutionMode;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Parser, Debug)]
#[command(
    name = "silc",
    version,
    about = "ThoughtPivot Silc compiler CLI",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate or modify a Silc program with silclm assist
    Assist {
        /// What to build or change
        task: String,
        /// Path to the `.silc` file (created if missing)
        path: PathBuf,
        /// Extra corpus directory (`.silc` / `.md` / `.txt`)
        #[arg(long)]
        corpus: Option<PathBuf>,
        /// Max root model turns (default 24)
        #[arg(long)]
        max_turns: Option<usize>,
        /// Max compiler checks (default 16)
        #[arg(long)]
        max_checks: Option<usize>,
        /// Max nested llm_query calls (default 24)
        #[arg(long)]
        max_llm_queries: Option<usize>,
        /// Wall-clock limit in seconds (default 120)
        #[arg(long)]
        wall_clock_secs: Option<u64>,
        /// After draft-first fails, run the slower closed-tool explore loop
        #[arg(long)]
        explore: bool,
    },
    /// Scaffold a new Silc project
    Init {
        /// Project directory (default: current directory)
        path: Option<String>,
    },
    /// Compile a program without running it
    Build {
        /// Path to the `.silc` entry file
        path: PathBuf,
    },
    /// Run a pipeline-only program with JSON input
    Run {
        /// Path to the `.silc` entry file
        path: PathBuf,
        /// Inline JSON input (must include string field `url`)
        #[arg(long, group = "run_input")]
        input_json: Option<String>,
        /// Path to a JSON input file
        #[arg(long, group = "run_input")]
        input: Option<PathBuf>,
    },
}

fn main() {
    let raw: Vec<String> = env::args().collect();
    if raw.len() >= 2 && looks_like_direct_run(&raw[1]) {
        let path = &raw[1];
        let rest: Vec<String> = raw[2..].to_vec();
        match parse_direct_run_args(path, &rest, env_truthy("SILC_TERMINAL")) {
            Ok(opts) => {
                if let Err(err) = compile_and_maybe_run(&opts.path, opts.attach_terminal) {
                    eprintln!("silc: {err}");
                    process::exit(1);
                }
            }
            Err(err) => {
                eprintln!("silc: {err}");
                process::exit(1);
            }
        }
        return;
    }

    let cli = Cli::parse();
    match cli.command {
        None => {
            println!("silc {}", env!("CARGO_PKG_VERSION"));
            println!("usage: silc <program.silc> [--terminal]");
            println!("       silc build <program.silc>");
            println!("       silc run <program.silc> --input-json '<json>'");
            println!("       silc run <program.silc> --input <file.json>");
            println!("       silc init [path]");
            println!("       silc assist \"<task>\" <path.silc> [--max-turns N] [--explore]");
        }
        Some(Commands::Assist {
            task,
            path,
            corpus,
            max_turns,
            max_checks,
            max_llm_queries,
            wall_clock_secs,
            explore,
        }) => {
            let args = assist::AssistArgs {
                task,
                path,
                corpus,
                max_turns,
                max_checks,
                max_llm_queries,
                wall_clock_secs,
                explore,
            };
            if let Err(err) = assist::run(args) {
                eprintln!("silc: {err}");
                process::exit(1);
            }
        }
        Some(Commands::Init { path }) => {
            if let Err(err) = init::run(path.as_deref()) {
                eprintln!("silc: {err}");
                process::exit(1);
            }
        }
        Some(Commands::Build { path }) => {
            if let Err(err) = build_only(&path) {
                eprintln!("silc: {err}");
                process::exit(1);
            }
        }
        Some(Commands::Run {
            path,
            input_json,
            input,
        }) => {
            let input_json = match (input_json, input) {
                (Some(value), None) => value,
                (None, Some(file)) => match fs::read_to_string(&file) {
                    Ok(value) => value,
                    Err(error) => {
                        eprintln!("silc: read pipeline input {}: {error}", file.display());
                        process::exit(1);
                    }
                },
                _ => {
                    eprintln!(
                        "silc: choose exactly one of --input-json or --input (usage: silc run <program.silc> --input-json '<json>')"
                    );
                    process::exit(1);
                }
            };
            if let Err(err) = compile_and_run_pipeline(&path, &input_json) {
                eprintln!("silc: {err}");
                process::exit(1);
            }
        }
    }
}

fn looks_like_direct_run(first: &str) -> bool {
    if first.starts_with('-') {
        return false;
    }
    !matches!(
        first,
        "assist" | "init" | "build" | "run" | "help" | "--help" | "-h" | "--version" | "-V"
    )
}

#[derive(Debug, PartialEq, Eq)]
struct DirectRunArgs {
    path: PathBuf,
    attach_terminal: bool,
}

/// Parse `silc <program.silc> [--terminal]`. `env_terminal` mirrors `SILC_TERMINAL`.
fn parse_direct_run_args(
    path: &str,
    rest: &[String],
    env_terminal: bool,
) -> Result<DirectRunArgs, String> {
    let mut attach_terminal = env_terminal;
    for flag in rest {
        match flag.as_str() {
            "--terminal" => attach_terminal = true,
            other if other.starts_with('-') => {
                return Err(format!(
                    "unknown option `{other}` (usage: silc <program.silc> [--terminal])"
                ));
            }
            other => {
                return Err(format!(
                    "unexpected argument `{other}` (usage: silc <program.silc> [--terminal])"
                ));
            }
        }
    }
    Ok(DirectRunArgs {
        path: PathBuf::from(path),
        attach_terminal,
    })
}

fn env_truthy(name: &str) -> bool {
    matches!(env::var(name).as_deref(), Ok("1") | Ok("true") | Ok("yes"))
}

fn compile_and_run_pipeline(entry: &Path, input_json: &str) -> Result<(), String> {
    let input: serde_json::Value = serde_json::from_str(input_json)
        .map_err(|error| format!("invalid pipeline input JSON: {error}"))?;
    let url = input
        .get("url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "pipeline input must contain string field `url`".to_string())?;
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("pipeline input `url` must use http or https".into());
    }
    let (_workdir, output, lock) = compile_common(entry)?;
    let graph = output
        .graph
        .as_ref()
        .ok_or_else(|| "program is not executable in Silc 0.4.0".to_string())?;
    if !graph.is_pipeline_only() {
        return Err("`silc run --input-*` requires a pipeline-only program".into());
    }
    supervisor::run_pipeline(&output, &lock, input_json)
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
        println!("stub emit only — this program is not executable in Silc 0.4.0");
    }
    Ok(())
}

fn compile_and_maybe_run(entry: &Path, attach_terminal: bool) -> Result<(), String> {
    let (_workdir, output, lock) = compile_common(entry)?;
    println!("silc {}", env!("CARGO_PKG_VERSION"));
    println!("entry:    {}", entry.display());
    println!("runtime:  {}", output.root.display());
    println!("manifest: {}", output.manifest.display());
    println!("mode:     {}", output.execution_mode);

    match output.execution_mode {
        ExecutionMode::Stub => {
            println!();
            println!("stub emit complete — worker execution requires runnable 0.4.0 operations");
            println!(
                "(app with ui::web + ui::terminal, resources/actions, optional text::score or llm::complete, or service::http)"
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
            } else if graph.has_game() {
                supervisor::run_game(&output, &lock)
            } else if graph.is_pipeline_only() {
                Err(
                    "pipeline-only program: use `silc run <program.silc> --input-json '{\"url\":\"https://…\"}'`"
                        .into(),
                )
            } else {
                supervisor::run_app(&output, &lock, attach_terminal)
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
    let ext = entry.extension().and_then(|ext| ext.to_str());
    if ext != Some("silc") {
        let hint = match ext {
            Some("raku") | Some("sil") => "rename to .silc — Silc does not accept .raku or .sil",
            _ => "expected a .silc entry file",
        };
        return Err(format!("{hint}, got {}", entry.display()));
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
    program.validate_source_version(env!("CARGO_PKG_VERSION"))?;
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
        if graph.has_game() {
            supervisor::build_game_python_bake(&lock, &output.root)?;
            supervisor::build_game_web(&lock, &output.root)?;
            supervisor::build_go_worker(&lock, &output.root)?;
        }
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
            if graph.needs_scrape_crawl() {
                supervisor::build_scrape_crawl(&lock, &output.root)?;
            }
            if graph.needs_scrape_browser() {
                supervisor::build_scrape_python(&lock, &output.root)?;
            }
            if graph.has_doc() {
                supervisor::build_doc_python(&lock, &output.root)?;
            }
        }
        if graph.has_api() {
            supervisor::build_go_api_worker(&lock, &output.root)?;
        }
        if graph.is_pipeline_only() {
            supervisor::build_go_worker(&lock, &output.root)?;
            let model_id = graph
                .model_ref
                .as_deref()
                .ok_or_else(|| "tensor pipeline missing model_ref".to_string())?;
            models::ensure_embedding_model(model_id)?;
            supervisor::build_tensor_python(&lock, &output.root)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_run_defaults_to_web_only() {
        let opts = parse_direct_run_args("main.silc", &[], false).unwrap();
        assert_eq!(opts.path, PathBuf::from("main.silc"));
        assert!(!opts.attach_terminal);
    }

    #[test]
    fn direct_run_terminal_flag_attaches() {
        let opts =
            parse_direct_run_args("main.silc", &[String::from("--terminal")], false).unwrap();
        assert!(opts.attach_terminal);
    }

    #[test]
    fn direct_run_env_terminal_attaches() {
        let opts = parse_direct_run_args("main.silc", &[], true).unwrap();
        assert!(opts.attach_terminal);
    }

    #[test]
    fn direct_run_flag_or_env_attaches() {
        let opts = parse_direct_run_args("app.silc", &[String::from("--terminal")], true).unwrap();
        assert!(opts.attach_terminal);
    }

    #[test]
    fn direct_run_rejects_unknown_flag() {
        let err =
            parse_direct_run_args("main.silc", &[String::from("--force")], false).unwrap_err();
        assert!(err.contains("unknown option `--force`"));
    }

    #[test]
    fn direct_run_rejects_extra_positional() {
        let err =
            parse_direct_run_args("main.silc", &[String::from("extra.silc")], false).unwrap_err();
        assert!(err.contains("unexpected argument `extra.silc`"));
    }

    #[test]
    fn looks_like_direct_run_for_silc_files() {
        assert!(looks_like_direct_run("main.silc"));
        assert!(!looks_like_direct_run("assist"));
        assert!(!looks_like_direct_run("build"));
        assert!(!looks_like_direct_run("--help"));
    }
}
