//! Bank compiler-validated (prompt, program) pairs for silclm SFT.

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use uuid::Uuid;

use crate::check::{check_source, extract_program};
use crate::prompts::sha256_hex;
use crate::schema::{AcceptedRecord, CandidateRecord, RejectedRecord};

const TARGET_MODEL: &str = "silclm";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BankStats {
    pub accepted: usize,
    pub rejected: usize,
    pub duplicates: usize,
}

pub fn bank_candidates(
    candidates_path: &Path,
    accepted_path: &Path,
    rejected_path: &Path,
    emit: bool,
) -> Result<BankStats, String> {
    let candidates = read_candidates(candidates_path)?;
    let mut seen = load_accepted_hashes(accepted_path)?;
    let mut stats = BankStats::default();
    let tmp = std::env::temp_dir().join(format!("sil-training-{}", Uuid::new_v4()));

    for candidate in candidates {
        let program = extract_program(&candidate.completion);
        let program_sha = sha256_hex(&program);
        if seen.contains(&program_sha) {
            stats.duplicates += 1;
            continue;
        }

        let emit_dir = if emit { Some(tmp.as_path()) } else { None };
        match check_source(&program, emit_dir) {
            Ok(result) => {
                let record = AcceptedRecord {
                    id: Uuid::new_v4().to_string(),
                    prompt_id: candidate.prompt_id.clone(),
                    task: candidate.task.clone(),
                    prompt: candidate.prompt.clone(),
                    program,
                    program_sha256: program_sha.clone(),
                    compiler_version: env!("CARGO_PKG_VERSION").into(),
                    execution_mode: result.execution_mode.to_string(),
                    validation_tier: result.validation_tier,
                    target_model: TARGET_MODEL.into(),
                    generator_model: candidate.model.clone(),
                    category: candidate.category.clone(),
                };
                append_jsonl(accepted_path, &record)?;
                seen.insert(program_sha);
                stats.accepted += 1;
            }
            Err(error) => {
                let stage = error
                    .split_once(':')
                    .map(|(stage, _)| stage.to_string())
                    .unwrap_or_else(|| "unknown".into());
                let record = RejectedRecord {
                    id: Uuid::new_v4().to_string(),
                    prompt_id: candidate.prompt_id.clone(),
                    task: candidate.task.clone(),
                    prompt: candidate.prompt.clone(),
                    completion: candidate.completion.clone(),
                    error,
                    stage,
                    compiler_version: env!("CARGO_PKG_VERSION").into(),
                    generator_model: candidate.model.clone(),
                };
                append_jsonl(rejected_path, &record)?;
                stats.rejected += 1;
            }
        }
    }

    let _ = fs::remove_dir_all(&tmp);
    Ok(stats)
}

fn read_candidates(path: &Path) -> Result<Vec<CandidateRecord>, String> {
    let file = fs::File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut out = Vec::new();
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|error| format!("read {}: {error}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: CandidateRecord = serde_json::from_str(&line)
            .map_err(|error| format!("parse {}:{}: {error}", path.display(), idx + 1))?;
        out.push(record);
    }
    Ok(out)
}

fn load_accepted_hashes(path: &Path) -> Result<HashSet<String>, String> {
    let mut seen = HashSet::new();
    if !path.is_file() {
        return Ok(seen);
    }
    let file = fs::File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| format!("read {}: {error}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(&line)
            .map_err(|error| format!("parse {}: {error}", path.display()))?;
        if let Some(hash) = value.get("program_sha256").and_then(|v| v.as_str()) {
            seen.insert(hash.to_string());
        }
    }
    Ok(seen)
}

fn append_jsonl<T: serde::Serialize>(path: &Path, record: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    let line =
        serde_json::to_string(record).map_err(|error| format!("serialize bank record: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("write {}: {error}", path.display()))?;
    file.flush()
        .map_err(|error| format!("flush {}: {error}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("sil-training-test-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn banks_valid_and_rejects_invalid() {
        let dir = temp_dir();
        let candidates = dir.join("candidates.jsonl");
        let accepted = dir.join("accepted.jsonl");
        let rejected = dir.join("rejected.jsonl");

        let valid = r#"@version("0.2.0")
class Note { has Str $.text; }
class NotePage is component {
    has state Str $.text = "";
    method render() { ui::page(ui::heading(:text("Hi"), :level(1))) }
}
class NoteApp is app {
    route "/" => NotePage;
    method serve() {
        ui::web(:root(NoteApp), :port(18080), :route("/"))
            ==> ui::terminal(:port(18023))
    }
}
"#;
        let lines = [
            serde_json::json!({
                "prompt_id": "p1",
                "prompt": "task one",
                "task": "note form",
                "completion": format!("```silc\n{valid}\n```"),
                "model": "teacher"
            }),
            serde_json::json!({
                "prompt_id": "p2",
                "prompt": "task two",
                "task": "broken",
                "completion": "class broken {{{",
                "model": "teacher"
            }),
        ];
        let mut file = fs::File::create(&candidates).unwrap();
        for line in &lines {
            writeln!(file, "{}", serde_json::to_string(line).unwrap()).unwrap();
        }

        let stats = bank_candidates(&candidates, &accepted, &rejected, true).unwrap();
        assert_eq!(stats.accepted, 1);
        assert_eq!(stats.rejected, 1);
        assert_eq!(stats.duplicates, 0);

        let accepted_text = fs::read_to_string(&accepted).unwrap();
        assert!(accepted_text.contains("\"target_model\":\"silclm\""));
        assert!(accepted_text.contains("generator_model"));

        // Dedup on second pass.
        let stats2 = bank_candidates(&candidates, &accepted, &rejected, false).unwrap();
        assert_eq!(stats2.duplicates, 1);
        assert_eq!(stats2.accepted, 0);

        let _ = fs::remove_dir_all(dir);
    }
}
