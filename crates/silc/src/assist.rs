//! `silc assist` CLI orchestration (ADR-008).

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use sil_rlm::{run_assist, Budgets, Completer, Corpus};

use crate::models::ensure_model;
use crate::runtimes::{cache_root, ensure_runtimes};

const ASSIST_REQUIREMENTS: &str = include_str!("../../sil-rlm/templates/requirements.txt");
const ASSIST_COMPLETE_PY: &str = include_str!("../../sil-rlm/templates/assist_complete.py");

#[derive(Debug, Default)]
pub struct AssistArgs {
    pub task: String,
    pub out: Option<PathBuf>,
    pub corpus: Option<PathBuf>,
    pub max_turns: Option<usize>,
    pub max_checks: Option<usize>,
    pub max_llm_queries: Option<usize>,
    pub wall_clock_secs: Option<u64>,
}

pub fn run(args: AssistArgs) -> Result<(), String> {
    if args.task.trim().is_empty() {
        return Err("assist requires a non-empty task string".into());
    }

    let mut corpus = Corpus::builtin();
    if let Some(dir) = &args.corpus {
        let added = corpus.load_extra_dir(dir)?;
        eprintln!("silc assist: loaded {added} extra corpus file(s) from {}", dir.display());
    }

    let mut budgets = Budgets::default();
    if let Some(n) = args.max_turns {
        budgets.max_root_turns = n;
    }
    if let Some(n) = args.max_checks {
        budgets.max_silc_check = n;
    }
    if let Some(n) = args.max_llm_queries {
        budgets.max_llm_query = n;
    }
    if let Some(n) = args.wall_clock_secs {
        budgets.wall_clock_secs = n;
    }

    let model_path = ensure_model("silclm")?;
    let python = ensure_assist_python()?;
    let mut completer = LlamaCompleter::new(python, model_path)?;

    let mut stderr = io::stderr();
    let result = run_assist(
        &args.task,
        &corpus,
        &mut completer,
        &budgets,
        Some(&mut stderr),
    )
    .map_err(|e| e.to_string())?;

    eprintln!(
        "silc assist: done turns={} checks={} llm_queries={}",
        result.stats.root_turns, result.stats.checks, result.stats.llm_queries
    );

    if let Some(path) = args.out {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("create {}: {e}", parent.display()))?;
            }
        }
        fs::write(&path, &result.program)
            .map_err(|e| format!("write {}: {e}", path.display()))?;
        eprintln!("silc assist: wrote {}", path.display());
    } else {
        println!("{}", result.program);
    }
    Ok(())
}

pub fn parse_args(args: Vec<String>) -> Result<AssistArgs, String> {
    let mut out = AssistArgs::default();
    let mut rest = args.into_iter();
    let Some(first) = rest.next() else {
        return Err(
            "usage: silc assist \"<task>\" [--out path.silc] [--corpus <dir>] [--max-turns N]"
                .into(),
        );
    };
    if first.starts_with('-') {
        return Err("assist task must be the first argument (quoted string)".into());
    }
    out.task = first;
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--out" => {
                let path = rest
                    .next()
                    .ok_or_else(|| "--out requires a path".to_string())?;
                out.out = Some(PathBuf::from(path));
            }
            "--corpus" => {
                let path = rest
                    .next()
                    .ok_or_else(|| "--corpus requires a directory".to_string())?;
                out.corpus = Some(PathBuf::from(path));
            }
            "--max-turns" => {
                let value = rest
                    .next()
                    .ok_or_else(|| "--max-turns requires a number".to_string())?;
                out.max_turns = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid --max-turns `{value}`"))?,
                );
            }
            "--max-checks" => {
                let value = rest
                    .next()
                    .ok_or_else(|| "--max-checks requires a number".to_string())?;
                out.max_checks = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid --max-checks `{value}`"))?,
                );
            }
            "--max-llm-queries" => {
                let value = rest
                    .next()
                    .ok_or_else(|| "--max-llm-queries requires a number".to_string())?;
                out.max_llm_queries = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid --max-llm-queries `{value}`"))?,
                );
            }
            "--wall-clock-secs" => {
                let value = rest
                    .next()
                    .ok_or_else(|| "--wall-clock-secs requires a number".to_string())?;
                out.wall_clock_secs = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid --wall-clock-secs `{value}`"))?,
                );
            }
            other => return Err(format!("unknown assist option `{other}`")),
        }
    }
    Ok(out)
}

struct LlamaCompleter {
    python: PathBuf,
    script: PathBuf,
    model_path: PathBuf,
    /// Keep one warm process? Phase 1: one process per complete (simple, slower).
    _lock: &'static Mutex<()>,
}

impl LlamaCompleter {
    fn new(python: PathBuf, model_path: PathBuf) -> Result<Self, String> {
        let script = assist_home()?.join("assist_complete.py");
        if !script.is_file() {
            fs::write(&script, ASSIST_COMPLETE_PY)
                .map_err(|e| format!("write {}: {e}", script.display()))?;
        }
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        Ok(Self {
            python,
            script,
            model_path,
            _lock: LOCK.get_or_init(|| Mutex::new(())),
        })
    }
}

impl Completer for LlamaCompleter {
    fn complete(&mut self, prompt: &str) -> Result<String, String> {
        let _guard = self
            ._lock
            .lock()
            .map_err(|_| "assist completer lock poisoned".to_string())?;
        let mut child = Command::new(&self.python)
            .arg(&self.script)
            .env("SILC_LLM_MODEL_PATH", &self.model_path)
            .env(
                "SILC_LLM_N_CTX",
                sil_core::DEFAULT_LLM_N_CTX.to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn assist completer: {e}"))?;
        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| "assist completer stdin missing".to_string())?;
            stdin
                .write_all(prompt.as_bytes())
                .map_err(|e| format!("write assist prompt: {e}"))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| format!("assist completer wait: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "assist completer failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

fn assist_home() -> Result<PathBuf, String> {
    let dir = cache_root()?.join("assist");
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn ensure_assist_python() -> Result<PathBuf, String> {
    let lock = ensure_runtimes()?;
    let home = assist_home()?;
    let requirements = home.join("requirements.txt");
    if !requirements.is_file()
        || fs::read_to_string(&requirements).unwrap_or_default() != ASSIST_REQUIREMENTS
    {
        fs::write(&requirements, ASSIST_REQUIREMENTS)
            .map_err(|e| format!("write {}: {e}", requirements.display()))?;
    }
    let script = home.join("assist_complete.py");
    fs::write(&script, ASSIST_COMPLETE_PY)
        .map_err(|e| format!("write {}: {e}", script.display()))?;

    let venv = home.join(".venv");
    let python = venv.join("bin/python");
    if !python.is_file() {
        eprintln!("silc assist: creating Python venv…");
        let output = Command::new(&lock.python_bin)
            .args(["-m", "venv"])
            .arg(&venv)
            .output()
            .map_err(|e| format!("create assist venv: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "assist venv failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    let marker = venv.join(".silc-assist-deps-ok");
    if !marker.is_file() {
        eprintln!("silc assist: installing llama-cpp-python==0.3.7…");
        let status = Command::new(&python)
            .args(["-m", "pip", "install", "--disable-pip-version-check", "-r"])
            .arg(&requirements)
            .status()
            .map_err(|e| format!("pip install assist deps: {e}"))?;
        if !status.success() {
            return Err("Silc install of assist llama-cpp-python failed".into());
        }
        fs::write(&marker, b"ok").map_err(|e| format!("write {}: {e}", marker.display()))?;
    }
    Ok(python)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_assist_args_basic() {
        let args = parse_args(vec![
            "build a notes app".into(),
            "--out".into(),
            "notes.silc".into(),
            "--max-turns".into(),
            "6".into(),
        ])
        .unwrap();
        assert_eq!(args.task, "build a notes app");
        assert_eq!(args.out.unwrap(), PathBuf::from("notes.silc"));
        assert_eq!(args.max_turns, Some(6));
    }
}
