//! Source locations and type expressions.

/// Inclusive-start / exclusive-end UTF-8 byte span, plus 1-based line/col of the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn new(start: u32, end: u32, line: usize, col: usize) -> Self {
        Self {
            start,
            end,
            line,
            col,
        }
    }

    pub fn contains_offset(&self, offset: u32) -> bool {
        if self.start == self.end {
            offset == self.start
        } else {
            offset >= self.start && offset < self.end
        }
    }

    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub fn cover(a: Span, b: Span) -> Span {
        Span {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
            line: a.line,
            col: a.col,
        }
    }

    /// Convert to 0-based line/UTF-16 character range for LSP: (start_line, start_col, end_line, end_col).
    pub fn to_lsp_range(&self, source: &str) -> (u32, u32, u32, u32) {
        let (sl, sc) = offset_to_lsp(source, self.start as usize);
        let (el, ec) = offset_to_lsp(source, self.end as usize);
        (sl, sc, el, ec)
    }
}

/// Convert a UTF-8 byte offset into LSP (line, character) using UTF-16 code units.
pub fn offset_to_lsp(source: &str, offset: usize) -> (u32, u32) {
    let offset = offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_text = &prefix[line_start..];
    let character = line_text.encode_utf16().count() as u32;
    (line, character)
}

/// Convert an LSP (line, UTF-16 character) position into a UTF-8 byte offset.
pub fn lsp_to_offset(source: &str, line: u32, character: u32) -> Option<u32> {
    let mut byte_start = 0usize;
    let mut lineno = 0u32;
    let bytes = source.as_bytes();
    while lineno < line {
        match bytes[byte_start..].iter().position(|&b| b == b'\n') {
            Some(rel) => {
                byte_start += rel + 1;
                lineno += 1;
            }
            None => return None,
        }
    }

    let line_end = bytes[byte_start..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|rel| byte_start + rel)
        .unwrap_or(source.len());
    let line_text = &source[byte_start..line_end];

    let mut utf16 = 0u32;
    let mut byte = byte_start;
    if character == 0 {
        return Some(byte_start as u32);
    }
    for ch in line_text.chars() {
        let units = ch.len_utf16() as u32;
        if utf16 + units > character {
            return Some(byte as u32);
        }
        utf16 += units;
        byte += ch.len_utf8();
        if utf16 == character {
            return Some(byte as u32);
        }
    }
    Some(byte as u32)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    Named(String),
    /// Fixed-length vector `Vec[num32; 768]`.
    Vec {
        elem: String,
        len: Option<u64>,
    },
    /// Open array `[Product]` / `[Str]`.
    Array(Box<TypeExpr>),
    Optional(Box<TypeExpr>),
}

impl TypeExpr {
    pub fn name(&self) -> &str {
        match self {
            TypeExpr::Named(n) => n,
            TypeExpr::Vec { elem, .. } => elem,
            TypeExpr::Array(inner) => inner.name(),
            TypeExpr::Optional(inner) => inner.name(),
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, TypeExpr::Array(_))
    }

    pub fn elem_type(&self) -> Option<&TypeExpr> {
        match self {
            TypeExpr::Array(inner) | TypeExpr::Optional(inner) => Some(inner),
            _ => None,
        }
    }

    pub fn display(&self) -> String {
        match self {
            TypeExpr::Named(n) => n.clone(),
            TypeExpr::Vec { elem, len } => match len {
                Some(n) => format!("Vec[{elem}; {n}]"),
                None => format!("Vec[{elem}]"),
            },
            TypeExpr::Array(inner) => format!("[{}]", inner.display()),
            TypeExpr::Optional(inner) => format!("{}?", inner.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_roundtrip_ascii() {
        let src = "abc\ndef";
        assert_eq!(lsp_to_offset(src, 0, 0), Some(0));
        assert_eq!(lsp_to_offset(src, 0, 2), Some(2));
        assert_eq!(lsp_to_offset(src, 1, 0), Some(4));
        assert_eq!(lsp_to_offset(src, 1, 2), Some(6));
        assert_eq!(offset_to_lsp(src, 4), (1, 0));
    }

    #[test]
    fn lsp_handles_utf16_surrogate_pair() {
        // 😀 is one Unicode scalar, two UTF-16 code units.
        let src = "a😀b";
        assert_eq!(lsp_to_offset(src, 0, 0), Some(0));
        assert_eq!(lsp_to_offset(src, 0, 1), Some(1));
        // character 3 is after the emoji (2 UTF-16 units)
        assert_eq!(lsp_to_offset(src, 0, 3), Some("a😀".len() as u32));
        assert_eq!(offset_to_lsp(src, "a😀".len()), (0, 3));
    }

    #[test]
    fn span_contains_offset() {
        let s = Span::new(10, 15, 1, 11);
        assert!(s.contains_offset(10));
        assert!(s.contains_offset(14));
        assert!(!s.contains_offset(15));
    }
}
