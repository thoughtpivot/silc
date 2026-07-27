//! Read-only assist corpus (ADR-008).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// One document in the assist environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusDoc {
    pub id: String,
    pub body: String,
}

/// Sorted map of corpus documents.
#[derive(Debug, Clone, Default)]
pub struct Corpus {
    docs: BTreeMap<String, String>,
}

impl Corpus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Built-in Silc authoring corpus embedded at compile time.
    pub fn builtin() -> Self {
        let mut corpus = Self::new();
        corpus.insert("agents", include_str!("../../silc/templates/AGENTS.md"));
        corpus.insert(
            "example/chatApp/main.silc",
            include_str!("../../../examples/chatApp/main.silc"),
        );
        corpus.insert(
            "example/inventoryApp/main.silc",
            include_str!("../../../examples/inventoryApp/main.silc"),
        );
        corpus.insert(
            "example/scraperApp/main.silc",
            include_str!("../../../examples/scraperApp/main.silc"),
        );
        corpus.insert(
            "example/pipelineApp/main.silc",
            include_str!("../../../examples/pipelineApp/main.silc"),
        );
        corpus.insert(
            "example/blogApp/main.silc",
            include_str!("../../../examples/blogApp/main.silc"),
        );
        corpus.insert(
            "fixture/scored_form.silc",
            include_str!("../../silc/tests/fixtures/scored_form.silc"),
        );
        corpus.insert(
            "fixture/shopping_app.silc",
            include_str!("../../silc/tests/fixtures/shopping_app.silc"),
        );
        corpus.insert(
            "fixture/data_pipeline.silc",
            include_str!("../../silc/tests/fixtures/data_pipeline.silc"),
        );
        corpus.insert(
            "fixture/data_pipeline_runnable.silc",
            include_str!("../../silc/tests/fixtures/data_pipeline_runnable.silc"),
        );
        corpus
    }

    pub fn insert(&mut self, id: impl Into<String>, body: impl Into<String>) {
        self.docs.insert(id.into(), body.into());
    }

    pub fn get(&self, id: &str) -> Option<&str> {
        self.docs.get(id).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    pub fn total_chars(&self) -> usize {
        self.docs.values().map(String::len).sum()
    }

    pub fn list(&self) -> Vec<(String, usize)> {
        self.docs
            .iter()
            .map(|(id, body)| (id.clone(), body.len()))
            .collect()
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.docs.keys().map(String::as_str)
    }

    /// Load `.silc` / `.md` files under `dir` (non-recursive except one level of subdirs).
    pub fn load_extra_dir(&mut self, dir: &Path) -> Result<usize, String> {
        if !dir.is_dir() {
            return Err(format!("corpus directory not found: {}", dir.display()));
        }
        let mut added = 0;
        for entry in fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
            let path = entry.path();
            if path.is_dir() {
                for child in
                    fs::read_dir(&path).map_err(|e| format!("read {}: {e}", path.display()))?
                {
                    let child = child.map_err(|e| format!("read {}: {e}", path.display()))?;
                    let child_path = child.path();
                    if is_corpus_file(&child_path) {
                        added += self.load_file(&child_path, &path)?;
                    }
                }
            } else if is_corpus_file(&path) {
                added += self.load_file(&path, dir)?;
            }
        }
        Ok(added)
    }

    fn load_file(&mut self, path: &Path, root: &Path) -> Result<usize, String> {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let id = format!("extra/{rel}");
        let body = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        self.insert(id, body);
        Ok(1)
    }

    pub fn grep(&self, pattern: &str, path_filter: Option<&str>) -> Result<Vec<String>, String> {
        let re = regex::Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;
        let mut hits = Vec::new();
        for (id, body) in &self.docs {
            if let Some(filter) = path_filter {
                if !id.contains(filter) {
                    continue;
                }
            }
            for (line_no, line) in body.lines().enumerate() {
                if re.is_match(line) {
                    hits.push(format!("{id}:{}:{}", line_no + 1, truncate(line, 200)));
                    if hits.len() >= 40 {
                        hits.push("… truncated (40 match cap)".into());
                        return Ok(hits);
                    }
                }
            }
        }
        if hits.is_empty() {
            hits.push("(no matches)".into());
        }
        Ok(hits)
    }

    pub fn read_slice(
        &self,
        id: &str,
        start: usize,
        len: usize,
        max_read_chars: usize,
    ) -> Result<String, String> {
        let body = self
            .get(id)
            .ok_or_else(|| format!("unknown corpus id `{id}`"))?;
        let len = len.min(max_read_chars);
        if start >= body.len() {
            return Ok(format!(
                "id={id} start={start} len=0 total={} (start beyond end)",
                body.len()
            ));
        }
        let end = (start + len).min(body.len());
        let slice = &body[start..end];
        Ok(format!(
            "id={id} start={start} end={end} total={}\n{slice}",
            body.len()
        ))
    }
}

fn is_corpus_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("silc") | Some("md") | Some("txt")
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_has_agents_and_examples() {
        let c = Corpus::builtin();
        assert!(c.get("agents").unwrap().contains("Silc"));
        assert!(c
            .get("example/chatApp/main.silc")
            .unwrap()
            .contains("@version"));
        assert!(c.len() >= 8);
    }

    #[test]
    fn read_slice_respects_cap() {
        let mut c = Corpus::new();
        c.insert("t", "abcdefghijklmnopqrstuvwxyz");
        let out = c.read_slice("t", 0, 100, 5).unwrap();
        assert!(out.contains("end=5"));
        assert!(out.contains("abcde"));
        assert!(!out.contains("fg"));
    }
}
