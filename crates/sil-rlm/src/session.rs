//! Assist session loop (ADR-008).

use std::time::Instant;

use crate::author::run_author_with_failure;
use crate::complete::Completer;
use crate::corpus::Corpus;
use crate::progress::{
    count_grep_matches, draft_preview, parse_read_meta, truncate_one_line, ActionKind,
    ProgressEvent, ProgressReporter,
};
use crate::prompt::{root_bootstrap, truncate_for_history};
use crate::tools::{
    execute_tool, parse_turn, resolve_final, resolve_final_var, BudgetStats, Budgets, ParsedTurn,
    ToolOutcome, ToolState, MIN_DRAFT_CHARS,
};

#[derive(Debug, Clone)]
pub struct AssistResult {
    pub program: String,
    pub stats: BudgetStats,
    /// True when the model finished with FINAL; false when the loop salvaged a
    /// check-passing draft after a budget ran out.
    pub finalized: bool,
}

#[derive(Debug)]
pub enum AssistError {
    Budget(String),
    Completer(String),
    Failed(String),
    /// Draft-first exhausted its attempts; carries the closest rejected draft so
    /// the CLI can save it for inspection instead of throwing the work away.
    DraftRejected {
        message: String,
        draft: Option<String>,
    },
}

impl std::fmt::Display for AssistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Budget(m) | Self::Completer(m) | Self::Failed(m) => write!(f, "{m}"),
            Self::DraftRejected { message, .. } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for AssistError {}

/// Optional seed when modifying an existing `.silc` file.
#[derive(Debug, Clone, Default)]
pub struct AssistSeed {
    /// Pre-loaded draft (e.g. existing target file contents).
    pub draft: Option<String>,
}

/// Soft cap on root history chars — leaves room under context for generation.
pub const HISTORY_CAP: usize = 16_000;

/// Run draft-first authoring, then the closed-tool RLM loop until FINAL or budgets exhaust.
pub fn run_assist(
    task: &str,
    corpus: &Corpus,
    completer: &mut dyn Completer,
    budgets: &Budgets,
    mut progress: Option<&mut dyn ProgressReporter>,
    seed: AssistSeed,
) -> Result<AssistResult, AssistError> {
    let started = Instant::now();
    let seed_present = seed
        .draft
        .as_ref()
        .map(|d| !d.trim().is_empty())
        .unwrap_or(false);
    let mut state = ToolState::default();
    if let Some(draft) = seed.draft {
        if !draft.trim().is_empty() {
            state.seed = draft.clone();
            state.draft = draft;
            // Seeded programs still need an explicit check before FINAL.
            state.last_check_ok = false;
        }
    }

    // Primary path: inject context and ask for a complete program via chat.
    if budgets.max_draft_attempts > 0 {
        let (result, failure) =
            run_author_with_failure(task, corpus, completer, budgets, &mut progress, &mut state)?;
        if let Some(result) = result {
            return Ok(result);
        }
        if !budgets.allow_explore {
            let detail = failure
                .last_error
                .unwrap_or_else(|| "draft-first attempts exhausted".into());
            return Err(AssistError::DraftRejected {
                message: format!(
                    "could not produce a valid program after {} draft attempt(s): {detail}. Re-run with --explore to enable the slower tool loop, or refine the task.",
                    budgets.max_draft_attempts
                ),
                draft: failure.best_draft,
            });
        }
    }

    let mut history = root_bootstrap(task, corpus, seed_present);
    let mut recent_calls: Vec<String> = Vec::new();
    let mut empty_grep_streak = 0usize;
    let mut explore_streak = 0usize;
    let mut short_draft_streak = 0usize;
    let mut invalid_streak = 0usize;
    let mut grep_budget: usize = 8; // stop grepping after sustained misses
    let mut greps_blocked = false;

    while state.stats.root_turns < budgets.max_root_turns {
        let elapsed_wall = started.elapsed().as_secs();
        if elapsed_wall >= budgets.wall_clock_secs {
            return salvage_draft(
                state,
                progress,
                budgets.max_root_turns,
                &format!("wall clock budget exhausted ({}s)", budgets.wall_clock_secs),
            );
        }

        state.stats.root_turns += 1;
        emit(
            &mut progress,
            ProgressEvent::Thinking {
                turn: state.stats.root_turns,
                max_turns: budgets.max_root_turns,
            },
        );

        let turn_started = Instant::now();
        let response = completer
            .complete(&history)
            .map_err(AssistError::Completer)?;
        let turn_secs = turn_started.elapsed().as_secs_f64();

        match parse_turn(&response) {
            ParsedTurn::Tool(call) => {
                invalid_streak = 0;
                // Refuse further greps after sustained no-match thrashing.
                if greps_blocked && call.name == "corpus_grep" {
                    history.push_str("\n# Assistant\n");
                    history.push_str(&truncate_for_history(&response, 400));
                    history.push_str(
                        "\n# Tool result\ncorpus_grep blocked: too many empty searches. Write the COMPLETE program now in a <silc>…</silc> block.\n",
                    );
                    history.push_str(
                        "\n# Next\nSTOP exploring. Reply NOW with the COMPLETE Silc program for the task inside a <silc>…</silc> block.\n",
                    );
                    history = truncate_history(&history, HISTORY_CAP);
                    emit_action(
                        &mut progress,
                        state.stats.root_turns,
                        budgets.max_root_turns,
                        turn_secs,
                        ActionKind::StillRefining {
                            reason: "grep budget exhausted — write the program".into(),
                        },
                    );
                    continue;
                }
                let outcome = execute_tool(&call, corpus, &mut state, budgets, completer)
                    .map_err(AssistError::Failed)?;
                match outcome {
                    ToolOutcome::Continue(meta) => {
                        emit_action_from_tool(
                            &mut progress,
                            state.stats.root_turns,
                            budgets.max_root_turns,
                            turn_secs,
                            &call,
                            &meta,
                            &state,
                            corpus,
                        );
                        let call_key = format!("{}:{}", call.name, call.args);
                        let repeated = recent_calls.contains(&call_key);
                        recent_calls.push(call_key);
                        if recent_calls.len() > 4 {
                            recent_calls.remove(0);
                        }

                        let no_grep_hits =
                            call.name == "corpus_grep" && meta.contains("(no matches)");
                        if call.name == "corpus_grep" {
                            if grep_budget > 0 {
                                grep_budget -= 1;
                            }
                            if no_grep_hits {
                                empty_grep_streak += 1;
                            } else {
                                empty_grep_streak = 0;
                            }
                            if empty_grep_streak >= 2 || grep_budget == 0 {
                                greps_blocked = true;
                            }
                        } else if call.name.starts_with("corpus_") {
                            empty_grep_streak = 0;
                        }

                        let is_explore = matches!(
                            call.name.as_str(),
                            "corpus_list" | "corpus_grep" | "corpus_read" | "draft_get" | "llm_query"
                        );
                        let made_progress = call.name == "draft_set"
                            && !meta.contains("draft_set: rejected")
                            && !state.is_unchanged_seed(&state.draft);
                        if made_progress {
                            explore_streak = 0;
                            short_draft_streak = 0;
                        } else if meta.contains("draft_set: rejected") {
                            short_draft_streak += 1;
                            explore_streak += 1;
                        } else if is_explore {
                            explore_streak += 1;
                        }

                        history.push_str("\n# Assistant\n");
                        history.push_str(&truncate_for_history(&response, 1200));
                        history.push_str("\n# Tool result\n");
                        history.push_str(&truncate_for_history(&meta, 3000));
                        if repeated {
                            history.push_str(
                                "\n# Note\nYou already made this exact tool call; the result is unchanged. Do NOT repeat it.\n",
                            );
                        }
                        if no_grep_hits || empty_grep_streak >= 1 {
                            history.push_str(
                                "\n# Note\nNo corpus matches. Stop grepping. Write the COMPLETE program now in a <silc>…</silc> block.\n",
                            );
                        }

                        // Seeded modify sessions start with a non-empty draft; "unchanged"
                        // is the real stuck signal, not emptiness.
                        let needs_edit = seed_present && state.is_unchanged_seed(&state.draft);
                        let exploring_too_long = explore_streak >= 2
                            || state.stats.root_turns >= 3
                                && (needs_edit
                                    || state.draft.trim().is_empty()
                                    || short_draft_streak >= 2);

                        if meta.contains("draft_set: rejected") {
                            history.push_str(&format!(
                                "\n# Next\nDraft rejected as too short (need ≥{MIN_DRAFT_CHARS} chars). Reply with the FULL program in a <silc>…</silc> block — shebang, @version(\"0.4.0\"), contract, component with the task fields, app route, optional processor. No more corpus_read.\n",
                            ));
                        } else if exploring_too_long || repeated || greps_blocked {
                            history.push_str(
                                "\n# Next\nSTOP exploring. Reply NOW with the COMPLETE Silc program for the task inside a <silc>…</silc> block (adapt the current draft/target). Do not call corpus tools again.\n",
                            );
                        } else if state.last_check_ok && !state.is_unchanged_seed(&state.draft) {
                            history.push_str(
                                "\n# Next\nsilc_check passed. If the draft fulfills the task, reply with exactly this line and nothing else:\nFINAL_VAR(draft)\nOtherwise improve the draft with draft_set and re-run silc_check.\n",
                            );
                        } else {
                            history.push_str(
                                "\n# Next\nCall the next tool, or write the complete program in a <silc>…</silc> block.\n",
                            );
                        }
                        history = truncate_history(&history, HISTORY_CAP);
                    }
                    ToolOutcome::Finished(program) => {
                        emit_action(
                            &mut progress,
                            state.stats.root_turns,
                            budgets.max_root_turns,
                            turn_secs,
                            ActionKind::Accepted,
                        );
                        return Ok(AssistResult {
                            program,
                            stats: state.stats,
                            finalized: true,
                        });
                    }
                }
            }
            ParsedTurn::Final(program) => {
                invalid_streak = 0;
                match resolve_final(&program, &mut state) {
                Ok(program) => {
                    emit_action(
                        &mut progress,
                        state.stats.root_turns,
                        budgets.max_root_turns,
                        turn_secs,
                            ActionKind::Accepted,
                    );
                    return Ok(AssistResult {
                        program,
                        stats: state.stats,
                        finalized: true,
                    });
                }
                Err(error) => {
                    emit_action(
                        &mut progress,
                        state.stats.root_turns,
                        budgets.max_root_turns,
                        turn_secs,
                            ActionKind::StillRefining {
                            reason: if error.contains("unchanged") {
                                friendly_final_var_reason(&error)
                            } else {
                                "program did not pass the compiler check".into()
                            },
                        },
                    );
                    history.push_str("\n# Assistant\n");
                    history.push_str(&truncate_for_history(&response, 1200));
                    history.push_str("\n# Error\n");
                    history.push_str(&error);
                    history.push_str("\n# Next\nRepair with tools, then FINAL again.\n");
                    history = truncate_history(&history, HISTORY_CAP);
                }
            }
            }
            ParsedTurn::FinalVar => {
                invalid_streak = 0;
                match resolve_final_var(&state) {
                Ok(program) => {
                    emit_action(
                        &mut progress,
                        state.stats.root_turns,
                        budgets.max_root_turns,
                        turn_secs,
                            ActionKind::Accepted,
                    );
                    return Ok(AssistResult {
                        program,
                        stats: state.stats,
                        finalized: true,
                    });
                }
                Err(error) => {
                    emit_action(
                        &mut progress,
                        state.stats.root_turns,
                        budgets.max_root_turns,
                        turn_secs,
                            ActionKind::StillRefining {
                            reason: friendly_final_var_reason(&error),
                        },
                    );
                    history.push_str("\n# Assistant\nFINAL_VAR(draft)\n# Error\n");
                    history.push_str(&error);
                    if error.contains("unchanged") {
                        history.push_str(
                            "\n# Next\nWrite the edited program now: reply with the COMPLETE modified Silc program in a <silc>…</silc> block (keep valid structure, apply the task), then silc_check.\n",
                        );
                    } else {
                        history.push_str(
                            "\n# Next\nUse silc_check on the draft, then FINAL_VAR(draft).\n",
                        );
                    }
                    history = truncate_history(&history, HISTORY_CAP);
                }
            }
            }
            ParsedTurn::Invalid(msg) => {
                invalid_streak += 1;
                emit_action(
                    &mut progress,
                    state.stats.root_turns,
                    budgets.max_root_turns,
                    turn_secs,
                    ActionKind::InvalidTurn {
                        detail: truncate_one_line(&msg, 80),
                    },
                );
                // Cap and strip backticks so empty fences are not re-taught.
                let sanitized = sanitize_history_reply(&response, 200);
                history.push_str("\n# Assistant\n");
                history.push_str(&sanitized);
                history.push_str("\n# Error\n");
                history.push_str(&msg);
                if invalid_streak >= 3 {
                    return salvage_draft(
                        state,
                        progress,
                        budgets.max_root_turns,
                        "too many invalid replies; abandoning tool loop",
                    );
                } else if invalid_streak >= 2 {
                    history.push_str(
                        "\n# Next\nStop using tools. Reply with ONLY the complete Silc program for the task, starting with #!/usr/bin/env silc. No commentary.\n",
                    );
                } else {
                    history.push_str(
                        "\n# Next\nEmit a valid tool call as strict JSON inside <tool>…</tool>, e.g. <tool>{\"name\":\"corpus_list\",\"args\":{}}</tool>.\n",
                    );
                }
                history = truncate_history(&history, HISTORY_CAP);
            }
        }
    }

    let reason = format!("root turn budget exhausted ({})", budgets.max_root_turns);
    salvage_draft(state, progress, budgets.max_root_turns, &reason)
}

/// Strip backticks and cap length so invalid replies do not teach empty fences.
fn sanitize_history_reply(text: &str, max: usize) -> String {
    let cleaned = text.replace('`', "'");
    truncate_for_history(&cleaned, max)
}

fn emit(progress: &mut Option<&mut dyn ProgressReporter>, event: ProgressEvent) {
    if let Some(p) = progress.as_mut() {
        p.on_event(event);
    }
}

fn emit_action(
    progress: &mut Option<&mut dyn ProgressReporter>,
    turn: usize,
    max_turns: usize,
    elapsed_secs: f64,
    kind: ActionKind,
) {
    emit(
        progress,
        ProgressEvent::Action {
            turn,
            max_turns,
            elapsed_secs,
            kind,
        },
    );
}

fn emit_action_from_tool(
    progress: &mut Option<&mut dyn ProgressReporter>,
    turn: usize,
    max_turns: usize,
    elapsed_secs: f64,
    call: &crate::tools::ToolCall,
    meta: &str,
    state: &ToolState,
    corpus: &Corpus,
) {
    let kind = match call.name.as_str() {
        "corpus_list" => ActionKind::ListedCorpus {
            docs: corpus.len(),
        },
        "corpus_grep" => {
            let pattern = call
                .args
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let path = call
                .args
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let (match_count, no_matches) = count_grep_matches(meta);
            ActionKind::Searched {
                pattern: truncate_one_line(pattern, 48),
                path,
                match_count,
                no_matches,
            }
        }
        "corpus_read" => {
            if let Some((id, start, end, total)) = parse_read_meta(meta) {
                ActionKind::ReadCorpus {
                    id: friendly_corpus_id(&id),
                    start,
                    end,
                    total,
                }
            } else {
                let id = call
                    .args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("corpus");
                ActionKind::ReadCorpus {
                    id: friendly_corpus_id(id),
                    start: call
                        .args
                        .get("start")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize,
                    end: 0,
                    total: 0,
                }
            }
        }
        "llm_query" => {
            let prompt = call
                .args
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("sub-question");
            ActionKind::Queried {
                purpose: truncate_one_line(prompt, 64),
            }
        }
        "draft_set" => {
            let short_rejected = meta.contains("draft_set: rejected");
            let unchanged = meta.contains("unchanged") || state.is_unchanged_seed(&state.draft);
            ActionKind::PreparedCode {
                chars: if short_rejected { 0 } else { state.draft.len() },
                preview: if short_rejected || unchanged {
                    String::new()
                } else {
                    draft_preview(&state.draft, 6)
                },
                short_rejected,
                unchanged,
            }
        }
        "draft_get" => ActionKind::InspectedDraft {
            chars: state.draft.len(),
            empty: state.draft.trim().is_empty(),
        },
        "silc_check" => {
            if meta.contains("silc_check: ok") {
                ActionKind::Checked {
                    ok: true,
                    detail: "compiler check passed".into(),
                }
            } else {
                ActionKind::Checked {
                    ok: false,
                    detail: truncate_one_line(meta, 100),
                }
            }
        }
        other => ActionKind::UnknownTool {
            name: other.to_string(),
        },
    };
    emit_action(progress, turn, max_turns, elapsed_secs, kind);

    // draft_set auto-runs silc_check — surface that as a second durable line.
    if call.name == "draft_set" {
        if meta.contains("silc_check: ok") {
            emit_action(
                progress,
                turn,
                max_turns,
                elapsed_secs,
                ActionKind::Checked {
                    ok: true,
                    detail: "compiler check passed".into(),
                },
            );
        } else if meta.contains("silc_check: fail") {
            emit_action(
                progress,
                turn,
                max_turns,
                elapsed_secs,
                ActionKind::Checked {
                    ok: false,
                    detail: truncate_one_line(meta, 100),
                },
            );
        }
    }
}

fn friendly_corpus_id(id: &str) -> String {
    if id == "agents" || id.ends_with("/AGENTS.md") || id == "project/agents" {
        if id == "project/agents" {
            "project AGENTS.md".into()
        } else if id == "agents" {
            "AGENTS.md".into()
        } else {
            format!("{id}")
        }
    } else if id == "target" {
        "target program".into()
    } else {
        id.to_string()
    }
}

fn friendly_final_var_reason(error: &str) -> String {
    if error.contains("unchanged") {
        "program not edited yet — applying your request".into()
    } else if error.contains("empty") {
        "no program drafted yet".into()
    } else if error.contains("rejected") || error.contains("silc_check") {
        "draft still needs fixes".into()
    } else {
        "still refining the program".into()
    }
}

/// On budget exhaustion, return a check-passing draft rather than failing.
fn salvage_draft(
    state: ToolState,
    mut progress: Option<&mut dyn ProgressReporter>,
    max_turns: usize,
    reason: &str,
) -> Result<AssistResult, AssistError> {
    if state.is_unchanged_seed(&state.draft) {
        return Err(AssistError::Budget(format!(
            "{reason}; the program was never edited — nothing written"
        )));
    }
    if state.last_check_ok && !state.draft.trim().is_empty() {
        emit_action(
            &mut progress,
            state.stats.root_turns,
            max_turns,
            0.0,
            ActionKind::Salvaged {
                reason: reason.to_string(),
            },
        );
        return Ok(AssistResult {
            program: state.draft,
            stats: state.stats,
            finalized: false,
        });
    }
    Err(AssistError::Budget(reason.to_string()))
}

/// Truncate history while preserving the bootstrap prefix so KV-cache reuse
/// stays effective across turns. Drops from the middle of the transcript.
fn truncate_history(history: &str, max_chars: usize) -> String {
    let count = history.chars().count();
    if count <= max_chars {
        return history.to_string();
    }

    let split_at = history
        .find("\n# Assistant\n")
        .unwrap_or(0);
    let (prefix, rest) = history.split_at(split_at);
    let prefix_len = prefix.chars().count();
    if prefix_len >= max_chars {
        return prefix.chars().take(max_chars).collect();
    }

    let marker = "\n…[history truncated]\n";
    let marker_len = marker.chars().count();
    let rest_budget = max_chars.saturating_sub(prefix_len).saturating_sub(marker_len);
    let rest_count = rest.chars().count();
    if rest_count <= rest_budget {
        return history.to_string();
    }
    let skip = rest_count - rest_budget;
    let mut trimmed: String = rest.chars().skip(skip).collect();
    if let Some(i) = trimmed.find("\n# ") {
        trimmed = trimmed[i..].to_string();
    }
    format!("{prefix}{marker}{trimmed}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complete::ScriptedCompleter;
    use crate::progress::NullProgress;

    #[test]
    fn scripted_loop_checks_and_finalizes() {
        let corpus = Corpus::builtin();
        let source = corpus.get("fixture/scored_form.silc").unwrap().to_string();
        let escaped = serde_json::to_string(&source).unwrap();
        let mut completer = ScriptedCompleter::new([
            format!("```tool\n{{\"name\":\"corpus_list\",\"args\":{{}}}}\n```"),
            format!("```tool\n{{\"name\":\"draft_set\",\"args\":{{\"source\":{escaped}}}}}\n```"),
            format!("```tool\n{{\"name\":\"silc_check\",\"args\":{{}}}}\n```"),
            "FINAL_VAR(draft)".to_string(),
        ]);
        let budgets = Budgets {
            max_draft_attempts: 0,
            ..Budgets::default()
        };
        let result = run_assist(
            "make a scored form",
            &corpus,
            &mut completer,
            &budgets,
            None,
            AssistSeed::default(),
        )
        .expect("assist");
        assert!(result.program.contains("FeedbackRecord"));
        assert_eq!(result.stats.root_turns, 4);
        // draft_set auto-checks, then the explicit silc_check call.
        assert_eq!(result.stats.checks, 2);
        assert!(result.finalized);
    }

    #[test]
    fn budget_exhaustion() {
        let corpus = Corpus::builtin();
        let mut completer = ScriptedCompleter::new([
            "```tool\n{\"name\":\"corpus_list\",\"args\":{}}\n```".to_string(),
            "```tool\n{\"name\":\"corpus_list\",\"args\":{}}\n```".to_string(),
        ]);
        let budgets = Budgets {
            max_root_turns: 2,
            max_draft_attempts: 0,
            ..Budgets::default()
        };
        let err = run_assist(
            "x",
            &corpus,
            &mut completer,
            &budgets,
            None,
            AssistSeed::default(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("root turn budget"));
    }

    #[test]
    fn final_var_reject_emits_progress() {
        let corpus = Corpus::builtin();
        let mut completer = ScriptedCompleter::new([
            "FINAL_VAR(draft)".to_string(),
            "```tool\n{\"name\":\"corpus_list\",\"args\":{}}\n```".to_string(),
        ]);
        let budgets = Budgets {
            max_root_turns: 2,
            max_draft_attempts: 0,
            ..Budgets::default()
        };
        struct Capture {
            events: Vec<String>,
        }
        impl ProgressReporter for Capture {
            fn on_event(&mut self, event: ProgressEvent) {
                self.events.push(format!("{event:?}"));
            }
        }
        let mut cap = Capture { events: vec![] };
        let _ = run_assist(
            "x",
            &corpus,
            &mut completer,
            &budgets,
            Some(&mut cap),
            AssistSeed::default(),
        );
        assert!(
            cap.events.iter().any(|e| e.contains("StillRefining")),
            "expected StillRefining in {:?}",
            cap.events
        );
    }

    #[test]
    fn action_trace_covers_search_read_query_and_accept() {
        let corpus = Corpus::builtin();
        let source = corpus.get("fixture/scored_form.silc").unwrap().to_string();
        let escaped = serde_json::to_string(&source).unwrap();
        let mut completer = ScriptedCompleter::new([
            "```tool\n{\"name\":\"corpus_grep\",\"args\":{\"pattern\":\"FeedbackRecord\"}}\n```"
                .to_string(),
            "```tool\n{\"name\":\"corpus_read\",\"args\":{\"id\":\"agents\",\"start\":0,\"len\":200}}\n```"
                .to_string(),
            "```tool\n{\"name\":\"llm_query\",\"args\":{\"prompt\":\"How should a dual-surface form look?\"}}\n```"
                .to_string(),
            // Nested completion consumed by llm_query itself.
            "Prefer a dual-surface ui::form with labeled fields.".to_string(),
            format!("```tool\n{{\"name\":\"draft_set\",\"args\":{{\"source\":{escaped}}}}}\n```"),
            "FINAL_VAR(draft)".to_string(),
        ]);
        let budgets = Budgets {
            max_draft_attempts: 0,
            ..Budgets::default()
        };
        struct Capture {
            kinds: Vec<String>,
        }
        impl ProgressReporter for Capture {
            fn on_event(&mut self, event: ProgressEvent) {
                match event {
                    ProgressEvent::Thinking { .. } => self.kinds.push("Thinking".into()),
                    ProgressEvent::Action { kind, .. } => self.kinds.push(format!("{kind:?}")),
                }
            }
        }
        let mut cap = Capture { kinds: vec![] };
        let result = run_assist(
            "make a scored form",
            &corpus,
            &mut completer,
            &budgets,
            Some(&mut cap),
            AssistSeed::default(),
        )
        .expect("assist");
        assert!(result.finalized);
        let joined = cap.kinds.join("\n");
        assert!(joined.contains("Searched"), "{joined}");
        assert!(joined.contains("ReadCorpus"), "{joined}");
        assert!(joined.contains("Queried"), "{joined}");
        assert!(joined.contains("PreparedCode"), "{joined}");
        assert!(joined.contains("Accepted"), "{joined}");
        // grep should report at least one match for FeedbackRecord
        assert!(
            cap.kinds.iter().any(|k| k.contains("match_count") && !k.contains("match_count: 0")),
            "{joined}"
        );
    }

    #[test]
    fn unchanged_seed_is_never_finalized() {
        let corpus = Corpus::builtin();
        let source = corpus.get("fixture/scored_form.silc").unwrap().to_string();
        let mut completer = ScriptedCompleter::new([
            "FINAL_VAR(draft)".to_string(),
            "FINAL_VAR(draft)".to_string(),
        ]);
        let budgets = Budgets {
            max_root_turns: 2,
            max_draft_attempts: 0,
            ..Budgets::default()
        };
        let mut null = NullProgress;
        let err = run_assist(
            "tweak the form",
            &corpus,
            &mut completer,
            &budgets,
            Some(&mut null),
            AssistSeed {
                draft: Some(source),
            },
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("never edited"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn seed_finalizes_once_actually_edited() {
        let corpus = Corpus::builtin();
        let seed_source = corpus.get("fixture/scored_form.silc").unwrap().to_string();
        let edited = seed_source.replace("Share feedback", "Newspaper front page");
        let escaped = serde_json::to_string(&edited).unwrap();
        let mut completer = ScriptedCompleter::new([
            format!("```tool\n{{\"name\":\"draft_set\",\"args\":{{\"source\":{escaped}}}}}\n```"),
            "FINAL_VAR(draft)".to_string(),
        ]);
        let budgets = Budgets {
            max_draft_attempts: 0,
            ..Budgets::default()
        };
        let mut null = NullProgress;
        let result = run_assist(
            "make it a newspaper front page",
            &corpus,
            &mut completer,
            &budgets,
            Some(&mut null),
            AssistSeed {
                draft: Some(seed_source.clone()),
            },
        )
        .expect("edited final");
        assert!(result.finalized);
        assert!(result.program.contains("Newspaper front page"));
        assert_ne!(result.program.trim(), seed_source.trim());
    }

    #[test]
    fn seed_bootstrap_pushes_edit_not_agents_loop() {
        let corpus = Corpus::builtin();
        let boot = root_bootstrap("hotel signup form", &corpus, true);
        assert!(boot.contains("MUST change"));
        assert!(boot.contains("<silc>"));
        assert!(!boot.contains("```"));
        assert!(!boot.contains("corpus_read `agents` (or `project/agents`) then draft"));
    }

    #[test]
    fn session_hints_have_no_markdown_fences() {
        // Permanent regression guard: empty ```tool``` hints caused the EOF parse loop.
        let corpus = Corpus::builtin();
        let boot = root_bootstrap("x", &corpus, true);
        assert!(!boot.contains("```"));
        let boot_new = root_bootstrap("x", &corpus, false);
        assert!(!boot_new.contains("```"));
    }

    #[test]
    fn draft_first_without_explore_does_not_enter_tool_loop() {
        let corpus = Corpus::builtin();
        let mut completer = ScriptedCompleter::new(["not a program", "still bad", "nope", "no"]);
        let budgets = Budgets {
            max_draft_attempts: 2,
            allow_explore: false,
            max_root_turns: 24,
            ..Budgets::default()
        };
        let err = run_assist(
            "anything",
            &corpus,
            &mut completer,
            &budgets,
            None,
            AssistSeed::default(),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("could not produce a valid program") || msg.contains("--explore"),
            "unexpected error: {msg}"
        );
        // Scripted completer should not have been asked for tool-loop turns
        // beyond the 2 draft attempts (4 responses provided; only 2 consumed).
    }

    #[test]
    fn draft_first_path_accepts_program_without_tools() {
        let corpus = Corpus::builtin();
        let source = corpus.get("fixture/scored_form.silc").unwrap().to_string();
        let mut completer = ScriptedCompleter::new([source.clone()]);
        let budgets = Budgets {
            max_draft_attempts: 2,
            max_root_turns: 1,
            ..Budgets::default()
        };
        let result = run_assist(
            "make a scored form",
            &corpus,
            &mut completer,
            &budgets,
            None,
            AssistSeed::default(),
        )
        .expect("draft-first");
        assert!(result.finalized);
        assert!(result.program.contains("FeedbackRecord"));
    }

    #[test]
    fn repeated_explore_on_seed_forces_write_nudge() {
        let corpus = Corpus::builtin();
        let source = corpus.get("fixture/scored_form.silc").unwrap().to_string();
        let read = "```tool\n{\"name\":\"corpus_read\",\"args\":{\"id\":\"project/agents\",\"start\":0,\"len\":1000}}\n```";
        // Insert a fake project/agents so the read succeeds.
        let mut corpus = corpus;
        corpus.insert("project/agents", "# agents\n".repeat(50));

        struct Rec {
            responses: Vec<String>,
            prompts: Vec<String>,
            i: usize,
        }
        impl Completer for Rec {
            fn complete(&mut self, prompt: &str) -> Result<String, String> {
                self.prompts.push(prompt.to_string());
                let out = self
                    .responses
                    .get(self.i)
                    .cloned()
                    .unwrap_or_else(|| {
                        "```tool\n{\"name\":\"corpus_list\",\"args\":{}}\n```".into()
                    });
                self.i += 1;
                Ok(out)
            }
        }
        let mut rec = Rec {
            responses: vec![read.to_string(), read.to_string(), read.to_string()],
            prompts: vec![],
            i: 0,
        };
        let budgets = Budgets {
            max_root_turns: 3,
            max_draft_attempts: 0,
            ..Budgets::default()
        };
        let _ = run_assist(
            "hotel form",
            &corpus,
            &mut rec,
            &budgets,
            None,
            AssistSeed {
                draft: Some(source),
            },
        );
        let last = rec.prompts.last().cloned().unwrap_or_default();
        assert!(
            last.contains("STOP exploring") || last.contains("COMPLETE Silc program"),
            "expected force-write nudge in history, got tail:\n{}",
            last.chars().rev().take(800).collect::<String>().chars().rev().collect::<String>()
        );
    }

    #[test]
    fn truncate_history_preserves_bootstrap_prefix() {
        let bootstrap = "# System\nprompt\n# Begin\nCall a tool now.\n";
        let mut history = bootstrap.to_string();
        for i in 0..200 {
            history.push_str(&format!(
                "\n# Assistant\nturn-{i} {}\n# Tool result\nmeta-{i} {}\n# Next\nGo.\n",
                "x".repeat(40),
                "y".repeat(40),
            ));
        }
        assert!(
            history.chars().count() > HISTORY_CAP,
            "fixture too small: {}",
            history.chars().count()
        );
        let truncated = truncate_history(&history, HISTORY_CAP);
        assert!(truncated.starts_with(bootstrap.trim_end()));
        assert!(truncated.contains("…[history truncated]"));
        assert!(truncated.contains("# Assistant\n"));
        assert!(truncated.chars().count() <= HISTORY_CAP + 40);
        assert!(truncated.contains("turn-199") || truncated.contains("turn-198"));
        assert!(!truncated.contains("turn-0 "));
    }
}
