//! Silc lexer for the independent intent surface (ADR-002 / 0.4.0).

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
    #[token("contract")]
    Contract,
    #[token("component")]
    Component,
    #[token("resource")]
    Resource,
    #[token("app")]
    App,
    #[token("service")]
    Service,
    #[token("processor")]
    Processor,
    #[token("sink")]
    Sink,
    #[token("task")]
    Task,
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
    #[token("query")]
    Query,
    #[token("mutation")]
    Mutation,
    #[token("seed")]
    Seed,
    #[token("slot")]
    Slot,
    #[token("emit")]
    Emit,
    #[token("state")]
    State,
    #[token("when")]
    When,
    #[token("for")]
    For,
    #[token("else")]
    Else,
    #[token("route")]
    Route,
    #[token("await")]
    Await,

    #[token("==>")]
    Feed,
    #[token("=>")]
    FatArrow,
    #[token("::")]
    DoubleColon,
    #[token("->")]
    Arrow,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("!")]
    Bang,

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

    #[regex(r"[0-9]+(\.[0-9]+)?", |lex| lex.slice().to_string())]
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

    #[test]
    fn lexes_component_keywords() {
        let tokens = lex("component X { has state Str $.q; }").expect("lex");
        assert!(tokens.iter().any(|t| matches!(t.token, Token::State)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::Component)));
    }

    #[test]
    fn lexes_fat_arrow_and_ops() {
        let tokens = lex(r#"route "/" => ShopPage; $.a == 1 && $.b != 2"#).expect("lex");
        assert!(tokens.iter().any(|t| matches!(t.token, Token::FatArrow)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::EqEq)));
        assert!(tokens.iter().any(|t| matches!(t.token, Token::AndAnd)));
    }
}
