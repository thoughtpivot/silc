use std::env;
use std::path::PathBuf;
use std::process;

use sil_training::{
    bank_candidates, build_prompt_records, load_tasks, run_benchmark, write_prompt_jsonl,
};

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        None | Some("help") | Some("-h") | Some("--help") => {
            print_usage();
        }
        Some("prompts") => {
            if let Err(error) = run_prompts(args.collect()) {
                eprintln!("sil-training: {error}");
                process::exit(1);
            }
        }
        Some("bank") => {
            if let Err(error) = run_bank(args.collect()) {
                eprintln!("sil-training: {error}");
                process::exit(1);
            }
        }
        Some("subject-first-bench") => {
            if let Err(error) = run_subject_first_bench(args.collect()) {
                eprintln!("sil-training: {error}");
                process::exit(1);
            }
        }
        Some(other) => {
            eprintln!("sil-training: unknown command `{other}`");
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    println!("sil-training {}", env!("CARGO_PKG_VERSION"));
    println!("usage:");
    println!("  sil-training prompts --agents <AGENTS.md> --tasks <dir> --out <prompts.jsonl>");
    println!(
        "  sil-training bank --candidates <candidates.jsonl> --accepted <accepted.jsonl> --rejected <rejected.jsonl> [--no-emit]"
    );
    println!(
        "  sil-training subject-first-bench --agents <AGENTS.md> --tasks <dir> --out <report.json>"
    );
}

fn run_prompts(args: Vec<String>) -> Result<(), String> {
    let agents = flag_path(&args, "--agents")?;
    let tasks = flag_path(&args, "--tasks")?;
    let out = flag_path(&args, "--out")?;
    let agents_md = std::fs::read_to_string(&agents)
        .map_err(|error| format!("read {}: {error}", agents.display()))?;
    let seeds = load_tasks(&tasks)?;
    let records = build_prompt_records(&agents_md, &seeds);
    write_prompt_jsonl(&out, &records)?;
    println!(
        "sil-training: wrote {} prompt(s) → {}",
        records.len(),
        out.display()
    );
    Ok(())
}

fn run_bank(args: Vec<String>) -> Result<(), String> {
    let candidates = flag_path(&args, "--candidates")?;
    let accepted = flag_path(&args, "--accepted")?;
    let rejected = flag_path(&args, "--rejected")?;
    let emit = !args.iter().any(|arg| arg == "--no-emit");
    let stats = bank_candidates(&candidates, &accepted, &rejected, emit)?;
    println!(
        "sil-training: accepted={} rejected={} duplicates={}",
        stats.accepted, stats.rejected, stats.duplicates
    );
    Ok(())
}

fn run_subject_first_bench(args: Vec<String>) -> Result<(), String> {
    let agents = flag_path(&args, "--agents")?;
    let tasks = flag_path(&args, "--tasks")?;
    let out = flag_path(&args, "--out")?;
    let report = run_benchmark(&agents, &tasks, &out)?;
    println!(
        "sil-training: subject-first bench → {} (prompts={}, trials={})",
        out.display(),
        report.prompts.len(),
        report.trials.len()
    );
    for summary in &report.summaries {
        println!(
            "  {}: first_pass={}/{} ({:.0}%) mean_repair={:.2}",
            summary.variant.as_str(),
            summary.first_pass_ok,
            summary.trials,
            summary.first_pass_rate * 100.0,
            summary.mean_repair_turns
        );
    }
    Ok(())
}

fn flag_path(args: &[String], flag: &str) -> Result<PathBuf, String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            let value = iter
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            return Ok(PathBuf::from(value));
        }
    }
    Err(format!("missing required flag {flag}"))
}
