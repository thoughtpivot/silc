//! Open document state for Silc IDE features.

use sil_core::{lsp_to_offset, offset_to_lsp, Program, Span};
use sil_lexer::SpannedToken;
use sil_parser::{parse_with_tokens, ParseError};

#[derive(Debug, Clone)]
pub struct HoverRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

impl HoverRange {
    pub fn from_span(source: &str, span: Span) -> Self {
        let (sl, sc, el, ec) = span.to_lsp_range(source);
        Self {
            start_line: sl,
            start_character: sc,
            end_line: el,
            end_character: ec,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HoverContent {
    pub markdown: String,
    pub range: HoverRange,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub uri: String,
    pub version: i32,
    pub source: String,
    pub program: Program,
    pub tokens: Vec<SpannedToken>,
    pub parse_error: Option<String>,
}

impl Document {
    pub fn open(uri: impl Into<String>, version: i32, source: impl Into<String>) -> Self {
        let uri = uri.into();
        let source = source.into();
        Self::from_source(uri, version, source)
    }

    pub fn update(&mut self, version: i32, source: impl Into<String>) {
        let source = source.into();
        let rebuilt = Self::from_source(self.uri.clone(), version, source);
        *self = rebuilt;
    }

    fn from_source(uri: String, version: i32, source: String) -> Self {
        match parse_with_tokens(&source) {
            Ok((program, tokens)) => Self {
                uri,
                version,
                source,
                program,
                tokens,
                parse_error: None,
            },
            Err(err) => {
                let tokens = sil_lexer::lex(&source).unwrap_or_default();
                // Best-effort empty program when parse fails; keyword/token hover still works.
                Self {
                    uri,
                    version,
                    source,
                    program: Program {
                        version: None,
                        subsets: vec![],
                        contracts: vec![],
                        modules: vec![],
                        components: vec![],
                        resources: vec![],
                        apps: vec![],
                        games: vec![],
                    },
                    tokens,
                    parse_error: Some(format_parse_error(&err)),
                }
            }
        }
    }

    pub fn offset_at_lsp(&self, line: u32, character: u32) -> Option<u32> {
        lsp_to_offset(&self.source, line, character)
    }

    pub fn lsp_at_offset(&self, offset: u32) -> (u32, u32) {
        offset_to_lsp(&self.source, offset as usize)
    }

    pub fn token_at(&self, offset: u32) -> Option<&SpannedToken> {
        self.tokens
            .iter()
            .find(|t| offset >= t.start && offset < t.end)
            .or_else(|| {
                // Allow hovering at the exact end of a token (common caret placement).
                self.tokens.iter().find(|t| offset == t.end && t.start < t.end)
            })
    }

    pub fn token_index_at(&self, offset: u32) -> Option<usize> {
        self.tokens
            .iter()
            .position(|t| offset >= t.start && offset < t.end)
            .or_else(|| {
                self.tokens
                    .iter()
                    .position(|t| offset == t.end && t.start < t.end)
            })
    }
}

fn format_parse_error(err: &ParseError) -> String {
    format!("{}:{}: {}", err.line, err.col, err.message)
}
