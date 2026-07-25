//! Recursive-descent parser for the first-pass Silc grammar.

use sil_core::{
    Contract, Field, Method, Module, ModuleKind, Param, Pipeline, PipelineStep, Program, Span,
    Subset, TraitArg, TypeExpr, UiNode, UiValue, UiView,
};
use sil_lexer::{lex, SpannedToken, Token};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(source: &str) -> Result<Program, ParseError> {
    let tokens = lex(source).map_err(|message| ParseError {
        message,
        line: 1,
        col: 1,
    })?;
    Parser::new(tokens).parse_program()
}

struct ClassAst {
    contract: Option<Contract>,
    module: Option<Module>,
    view: Option<UiView>,
}

struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<SpannedToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_program(mut self) -> Result<Program, ParseError> {
        let mut program = Program::default();

        if matches!(self.peek(), Some(Token::Annotation(name)) if name == "version") {
            self.advance();
            self.expect_simple(Token::LParen, "`(` after @version")?;
            let version = match self.advance_token()? {
                Token::StringLit(value) => value.trim_matches('"').to_string(),
                _ => return Err(self.error_here("expected version string")),
            };
            self.expect_simple(Token::RParen, "`)` after version")?;
            program.version = Some(version);
        }

        while self.peek().is_some() {
            match self.peek() {
                Some(Token::Subset) => program.subsets.push(self.parse_subset()?),
                Some(Token::Class) => {
                    let class = self.parse_class()?;
                    if let Some(contract) = class.contract {
                        program.contracts.push(contract);
                    }
                    if let Some(module) = class.module {
                        program.modules.push(module);
                    }
                    if let Some(view) = class.view {
                        program.views.push(view);
                    }
                }
                _ => {
                    return Err(self.error_here(
                        "unsupported construct; expected `subset` or `class` in Silc grammar",
                    ))
                }
            }
        }
        Ok(program)
    }

    fn parse_subset(&mut self) -> Result<Subset, ParseError> {
        let start = self.current_span();
        self.expect_simple(Token::Subset, "`subset`")?;
        let name = self.expect_ident("subset name")?;
        self.expect_simple(Token::Of, "`of`")?;
        let base = self.parse_type()?;
        let predicate = if matches!(self.peek(), Some(Token::Where)) {
            self.advance();
            self.expect_simple(Token::LBrace, "`{` after where")?;
            Some(self.collect_balanced_brace_text()?)
        } else {
            None
        };
        if matches!(self.peek(), Some(Token::Semi)) {
            self.advance();
        }
        Ok(Subset {
            name,
            base,
            predicate,
            span: start,
        })
    }

    fn parse_class(&mut self) -> Result<ClassAst, ParseError> {
        let start = self.current_span();
        self.expect_simple(Token::Class, "`class`")?;
        let name = self.expect_ident("class name")?;
        let mut traits = Vec::new();
        let mut kind = ModuleKind::Unknown;
        let mut is_view = false;

        while matches!(self.peek(), Some(Token::Is)) {
            self.advance();
            let trait_name = self.expect_ident("trait name after `is`")?;
            let value = if matches!(self.peek(), Some(Token::LParen)) {
                self.advance();
                self.collect_until_matching_paren()?
            } else {
                String::new()
            };
            if trait_name == "view" {
                if !value.is_empty() {
                    return Err(self.error_here("`is view` does not take arguments"));
                }
                is_view = true;
                continue;
            }
            let parsed_kind = ModuleKind::parse(&trait_name);
            if parsed_kind != ModuleKind::Unknown {
                kind = parsed_kind;
            } else {
                traits.push(TraitArg {
                    name: trait_name,
                    value,
                });
            }
        }

        self.expect_simple(Token::LBrace, "`{` after class declaration")?;

        if is_view {
            if kind != ModuleKind::Unknown {
                return Err(ParseError {
                    message: format!("view `{name}` cannot also be a service/processor/sink"),
                    line: start.line,
                    col: start.col,
                });
            }
            if !traits.is_empty() {
                return Err(ParseError {
                    message: format!("view `{name}` does not accept constraint traits"),
                    line: start.line,
                    col: start.col,
                });
            }
            let root = self.parse_view_body(&name)?;
            self.expect_simple(Token::RBrace, "`}` after view class")?;
            return Ok(ClassAst {
                contract: None,
                module: None,
                view: Some(UiView {
                    name,
                    root,
                    span: start,
                }),
            });
        }

        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace)) {
            match self.peek() {
                Some(Token::Has) => fields.push(self.parse_field()?),
                Some(Token::Method) => methods.push(self.parse_method()?),
                None => return Err(self.error_here("unterminated class body")),
                _ => return Err(self.error_here("expected `has`, `method`, or `}`")),
            }
        }
        self.advance();

        if kind == ModuleKind::Unknown {
            Ok(ClassAst {
                contract: Some(Contract {
                    name,
                    fields,
                    span: start,
                }),
                module: None,
                view: None,
            })
        } else {
            Ok(ClassAst {
                contract: None,
                module: Some(Module {
                    name,
                    kind,
                    traits,
                    fields,
                    methods,
                    span: start,
                }),
                view: None,
            })
        }
    }

    fn parse_field(&mut self) -> Result<Field, ParseError> {
        self.expect_simple(Token::Has, "`has`")?;
        let ty = self.parse_type()?;
        self.expect_simple(Token::Dollar, "`$` in attribute")?;
        self.expect_simple(Token::Dot, "`.` in attribute")?;
        let name = self.expect_ident("attribute name")?;
        let default = if matches!(self.peek(), Some(Token::Eq)) {
            self.advance();
            Some(self.collect_until_semi()?)
        } else {
            self.expect_simple(Token::Semi, "`;` after attribute")?;
            None
        };
        Ok(Field { name, ty, default })
    }

    fn parse_method(&mut self) -> Result<Method, ParseError> {
        self.expect_simple(Token::Method, "`method`")?;
        let name = self.expect_ident("method name")?;
        self.expect_simple(Token::LParen, "`(` after method name")?;
        let param_tokens = self.take_balanced_paren_tokens()?;
        let params = parse_params(&param_tokens);
        self.expect_simple(Token::LBrace, "`{` before method body")?;

        let mut segments: Vec<Vec<SpannedToken>> = vec![Vec::new()];
        let mut paren_depth = 0usize;
        while let Some(token) = self.peek() {
            match token {
                Token::RBrace if paren_depth == 0 => break,
                Token::Feed if paren_depth == 0 => {
                    self.advance();
                    segments.push(Vec::new());
                }
                Token::LParen => {
                    paren_depth += 1;
                    segments.last_mut().unwrap().push(self.advance().unwrap());
                }
                Token::RParen => {
                    paren_depth = paren_depth.saturating_sub(1);
                    segments.last_mut().unwrap().push(self.advance().unwrap());
                }
                _ => segments.last_mut().unwrap().push(self.advance().unwrap()),
            }
        }
        self.expect_simple(Token::RBrace, "`}` after method body")?;

        let steps = segments
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(|s| parse_step(&s))
            .collect();
        Ok(Method {
            name,
            params,
            pipeline: Pipeline { steps },
        })
    }

    /// View classes contain exactly one `method render() { ui::... }` body.
    fn parse_view_body(&mut self, view_name: &str) -> Result<UiNode, ParseError> {
        if matches!(self.peek(), Some(Token::Has)) {
            return Err(self.error_here(&format!("view `{view_name}` cannot declare `has` fields")));
        }
        self.expect_simple(Token::Method, "`method`")?;
        let method_name = self.expect_ident("method name")?;
        if method_name != "render" {
            return Err(self.error_here(&format!(
                "view `{view_name}` must declare `method render()` (found `{method_name}`)"
            )));
        }
        self.expect_simple(Token::LParen, "`(` after method name")?;
        let param_tokens = self.take_balanced_paren_tokens()?;
        if !param_tokens.is_empty() {
            return Err(self.error_here("view `render()` takes no parameters"));
        }
        self.expect_simple(Token::LBrace, "`{` before method body")?;
        let root = self.parse_ui_node()?;
        if matches!(self.peek(), Some(Token::Semi)) {
            self.advance();
        }
        self.expect_simple(Token::RBrace, "`}` after view render body")?;
        if !matches!(self.peek(), Some(Token::RBrace)) {
            return Err(self.error_here(&format!(
                "view `{view_name}` must declare exactly one `method render()`"
            )));
        }
        Ok(root)
    }

    fn parse_ui_node(&mut self) -> Result<UiNode, ParseError> {
        let start = self.current_span();
        let ns = self.expect_ident("ui namespace")?;
        if ns != "ui" {
            return Err(self.error_here("UI components must use the `ui::` namespace"));
        }
        self.expect_simple(Token::DoubleColon, "`::` in ui component")?;
        let component = self.expect_ident("ui component name")?;
        self.expect_simple(Token::LParen, "`(` after ui component")?;

        let mut props = Vec::new();
        let mut slots = Vec::new();
        let mut children = Vec::new();

        while !matches!(self.peek(), Some(Token::RParen)) {
            if matches!(self.peek(), Some(Token::Colon)) {
                self.advance();
                let key = self.expect_ident("prop or slot name")?;
                if matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    if matches!(self.peek(), Some(Token::Ident(name)) if name == "ui")
                        && matches!(
                            self.tokens.get(self.pos + 1).map(|t| &t.token),
                            Some(Token::DoubleColon)
                        )
                    {
                        let node = self.parse_ui_node()?;
                        self.expect_simple(Token::RParen, "`)` after slot")?;
                        slots.push((key, node));
                    } else if matches!(self.peek(), Some(Token::RParen)) {
                        self.advance();
                        props.push((key, UiValue::Flag));
                    } else {
                        let value = self.parse_ui_value()?;
                        self.expect_simple(Token::RParen, "`)` after prop value")?;
                        props.push((key, value));
                    }
                } else if matches!(self.peek(), Some(Token::Comma) | Some(Token::RParen) | None) {
                    props.push((key, UiValue::Flag));
                } else {
                    return Err(
                        self.error_here(&format!("expected `:`{key}`(...)` or bare `:{key}` flag"))
                    );
                }
            } else if matches!(self.peek(), Some(Token::Ident(name)) if name == "ui") {
                children.push(self.parse_ui_node()?);
            } else {
                return Err(self.error_here(
                    "expected named prop/slot (`:name(...)`) or child `ui::component(...)`",
                ));
            }
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
            }
        }
        self.expect_simple(Token::RParen, "`)` after ui component args")?;
        Ok(UiNode {
            component,
            props,
            slots,
            children,
            span: start,
        })
    }

    fn parse_ui_value(&mut self) -> Result<UiValue, ParseError> {
        match self.peek() {
            Some(Token::StringLit(_)) => {
                let Token::StringLit(raw) = self.advance_token()? else {
                    unreachable!()
                };
                Ok(UiValue::String(raw.trim_matches('"').to_string()))
            }
            Some(Token::Ident(_)) => {
                let name = self.expect_ident("identifier value")?;
                match name.as_str() {
                    "True" | "true" => Ok(UiValue::Bool(true)),
                    "False" | "false" => Ok(UiValue::Bool(false)),
                    _ => Ok(UiValue::Ident(name)),
                }
            }
            Some(Token::LBracket) => {
                self.advance();
                let mut items = Vec::new();
                while !matches!(self.peek(), Some(Token::RBracket)) {
                    match self.advance_token()? {
                        Token::StringLit(raw) => {
                            items.push(raw.trim_matches('"').to_string());
                        }
                        _ => {
                            return Err(
                                self.error_here("UI string lists may only contain string literals")
                            )
                        }
                    }
                    if matches!(self.peek(), Some(Token::Comma)) {
                        self.advance();
                    }
                }
                self.expect_simple(Token::RBracket, "`]` after options list")?;
                Ok(UiValue::StringList(items))
            }
            Some(Token::Number(_)) => match self.advance_token()? {
                Token::Number(n) => Ok(UiValue::Ident(n)),
                _ => unreachable!(),
            },
            _ => Err(self.error_here("unsupported UI prop value")),
        }
    }

    fn parse_type(&mut self) -> Result<TypeExpr, ParseError> {
        let name = self.expect_ident("type")?;
        if matches!(self.peek(), Some(Token::LBracket)) {
            self.advance();
            let elem = self.expect_ident("vector element type")?;
            self.expect_simple(Token::Semi, "`;` in vector type")?;
            let len = match self.advance_token()? {
                Token::Number(n) => n.parse().ok(),
                _ => return Err(self.error_here("expected vector length")),
            };
            self.expect_simple(Token::RBracket, "`]` after vector type")?;
            Ok(TypeExpr::Vec { elem, len })
        } else {
            Ok(TypeExpr::Named(name))
        }
    }

    fn collect_balanced_brace_text(&mut self) -> Result<String, ParseError> {
        let mut depth = 1usize;
        let mut parts = Vec::new();
        while let Some(tok) = self.advance() {
            match tok.token {
                Token::LBrace => depth += 1,
                Token::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(parts.join(" "));
                    }
                }
                _ => {}
            }
            parts.push(tok.slice);
        }
        Err(self.error_here("unterminated where block"))
    }

    fn collect_until_matching_paren(&mut self) -> Result<String, ParseError> {
        let tokens = self.take_balanced_paren_tokens()?;
        Ok(tokens
            .iter()
            .map(|t| t.slice.as_str())
            .collect::<Vec<_>>()
            .join(""))
    }

    fn take_balanced_paren_tokens(&mut self) -> Result<Vec<SpannedToken>, ParseError> {
        let mut depth = 1usize;
        let mut out = Vec::new();
        while let Some(tok) = self.advance() {
            match tok.token {
                Token::LParen => depth += 1,
                Token::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(out);
                    }
                }
                _ => {}
            }
            out.push(tok);
        }
        Err(self.error_here("unterminated parentheses"))
    }

    fn collect_until_semi(&mut self) -> Result<String, ParseError> {
        let mut parts = Vec::new();
        while let Some(tok) = self.advance() {
            if matches!(tok.token, Token::Semi) {
                return Ok(parts.join(""));
            }
            parts.push(tok.slice);
        }
        Err(self.error_here("expected `;`"))
    }

    fn expect_ident(&mut self, what: &str) -> Result<String, ParseError> {
        match self.advance_token()? {
            Token::Ident(value) => Ok(value),
            _ => Err(self.error_here(&format!("expected {what}"))),
        }
    }

    fn expect_simple(&mut self, expected: Token, label: &str) -> Result<(), ParseError> {
        let actual = self.advance_token()?;
        if std::mem::discriminant(&actual) == std::mem::discriminant(&expected) {
            Ok(())
        } else {
            Err(self.error_here(&format!("expected {label}")))
        }
    }

    fn advance_token(&mut self) -> Result<Token, ParseError> {
        self.advance()
            .map(|t| t.token)
            .ok_or_else(|| self.error_here("unexpected end of file"))
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.token)
    }

    fn advance(&mut self) -> Option<SpannedToken> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| Span {
                line: t.line,
                col: t.col,
            })
            .unwrap_or_default()
    }

    fn error_here(&self, message: &str) -> ParseError {
        let span = self.current_span();
        ParseError {
            message: message.into(),
            line: span.line,
            col: span.col,
        }
    }
}

fn parse_params(tokens: &[SpannedToken]) -> Vec<Param> {
    split_top_level(tokens, TokenKind::Comma)
        .into_iter()
        .filter(|chunk| !chunk.is_empty())
        .filter_map(|chunk| {
            let slices: Vec<&str> = chunk.iter().map(|t| t.slice.as_str()).collect();
            if slices.first() == Some(&":") {
                let name = slices
                    .iter()
                    .position(|s| *s == "$")
                    .and_then(|i| slices.get(i + 1))
                    .copied()?;
                let default = slices
                    .iter()
                    .position(|s| *s == "=")
                    .map(|i| slices[i + 1..].join(""));
                Some(Param {
                    name: name.into(),
                    ty: None,
                    named: true,
                    default,
                })
            } else {
                let dollar = slices.iter().position(|s| *s == "$")?;
                Some(Param {
                    name: slices.get(dollar + 1)?.to_string(),
                    ty: slices.first().map(|s| TypeExpr::Named((*s).into())),
                    named: false,
                    default: None,
                })
            }
        })
        .collect()
}

fn parse_step(tokens: &[SpannedToken]) -> PipelineStep {
    let slices: Vec<&str> = tokens.iter().map(|t| t.slice.as_str()).collect();
    if let Some(double_colon) = slices.iter().position(|s| *s == "::") {
        let namespace = slices
            .get(double_colon.wrapping_sub(1))
            .unwrap_or(&"")
            .to_string();
        let name = slices
            .get(double_colon + 1)
            .unwrap_or(&"unknown")
            .to_string();
        let args = parse_call_args(tokens);
        PipelineStep::Call {
            namespace: Some(namespace),
            name,
            args,
        }
    } else if slices.first() == Some(&"$") {
        let base = slices.get(1).unwrap_or(&"value").to_string();
        if slices.get(2) == Some(&".") {
            PipelineStep::FieldAccess {
                base,
                field: slices.get(3).unwrap_or(&"field").to_string(),
            }
        } else {
            PipelineStep::Name(base)
        }
    } else {
        PipelineStep::Name(slices.join(""))
    }
}

fn parse_call_args(tokens: &[SpannedToken]) -> Vec<TraitArg> {
    let slices: Vec<&str> = tokens.iter().map(|t| t.slice.as_str()).collect();
    let mut args = Vec::new();
    let mut i = 0;
    while i < slices.len() {
        if slices[i] == ":" && i + 1 < slices.len() {
            let name = slices[i + 1].to_string();
            i += 2;
            let mut value = String::new();
            if i < slices.len() && (slices[i] == "(" || slices[i] == "<") {
                let close = if slices[i] == "(" { ")" } else { ">" };
                i += 1;
                while i < slices.len() && slices[i] != close {
                    value.push_str(slices[i]);
                    i += 1;
                }
            }
            args.push(TraitArg { name, value });
        }
        i += 1;
    }
    args
}

#[derive(Clone, Copy)]
enum TokenKind {
    Comma,
}

fn split_top_level(tokens: &[SpannedToken], kind: TokenKind) -> Vec<Vec<SpannedToken>> {
    let mut out = vec![Vec::new()];
    let mut depth = 0usize;
    for token in tokens {
        match token.token {
            Token::LParen | Token::LBracket | Token::LAngle => depth += 1,
            Token::RParen | Token::RBracket | Token::RAngle => depth = depth.saturating_sub(1),
            _ => {}
        }
        let split = depth == 0 && matches!((&token.token, kind), (Token::Comma, TokenKind::Comma));
        if split {
            out.push(Vec::new());
        } else {
            out.last_mut().unwrap().push(token.clone());
        }
    }
    out
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
    fn parses_all_examples() {
        for name in [
            "article_pipeline.silc",
            "sensor_alert.silc",
            "csv_summary.raku",
            "url_health.silc",
            "log_anomaly.raku",
        ] {
            let src = fs::read_to_string(examples_dir().join(name)).unwrap();
            let program = parse(&src).unwrap_or_else(|e| panic!("parse {name}: {e}"));
            assert_eq!(program.modules.len(), 3, "{name}");
            assert_eq!(program.contracts.len(), 1, "{name}");
            program.validate().unwrap();
        }
    }

    #[test]
    fn parses_declarative_ui_view() {
        let src = r#"
@version("1.0")
class FeedbackRecord {
    has Str $.author;
    has Str $.text;
    has Str $.rating;
}
class FeedbackView is view {
    method render() {
        ui::page(
            :app_bar(ui::app_bar(:title("Feedback"))),
            :side_panel(ui::side_panel(
                ui::nav_item(:label("Inbox"), :active)
            )),
            ui::form(
                ui::stack(
                    ui::text_input(:field(author), :label("Author")),
                    ui::textarea(:field(text), :label("Feedback")),
                    ui::radio_group(:field(rating), :options(["Good", "Okay", "Bad"])),
                    ui::toolbar(
                        ui::button(:label("Submit"), :variant(primary), :submit)
                    )
                )
            )
        )
    }
}
class WebPortal is service {
    method listen(:$port = 18088) {
        FeedbackRecord ==> ui::web(:view(FeedbackView), :port(18088), :route("/"))
    }
}
class TextAnalyzer is processor {
    method analyze(FeedbackRecord $record) { $record.text ==> text::score() }
}
class FeedbackDb is sink is latency(5ms) is storage(SQLite) {
    method persist(FeedbackRecord $record) {
        $record ==> ipc::publish() ==> store::sqlite(:table(feedback)) ==> store::commit()
    }
}
"#;
        let program = parse(src).expect("parse view program");
        assert_eq!(program.views.len(), 1);
        assert_eq!(program.views[0].name, "FeedbackView");
        assert_eq!(program.views[0].root.component, "page");
        assert_eq!(program.views[0].root.slots.len(), 2);
        assert!(program.views[0].root.has_submit_button());
        program.validate().expect("validate view program");
        let graph = sil_core::infer_graph(&program).unwrap().unwrap();
        assert_eq!(graph.ui_view.as_ref().unwrap().name, "FeedbackView");
        assert_eq!(graph.ui_contract.as_deref(), Some("FeedbackRecord"));
    }

    #[test]
    fn rejects_unknown_ui_component() {
        let src = r#"
class BadView is view {
    method render() {
        ui::page(ui::form(ui::magic_widget(:label("Nope"))))
    }
}
"#;
        let program = parse(src).expect("parse");
        let err = program.validate().unwrap_err();
        assert!(err.contains("unknown UI component"), "{err}");
    }
}
