//! Fast compiler validation for training candidates (no worker builds).

use std::fs;
use std::path::{Path, PathBuf};

use sil_core::{classify_program, ExecutionMode};
use sil_router::route_program;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub execution_mode: ExecutionMode,
    pub validation_tier: u8,
    pub emit_root: Option<PathBuf>,
}

/// Parse → validate → route → optional emit. Never provisions runtimes.
pub fn check_source(source: &str, emit_dir: Option<&Path>) -> Result<CheckResult, String> {
    let program = sil_parser::parse(source).map_err(|error| format!("parse: {error}"))?;
    program
        .validate_source_version(env!("CARGO_PKG_VERSION"))
        .map_err(|error| format!("version: {error}"))?;
    program
        .validate()
        .map_err(|error| format!("validate: {error}"))?;
    let decisions = route_program(&program);
    let mode = classify_program(&program).map_err(|error| format!("classify: {error}"))?;

    if let Some(root) = emit_dir {
        if root.exists() {
            let _ = fs::remove_dir_all(root);
        }
        fs::create_dir_all(root).map_err(|error| format!("emit mkdir: {error}"))?;
        let entry = root.join("candidate.silc");
        fs::write(&entry, source).map_err(|error| format!("emit write source: {error}"))?;
        sil_codegen::emit(
            &program,
            &decisions,
            &entry,
            root,
            env!("CARGO_PKG_VERSION"),
        )
        .map_err(|error| format!("emit: {error}"))?;
        return Ok(CheckResult {
            execution_mode: mode,
            validation_tier: 3,
            emit_root: Some(root.to_path_buf()),
        });
    }

    Ok(CheckResult {
        execution_mode: mode,
        validation_tier: 2,
        emit_root: None,
    })
}

/// Pull a `.silc` program out of a completion (fenced or raw).
pub fn extract_program(completion: &str) -> String {
    let trimmed = completion.trim();
    // Sentinel form used by assist prompts (avoids teaching the model empty
    // markdown fences that break tool parsing).
    if let Some(start) = trimmed.find("<silc>") {
        let rest = &trimmed[start + "<silc>".len()..];
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        if let Some(end) = rest.find("</silc>") {
            return rest[..end].trim().to_string();
        }
        return rest.trim().to_string();
    }
    if let Some(start) = trimmed.find("```silc") {
        let rest = &trimmed[start + "```silc".len()..];
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_string();
        }
    }
    // Generic markdown fence — skip ```tool (and empty ```tool```) so they are
    // never mistaken for program source.
    let mut search_from = 0usize;
    while let Some(rel) = trimmed[search_from..].find("```") {
        let start = search_from + rel;
        let after = start + 3;
        let rest = &trimmed[after..];
        // Language tag ends at newline or at a nested ``` (empty ```tool```).
        let lang_end = rest
            .find('\n')
            .unwrap_or(rest.len())
            .min(rest.find("```").unwrap_or(rest.len()));
        let lang = rest[..lang_end].trim();
        if lang == "tool" || lang.starts_with("tool") {
            // Empty ```tool```: closing fence begins at lang_end when no newline.
            if rest[lang_end..].starts_with("```") {
                search_from = after + lang_end + 3;
                continue;
            }
            // ```tool\n...\n```
            let body_start = after + lang_end + 1; // skip newline
            if body_start < trimmed.len() {
                if let Some(end) = trimmed[body_start..].find("```") {
                    search_from = body_start + end + 3;
                    continue;
                }
            }
            // Unclosed tool fence — take remainder after the tag.
            return trimmed[after + lang.len()..].trim_start_matches('\n').trim().to_string();
        }
        let body_start = after + lang_end + usize::from(lang_end < rest.len() && rest.as_bytes().get(lang_end) == Some(&b'\n'));
        if let Some(end) = trimmed.get(body_start..).and_then(|b| b.find("```")) {
            return trimmed[body_start..body_start + end].trim().to_string();
        }
        break;
    }
    trimmed[search_from..].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"@version("0.4.0")
contract Note { has Str $.text; }
component NotePage {
    has state Str $.text = "";
    method render() {
        ui::page(ui::heading(:text("Hi"), :level(1)))
    }
}
app NoteApp {
    route "/" => NotePage;
}
"#;

    #[test]
    fn accepts_valid_program() {
        let result = check_source(VALID, None).unwrap();
        assert_eq!(result.execution_mode, ExecutionMode::Runnable);
        assert_eq!(result.validation_tier, 2);
    }

    #[test]
    fn rejects_parse_errors() {
        let err = check_source("class broken {{{", None).unwrap_err();
        assert!(err.starts_with("parse:"));
    }

    #[test]
    fn extracts_fenced_silc() {
        let raw = "Here you go:\n```silc\n@version(\"0.4.0\")\ncontract X {}\n```\n";
        assert_eq!(extract_program(raw), "@version(\"0.4.0\")\ncontract X {}");
    }

    #[test]
    fn extracts_sentinel_silc() {
        let raw = "Here:\n<silc>\n@version(\"0.4.0\")\ncontract X {}\n</silc>\n";
        assert_eq!(extract_program(raw), "@version(\"0.4.0\")\ncontract X {}");
    }
}
