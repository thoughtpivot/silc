//! Structured progress events for `silc assist` (ADR-008).
//!
//! The CLI maps these to user-facing action-trace lines. The model loop never
//! prints RLM jargon (`FINAL_VAR`, `draft_set`) directly to the terminal.

/// One observable step in the assist session.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// About to call the completer for this root turn (spinner only).
    Thinking {
        turn: usize,
        max_turns: usize,
    },
    /// Durable, user-facing action completed on a turn.
    Action {
        turn: usize,
        max_turns: usize,
        /// Wall time spent on the model turn that produced this action (seconds).
        elapsed_secs: f64,
        kind: ActionKind,
    },
}

/// Semantic action kinds — never expose tool protocol names in the CLI.
#[derive(Debug, Clone)]
pub enum ActionKind {
    ListedCorpus {
        docs: usize,
    },
    Searched {
        pattern: String,
        path: Option<String>,
        match_count: usize,
        no_matches: bool,
    },
    ReadCorpus {
        id: String,
        start: usize,
        end: usize,
        total: usize,
    },
    Queried {
        purpose: String,
    },
    /// Draft-first author attempt (chat template, program-only).
    Drafting {
        attempt: usize,
        attempts: usize,
    },
    /// Draft-first repair after a failed check or rejected draft.
    Repairing {
        reason: String,
    },
    /// Error-targeted corpus hits injected into a repair prompt.
    RetrievedEvidence {
        hits: usize,
        ids: Vec<String>,
    },
    /// Deterministic repair applied by assist itself (no model call).
    AutoFixed {
        what: String,
    },
    PreparedCode {
        chars: usize,
        preview: String,
        short_rejected: bool,
        unchanged: bool,
    },
    InspectedDraft {
        chars: usize,
        empty: bool,
    },
    Checked {
        ok: bool,
        detail: String,
    },
    StillRefining {
        reason: String,
    },
    Accepted,
    Salvaged {
        reason: String,
    },
    InvalidTurn {
        detail: String,
    },
    UnknownTool {
        name: String,
    },
}

/// Sink for assist progress (CLI UI, tests, or null).
pub trait ProgressReporter {
    fn on_event(&mut self, event: ProgressEvent);
}

/// No-op reporter for scripted tests.
pub struct NullProgress;

impl ProgressReporter for NullProgress {
    fn on_event(&mut self, _event: ProgressEvent) {}
}

/// Build a short dim-friendly preview of a draft (first non-empty lines).
pub fn draft_preview(source: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .take(max_lines)
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    let mut out = lines.join("\n");
    let total = source.lines().filter(|l| !l.trim().is_empty()).count();
    if total > max_lines {
        out.push_str("\n…");
    }
    out
}

/// Truncate a string for a single terminal-safe summary line.
pub fn truncate_one_line(text: &str, max: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let count = flat.chars().count();
    if count <= max {
        flat
    } else {
        let mut out: String = flat.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Count grep hits from `corpus_grep` tool metadata (excluding the header / caps).
pub fn count_grep_matches(meta: &str) -> (usize, bool) {
    if meta.contains("(no matches)") {
        return (0, true);
    }
    let mut count = 0usize;
    for line in meta.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("corpus_grep:") {
            continue;
        }
        if line.starts_with('…') || line.starts_with("...") {
            continue;
        }
        // Hits look like `id:lineno:text`
        if line.contains(':') {
            count += 1;
        }
    }
    (count, false)
}

/// Parse `id=… start=… end=… total=…` from a corpus_read result header.
pub fn parse_read_meta(meta: &str) -> Option<(String, usize, usize, usize)> {
    let header = meta.lines().next()?.trim();
    let mut id = None;
    let mut start = None;
    let mut end = None;
    let mut total = None;
    for part in header.split_whitespace() {
        if let Some(v) = part.strip_prefix("id=") {
            id = Some(v.to_string());
        } else if let Some(v) = part.strip_prefix("start=") {
            start = v.parse().ok();
        } else if let Some(v) = part.strip_prefix("end=") {
            end = v.parse().ok();
        } else if let Some(v) = part.strip_prefix("total=") {
            total = v.parse().ok();
        }
    }
    Some((id?, start?, end?, total?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_to_one_line() {
        let out = truncate_one_line("hello\nworld  and   more", 11);
        assert_eq!(out, "hello worl…");
        assert_eq!(truncate_one_line("short", 20), "short");
    }

    #[test]
    fn counts_grep_hits() {
        let meta = "corpus_grep:\nexample/a.silc:1:hotel\nexample/b.silc:2:signup\n";
        assert_eq!(count_grep_matches(meta), (2, false));
        assert_eq!(
            count_grep_matches("corpus_grep:\n(no matches)\n"),
            (0, true)
        );
    }

    #[test]
    fn parses_read_header() {
        let meta = "id=agents start=0 end=4000 total=19000\n# Silc…";
        let (id, start, end, total) = parse_read_meta(meta).unwrap();
        assert_eq!(id, "agents");
        assert_eq!((start, end, total), (0, 4000, 19000));
    }
}
