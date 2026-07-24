//! Silc lexer for the Raku-inspired surface (ADR-002).
//! Accepts tokens from `.silc` and `.raku` entry files.

use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\f]+")]
#[logos(skip r"#[^\n]*")]
pub enum Token {
    #[token("\n")]
    Newline,

    #[token("subset")]
    Subset,
    #[token("class")]
    Class,
    #[token("has")]
    Has,
    #[token("method")]
    Method,
    #[token("is")]
    Is,
    #[token("of")]
    Of,
    #[token("where")]
    Where,

    #[token("==>")]
    Feed,
    #[token("::")]
    DoubleColon,
    #[token("->")]
    Arrow,

    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("<")]
    LAngle,
    #[token(">")]
    RAngle,
    #[token(",")]
    Comma,
    #[token(";")]
    Semi,
    #[token(":")]
    Colon,
    #[token("=")]
    Eq,
    #[token(".")]
    Dot,
    #[token("$")]
    Dollar,

    #[regex(r"@[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().trim_start_matches('@').to_string())]
    Annotation(String),

    #[regex(r"[0-9]+(ms|s|MB|GB|rps|ops)", |lex| lex.slice().to_string())]
    UnitLiteral(String),

    #[regex(r"[0-9]+", |lex| lex.slice().to_string())]
    Number(String),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice().to_string())]
    StringLit(String),

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub line: usize,
    pub col: usize,
    pub slice: String,
}

pub fn lex(source: &str) -> Result<Vec<SpannedToken>, String> {
    let mut tokens = Vec::new();
    let mut lexer = Token::lexer(source);

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        let slice = lexer.slice().to_string();
        // Update line from source prefix
        let prefix = &source[..span.start];
        let line = prefix.chars().filter(|c| *c == '\n').count() + 1;
        let last_newline = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = span.start - last_newline + 1;

        match result {
            Ok(Token::Newline) => continue,
            Ok(token) => tokens.push(SpannedToken {
                token,
                line,
                col,
                slice,
            }),
            Err(()) => {
                return Err(format!("lexer error at {}:{} near `{}`", line, col, slice));
            }
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn examples_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
    }

    #[test]
    fn lexes_article_pipeline() {
        let path = examples_dir().join("article_pipeline.silc");
        let src = fs::read_to_string(&path).expect("read example");
        let tokens = lex(&src).expect("lex");
        assert!(tokens.iter().any(|t| matches!(t.token, Token::Class)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::Feed)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::Subset)));
    }

    #[test]
    fn lexes_all_examples() {
        for name in [
            "article_pipeline.silc",
            "sensor_alert.silc",
            "csv_summary.raku",
            "url_health.silc",
            "log_anomaly.raku",
        ] {
            let path = examples_dir().join(name);
            let src = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {name}"));
            lex(&src).unwrap_or_else(|e| panic!("lex {name}: {e}"));
        }
    }
}
