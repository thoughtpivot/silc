//! Completer trait for root and depth-1 `llm_query` calls.

/// Chat-style request (uses the model chat template when the worker supports it).
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: usize,
    /// Stop sequences for chat completion (e.g. `"\n# END"`).
    pub stop: Vec<String>,
    /// Sampling temperature; escalated on repeated identical drafts.
    pub temperature: Option<f32>,
}

impl ChatRequest {
    pub fn new(system: impl Into<String>, user: impl Into<String>, max_tokens: usize) -> Self {
        Self {
            system: system.into(),
            user: user.into(),
            max_tokens,
            stop: Vec::new(),
            temperature: None,
        }
    }
}

/// Reply from a chat or folded completion call.
#[derive(Debug, Clone)]
pub struct ChatReply {
    pub text: String,
    /// True when generation stopped because `max_tokens` was hit.
    pub truncated: bool,
}

/// Sync text completion used by the assist loop.
pub trait Completer {
    fn complete(&mut self, prompt: &str) -> Result<String, String>;

    /// Prefer chat-template inference when available. Default folds into a
    /// single raw prompt so scripted tests keep working unchanged.
    fn chat(&mut self, req: &ChatRequest) -> Result<ChatReply, String> {
        let prompt = format!(
            "{}\n\n{}\n\n# Assistant\n",
            req.system.trim(),
            req.user.trim()
        );
        let text = self.complete(&prompt)?;
        Ok(ChatReply {
            text,
            truncated: false,
        })
    }
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
