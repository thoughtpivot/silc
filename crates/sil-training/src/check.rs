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
    if let Some(start) = trimmed.find("```silc") {
        let rest = &trimmed[start + "```silc".len()..];
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_string();
        }
    }
    if let Some(start) = trimmed.find("```") {
        let rest = &trimmed[start + 3..];
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_string();
        }
    }
    trimmed.to_string()
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
}
