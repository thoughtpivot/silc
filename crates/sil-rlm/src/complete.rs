//! Completer trait for root and depth-1 `llm_query` calls.

/// Sync text completion used by the assist loop.
pub trait Completer {
    fn complete(&mut self, prompt: &str) -> Result<String, String>;
}

/// Scripted completer for offline tests (pops responses in order).
pub struct ScriptedCompleter {
    responses: Vec<String>,
    index: usize,
}

impl ScriptedCompleter {
    pub fn new(responses: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            responses: responses.into_iter().map(Into::into).collect(),
            index: 0,
        }
    }
}

impl Completer for ScriptedCompleter {
    fn complete(&mut self, _prompt: &str) -> Result<String, String> {
        if self.index >= self.responses.len() {
            return Err("scripted completer exhausted".into());
        }
        let out = self.responses[self.index].clone();
        self.index += 1;
        Ok(out)
    }
}
