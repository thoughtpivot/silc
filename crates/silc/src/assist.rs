//! `silc assist` CLI orchestration (ADR-008).

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use console::{style, Emoji};
use indicatif::{ProgressBar, ProgressStyle};
use sil_rlm::{
    run_assist, truncate_one_line, ActionKind, AssistSeed, Budgets, ChatReply, ChatRequest,
    Completer, Corpus, ProgressEvent, ProgressReporter,
};

use crate::models::ensure_model;
use crate::runtimes::{cache_root, ensure_runtimes};

const ASSIST_REQUIREMENTS: &str = include_str!("../../sil-rlm/templates/requirements.txt");
const ASSIST_COMPLETE_PY: &str = include_str!("../../sil-rlm/templates/assist_complete.py");

static SPARKLE: Emoji<'_, '_> = Emoji("✦ ", "* ");
static SEARCH: Emoji<'_, '_> = Emoji("⌕ ", "> ");
static BOOK: Emoji<'_, '_> = Emoji("▤ ", "> ");
static PEN: Emoji<'_, '_> = Emoji("✎ ", "> ");
static CHECK: Emoji<'_, '_> = Emoji("✓ ", "+ ");
static CROSS: Emoji<'_, '_> = Emoji("✗ ", "x ");
static GEAR: Emoji<'_, '_> = Emoji("⚙ ", "* ");
static WRITE: Emoji<'_, '_> = Emoji("→ ", "> ");

#[derive(Debug, Default)]
pub struct AssistArgs {
    pub task: String,
    pub path: PathBuf,
    pub corpus: Option<PathBuf>,
    pub max_turns: Option<usize>,
    pub max_checks: Option<usize>,
    pub max_llm_queries: Option<usize>,
    pub wall_clock_secs: Option<u64>,
    /// Enable the slower closed-tool RLM explore loop after draft-first fails.
    pub explore: bool,
}

pub fn run(args: AssistArgs) -> Result<(), String> {
    if args.task.trim().is_empty() {
        return Err("assist requires a non-empty task string".into());
    }
    if args.path.as_os_str().is_empty() {
        return Err("assist requires a target .silc path".into());
    }

    let target_existed = args.path.is_file();
    let mut corpus = Corpus::builtin();
    if let Some(agents_path) = corpus.load_project_agents(&args.path) {
        eprintln!(
            "{} {}",
            style(format!("{BOOK}loaded")).cyan(),
            style(format!("project AGENTS.md ({})", agents_path.display())).dim()
        );
    }
    if let Some(dir) = &args.corpus {
        let added = corpus.load_extra_dir(dir)?;
        eprintln!(
            "{} {}",
            style(format!("{BOOK}loaded")).cyan(),
            style(format!("{added} extra corpus file(s) from {}", dir.display())).dim()
        );
    }

    let mut seed = AssistSeed::default();
    let mut original: Option<String> = None;
    if target_existed {
        let body = fs::read_to_string(&args.path)
            .map_err(|e| format!("read {}: {e}", args.path.display()))?;
        if !body.trim().is_empty() {
            corpus.insert("target", body.clone());
            seed.draft = Some(body.clone());
            original = Some(body);
            eprintln!(
                "{} {}",
                style(format!("{PEN}editing")).cyan(),
                style(args.path.display().to_string()).bold()
            );
        }
    } else {
        eprintln!(
            "{} {}",
            style(format!("{SPARKLE}creating")).cyan(),
            style(args.path.display().to_string()).bold()
        );
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
    budgets.allow_explore = args.explore;

    let model_path = ensure_model("silclm")?;
    let python = ensure_assist_python()?;
    let mut completer = LlamaCompleter::new(python, model_path)?;

    let mut ui = AssistUi::new();
    let result = match run_assist(
        &args.task,
        &corpus,
        &mut completer,
        &budgets,
        Some(&mut ui),
        seed,
    ) {
        Ok(result) => result,
        Err(err) => {
            ui.finish();
            let message = err.to_string();
            if let sil_rlm::AssistError::DraftRejected {
                draft: Some(draft), ..
            } = &err
            {
                return Err(match save_rejected_draft(&args.path, draft) {
                    Ok(path) => format!(
                        "{message}\nThe closest draft was saved to {} for inspection.",
                        path.display()
                    ),
                    Err(_) => message,
                });
            }
            return Err(message);
        }
    };
    ui.finish();

    eprintln!(
        "{} {}",
        style(format!("{GEAR}done")).dim(),
        style(format!(
            "turns={} checks={} queries={}",
            result.stats.root_turns, result.stats.checks, result.stats.llm_queries
        ))
        .dim()
    );
    if !result.finalized {
        eprintln!(
            "{} {}",
            style("!").yellow().bold(),
            style("finished on a compiler-checked draft before an explicit accept — review the file").yellow()
        );
    }

    if let Some(before) = &original {
        if before.trim() == result.program.trim() {
            return Err(format!(
                "assist produced no changes to {} — the file was left untouched. Try a more specific request, or raise --max-turns.",
                args.path.display()
            ));
        }
    }

    write_program(&args.path, &result.program)?;
    eprintln!(
        "{} {}",
        style(format!("{WRITE}wrote")).green().bold(),
        style(args.path.display().to_string()).green()
    );
    Ok(())
}

fn write_program(path: &Path, program: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
    }
    fs::write(path, program).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Park the closest rejected draft beside the target so a failed run is still
/// inspectable (and never overwrites the real file).
fn save_rejected_draft(target: &Path, draft: &str) -> Result<PathBuf, String> {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "assist".into());
    let path = target.with_file_name(format!("{name}.rejected"));
    write_program(&path, draft)?;
    Ok(path)
}

/// Friendly terminal progress for the assist loop.
struct AssistUi {
    spinner: ProgressBar,
}

impl AssistUi {
    fn new() -> Self {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(thinking_style());
        spinner.enable_steady_tick(Duration::from_millis(80));
        spinner.set_message(format!(
            "{}Thinking about the next step…",
            SPARKLE
        ));
        Self { spinner }
    }

    fn finish(&mut self) {
        self.spinner.finish_and_clear();
    }

    fn log_line(&self, line: String) {
        self.spinner.suspend(|| {
            let _ = writeln!(io::stderr(), "{line}");
        });
    }

    fn set_thinking(&mut self, turn: usize, max_turns: usize) {
        self.spinner.reset_elapsed();
        self.spinner.set_style(thinking_style());
        self.spinner.set_message(format!(
            "{}Thinking about the next step… {}",
            SPARKLE,
            style(format!("({turn}/{max_turns})")).dim()
        ));
    }

    fn log_action(&self, turn: usize, max_turns: usize, elapsed_secs: f64, kind: &ActionKind) {
        let prefix = style(format!("{turn:>2}/{max_turns}")).dim().to_string();
        let timing = if elapsed_secs > 0.05 {
            format!("  {}", style(format!("{elapsed_secs:.1}s")).dim())
        } else {
            String::new()
        };
        match kind {
            ActionKind::ListedCorpus { docs } => {
                self.log_line(format!(
                    "  {prefix}  {}Listed {} corpus documents{timing}",
                    SEARCH,
                    style(docs).cyan()
                ));
            }
            ActionKind::Searched {
                pattern,
                path,
                match_count,
                no_matches,
            } => {
                let where_ = path
                    .as_ref()
                    .map(|p| format!(" in {}", style(p).dim()))
                    .unwrap_or_default();
                let outcome = if *no_matches {
                    style("no matches").yellow().to_string()
                } else {
                    style(format!(
                        "{match_count} match{}",
                        if *match_count == 1 { "" } else { "es" }
                    ))
                    .green()
                    .to_string()
                };
                self.log_line(format!(
                    "  {prefix}  {}Searching examples for \"{}\"{where_}…  {outcome}{timing}",
                    SEARCH,
                    style(pattern).cyan()
                ));
            }
            ActionKind::ReadCorpus {
                id,
                start,
                end,
                total,
            } => {
                let range = if *total == 0 && *end == 0 {
                    String::new()
                } else {
                    format!(
                        " {}",
                        style(format!("(chars {start}–{end} of {total})")).dim()
                    )
                };
                self.log_line(format!(
                    "  {prefix}  {}Reading {id}{range}{timing}",
                    BOOK
                ));
            }
            ActionKind::Queried { purpose } => {
                self.log_line(format!(
                    "  {prefix}  {}Planning with silclm… {}{timing}",
                    SPARKLE,
                    style(purpose).dim()
                ));
            }
            ActionKind::Drafting { attempt, attempts } => {
                self.log_line(format!(
                    "  {prefix}  {}Drafting program (attempt {attempt}/{attempts}){timing}",
                    PEN
                ));
            }
            ActionKind::Repairing { reason } => {
                self.log_line(format!(
                    "  {prefix}  {}Repairing draft… {}{timing}",
                    GEAR,
                    style(reason).dim()
                ));
            }
            ActionKind::RetrievedEvidence { hits, ids } => {
                let id_preview = if ids.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", ids.iter().take(3).cloned().collect::<Vec<_>>().join(", "))
                };
                self.log_line(format!(
                    "  {prefix}  {}Searched corpus for fix — {hits} hit(s){id_preview}{timing}",
                    BOOK
                ));
            }
            ActionKind::AutoFixed { what } => {
                self.log_line(format!(
                    "  {prefix}  {}{}{timing}",
                    WRITE,
                    style(format!("auto-fixed — {what}")).yellow()
                ));
            }
            ActionKind::PreparedCode {
                chars,
                preview,
                short_rejected,
                unchanged,
            } => {
                if *short_rejected {
                    self.log_line(format!(
                        "  {prefix}  {}Draft too short — need a complete program{timing}",
                        CROSS
                    ));
                } else if *unchanged {
                    self.log_line(format!(
                        "  {prefix}  {}Draft unchanged from original — still editing{timing}",
                        GEAR
                    ));
                } else {
                    self.log_line(format!(
                        "  {prefix}  {}Prepared revised program ({chars} chars){timing}",
                        PEN
                    ));
                    for line in preview.lines() {
                        self.log_line(format!("         {}", style(line).dim()));
                    }
                }
            }
            ActionKind::InspectedDraft { chars, empty } => {
                let detail = if *empty {
                    "draft is empty".to_string()
                } else {
                    format!("{chars} chars")
                };
                self.log_line(format!(
                    "  {prefix}  {}Reviewed current draft ({detail}){timing}",
                    BOOK
                ));
            }
            ActionKind::Checked { ok, detail } => {
                if *ok {
                    self.log_line(format!(
                        "  {prefix}  {}Checking program… {}{timing}",
                        CHECK,
                        style("passed").green()
                    ));
                } else {
                    self.log_line(format!(
                        "  {prefix}  {}Checking program… {} {}{timing}",
                        CROSS,
                        style("failed").yellow(),
                        style(detail).dim()
                    ));
                }
            }
            ActionKind::StillRefining { reason } => {
                self.log_line(format!(
                    "  {prefix}  {}Still refining… {}{timing}",
                    GEAR,
                    style(reason).dim()
                ));
            }
            ActionKind::Accepted => {
                self.log_line(format!(
                    "  {prefix}  {}Accepted revised program{timing}",
                    CHECK
                ));
            }
            ActionKind::Salvaged { reason } => {
                self.log_line(format!(
                    "  {prefix}  {}Using last good draft — {}{timing}",
                    CHECK,
                    style(reason).dim()
                ));
            }
            ActionKind::InvalidTurn { detail } => {
                self.log_line(format!(
                    "  {prefix}  {}Retrying — {}{timing}",
                    GEAR,
                    style(detail).dim()
                ));
            }
            ActionKind::UnknownTool { name } => {
                self.log_line(format!(
                    "  {prefix}  {}Unrecognized step ({}){timing}",
                    GEAR,
                    style(truncate_one_line(name, 32)).dim()
                ));
            }
        }
    }
}

fn thinking_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} {msg} {elapsed_precise:.dim}")
        .unwrap()
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "*"])
}

impl ProgressReporter for AssistUi {
    fn on_event(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::Thinking { turn, max_turns } => {
                self.set_thinking(turn, max_turns);
            }
            ProgressEvent::Action {
                turn,
                max_turns,
                elapsed_secs,
                kind,
            } => {
                self.log_action(turn, max_turns, elapsed_secs, &kind);
            }
        }
    }
}

/// Format a durable action line without styling (for unit tests).
#[cfg(test)]
fn format_action_plain(turn: usize, max_turns: usize, kind: &ActionKind) -> String {
    let prefix = format!("{turn:>2}/{max_turns}");
    match kind {
        ActionKind::Searched {
            pattern,
            path,
            match_count,
            no_matches,
        } => {
            let where_ = path
                .as_ref()
                .map(|p| format!(" in {p}"))
                .unwrap_or_default();
            let outcome = if *no_matches {
                "no matches".to_string()
            } else {
                format!(
                    "{match_count} match{}",
                    if *match_count == 1 { "" } else { "es" }
                )
            };
            format!("{prefix}  Searching examples for \"{pattern}\"{where_}…  {outcome}")
        }
        ActionKind::ReadCorpus {
            id,
            start,
            end,
            total,
        } => format!("{prefix}  Reading {id} (chars {start}–{end} of {total})"),
        ActionKind::Queried { purpose } => {
            format!("{prefix}  Planning with silclm… {purpose}")
        }
        ActionKind::Drafting { attempt, attempts } => {
            format!("{prefix}  Drafting program (attempt {attempt}/{attempts})")
        }
        ActionKind::Repairing { reason } => {
            format!("{prefix}  Repairing draft… {reason}")
        }
        ActionKind::RetrievedEvidence { hits, ids } => {
            format!(
                "{prefix}  Searched corpus for fix — {hits} hit(s) ({})",
                ids.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
            )
        }
        ActionKind::AutoFixed { what } => format!("{prefix}  Auto-fixed — {what}"),
        ActionKind::PreparedCode {
            chars,
            short_rejected,
            unchanged,
            ..
        } => {
            if *short_rejected {
                format!("{prefix}  Draft too short — need a complete program")
            } else if *unchanged {
                format!("{prefix}  Draft unchanged from original — still editing")
            } else {
                format!("{prefix}  Prepared revised program ({chars} chars)")
            }
        }
        ActionKind::Checked { ok, .. } => {
            if *ok {
                format!("{prefix}  Checking program… passed")
            } else {
                format!("{prefix}  Checking program… failed")
            }
        }
        ActionKind::StillRefining { reason } => {
            format!("{prefix}  Still refining… {reason}")
        }
        ActionKind::Accepted => format!("{prefix}  Accepted revised program"),
        other => format!("{prefix}  {other:?}"),
    }
}

struct LlamaCompleter {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    lock: &'static Mutex<()>,
}

impl LlamaCompleter {
    fn new(python: PathBuf, model_path: PathBuf) -> Result<Self, String> {
        let script = assist_home()?.join("assist_complete.py");
        // Always refresh the worker script so ADR-008 template changes ship.
        fs::write(&script, ASSIST_COMPLETE_PY)
            .map_err(|e| format!("write {}: {e}", script.display()))?;

        eprintln!(
            "{} {}",
            style(format!("{GEAR}loading")).cyan(),
            style("silclm (warm worker)…").dim()
        );

        // Assist needs more room than in-app chat: target + examples + draft.
        const ASSIST_N_CTX: u32 = 32_768;
        let n_ctx = std::env::var("SILC_LLM_N_CTX")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(ASSIST_N_CTX);

        let mut child = Command::new(&python)
            .arg(&script)
            .env("SILC_LLM_MODEL_PATH", &model_path)
            .env("SILC_LLM_N_CTX", n_ctx.to_string())
            .env(
                "SILC_LLM_N_GPU_LAYERS",
                std::env::var("SILC_LLM_N_GPU_LAYERS").unwrap_or_else(|_| "-1".into()),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // llama.cpp logs to stderr; an unread pipe deadlocks at 64KB.
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn assist completer: {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "assist completer stdin missing".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "assist completer stdout missing".to_string())?;
        let mut stdout = BufReader::new(stdout);

        let ready = read_json_line(&mut stdout)?;
        if ready.get("ready").and_then(|v| v.as_bool()) != Some(true) {
            if let Some(err) = ready.get("error").and_then(|v| v.as_str()) {
                return Err(format!("assist completer failed to start: {err}"));
            }
            return Err(format!(
                "assist completer did not become ready: {ready}"
            ));
        }

        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        Ok(Self {
            child,
            stdin,
            stdout,
            lock: LOCK.get_or_init(|| Mutex::new(())),
        })
    }
}

impl LlamaCompleter {
    fn request_json(&mut self, req: serde_json::Value) -> Result<serde_json::Value, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "assist completer lock poisoned".to_string())?;

        let mut line = serde_json::to_string(&req).map_err(|e| format!("encode request: {e}"))?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| format!("write assist prompt: {e}"))?;
        self.stdin
            .flush()
            .map_err(|e| format!("flush assist prompt: {e}"))?;

        let resp = read_json_line(&mut self.stdout)?;
        if let Some(err) = resp.get("error").and_then(|v| v.as_str()) {
            return Err(format!("assist completer error: {err}"));
        }
        Ok(resp)
    }

    fn text_from_resp(resp: &serde_json::Value) -> Result<(String, bool), String> {
        let text = resp
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .ok_or_else(|| format!("assist completer response missing text: {resp}"))?;
        let truncated = resp
            .get("truncated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok((text, truncated))
    }
}

impl Completer for LlamaCompleter {
    fn complete(&mut self, prompt: &str) -> Result<String, String> {
        let resp = self.request_json(serde_json::json!({ "prompt": prompt }))?;
        let (text, _) = Self::text_from_resp(&resp)?;
        Ok(text)
    }

    fn chat(&mut self, req: &ChatRequest) -> Result<ChatReply, String> {
        let mut payload = serde_json::json!({
            "messages": [
                { "role": "system", "content": req.system },
                { "role": "user", "content": req.user },
            ],
            "max_tokens": req.max_tokens,
        });
        if !req.stop.is_empty() {
            payload["stop"] = serde_json::json!(req.stop);
        }
        if let Some(t) = req.temperature {
            payload["temperature"] = serde_json::json!(t);
        }
        let resp = self.request_json(payload)?;
        let (text, truncated) = Self::text_from_resp(&resp)?;
        Ok(ChatReply { text, truncated })
    }
}

impl Drop for LlamaCompleter {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_json_line(reader: &mut BufReader<std::process::ChildStdout>) -> Result<serde_json::Value, String> {
    let mut line = String::new();
    let n = reader
        .read_line(&mut line)
        .map_err(|e| format!("read assist response: {e}"))?;
    if n == 0 {
        return Err("assist completer closed stdout unexpectedly".into());
    }
    serde_json::from_str(line.trim())
        .map_err(|e| format!("parse assist response `{line}`: {e}"))
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
        eprintln!(
            "{} {}",
            style(format!("{GEAR}setup")).cyan(),
            style("creating Python venv…").dim()
        );
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
        eprintln!(
            "{} {}",
            style(format!("{GEAR}setup")).cyan(),
            style("installing llama-cpp-python…").dim()
        );
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
    fn assist_args_defaults() {
        let args = AssistArgs {
            task: "build a notes app".into(),
            path: PathBuf::from("notes.silc"),
            max_turns: Some(6),
            ..AssistArgs::default()
        };
        assert_eq!(args.task, "build a notes app");
        assert_eq!(args.path, PathBuf::from("notes.silc"));
        assert_eq!(args.max_turns, Some(6));
    }

    #[test]
    fn formats_search_action() {
        let line = format_action_plain(
            3,
            24,
            &ActionKind::Searched {
                pattern: "hotel|signup".into(),
                path: Some("example".into()),
                match_count: 6,
                no_matches: false,
            },
        );
        assert_eq!(
            line,
            " 3/24  Searching examples for \"hotel|signup\" in example…  6 matches"
        );
    }

    #[test]
    fn formats_read_and_check_actions() {
        let read = format_action_plain(
            2,
            24,
            &ActionKind::ReadCorpus {
                id: "AGENTS.md".into(),
                start: 0,
                end: 4000,
                total: 19000,
            },
        );
        assert!(read.contains("Reading AGENTS.md (chars 0–4000 of 19000)"));
        let ok = format_action_plain(
            6,
            24,
            &ActionKind::Checked {
                ok: true,
                detail: "ok".into(),
            },
        );
        assert_eq!(ok, " 6/24  Checking program… passed");
    }

    #[test]
    fn formats_timing_suffix_when_present() {
        // Durable lines append elapsed seconds when inference took measurable time.
        let line = format!(
            "{}  {:.1}s",
            format_action_plain(
                3,
                24,
                &ActionKind::ReadCorpus {
                    id: "AGENTS.md".into(),
                    start: 0,
                    end: 4000,
                    total: 19000,
                },
            ),
            4.1
        );
        assert!(line.ends_with("4.1s"));
        assert!(line.contains("Reading AGENTS.md"));
    }

    #[test]
    fn json_line_request_round_trips() {
        let req = serde_json::json!({
            "prompt": "hello\nworld",
            "max_tokens": 64
        });
        let encoded = format!("{}\n", serde_json::to_string(&req).unwrap());
        assert!(!encoded[..encoded.len() - 1].contains('\n'));
        let parsed: serde_json::Value = serde_json::from_str(encoded.trim()).unwrap();
        assert_eq!(parsed["prompt"], "hello\nworld");
        assert_eq!(parsed["max_tokens"], 64);

        let resp: serde_json::Value =
            serde_json::from_str(r#"{"text":"```tool\n{\"name\":\"corpus_list\"}\n```"}"#).unwrap();
        assert!(resp["text"].as_str().unwrap().contains("corpus_list"));
    }
}
