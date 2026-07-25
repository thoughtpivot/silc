//! Build model-ready prompts from AGENTS.md + task seeds.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

pub use crate::schema::{PromptRecord, TaskSeed};

const TARGET_MODEL: &str = "silclm";
const AGENTS_MD_VERSION: &str = "0.2.0";

pub fn load_tasks(tasks_dir: &Path) -> Result<Vec<TaskSeed>, String> {
    if !tasks_dir.is_dir() {
        return Err(format!(
            "tasks directory not found: {}",
            tasks_dir.display()
        ));
    }
    let mut tasks = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(tasks_dir)
        .map_err(|error| format!("read tasks dir: {error}"))?
        .filter_map(|entry| entry.ok())
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let task: TaskSeed = serde_json::from_str(&text)
            .map_err(|error| format!("parse {}: {error}", path.display()))?;
        tasks.push(task);
    }
    if tasks.is_empty() {
        return Err(format!("no task JSON files in {}", tasks_dir.display()));
    }
    Ok(tasks)
}

pub fn build_prompt_records(agents_md: &str, tasks: &[TaskSeed]) -> Vec<PromptRecord> {
    tasks
        .iter()
        .map(|task| {
            let prompt = format_prompt(agents_md, &task.description);
            let prompt_sha256 = sha256_hex(&prompt);
            PromptRecord {
                id: format!("prompt-{}", task.id),
                task_id: task.id.clone(),
                category: task.category.clone(),
                task: task.description.clone(),
                agents_md_version: AGENTS_MD_VERSION.into(),
                prompt,
                prompt_sha256,
                target_model: TARGET_MODEL.into(),
            }
        })
        .collect()
}

pub fn format_prompt(agents_md: &str, task: &str) -> String {
    format!(
        r#"You are silclm, Silc's local language model. Write a complete, valid Silc 0.2.0 program for the task below.

Follow the project guidance exactly. Output only a Silc program (optionally in a ```silc fence). Do not invent React, package.json, Ollama, or hand-edited `.runtime/` files.

# Silc guidance

{agents_md}

# Task

{task}

# Program
"#
    )
}

pub fn write_prompt_jsonl(path: &Path, records: &[PromptRecord]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut file =
        fs::File::create(path).map_err(|error| format!("create {}: {error}", path.display()))?;
    for record in records {
        let line = serde_json::to_string(record)
            .map_err(|error| format!("serialize prompt record: {error}"))?;
        writeln!(file, "{line}").map_err(|error| format!("write {}: {error}", path.display()))?;
    }
    Ok(())
}

pub fn read_prompt_jsonl(path: &Path) -> Result<Vec<PromptRecord>, String> {
    let file = fs::File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut out = Vec::new();
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|error| format!("read {}: {error}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: PromptRecord = serde_json::from_str(&line)
            .map_err(|error| format!("parse {}:{}: {error}", path.display(), idx + 1))?;
        out.push(record);
    }
    Ok(out)
}

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn prompts_are_deterministic() {
        let tasks = [TaskSeed {
            id: "demo".into(),
            category: "form".into(),
            description: "Build a feedback form".into(),
            tags: vec![],
        }];
        let a = build_prompt_records("# guide", &tasks);
        let b = build_prompt_records("# guide", &tasks);
        assert_eq!(a, b);
        assert!(a[0].prompt.contains("silclm"));
        assert!(a[0].prompt.contains("Build a feedback form"));
        assert_eq!(a[0].target_model, "silclm");
    }

    #[test]
    fn loads_repo_task_seeds() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../training/tasks");
        let tasks = load_tasks(&root).expect("training/tasks");
        assert!(tasks.len() >= 6);
        assert!(tasks.iter().any(|task| task.id == "chat_assistant"));
        assert!(tasks.iter().any(|task| task.description.contains("silclm")));
    }
}
