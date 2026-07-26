//! Contract subject: schemas from `class` / `has` / `subset`.

use crate::types::{Span, TypeExpr};

/// Closed v1 `where` predicates for `subset` (ADR-002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubsetPredicate {
    Contains(String),
    StartsWith(String),
    EndsWith(String),
}

impl SubsetPredicate {
    pub const HELP: &'static str =
        "v1 subset predicates (Str base): `.contains(\"lit\")`, `.starts-with(\"lit\")`, `.ends-with(\"lit\")`";

    /// Parse a brace body such as `.contains("://")`.
    /// Whitespace between tokens is ignored (parser brace collection may insert spaces).
    pub fn parse(body: &str) -> Result<Self, String> {
        let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
        let (kind, rest) = if let Some(r) = compact.strip_prefix(".contains") {
            ("contains", r)
        } else if let Some(r) = compact.strip_prefix(".starts-with") {
            ("starts-with", r)
        } else if let Some(r) = compact.strip_prefix(".ends-with") {
            ("ends-with", r)
        } else {
            return Err(format!(
                "unsupported subset where predicate `{}`; {}",
                body.trim(),
                Self::HELP
            ));
        };
        let lit = parse_paren_string_literal(rest).map_err(|e| {
            format!("invalid `{kind}` predicate: {e}; {}", Self::HELP)
        })?;
        Ok(match kind {
            "contains" => Self::Contains(lit),
            "starts-with" => Self::StartsWith(lit),
            "ends-with" => Self::EndsWith(lit),
            _ => unreachable!(),
        })
    }

    pub fn check_str(&self, value: &str) -> bool {
        match self {
            Self::Contains(lit) => value.contains(lit),
            Self::StartsWith(lit) => value.starts_with(lit),
            Self::EndsWith(lit) => value.ends_with(lit),
        }
    }

    /// Emit a JS expression `valueExpr` that evaluates to boolean.
    pub fn to_js_check(&self, value_expr: &str) -> String {
        match self {
            Self::Contains(lit) => {
                format!("{value_expr}.includes({})", js_string_literal(lit))
            }
            Self::StartsWith(lit) => {
                format!("{value_expr}.startsWith({})", js_string_literal(lit))
            }
            Self::EndsWith(lit) => {
                format!("{value_expr}.endsWith({})", js_string_literal(lit))
            }
        }
    }

    /// Emit a Go expression over `valueExpr` (string) that evaluates to bool.
    pub fn to_go_check(&self, value_expr: &str) -> String {
        match self {
            Self::Contains(lit) => {
                format!("strings.Contains({value_expr}, {})", go_string_literal(lit))
            }
            Self::StartsWith(lit) => {
                format!(
                    "strings.HasPrefix({value_expr}, {})",
                    go_string_literal(lit)
                )
            }
            Self::EndsWith(lit) => {
                format!(
                    "strings.HasSuffix({value_expr}, {})",
                    go_string_literal(lit)
                )
            }
        }
    }

    pub fn requires_strings_import(&self) -> bool {
        true
    }
}

fn parse_paren_string_literal(rest: &str) -> Result<String, String> {
    let rest = rest.trim();
    let inner = rest
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| "expected `(\"…\")`".to_string())?
        .trim();
    if (inner.starts_with('"') && inner.ends_with('"'))
        || (inner.starts_with('\'') && inner.ends_with('\''))
    {
        let quote = inner.chars().next().unwrap();
        let content = &inner[1..inner.len() - 1];
        if content.contains(quote) {
            return Err("escaped quotes are not supported in v1 predicates".into());
        }
        Ok(content.to_string())
    } else {
        Err("expected a string literal".into())
    }
}

fn js_string_literal(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn go_string_literal(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subset {
    pub name: String,
    pub base: TypeExpr,
    pub predicate: Option<SubsetPredicate>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
    pub default: Option<String>,
    pub is_state: bool,
}

impl Field {
    pub fn new(name: impl Into<String>, ty: TypeExpr) -> Self {
        Self {
            name: name.into(),
            ty,
            default: None,
            is_state: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    pub name: String,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_contains() {
        let p = SubsetPredicate::parse(r#".contains("://")"#).unwrap();
        assert_eq!(p, SubsetPredicate::Contains("://".into()));
        assert!(p.check_str("https://example.com"));
        assert!(!p.check_str("example.com"));
    }

    #[test]
    fn parses_starts_and_ends() {
        assert_eq!(
            SubsetPredicate::parse(r#".starts-with("https://")"#).unwrap(),
            SubsetPredicate::StartsWith("https://".into())
        );
        assert_eq!(
            SubsetPredicate::parse(r#".ends-with(".com")"#).unwrap(),
            SubsetPredicate::EndsWith(".com".into())
        );
    }

    #[test]
    fn rejects_unknown_predicate() {
        let err = SubsetPredicate::parse(".len > 0").unwrap_err();
        assert!(err.contains("unsupported"));
    }
}
