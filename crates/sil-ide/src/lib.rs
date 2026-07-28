//! Semantic IDE helpers for Silc: document model and hover resolution.

mod docs;
mod document;
mod resolve;

pub use docs::{
    builtin_type_doc, keyword_doc, operator_doc, BUILTIN_TYPE_NAMES, KEYWORD_NAMES,
};
pub use document::{Document, HoverContent, HoverRange};
pub use resolve::resolve_hover;

use sil_core::lsp_to_offset;

/// Resolve hover at an LSP (line, character) position.
pub fn hover_at_lsp(doc: &Document, line: u32, character: u32) -> Option<HoverContent> {
    let offset = lsp_to_offset(&doc.source, line, character)?;
    resolve_hover(doc, offset)
}
