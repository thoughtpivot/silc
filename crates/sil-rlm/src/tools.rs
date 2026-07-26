//! Closed tool surface for silc assist (ADR-008).

use serde::Deserialize;
use serde_json::Value;
use sil_training::{check_source, extract_program};

use crate::complete::Completer;
use crate::corpus::Corpus;
use crate::prompt::truncate_for_history;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budgets {
    pub max_root_turns: usize,
    pub max_silc_check: usize,
    pub max_llm_query: usize,
    pub max_read_chars: usize,
    pub wall_clock_secs: u64,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            max_root_turns: 12,
            max_silc_check: 8,
            max_llm_query: 16,
            max_read_chars: 4000,
            // Each root turn reloads the GGUF in a fresh llama.cpp process;
            // 3B-class weights need well over 120s for a full session on CPU.
            wall_clock_secs: 900,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BudgetStats {
    pub root_turns: usize,
    pub checks: usize,
    pub llm_queries: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub name: String,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Clone)]
pub enum ParsedTurn {
    Tool(ToolCall),
    Final(String),
    FinalVar,
    Invalid(String),
}

#[derive(Debug)]
pub struct ToolState {
    pub draft: String,
    pub last_check_ok: bool,
    pub stats: BudgetStats,
}

impl Default for ToolState {
    fn default() -> Self {
        Self {
            draft: String::new(),
            last_check_ok: false,
            stats: BudgetStats::default(),
        }
    }
}

/// Parse a model turn into a tool call or FINAL.
pub fn parse_turn(text: &str) -> ParsedTurn {
    let trimmed = text.trim();
    if let Some(rest) = strip_prefix_ci(trimmed, "FINAL_VAR(") {
        let inner = rest.split(')').next().unwrap_or("").trim();
        if inner == "draft" || inner.is_empty() {
            return ParsedTurn::FinalVar;
        }
        return ParsedTurn::Invalid(format!("FINAL_VAR only supports draft, got `{inner}`"));
    }
    if let Some(rest) = strip_prefix_ci(trimmed, "FINAL(") {
        if let Some(end) = rest.rfind(')') {
            let program = rest[..end].trim();
            if !program.is_empty() {
                return ParsedTurn::Final(program.to_string());
            }
        }
    }
    // Also allow FINAL at end of a longer message.
    for line in trimmed.lines().rev() {
        let line = line.trim();
        if line.starts_with("FINAL_VAR(") {
            return parse_turn(line);
        }
        if line.starts_with("FINAL(") {
            return parse_turn(line);
        }
    }

    if let Some(json) = extract_tool_json(trimmed) {
        match serde_json::from_str::<ToolCall>(&json) {
            Ok(call) => ParsedTurn::Tool(call),
            Err(first_error) => {
                let repaired = sanitize_tool_json(&json);
                if let Ok(call) = serde_json::from_str::<ToolCall>(&repaired) {
                    return ParsedTurn::Tool(call);
                }
                if let Some(call) = recover_tool_name(&json) {
                    return ParsedTurn::Tool(call);
                }
                ParsedTurn::Invalid(format!(
                    "tool JSON parse error: {first_error}. Use strict JSON, e.g. {{\"name\":\"corpus_list\",\"args\":{{}}}}"
                ))
            }
        }
    } else if let Some(call) = implicit_draft_set(trimmed) {
        // Small models often reply with a bare program instead of a tool call;
        // treat it as draft_set so the work is not lost.
        ParsedTurn::Tool(call)
    } else {
        ParsedTurn::Invalid(
            "no tool call found. Reply with one fenced tool block, e.g.\n```tool\n{\"name\":\"corpus_list\",\"args\":{}}\n```\nor with FINAL_VAR(draft) after a successful silc_check.".into(),
        )
    }
}

/// Turn a bare Silc program reply into a `draft_set` call.
fn implicit_draft_set(text: &str) -> Option<ToolCall> {
    if !(text.contains("```silc") || text.contains("@version(")) {
        return None;
    }
    let mut program = extract_program(text);
    if !program.contains("@version(") {
        return None;
    }
    // Unfenced reply: drop any prose before the program itself.
    if !text.contains("```") {
        if let Some(start) = program.find("@version(") {
            program = program[start..].to_string();
        }
    }
    Some(ToolCall {
        name: "draft_set".to_string(),
        args: serde_json::json!({ "source": program }),
    })
}

/// Repair common small-model JSON mistakes: `{...}` placeholders, ellipses,
/// trailing commas, and single-quoted keys/strings without embedded quotes.
fn sanitize_tool_json(json: &str) -> String {
    let mut out = json
        .replace("{...}", "{}")
        .replace("{…}", "{}")
        .replace('…', "");
    // Remove bare ellipsis tokens left inside objects/arrays: `, ...` / `...`
    while let Some(pos) = out.find("...") {
        out.replace_range(pos..pos + 3, "");
    }
    // Trailing commas before a closing brace/bracket.
    let mut cleaned = String::with_capacity(out.len());
    let chars: Vec<char> = out.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == ',' {
            if let Some(&next) = chars[i + 1..].iter().find(|c| !c.is_whitespace()) {
                if next == '}' || next == ']' {
                    continue;
                }
            }
        }
        cleaned.push(ch);
    }
    // Single quotes → double quotes only when the payload has no double quotes
    // at all (avoids corrupting apostrophes inside proper JSON strings).
    if !cleaned.contains('"') && cleaned.contains('\'') {
        cleaned = cleaned.replace('\'', "\"");
    }
    cleaned
}

/// LLMs emit typographic punctuation the Silc lexer rejects; normalize it.
pub fn normalize_typography(source: &str) -> String {
    source
        .replace('—', "-")
        .replace('–', "-")
        .replace('…', "...")
        .replace(['“', '”'], "\"")
        .replace(['‘', '’'], "'")
}

/// Last resort: pull out a known tool name and run it with empty args so the
/// tool's own error message can guide the model, instead of burning the turn.
fn recover_tool_name(json: &str) -> Option<ToolCall> {
    const KNOWN: [&str; 7] = [
        "corpus_list",
        "corpus_grep",
        "corpus_read",
        "silc_check",
        "llm_query",
        "draft_set",
        "draft_get",
    ];
    let name = KNOWN.into_iter().find(|tool| json.contains(tool))?;
    Some(ToolCall {
        name: name.to_string(),
        args: Value::Object(serde_json::Map::new()),
    })
}

fn extract_tool_json(text: &str) -> Option<String> {
    if let Some(start) = text.find("```tool") {
        let rest = &text[start + "```tool".len()..];
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        if let Some(end) = rest.find("```") {
            return Some(rest[..end].trim().to_string());
        }
    }
    // Bare JSON object with "name"
    let start = text.find('{')?;
    let slice = &text[start..];
    if slice.contains("\"name\"") {
        // naive brace match
        let mut depth = 0i32;
        for (i, ch) in slice.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(slice[..=i].to_string());
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

pub enum ToolOutcome {
    Continue(String),
    Finished(String),
}

/// Execute one closed tool against corpus + draft state.
pub fn execute_tool(
    call: &ToolCall,
    corpus: &Corpus,
    state: &mut ToolState,
    budgets: &Budgets,
    completer: &mut dyn Completer,
) -> Result<ToolOutcome, String> {
    match call.name.as_str() {
        "corpus_list" => {
            let mut out = String::from("corpus_list:\n");
            for (id, len) in corpus.list() {
                out.push_str(&format!("- {id} ({len} chars)\n"));
            }
            Ok(ToolOutcome::Continue(out))
        }
        "corpus_grep" => {
            let pattern = arg_str(&call.args, "pattern")
                .ok_or_else(|| "corpus_grep requires args.pattern".to_string())?;
            let path = arg_str(&call.args, "path");
            let hits = corpus.grep(pattern, path)?;
            Ok(ToolOutcome::Continue(format!(
                "corpus_grep:\n{}",
                hits.join("\n")
            )))
        }
        "corpus_read" => {
            let id = arg_str(&call.args, "id")
                .ok_or_else(|| "corpus_read requires args.id".to_string())?;
            let start = arg_usize(&call.args, "start").unwrap_or(0);
            let len = arg_usize(&call.args, "len").unwrap_or(budgets.max_read_chars);
            let slice = corpus.read_slice(id, start, len, budgets.max_read_chars)?;
            Ok(ToolOutcome::Continue(slice))
        }
        "silc_check" => {
            if state.stats.checks >= budgets.max_silc_check {
                return Ok(ToolOutcome::Continue(format!(
                    "silc_check budget exhausted ({})",
                    budgets.max_silc_check
                )));
            }
            state.stats.checks += 1;
            let source = arg_str(&call.args, "source")
                .map(|s| extract_program(s))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| state.draft.clone());
            if source.trim().is_empty() {
                return Ok(ToolOutcome::Continue(
                    "silc_check: empty source (draft_set or pass args.source)".into(),
                ));
            }
            match check_source(&source, None) {
                Ok(result) => {
                    state.draft = source;
                    state.last_check_ok = true;
                    Ok(ToolOutcome::Continue(format!(
                        "silc_check: ok mode={:?} tier={}",
                        result.execution_mode, result.validation_tier
                    )))
                }
                Err(error) => {
                    state.last_check_ok = false;
                    let stage = error
                        .split_once(':')
                        .map(|(s, _)| s)
                        .unwrap_or("unknown");
                    Ok(ToolOutcome::Continue(format!(
                        "silc_check: fail stage={stage} error={}",
                        truncate_for_history(&error, 800)
                    )))
                }
            }
        }
        "llm_query" => {
            if state.stats.llm_queries >= budgets.max_llm_query {
                return Ok(ToolOutcome::Continue(format!(
                    "llm_query budget exhausted ({})",
                    budgets.max_llm_query
                )));
            }
            state.stats.llm_queries += 1;
            let prompt = arg_str(&call.args, "prompt")
                .ok_or_else(|| "llm_query requires args.prompt".to_string())?;
            let answer = completer.complete(prompt)?;
            Ok(ToolOutcome::Continue(format!(
                "llm_query result:\n{}",
                truncate_for_history(&answer, 2000)
            )))
        }
        "draft_set" => {
            let source = arg_str(&call.args, "source")
                .ok_or_else(|| "draft_set requires args.source".to_string())?;
            let program = normalize_typography(&extract_program(source));
            state.draft = program;
            state.last_check_ok = false;
            let mut msg = format!("draft_set: {} chars stored", state.draft.len());
            if state.draft.len() < 200 {
                msg.push_str(
                    " — warning: this draft is very short. Write the COMPLETE program for the task, not a fragment or an example from the instructions.",
                );
            }
            // Auto-validate: a compiler check is free (no LLM inference), and a
            // small model often forgets to call silc_check on its own.
            if state.stats.checks < budgets.max_silc_check {
                state.stats.checks += 1;
                match check_source(&state.draft, None) {
                    Ok(result) => {
                        state.last_check_ok = true;
                        msg.push_str(&format!(
                            "\nsilc_check: ok mode={:?} tier={}",
                            result.execution_mode, result.validation_tier
                        ));
                    }
                    Err(error) => {
                        let stage = error.split_once(':').map(|(s, _)| s).unwrap_or("unknown");
                        msg.push_str(&format!(
                            "\nsilc_check: fail stage={stage} error={}",
                            truncate_for_history(&error, 800)
                        ));
                    }
                }
            }
            Ok(ToolOutcome::Continue(msg))
        }
        "draft_get" => {
            if state.draft.is_empty() {
                Ok(ToolOutcome::Continue("draft_get: (empty)".into()))
            } else {
                Ok(ToolOutcome::Continue(format!(
                    "draft_get ({} chars):\n{}",
                    state.draft.len(),
                    truncate_for_history(&state.draft, 2000)
                )))
            }
        }
        other => Ok(ToolOutcome::Continue(format!(
            "unknown tool `{other}`; use corpus_list, corpus_grep, corpus_read, silc_check, llm_query, draft_set, draft_get"
        ))),
    }
}

pub fn resolve_final(program: &str, state: &mut ToolState) -> Result<String, String> {
    let source = extract_program(program);
    if source.trim().is_empty() {
        return Err("FINAL program is empty".into());
    }
    match check_source(&source, None) {
        Ok(_) => {
            state.draft = source.clone();
            state.last_check_ok = true;
            Ok(source)
        }
        Err(error) => Err(format!("FINAL rejected by silc_check: {error}")),
    }
}

pub fn resolve_final_var(state: &ToolState) -> Result<String, String> {
    if state.draft.trim().is_empty() {
        return Err("FINAL_VAR(draft): draft is empty".into());
    }
    if !state.last_check_ok {
        // Re-check in case draft was set after a prior ok.
        check_source(&state.draft, None).map_err(|e| format!("FINAL_VAR rejected: {e}"))?;
    }
    Ok(state.draft.clone())
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn arg_usize(args: &Value, key: &str) -> Option<usize> {
    args.get(key)
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n as u64)))
        .map(|n| n as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complete::ScriptedCompleter;
    use serde_json::json;

    #[test]
    fn parses_tool_fence() {
        let turn =
            parse_turn("thinking...\n```tool\n{\"name\":\"corpus_list\",\"args\":{}}\n```\n");
        match turn {
            ParsedTurn::Tool(c) => assert_eq!(c.name, "corpus_list"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn repairs_placeholder_args() {
        let turn = parse_turn("```tool\n{\"name\":\"corpus_list\",\"args\":{...}}\n```");
        match turn {
            ParsedTurn::Tool(c) => assert_eq!(c.name, "corpus_list"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn repairs_trailing_comma() {
        let turn = parse_turn(
            "```tool\n{\"name\":\"corpus_read\",\"args\":{\"id\":\"agents\",\"start\":0,}}\n```",
        );
        match turn {
            ParsedTurn::Tool(c) => {
                assert_eq!(c.name, "corpus_read");
                assert_eq!(c.args.get("id").and_then(|v| v.as_str()), Some("agents"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn recovers_tool_name_from_broken_json() {
        let turn = parse_turn("```tool\n{name: corpus_grep, pattern: service}\n```");
        match turn {
            ParsedTurn::Tool(c) => assert_eq!(c.name, "corpus_grep"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn bare_program_becomes_draft_set() {
        let turn = parse_turn(
            "Here is the program:\n@version(\"0.4.0\")\ncontract Note { has Str $.text; }\n",
        );
        match turn {
            ParsedTurn::Tool(c) => {
                assert_eq!(c.name, "draft_set");
                let source = c.args.get("source").and_then(|v| v.as_str()).unwrap();
                assert!(source.starts_with("@version"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_final_var() {
        assert!(matches!(
            parse_turn("FINAL_VAR(draft)"),
            ParsedTurn::FinalVar
        ));
    }

    #[test]
    fn silc_check_accepts_fixture_slice() {
        let corpus = Corpus::builtin();
        let source = corpus.get("fixture/scored_form.silc").unwrap().to_string();
        let mut state = ToolState::default();
        let budgets = Budgets::default();
        let mut completer = ScriptedCompleter::new(Vec::<String>::new());
        let call = ToolCall {
            name: "silc_check".into(),
            args: json!({"source": source}),
        };
        let out = execute_tool(&call, &corpus, &mut state, &budgets, &mut completer).unwrap();
        match out {
            ToolOutcome::Continue(msg) => assert!(msg.contains("ok"), "{msg}"),
            ToolOutcome::Finished(_) => panic!("unexpected finish"),
        }
        assert!(state.last_check_ok);
    }
}
