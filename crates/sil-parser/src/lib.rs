//! Recursive-descent parser for Silc 0.4.0 grammar.

use sil_core::{
    App, CompField, Component, Contract, EmitDecl, EventBinding, Expr, Field, Game, GameNode,
    Handler, Method, Module, ModuleKind, Param, Pipeline, PipelineStep, Program, QueryBinding,
    Resource, ResourceKind, ResourceMethod, ResourceSeed, Route, SlotDecl, Span, Subset,
    SubsetPredicate, TraitArg, TypeExpr, UiNode, UiTemplate,
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
    Ok(parse_with_tokens(source)?.0)
}

/// Parse a Silc source file, returning both the program AST and the spanned token stream.
pub fn parse_with_tokens(source: &str) -> Result<(Program, Vec<SpannedToken>), ParseError> {
    let tokens = lex(source).map_err(|message| ParseError {
        message,
        line: 1,
        col: 1,
    })?;
    let program = Parser::new(tokens.clone()).parse_program()?;
    Ok((program, tokens))
}

enum ClassKind {
    Contract,
    Module(ModuleKind),
    Component,
    Resource,
    App,
    Game,
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
                Some(Token::Contract) => {
                    self.parse_subject_into(&mut program, ClassKind::Contract, "contract")?
                }
                Some(Token::Component) => {
                    self.parse_subject_into(&mut program, ClassKind::Component, "component")?
                }
                Some(Token::Resource) => {
                    self.parse_subject_into(&mut program, ClassKind::Resource, "resource")?
                }
                Some(Token::App) => {
                    self.parse_subject_into(&mut program, ClassKind::App, "app")?
                }
                Some(Token::Game) => {
                    self.parse_subject_into(&mut program, ClassKind::Game, "game")?
                }
                Some(Token::Service) => self.parse_subject_into(
                    &mut program,
                    ClassKind::Module(ModuleKind::Service),
                    "service",
                )?,
                Some(Token::Processor) => self.parse_subject_into(
                    &mut program,
                    ClassKind::Module(ModuleKind::Processor),
                    "processor",
                )?,
                Some(Token::Sink) => {
                    return Err(self.error_here(
                        "author `sink` declarations are not supported in Silc 0.4.0; remove the sink — persistence is synthesized from the processor",
                    ))
                }
                Some(Token::Task) => self.parse_subject_into(
                    &mut program,
                    ClassKind::Module(ModuleKind::Task),
                    "task",
                )?,
                Some(Token::Class) => return Err(self.legacy_class_error()),
                _ => {
                    return Err(self.error_here(
                        "unsupported construct; expected `subset`, `contract`, `component`, `resource`, `app`, `game`, `service`, `processor`, or `task`",
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
            let body = self.collect_balanced_brace_text()?;
            Some(SubsetPredicate::parse(&body).map_err(|message| ParseError {
                message,
                line: start.line,
                col: start.col,
            })?)
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
            span: self.finish_span(start),
        })
    }

    fn legacy_class_error(&self) -> ParseError {
        let name = self
            .tokens
            .get(self.pos + 1)
            .and_then(|token| match &token.token {
                Token::Ident(name) => Some(name.as_str()),
                _ => None,
            })
            .unwrap_or("Name");
        let kind = self
            .tokens
            .get(self.pos + 3)
            .and_then(|token| match &token.token {
                Token::Component => Some("component"),
                Token::Resource => Some("resource"),
                Token::App => Some("app"),
                Token::Service => Some("service"),
                Token::Processor => Some("processor"),
                Token::Sink => Some("sink"),
                Token::Task => Some("task"),
                Token::Ident(kind) => match kind.as_str() {
                    "component" | "resource" | "app" | "service" | "processor" | "sink"
                    | "task" => Some(kind.as_str()),
                    // Historical `is view` maps to components (removed in 0.2.0).
                    "view" => Some("component"),
                    _ => None,
                },
                _ => None,
            })
            .unwrap_or("contract");
        let replacement = format!("{kind} {name} {{ ... }}");
        self.error_here(&format!(
            "legacy `class` declarator is not supported in Silc 0.4.0; use `{replacement}`"
        ))
    }

    fn parse_subject_into(
        &mut self,
        program: &mut Program,
        kind: ClassKind,
        keyword: &str,
    ) -> Result<(), ParseError> {
        let start = self.current_span();
        self.advance();
        let name = self.expect_ident(&format!("{keyword} name"))?;
        let mut traits = Vec::new();
        let mut resource_contract = None;

        if matches!(kind, ClassKind::Resource) && matches!(self.peek(), Some(Token::For)) {
            self.advance();
            resource_contract = Some(self.expect_ident("contract name after `resource … for`")?);
        }

        while matches!(self.peek(), Some(Token::Is)) {
            self.advance();
            let trait_name = self.expect_ident("trait name after `is`")?;
            let value = if matches!(self.peek(), Some(Token::LParen)) {
                self.advance();
                self.collect_until_matching_paren()?
            } else {
                String::new()
            };
            if matches!(kind, ClassKind::Module(_)) {
                traits.push(TraitArg {
                    name: trait_name,
                    value,
                });
            } else {
                return Err(self.error_here(&format!(
                    "`{keyword}` does not accept execution trait `is {trait_name}`"
                )));
            }
        }

        self.expect_simple(Token::LBrace, &format!("`{{` after {keyword} declaration"))?;

        match kind {
            ClassKind::Component => {
                let mut component = self.parse_component_body(name, start)?;
                self.expect_simple(Token::RBrace, "`}` after component")?;
                component.span = self.finish_span(start);
                program.components.push(component);
            }
            ClassKind::Resource => {
                let mut resource = self.parse_resource_body(name, start, resource_contract)?;
                self.expect_simple(Token::RBrace, "`}` after resource")?;
                resource.span = self.finish_span(start);
                program.resources.push(resource);
            }
            ClassKind::App => {
                let mut app = self.parse_app_body(name, start)?;
                self.expect_simple(Token::RBrace, "`}` after app")?;
                app.span = self.finish_span(start);
                program.apps.push(app);
            }
            ClassKind::Game => {
                let root = self.parse_game_node()?;
                self.expect_simple(Token::RBrace, "`}` after game")?;
                program.games.push(Game {
                    name,
                    root,
                    span: self.finish_span(start),
                });
            }
            ClassKind::Contract => {
                let mut fields = Vec::new();
                while !matches!(self.peek(), Some(Token::RBrace)) {
                    match self.peek() {
                        Some(Token::Has) => fields.push(self.parse_contract_field()?),
                        None => return Err(self.error_here("unterminated contract body")),
                        _ => return Err(self.error_here("expected `has` or `}` in contract")),
                    }
                }
                self.advance();
                program.contracts.push(Contract {
                    name,
                    fields,
                    span: self.finish_span(start),
                });
            }
            ClassKind::Module(module_kind) => {
                let mut fields = Vec::new();
                let mut methods = Vec::new();
                while !matches!(self.peek(), Some(Token::RBrace)) {
                    match self.peek() {
                        Some(Token::Has) => fields.push(self.parse_contract_field()?),
                        Some(Token::Method) => methods.push(self.parse_pipeline_method()?),
                        None => return Err(self.error_here("unterminated module body")),
                        _ => return Err(self.error_here("expected `has`, `method`, or `}`")),
                    }
                }
                self.advance();
                program.modules.push(Module {
                    name,
                    kind: module_kind,
                    traits,
                    fields,
                    methods,
                    span: self.finish_span(start),
                });
            }
        }
        Ok(())
    }

    fn parse_contract_field(&mut self) -> Result<Field, ParseError> {
        self.expect_simple(Token::Has, "`has`")?;
        let is_state = if matches!(self.peek(), Some(Token::State)) {
            self.advance();
            true
        } else {
            false
        };
        let ty = self.parse_type()?;
        self.expect_simple(Token::Dollar, "`$` in attribute")?;
        self.expect_simple(Token::Dot, "`.` in attribute")?;
        let (name, name_span) = self.expect_ident_spanned("attribute name")?;
        let default = if matches!(self.peek(), Some(Token::Eq)) {
            self.advance();
            Some(self.collect_until_semi()?)
        } else {
            self.expect_simple(Token::Semi, "`;` after attribute")?;
            None
        };
        Ok(Field {
            name,
            ty,
            default,
            is_state,
            span: name_span,
        })
    }

    fn parse_comp_field(&mut self) -> Result<CompField, ParseError> {
        let start = self.current_span();
        self.expect_simple(Token::Has, "`has`")?;
        let is_state = if matches!(self.peek(), Some(Token::State)) {
            self.advance();
            true
        } else {
            false
        };
        let ty = self.parse_type()?;
        self.expect_simple(Token::Dollar, "`$` in attribute")?;
        self.expect_simple(Token::Dot, "`.` in attribute")?;
        let name = self.expect_ident("attribute name")?;
        let default = if matches!(self.peek(), Some(Token::Eq)) {
            self.advance();
            let expr = self.parse_expr()?;
            self.expect_simple(Token::Semi, "`;` after default")?;
            Some(expr)
        } else {
            self.expect_simple(Token::Semi, "`;` after attribute")?;
            None
        };
        Ok(CompField {
            name,
            ty,
            default,
            is_state,
            span: start,
        })
    }

    fn parse_component_body(&mut self, name: String, start: Span) -> Result<Component, ParseError> {
        let mut props = Vec::new();
        let mut state = Vec::new();
        let mut slots = Vec::new();
        let mut emits = Vec::new();
        let mut queries = Vec::new();
        let mut handlers = Vec::new();
        let mut methods = Vec::new();
        let mut render: Option<UiTemplate> = None;

        while !matches!(self.peek(), Some(Token::RBrace)) {
            match self.peek() {
                Some(Token::Has) => {
                    let field = self.parse_comp_field()?;
                    if field.is_state {
                        state.push(field);
                    } else {
                        props.push(field);
                    }
                }
                Some(Token::Slot) => {
                    let span = self.current_span();
                    self.advance();
                    let slot_name = self.expect_ident("slot name")?;
                    self.expect_simple(Token::Semi, "`;` after slot")?;
                    slots.push(SlotDecl {
                        name: slot_name,
                        required: false,
                        span,
                    });
                }
                Some(Token::Emit) => {
                    let span = self.current_span();
                    self.advance();
                    let event = self.expect_ident("emit name")?;
                    let payload = if matches!(self.peek(), Some(Token::LParen)) {
                        self.advance();
                        let ty = self.expect_ident("emit payload type")?;
                        self.expect_simple(Token::RParen, "`)` after emit payload")?;
                        Some(ty)
                    } else {
                        None
                    };
                    self.expect_simple(Token::Semi, "`;` after emit")?;
                    emits.push(EmitDecl {
                        name: event,
                        payload,
                        span,
                    });
                }
                Some(Token::Query) => {
                    // `query $.name = Resource.method(args);`
                    let span = self.current_span();
                    self.advance();
                    self.expect_simple(Token::Dollar, "`$` in query binding")?;
                    self.expect_simple(Token::Dot, "`.` in query binding")?;
                    let (qname, name_span) = self.expect_ident_spanned("query name")?;
                    self.expect_simple(Token::Eq, "`=` in query binding")?;
                    let (resource, resource_span) = self.expect_ident_spanned("resource name")?;
                    self.expect_simple(Token::Dot, "`.` before resource method")?;
                    let (method, method_span) = self.expect_ident_spanned("resource method")?;
                    let mut args = Vec::new();
                    if matches!(self.peek(), Some(Token::LParen)) {
                        self.advance();
                        if !matches!(self.peek(), Some(Token::RParen)) {
                            loop {
                                args.push(self.parse_expr()?);
                                if matches!(self.peek(), Some(Token::Comma)) {
                                    self.advance();
                                    continue;
                                }
                                break;
                            }
                        }
                        self.expect_simple(Token::RParen, "`)` after query args")?;
                    }
                    self.expect_simple(Token::Semi, "`;` after query")?;
                    queries.push(QueryBinding {
                        name: qname,
                        resource,
                        method,
                        args,
                        span,
                        name_span,
                        resource_span,
                        method_span,
                    });
                }
                Some(Token::Method) => {
                    let method_name_peek = self.peek_ahead_ident(1);
                    if method_name_peek.as_deref() == Some("render") {
                        let tmpl = self.parse_render_method()?;
                        render = Some(tmpl);
                    } else {
                        // Peek for pipeline (`==>`) vs expression-body handler.
                        let saved = self.pos;
                        // Skip method name (params) {
                        let _ = self.advance(); // method
                        let _ = self.advance(); // name
                        if matches!(self.peek(), Some(Token::LParen)) {
                            self.advance();
                            let _ = self.take_balanced_paren_tokens();
                        }
                        if matches!(self.peek(), Some(Token::LBrace)) {
                            self.advance();
                        }
                        let is_pipeline = self.looks_like_pipeline_body();
                        self.pos = saved;
                        if is_pipeline {
                            methods.push(self.parse_pipeline_method()?);
                        } else {
                            handlers.push(self.parse_handler_method()?);
                        }
                    }
                }
                None => return Err(self.error_here("unterminated component body")),
                _ => {
                    return Err(self
                        .error_here("expected `has`, `slot`, `emit`, `query`, `method`, or `}`"))
                }
            }
        }

        let render = render.ok_or_else(|| ParseError {
            message: format!("component `{name}` must declare `method render()`"),
            line: start.line,
            col: start.col,
        })?;

        let _ = &methods; // reserved for pipeline-style methods if needed
        Ok(Component {
            name,
            props,
            state,
            slots,
            emits,
            queries,
            methods,
            handlers,
            render,
            span: self.finish_span(start),
        })
    }

    fn peek_ahead_ident(&self, offset: usize) -> Option<String> {
        self.tokens
            .get(self.pos + offset)
            .and_then(|t| match &t.token {
                Token::Ident(s) => Some(s.clone()),
                _ => None,
            })
    }

    fn parse_render_method(&mut self) -> Result<UiTemplate, ParseError> {
        self.expect_simple(Token::Method, "`method`")?;
        let name = self.expect_ident("method name")?;
        if name != "render" {
            return Err(self.error_here("expected `render`"));
        }
        self.expect_simple(Token::LParen, "`(` after render")?;
        self.expect_simple(Token::RParen, "`)` after render")?;
        self.expect_simple(Token::LBrace, "`{` before render body")?;
        let body = self.parse_template_block()?;
        self.expect_simple(Token::RBrace, "`}` after render body")?;
        Ok(body)
    }

    fn parse_template_block(&mut self) -> Result<UiTemplate, ParseError> {
        let mut items = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace)) && self.peek().is_some() {
            items.push(self.parse_template_item()?);
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
            }
        }
        if items.len() == 1 {
            Ok(items.remove(0))
        } else {
            Ok(UiTemplate::Block(items))
        }
    }

    fn parse_template_item(&mut self) -> Result<UiTemplate, ParseError> {
        match self.peek() {
            Some(Token::When) => self.parse_when(),
            Some(Token::For) => self.parse_for(),
            Some(Token::Ident(name)) if name == "ui" => Ok(UiTemplate::Node(self.parse_ui_node()?)),
            Some(Token::Ident(_)) => Ok(UiTemplate::Node(self.parse_component_call()?)),
            _ => Err(self.error_here("expected ui::…, component call, `when`, or `for`")),
        }
    }

    fn parse_when(&mut self) -> Result<UiTemplate, ParseError> {
        self.expect_simple(Token::When, "`when`")?;
        let condition = self.parse_expr()?;
        self.expect_simple(Token::LBrace, "`{` after when")?;
        let body = Box::new(self.parse_template_block()?);
        self.expect_simple(Token::RBrace, "`}` after when body")?;
        let else_body = if matches!(self.peek(), Some(Token::Else)) {
            self.advance();
            self.expect_simple(Token::LBrace, "`{` after else")?;
            let b = Box::new(self.parse_template_block()?);
            self.expect_simple(Token::RBrace, "`}` after else")?;
            Some(b)
        } else {
            None
        };
        Ok(UiTemplate::When {
            condition,
            body,
            else_body,
        })
    }

    fn parse_for(&mut self) -> Result<UiTemplate, ParseError> {
        self.expect_simple(Token::For, "`for`")?;
        let items = self.parse_expr()?;
        self.expect_simple(Token::Arrow, "`->` in for")?;
        self.expect_simple(Token::Dollar, "`$` before for item")?;
        let item_name = self.expect_ident("for item name")?;
        self.expect_simple(Token::LBrace, "`{` after for")?;
        let body = Box::new(self.parse_template_block()?);
        self.expect_simple(Token::RBrace, "`}` after for")?;
        Ok(UiTemplate::For {
            items,
            item_name,
            body,
        })
    }

    fn parse_ui_node(&mut self) -> Result<UiNode, ParseError> {
        let start = self.current_span();
        let ns = self.expect_ident("ui namespace")?;
        if ns != "ui" {
            return Err(self.error_here("expected `ui`"));
        }
        self.expect_simple(Token::DoubleColon, "`::` after ui")?;
        let (component, component_span) = self.expect_ident_spanned("component name")?;
        self.expect_simple(Token::LParen, "`(` after component")?;
        let (props, prop_spans, events, slots, children) = self.parse_node_args()?;
        Ok(UiNode {
            component,
            component_span,
            props,
            prop_spans,
            events,
            slots,
            children,
            span: start,
        })
    }

    fn parse_component_call(&mut self) -> Result<UiNode, ParseError> {
        let start = self.current_span();
        let (component, component_span) = self.expect_ident_spanned("component name")?;
        self.expect_simple(Token::LParen, "`(` after component")?;
        let (props, prop_spans, events, slots, children) = self.parse_node_args()?;
        Ok(UiNode {
            component,
            component_span,
            props,
            prop_spans,
            events,
            slots,
            children,
            span: start,
        })
    }

    fn parse_node_args(
        &mut self,
    ) -> Result<
        (
            Vec<(String, Expr)>,
            Vec<Span>,
            Vec<EventBinding>,
            Vec<(String, UiTemplate)>,
            Vec<UiTemplate>,
        ),
        ParseError,
    > {
        let mut props = Vec::new();
        let mut prop_spans = Vec::new();
        let mut events = Vec::new();
        let mut slots = Vec::new();
        let mut children = Vec::new();

        while !matches!(self.peek(), Some(Token::RParen)) {
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
                continue;
            }
            if matches!(self.peek(), Some(Token::Colon)) {
                self.advance();
                let (key, key_span) = self.expect_ident_spanned("prop/slot/event name")?;
                if key == "on" {
                    // :on(click(handler)) or :on(add => handler)
                    self.expect_simple(Token::LParen, "`(` after :on")?;
                    let event = self.expect_ident("event name")?;
                    let handler = if matches!(self.peek(), Some(Token::FatArrow)) {
                        self.advance();
                        self.expect_ident("handler name")?
                    } else if matches!(self.peek(), Some(Token::LParen)) {
                        self.advance();
                        let h = self.expect_ident("handler name")?;
                        self.expect_simple(Token::RParen, "`)` after handler")?;
                        h
                    } else {
                        return Err(self.error_here("expected handler after event"));
                    };
                    self.expect_simple(Token::RParen, "`)` after :on")?;
                    events.push(EventBinding {
                        event,
                        handler,
                        span: self.current_span(),
                    });
                } else if matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    // Could be slot (ui:: or component or when/for) or prop expr.
                    if matches!(self.peek(), Some(Token::Ident(n)) if n == "ui")
                        || matches!(self.peek(), Some(Token::When | Token::For))
                        || (matches!(self.peek(), Some(Token::Ident(_)))
                            && matches!(self.peek_n(1), Some(Token::LParen)))
                    {
                        let tmpl = self.parse_template_item()?;
                        self.expect_simple(Token::RParen, "`)` after slot")?;
                        slots.push((key, tmpl));
                    } else if matches!(self.peek(), Some(Token::RParen)) {
                        // bare flag :submit()
                        self.advance();
                        props.push((key, Expr::Bool(true)));
                        prop_spans.push(key_span);
                    } else {
                        let expr = self.parse_expr()?;
                        self.expect_simple(Token::RParen, "`)` after prop")?;
                        props.push((key, expr));
                        prop_spans.push(key_span);
                    }
                } else {
                    // bare flag :submit
                    props.push((key, Expr::Bool(true)));
                    prop_spans.push(key_span);
                }
            } else {
                children.push(self.parse_template_item()?);
            }
        }
        self.expect_simple(Token::RParen, "`)` after component args")?;
        Ok((props, prop_spans, events, slots, children))
    }

    fn parse_handler_method(&mut self) -> Result<Handler, ParseError> {
        let start = self.current_span();
        self.expect_simple(Token::Method, "`method`")?;
        let name = self.expect_ident("method name")?;
        self.expect_simple(Token::LParen, "`(` after method name")?;
        let param_tokens = self.take_balanced_paren_tokens()?;
        let params = parse_params(&param_tokens);
        self.expect_simple(Token::LBrace, "`{` before handler body")?;
        let mut body = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace)) {
            body.push(self.parse_expr()?);
            if matches!(self.peek(), Some(Token::Semi)) {
                self.advance();
            }
        }
        self.expect_simple(Token::RBrace, "`}` after handler")?;
        Ok(Handler {
            name,
            params,
            body,
            span: self.finish_span(start),
        })
    }

    fn looks_like_pipeline_body(&self) -> bool {
        // Heuristic: contains ==> before closing brace at depth 0 — expensive; simpler: if first tokens look like pipeline step then Feed.
        let mut i = self.pos;
        let mut depth = 0usize;
        while let Some(tok) = self.tokens.get(i) {
            match &tok.token {
                Token::RBrace if depth == 0 => return false,
                Token::LBrace => depth += 1,
                Token::RBrace => depth = depth.saturating_sub(1),
                Token::Feed if depth == 0 => return true,
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn finish_pipeline_method(
        &mut self,
        name: String,
        params: Vec<Param>,
        _start: Span,
    ) -> Result<Method, ParseError> {
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
        self.expect_simple(Token::RBrace, "`}` after method")?;
        let steps = segments
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(|s| parse_step(&s))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|message| self.error_here(&message))?;
        Ok(Method {
            name,
            params,
            pipeline: Pipeline { steps },
        })
    }

    fn parse_pipeline_method(&mut self) -> Result<Method, ParseError> {
        let start = self.current_span();
        self.expect_simple(Token::Method, "`method`")?;
        let name = self.expect_ident("method name")?;
        self.expect_simple(Token::LParen, "`(` after method name")?;
        let param_tokens = self.take_balanced_paren_tokens()?;
        let params = parse_params(&param_tokens);
        self.expect_simple(Token::LBrace, "`{` before method body")?;
        self.finish_pipeline_method(name, params, start)
    }

    fn parse_resource_body(
        &mut self,
        name: String,
        start: Span,
        contract: Option<String>,
    ) -> Result<Resource, ParseError> {
        let contract = contract.ok_or_else(|| {
            self.error_here(&format!(
                "resource `{name}` must declare a contract binding: `resource {name} for ContractName {{ … }}`"
            ))
        })?;
        let mut methods = Vec::new();
        let mut seeds = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace)) {
            match self.peek() {
                Some(Token::Query) | Some(Token::Mutation) => {
                    methods.push(self.parse_resource_method(&contract)?);
                }
                Some(Token::Seed) => {
                    seeds.push(self.parse_resource_seed(&contract)?);
                }
                Some(Token::Has) => {
                    return Err(self.error_here(
                        "resource metadata fields are not supported in Silc 0.4.0; use `resource Name for Contract` and capability declarations (`query list;`)",
                    ));
                }
                _ => return Err(self.error_here("expected `query`, `mutation`, `seed`, or `}`")),
            }
        }
        Ok(Resource {
            name,
            methods,
            contract: Some(contract),
            table: None,
            seeds,
            span: self.finish_span(start),
        })
    }

    fn parse_resource_seed(&mut self, contract: &str) -> Result<ResourceSeed, ParseError> {
        let start = self.current_span();
        self.expect_simple(Token::Seed, "`seed`")?;
        let expr = self.parse_expr()?;
        self.expect_simple(Token::Semi, "`;` after seed")?;
        match expr {
            Expr::New { ty, fields } => {
                if ty != contract {
                    return Err(self.error_here(&format!(
                        "resource seed must construct `{contract}`, got `{ty}.new(...)`"
                    )));
                }
                Ok(ResourceSeed {
                    contract: ty,
                    fields,
                    span: start,
                })
            }
            _ => Err(self.error_here(&format!(
                "resource seed must be `{contract}.new(:field(value), …);`"
            ))),
        }
    }

    fn parse_resource_method(&mut self, contract: &str) -> Result<ResourceMethod, ParseError> {
        let start = self.current_span();
        let kind = match self.peek() {
            Some(Token::Query) => {
                self.advance();
                ResourceKind::Query
            }
            Some(Token::Mutation) => {
                self.advance();
                ResourceKind::Mutation
            }
            _ => return Err(self.error_here("expected `query` or `mutation`")),
        };
        let name = self.expect_ident("resource method name")?;

        // Capability form: `query list;` / `mutation create;`
        if matches!(self.peek(), Some(Token::Semi)) {
            self.advance();
            let mut method = ResourceMethod {
                kind,
                name,
                params: vec![],
                return_ty: None,
                pipeline: Pipeline { steps: vec![] },
                span: start,
                shorthand: true,
            };
            method
                .expand_with_contract(contract)
                .map_err(|message| self.error_here(&message))?;
            return Ok(method);
        }

        if matches!(self.peek(), Some(Token::LParen)) || matches!(self.peek(), Some(Token::LBrace))
        {
            return Err(self.error_here(
                "resource method bodies are not supported in Silc 0.4.0; declare capabilities only (e.g. `query list;` / `mutation create;`)",
            ));
        }
        Err(self.error_here("expected `;` after resource capability name"))
    }

    fn parse_app_body(&mut self, name: String, start: Span) -> Result<App, ParseError> {
        let mut routes = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace)) {
            match self.peek() {
                Some(Token::Route) => {
                    let span = self.current_span();
                    self.advance();
                    let path = match self.advance_token()? {
                        Token::StringLit(s) => s.trim_matches('"').to_string(),
                        _ => return Err(self.error_here("expected route path string")),
                    };
                    self.expect_simple(Token::FatArrow, "`=>` in route")?;
                    let component = self.expect_ident("route component")?;
                    if matches!(self.peek(), Some(Token::Semi)) {
                        self.advance();
                    }
                    routes.push(Route {
                        path,
                        component,
                        span,
                    });
                }
                Some(Token::Method) => {
                    return Err(self.error_here(
                        "app `method serve()` is not supported in Silc 0.4.0; declare routes only — dual-surface web/terminal serving is synthesized by the compiler",
                    ));
                }
                _ => return Err(self.error_here("expected `route` or `}`")),
            }
        }
        Ok(App {
            name,
            routes,
            serve: None,
            span: self.finish_span(start),
        })
    }

    fn at_game_node(&self) -> bool {
        matches!(self.peek(), Some(Token::Ident(n)) if n == "game")
            || matches!(self.peek(), Some(Token::Game))
    }

    fn parse_game_node(&mut self) -> Result<GameNode, ParseError> {
        let start = self.current_span();
        match self.peek() {
            Some(Token::Ident(n)) if n == "game" => {
                self.advance();
            }
            Some(Token::Game) => {
                self.advance();
            }
            _ => return Err(self.error_here("expected `game` namespace")),
        }
        self.expect_simple(Token::DoubleColon, "`::` after game")?;
        let (name, name_span) = self.expect_ident_spanned("game node name")?;
        self.expect_simple(Token::LParen, "`(` after game node")?;
        let mut props = Vec::new();
        let mut prop_spans = Vec::new();
        let mut children = Vec::new();
        while !matches!(self.peek(), Some(Token::RParen)) {
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
                continue;
            }
            if matches!(self.peek(), Some(Token::Colon)) {
                self.advance();
                let (key, key_span) = self.expect_ident_spanned("game prop name")?;
                if matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    if matches!(self.peek(), Some(Token::RParen)) {
                        self.advance();
                        props.push((key, Expr::Bool(true)));
                        prop_spans.push(key_span);
                    } else if self.at_game_node() {
                        let child = self.parse_game_node()?;
                        self.expect_simple(Token::RParen, "`)` after nested game node")?;
                        children.push(child);
                    } else {
                        let expr = self.parse_game_prop_expr()?;
                        self.expect_simple(Token::RParen, "`)` after game prop")?;
                        props.push((key, expr));
                        prop_spans.push(key_span);
                    }
                } else {
                    props.push((key, Expr::Bool(true)));
                    prop_spans.push(key_span);
                }
            } else if self.at_game_node() {
                children.push(self.parse_game_node()?);
            } else {
                return Err(self.error_here("expected `:prop(...)` or `game::node(...)`"));
            }
        }
        self.expect_simple(Token::RParen, "`)` after game node")?;
        Ok(GameNode {
            name,
            name_span,
            props,
            prop_spans,
            children,
            span: self.finish_span(start),
        })
    }

    fn parse_game_prop_expr(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(Token::UnitLiteral(_)) => {
                let Token::UnitLiteral(raw) = self.advance_token()? else {
                    unreachable!();
                };
                let digits: String = raw
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                Ok(Expr::Number(digits))
            }
            _ => self.parse_expr(),
        }
    }

    // --- Expressions (Pratt) ---

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_prefix()?;
        loop {
            let op = match self.peek() {
                Some(Token::OrOr) if min_bp <= 1 => BinOpKind::Or,
                Some(Token::AndAnd) if min_bp <= 2 => BinOpKind::And,
                Some(Token::EqEq) if min_bp <= 3 => BinOpKind::Eq,
                Some(Token::NotEq) if min_bp <= 3 => BinOpKind::Ne,
                Some(Token::LAngle) if min_bp <= 4 => BinOpKind::Lt,
                Some(Token::RAngle) if min_bp <= 4 => BinOpKind::Gt,
                Some(Token::Le) if min_bp <= 4 => BinOpKind::Le,
                Some(Token::Ge) if min_bp <= 4 => BinOpKind::Ge,
                Some(Token::Plus) if min_bp <= 5 => BinOpKind::Add,
                Some(Token::Minus) if min_bp <= 5 => BinOpKind::Sub,
                Some(Token::Star) if min_bp <= 6 => BinOpKind::Mul,
                Some(Token::Slash) if min_bp <= 6 => BinOpKind::Div,
                Some(Token::Eq) if min_bp <= 0 => {
                    self.advance();
                    let rhs = self.parse_expr_bp(0)?;
                    return Ok(Expr::Assign {
                        target: Box::new(lhs),
                        value: Box::new(rhs),
                    });
                }
                Some(Token::Dot) => {
                    self.advance();
                    let field = self.expect_ident("member field")?;
                    lhs = Expr::Member {
                        base: Box::new(lhs),
                        field,
                    };
                    if matches!(self.peek(), Some(Token::LParen)) {
                        self.advance();
                        let mut args = Vec::new();
                        if !matches!(self.peek(), Some(Token::RParen)) {
                            loop {
                                args.push(self.parse_expr()?);
                                if matches!(self.peek(), Some(Token::Comma)) {
                                    self.advance();
                                    continue;
                                }
                                break;
                            }
                        }
                        self.expect_simple(Token::RParen, "`)` after call")?;
                        lhs = Expr::Call {
                            callee: Box::new(lhs),
                            args,
                        };
                    }
                    continue;
                }
                _ => break,
            };
            self.advance();
            let (l_bp, r_bp) = op.binding_power();
            if l_bp < min_bp {
                break;
            }
            let rhs = self.parse_expr_bp(r_bp)?;
            lhs = Expr::BinOp {
                op: op.into_core(),
                left: Box::new(lhs),
                right: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(Token::Bang) => {
                self.advance();
                Ok(Expr::Unary {
                    op: sil_core::UnaryOp::Not,
                    expr: Box::new(self.parse_prefix()?),
                })
            }
            Some(Token::Minus) => {
                self.advance();
                Ok(Expr::Unary {
                    op: sil_core::UnaryOp::Neg,
                    expr: Box::new(self.parse_prefix()?),
                })
            }
            Some(Token::Await) => {
                self.advance();
                Ok(Expr::Await(Box::new(self.parse_prefix()?)))
            }
            Some(Token::Emit) => {
                self.advance();
                let event = self.expect_ident("emit event")?;
                let payload = if matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    let e = self.parse_expr()?;
                    self.expect_simple(Token::RParen, "`)` after emit")?;
                    Some(Box::new(e))
                } else {
                    None
                };
                Ok(Expr::Emit { event, payload })
            }
            Some(Token::Dollar) => {
                self.advance();
                if matches!(self.peek(), Some(Token::Dot)) {
                    self.advance();
                    let name = self.expect_ident("variable name")?;
                    Ok(Expr::Var(name))
                } else {
                    let name = self.expect_ident("variable name")?;
                    Ok(Expr::Var(name))
                }
            }
            Some(Token::StringLit(_)) => {
                let s = match self.advance_token()? {
                    Token::StringLit(s) => s.trim_matches('"').to_string(),
                    _ => unreachable!(),
                };
                Ok(Expr::String(s))
            }
            Some(Token::Number(_)) => {
                let n = match self.advance_token()? {
                    Token::Number(n) => n,
                    _ => unreachable!(),
                };
                Ok(Expr::Number(n))
            }
            Some(Token::Ident(name)) if name == "true" || name == "false" => {
                let name = match self.advance_token()? {
                    Token::Ident(n) => n,
                    _ => unreachable!(),
                };
                Ok(Expr::Bool(name == "true"))
            }
            Some(Token::Ident(_)) => {
                let name = self.expect_ident("identifier")?;
                if name == "navigate" && matches!(self.peek(), Some(Token::LParen)) {
                    self.advance();
                    let path = match self.advance_token()? {
                        Token::StringLit(s) => s.trim_matches('"').to_string(),
                        _ => return Err(self.error_here("navigate expects string path")),
                    };
                    self.expect_simple(Token::RParen, "`)` after navigate")?;
                    return Ok(Expr::Navigate { path });
                }
                if matches!(self.peek(), Some(Token::LParen)) {
                    // Could be Type.new(...) or call
                    self.advance();
                    // Check for :field(expr) named args → New
                    if matches!(self.peek(), Some(Token::Colon)) {
                        let mut fields = Vec::new();
                        while !matches!(self.peek(), Some(Token::RParen)) {
                            if matches!(self.peek(), Some(Token::Comma)) {
                                self.advance();
                                continue;
                            }
                            self.expect_simple(Token::Colon, "`:` in constructor")?;
                            let field = self.expect_ident("field")?;
                            self.expect_simple(Token::LParen, "`(` after field")?;
                            let value = self.parse_expr()?;
                            self.expect_simple(Token::RParen, "`)` after field")?;
                            fields.push((field, value));
                        }
                        self.expect_simple(Token::RParen, "`)` after constructor")?;
                        // If name ends with .new — handle Name.new separately
                        return Ok(Expr::New { ty: name, fields });
                    }
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Token::RParen)) {
                        loop {
                            args.push(self.parse_expr()?);
                            if matches!(self.peek(), Some(Token::Comma)) {
                                self.advance();
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect_simple(Token::RParen, "`)` after call")?;
                    return Ok(Expr::Call {
                        callee: Box::new(Expr::Ident(name)),
                        args,
                    });
                }
                if matches!(self.peek(), Some(Token::Dot)) {
                    // Name.new(...) or Name.method
                    let base = Expr::Ident(name);
                    // Let postfix loop in parse_expr_bp handle Dot — but we're in prefix.
                    // Manually continue member chain.
                    let mut expr = base;
                    while matches!(self.peek(), Some(Token::Dot)) {
                        self.advance();
                        let field = self.expect_ident("member")?;
                        if field == "new" && matches!(self.peek(), Some(Token::LParen)) {
                            self.advance();
                            let mut fields = Vec::new();
                            while !matches!(self.peek(), Some(Token::RParen)) {
                                if matches!(self.peek(), Some(Token::Comma)) {
                                    self.advance();
                                    continue;
                                }
                                self.expect_simple(Token::Colon, "`:` in constructor")?;
                                let fname = self.expect_ident("field")?;
                                self.expect_simple(Token::LParen, "`(`")?;
                                let value = self.parse_expr()?;
                                self.expect_simple(Token::RParen, "`)`")?;
                                fields.push((fname, value));
                            }
                            self.expect_simple(Token::RParen, "`)` after new")?;
                            let ty = match expr {
                                Expr::Ident(t) => t,
                                _ => return Err(self.error_here("`.new` requires type name")),
                            };
                            return Ok(Expr::New { ty, fields });
                        }
                        expr = Expr::Member {
                            base: Box::new(expr),
                            field,
                        };
                        if matches!(self.peek(), Some(Token::LParen)) {
                            self.advance();
                            let mut args = Vec::new();
                            if !matches!(self.peek(), Some(Token::RParen)) {
                                loop {
                                    args.push(self.parse_expr()?);
                                    if matches!(self.peek(), Some(Token::Comma)) {
                                        self.advance();
                                        continue;
                                    }
                                    break;
                                }
                            }
                            self.expect_simple(Token::RParen, "`)`")?;
                            expr = Expr::Call {
                                callee: Box::new(expr),
                                args,
                            };
                        }
                    }
                    return Ok(expr);
                }
                Ok(Expr::Ident(name))
            }
            Some(Token::LBracket) => {
                self.advance();
                let mut items = Vec::new();
                if !matches!(self.peek(), Some(Token::RBracket)) {
                    loop {
                        items.push(self.parse_expr()?);
                        if matches!(self.peek(), Some(Token::Comma)) {
                            self.advance();
                            continue;
                        }
                        break;
                    }
                }
                self.expect_simple(Token::RBracket, "`]` after list")?;
                Ok(Expr::List(items))
            }
            Some(Token::LParen) => {
                self.advance();
                let e = self.parse_expr()?;
                self.expect_simple(Token::RParen, "`)` after expr")?;
                Ok(e)
            }
            _ => Err(self.error_here("expected expression")),
        }
    }

    fn parse_type(&mut self) -> Result<TypeExpr, ParseError> {
        if matches!(self.peek(), Some(Token::LBracket)) {
            self.advance();
            let inner = self.parse_type()?;
            self.expect_simple(Token::RBracket, "`]` after array type")?;
            return Ok(TypeExpr::Array(Box::new(inner)));
        }
        let name = self.expect_ident("type name")?;
        if name == "Vec" && matches!(self.peek(), Some(Token::LBracket)) {
            self.advance();
            let elem = self.expect_ident("vector element type")?;
            let len = if matches!(self.peek(), Some(Token::Semi)) {
                self.advance();
                match self.advance_token()? {
                    Token::Number(n) => Some(n.parse().unwrap_or(0)),
                    _ => None,
                }
            } else {
                None
            };
            self.expect_simple(Token::RBracket, "`]` after Vec")?;
            return Ok(TypeExpr::Vec { elem, len });
        }
        Ok(TypeExpr::Named(name))
    }

    // --- token helpers ---

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.token)
    }

    fn peek_n(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n).map(|t| &t.token)
    }

    fn advance(&mut self) -> Option<SpannedToken> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn advance_token(&mut self) -> Result<Token, ParseError> {
        self.advance()
            .map(|t| t.token)
            .ok_or_else(|| self.error_here("unexpected end of input"))
    }

    fn expect_simple(&mut self, expected: Token, what: &str) -> Result<(), ParseError> {
        match self.peek() {
            Some(tok) if std::mem::discriminant(tok) == std::mem::discriminant(&expected) => {
                self.advance();
                Ok(())
            }
            _ => Err(self.error_here(&format!("expected {what}"))),
        }
    }

    fn expect_ident(&mut self, what: &str) -> Result<String, ParseError> {
        Ok(self.expect_ident_spanned(what)?.0)
    }

    fn expect_ident_spanned(&mut self, what: &str) -> Result<(String, Span), ParseError> {
        let span = self.current_span();
        match self.advance_token()? {
            Token::Ident(s) => {
                // Prefer the token's full byte span when available.
                let span = self
                    .tokens
                    .get(self.pos.saturating_sub(1))
                    .map(|t| Span::new(t.start, t.end, t.line, t.col))
                    .unwrap_or(span);
                Ok((s, span))
            }
            _ => Err(self.error_here(&format!("expected {what}"))),
        }
    }

    fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| Span::new(t.start, t.end, t.line, t.col))
            .unwrap_or_default()
    }

    fn last_span(&self) -> Span {
        if self.pos == 0 {
            return Span::default();
        }
        self.tokens
            .get(self.pos - 1)
            .map(|t| Span::new(t.start, t.end, t.line, t.col))
            .unwrap_or_default()
    }

    fn finish_span(&self, start: Span) -> Span {
        Span::cover(start, self.last_span())
    }

    fn error_here(&self, message: &str) -> ParseError {
        let span = self.current_span();
        ParseError {
            message: message.into(),
            line: span.line,
            col: span.col,
        }
    }

    fn collect_until_matching_paren(&mut self) -> Result<String, ParseError> {
        let mut depth = 1usize;
        let mut out = String::new();
        while let Some(tok) = self.advance() {
            match tok.token {
                Token::LParen => {
                    depth += 1;
                    out.push('(');
                }
                Token::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(')');
                }
                _ => {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(&tok.slice);
                }
            }
        }
        Ok(out.trim().trim_matches('"').to_string())
    }

    fn collect_balanced_brace_text(&mut self) -> Result<String, ParseError> {
        let mut depth = 1usize;
        let mut out = String::new();
        while let Some(tok) = self.advance() {
            match tok.token {
                Token::LBrace => {
                    depth += 1;
                    out.push('{');
                }
                Token::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push('}');
                }
                _ => {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(&tok.slice);
                }
            }
        }
        Ok(out)
    }

    fn collect_until_semi(&mut self) -> Result<String, ParseError> {
        let mut out = String::new();
        while let Some(tok) = self.peek() {
            if matches!(tok, Token::Semi) {
                self.advance();
                break;
            }
            let t = self.advance().unwrap();
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&t.slice);
        }
        Ok(out.trim().trim_matches('"').to_string())
    }

    fn take_balanced_paren_tokens(&mut self) -> Result<Vec<SpannedToken>, ParseError> {
        let mut depth = 1usize;
        let mut out = Vec::new();
        while let Some(tok) = self.advance() {
            match tok.token {
                Token::LParen => {
                    depth += 1;
                    out.push(tok);
                }
                Token::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(tok);
                }
                _ => out.push(tok),
            }
        }
        Ok(out)
    }
}

enum BinOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinOpKind {
    fn binding_power(&self) -> (u8, u8) {
        match self {
            BinOpKind::Or => (1, 2),
            BinOpKind::And => (3, 4),
            BinOpKind::Eq | BinOpKind::Ne => (5, 6),
            BinOpKind::Lt | BinOpKind::Le | BinOpKind::Gt | BinOpKind::Ge => (7, 8),
            BinOpKind::Add | BinOpKind::Sub => (9, 10),
            BinOpKind::Mul | BinOpKind::Div => (11, 12),
        }
    }

    fn into_core(self) -> sil_core::BinOp {
        match self {
            BinOpKind::Add => sil_core::BinOp::Add,
            BinOpKind::Sub => sil_core::BinOp::Sub,
            BinOpKind::Mul => sil_core::BinOp::Mul,
            BinOpKind::Div => sil_core::BinOp::Div,
            BinOpKind::Eq => sil_core::BinOp::Eq,
            BinOpKind::Ne => sil_core::BinOp::Ne,
            BinOpKind::Lt => sil_core::BinOp::Lt,
            BinOpKind::Le => sil_core::BinOp::Le,
            BinOpKind::Gt => sil_core::BinOp::Gt,
            BinOpKind::Ge => sil_core::BinOp::Ge,
            BinOpKind::And => sil_core::BinOp::And,
            BinOpKind::Or => sil_core::BinOp::Or,
        }
    }
}

fn parse_params(tokens: &[SpannedToken]) -> Vec<Param> {
    if tokens.is_empty() {
        return Vec::new();
    }
    let parts = split_top_level(tokens, |t| matches!(t, Token::Comma));
    let mut params = Vec::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        let mut named = false;
        let mut idx = 0usize;
        if matches!(part.first().map(|t| &t.token), Some(Token::Colon)) {
            named = true;
            idx = 1;
        }
        // Type Name or Type $Name or :$name = default
        let mut ty = None;
        let mut name = String::new();
        let mut default = None;
        while idx < part.len() {
            match &part[idx].token {
                Token::Ident(id)
                    if ty.is_none()
                        && idx + 1 < part.len()
                        && matches!(part[idx + 1].token, Token::Dollar | Token::Ident(_)) =>
                {
                    // Could be type
                    if matches!(part.get(idx + 1).map(|t| &t.token), Some(Token::Dollar))
                        || (matches!(part.get(idx + 1).map(|t| &t.token), Some(Token::Ident(_)))
                            && idx + 2 < part.len()
                            && matches!(part[idx + 2].token, Token::Eq))
                    {
                        ty = Some(TypeExpr::Named(id.clone()));
                    } else if name.is_empty() && ty.is_some() {
                        name = id.clone();
                    } else if name.is_empty() {
                        name = id.clone();
                    }
                }
                Token::LBracket if ty.is_none() => {
                    // [Type]
                    if let Some(Token::Ident(elem)) = part.get(idx + 1).map(|t| &t.token) {
                        ty = Some(TypeExpr::Array(Box::new(TypeExpr::Named(elem.clone()))));
                        idx += 2; // skip ] later
                    }
                }
                Token::Dollar => {
                    if let Some(Token::Ident(n)) = part.get(idx + 1).map(|t| &t.token) {
                        name = n.clone();
                        idx += 1;
                    }
                }
                Token::Ident(id) if name.is_empty() => name = id.clone(),
                Token::Eq => {
                    let mut d = String::new();
                    for t in &part[idx + 1..] {
                        if !d.is_empty() {
                            d.push(' ');
                        }
                        d.push_str(&t.slice);
                    }
                    default = Some(d.trim().trim_matches('"').to_string());
                    break;
                }
                _ => {}
            }
            idx += 1;
        }
        if !name.is_empty() {
            let span = part
                .iter()
                .find(|t| matches!(&t.token, Token::Ident(id) if id == &name))
                .map(|t| Span::new(t.start, t.end, t.line, t.col))
                .unwrap_or_default();
            params.push(Param {
                name,
                ty,
                named,
                default,
                span,
            });
        }
    }
    params
}

fn parse_step(tokens: &[SpannedToken]) -> Result<PipelineStep, String> {
    if tokens.is_empty() {
        return Err("empty pipeline step".into());
    }
    // $base.field
    if matches!(tokens.first().map(|t| &t.token), Some(Token::Dollar)) {
        if tokens.len() >= 4
            && matches!(tokens.get(1).map(|t| &t.token), Some(Token::Ident(_)))
            && matches!(tokens.get(2).map(|t| &t.token), Some(Token::Dot))
            && matches!(tokens.get(3).map(|t| &t.token), Some(Token::Ident(_)))
        {
            let base = match &tokens[1].token {
                Token::Ident(s) => s.clone(),
                _ => unreachable!(),
            };
            let field = match &tokens[3].token {
                Token::Ident(s) => s.clone(),
                _ => unreachable!(),
            };
            return Ok(PipelineStep::FieldAccess { base, field });
        }
        if tokens.len() >= 2 {
            if let Token::Ident(s) = &tokens[1].token {
                return Ok(PipelineStep::Name(s.clone()));
            }
        }
    }
    // ns::name(...) — subject keywords may appear as namespaces (`resource::list`).
    if let Some(ns) = tokens.first().map(|t| &t.token).and_then(ident_like_name) {
        if matches!(tokens.get(1).map(|t| &t.token), Some(Token::DoubleColon)) {
            if let Some(name) = tokens.get(2).map(|t| &t.token).and_then(ident_like_name) {
                let args = if matches!(tokens.get(3).map(|t| &t.token), Some(Token::LParen)) {
                    parse_call_args(&tokens[4..])?
                } else {
                    vec![]
                };
                let span = Span::cover(
                    Span::new(tokens[0].start, tokens[0].end, tokens[0].line, tokens[0].col),
                    Span::new(tokens[2].start, tokens[2].end, tokens[2].line, tokens[2].col),
                );
                return Ok(PipelineStep::Call {
                    namespace: Some(ns),
                    name,
                    args,
                    span,
                });
            }
        }
        // bare name
        if tokens.len() == 1 {
            return Ok(PipelineStep::Name(ns));
        }
        // name(...) without namespace
        if matches!(tokens.get(1).map(|t| &t.token), Some(Token::LParen)) {
            let args = parse_call_args(&tokens[2..])?;
            let span = Span::new(tokens[0].start, tokens[0].end, tokens[0].line, tokens[0].col);
            return Ok(PipelineStep::Call {
                namespace: None,
                name: ns,
                args,
                span,
            });
        }
        return Ok(PipelineStep::Name(ns));
    }
    Err(format!(
        "unrecognized pipeline step near `{}`",
        tokens.first().map(|t| t.slice.as_str()).unwrap_or("?")
    ))
}

/// Identifiers and reserved words that are still valid as names / namespaces.
fn ident_like_name(token: &Token) -> Option<String> {
    match token {
        Token::Ident(n) => Some(n.clone()),
        Token::Contract => Some("contract".into()),
        Token::Component => Some("component".into()),
        Token::Resource => Some("resource".into()),
        Token::App => Some("app".into()),
        Token::Game => Some("game".into()),
        Token::Service => Some("service".into()),
        Token::Processor => Some("processor".into()),
        Token::Sink => Some("sink".into()),
        Token::Task => Some("task".into()),
        Token::Class => Some("class".into()),
        Token::Route => Some("route".into()),
        Token::Method => Some("method".into()),
        Token::Query => Some("query".into()),
        Token::Mutation => Some("mutation".into()),
        Token::Seed => Some("seed".into()),
        Token::State => Some("state".into()),
        Token::Slot => Some("slot".into()),
        Token::Emit => Some("emit".into()),
        Token::Has => Some("has".into()),
        Token::For => Some("for".into()),
        Token::When => Some("when".into()),
        Token::Else => Some("else".into()),
        Token::Await => Some("await".into()),
        _ => None,
    }
}

/// Named call args may reuse reserved words (`:route`, `:method`, `:query`, …).
fn arg_name_from_token(token: &Token) -> Option<String> {
    ident_like_name(token)
}

fn parse_call_args(tokens: &[SpannedToken]) -> Result<Vec<TraitArg>, String> {
    let mut args = Vec::new();
    let mut i = 0usize;
    while i < tokens.len() {
        match &tokens[i].token {
            Token::RParen => break,
            Token::Comma => {
                i += 1;
                continue;
            }
            Token::Colon => {
                i += 1;
                let name = match tokens
                    .get(i)
                    .map(|t| &t.token)
                    .and_then(arg_name_from_token)
                {
                    Some(n) => {
                        i += 1;
                        n
                    }
                    None => return Err("expected arg name after `:`".into()),
                };
                let value = if matches!(tokens.get(i).map(|t| &t.token), Some(Token::LParen)) {
                    i += 1;
                    let mut depth = 1usize;
                    let mut val = String::new();
                    while i < tokens.len() {
                        match &tokens[i].token {
                            Token::LParen => {
                                depth += 1;
                                val.push('(');
                            }
                            Token::RParen => {
                                depth -= 1;
                                if depth == 0 {
                                    i += 1;
                                    break;
                                }
                                val.push(')');
                            }
                            _ => {
                                if !val.is_empty() {
                                    val.push(' ');
                                }
                                val.push_str(&tokens[i].slice);
                            }
                        }
                        i += 1;
                    }
                    val.trim().trim_matches('"').to_string()
                } else {
                    String::new()
                };
                args.push(TraitArg { name, value });
            }
            _ => i += 1,
        }
    }
    Ok(args)
}

fn split_top_level(
    tokens: &[SpannedToken],
    pred: impl Fn(&Token) -> bool,
) -> Vec<Vec<SpannedToken>> {
    let mut parts = vec![Vec::new()];
    let mut depth = 0isize;
    for tok in tokens {
        match &tok.token {
            Token::LParen | Token::LBracket | Token::LBrace => {
                depth += 1;
                parts.last_mut().unwrap().push(tok.clone());
            }
            Token::RParen | Token::RBracket | Token::RBrace => {
                depth -= 1;
                parts.last_mut().unwrap().push(tok.clone());
            }
            t if depth == 0 && pred(t) => parts.push(Vec::new()),
            _ => parts.last_mut().unwrap().push(tok.clone()),
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_component_and_app() {
        let src = r#"
@version("0.4.0")
contract Product {
    has Str $.name;
    has num64 $.price;
}
component ProductCard {
    has Product $.product;
    emit add(Product);
    method render() {
        ui::card(
            ui::heading(:text($.product.name)),
            ui::button(:label("Add"), :on(click(add)))
        )
    }
    method add() {
        emit add($.product);
    }
}
component ShopPage {
    method render() {
        ui::page(
            ProductCard(:product($.product), :on(add => on_add))
        )
    }
    method on_add(Product $p) {
        navigate("/");
    }
}
app ShopApp {
    route "/" => ShopPage;
}
"#;
        let program = parse(src).expect("parse");
        assert_eq!(program.contracts.len(), 1);
        assert_eq!(program.components.len(), 2);
        assert_eq!(program.apps.len(), 1);
        assert_eq!(program.apps[0].routes[0].component, "ShopPage");
        assert!(program.apps[0].serve.is_none());
    }

    #[test]
    fn parses_intent_declarations() {
        let src = r#"
@version("0.4.0")
contract Record { has Str $.id; }
component Page { method render() { ui::page() } }
resource Records for Record {
    query list;
    mutation create;
}
app Demo {
    route "/" => Page;
}
service Api {}
processor Worker {}
task Cleanup {}
"#;
        let program = parse(src).expect("parse intent declarations");
        assert_eq!(program.contracts.len(), 1);
        assert_eq!(program.components.len(), 1);
        assert_eq!(program.resources.len(), 1);
        assert_eq!(program.resources[0].contract.as_deref(), Some("Record"));
        assert_eq!(program.resources[0].methods.len(), 2);
        assert_eq!(program.apps.len(), 1);
        assert_eq!(program.modules.len(), 3);
    }

    #[test]
    fn parses_resource_seeds() {
        let src = r#"
@version("0.4.0")
contract Article {
    has Str $.id;
    has Str $.title;
}
resource Articles for Article {
    query list;
    mutation create;
    seed Article.new(:id("a1"), :title("Hello"));
    seed Article.new(:id("a2"), :title("World"));
}
"#;
        let program = parse(src).expect("parse seeds");
        assert_eq!(program.resources[0].seeds.len(), 2);
        assert_eq!(program.resources[0].seeds[0].contract, "Article");
        assert_eq!(program.resources[0].seeds[0].fields[0].0, "id");
        program.validate().expect("validate seeds");
    }

    #[test]
    fn rejects_seed_wrong_contract() {
        let src = r#"
@version("0.4.0")
contract Article { has Str $.id; }
contract Other { has Str $.id; }
resource Articles for Article {
    query list;
    seed Other.new(:id("x"));
}
"#;
        let err = parse(src).unwrap_err();
        assert!(err.message.contains("Article"), "error: {err}");
    }

    #[test]
    fn validate_rejects_seed_without_id() {
        let src = r#"
@version("0.4.0")
contract Article {
    has Str $.id;
    has Str $.title;
}
resource Articles for Article {
    query list;
    seed Article.new(:title("Hello"));
}
component Page { method render() { ui::page() } }
app Demo { route "/" => Page; }
"#;
        let program = parse(src).expect("parse");
        let err = program.validate().unwrap_err();
        assert!(err.contains("id"), "error: {err}");
    }

    #[test]
    fn rejects_author_sink_with_migration_diagnostic() {
        let err = parse("sink Db is storage(SQLite) {}").unwrap_err();
        assert!(err.message.contains("sink"), "error: {err}");
        assert!(err.message.contains("0.4.0"), "error: {err}");
    }

    #[test]
    fn legacy_class_forms_get_actionable_migration_diagnostics() {
        let cases = [
            ("class Record {}", "contract Record { ... }"),
            ("class Page is component {}", "component Page { ... }"),
            ("class Records is resource {}", "resource Records { ... }"),
            ("class Demo is app {}", "app Demo { ... }"),
            ("class Api is service {}", "service Api { ... }"),
            ("class Worker is processor {}", "processor Worker { ... }"),
            ("class Cleanup is task {}", "task Cleanup { ... }"),
        ];
        for (source, replacement) in cases {
            let err = parse(source).unwrap_err();
            assert!(err.message.contains("legacy `class`"), "error: {err}");
            assert!(err.message.contains(replacement), "error: {err}");
        }
        let sink_err = parse("class Db is sink is storage(SQLite) {}").unwrap_err();
        assert!(sink_err.message.contains("legacy `class`"), "{sink_err}");
    }

    #[test]
    fn parses_subset_where_contains() {
        let src = r#"
@version("0.4.0")
subset Uri of Str where { .contains("://") }
contract Item {
    has Uri $.url;
}
"#;
        let program = parse(src).expect("parse");
        assert_eq!(program.subsets.len(), 1);
        assert_eq!(
            program.subsets[0].predicate,
            Some(SubsetPredicate::Contains("://".into()))
        );
        program.validate().expect("validate");
    }

    #[test]
    fn rejects_unsupported_subset_predicate() {
        let src = r#"
subset Uri of Str where { .len > 0 }
"#;
        let err = parse(src).unwrap_err();
        assert!(err.message.contains("unsupported"));
    }

    #[test]
    fn validate_rejects_bad_subset_literal() {
        let src = r#"
@version("0.4.0")
subset Uri of Str where { .contains("://") }
contract Product {
    has Uri $.url;
}
component Page {
    method render() {
        ui::page(ui::heading(:text("x")))
    }
    method bad() {
        Product.new(:url("notauri"));
    }
}
app App {
    route "/" => Page;
}
"#;
        let program = parse(src).expect("parse");
        let err = program.validate().unwrap_err();
        assert!(err.contains("does not satisfy subset `Uri`"), "{err}");
    }

    #[test]
    fn parses_minimal_game_scene() {
        let src = r#"
@version("0.4.0")
game Foo {
    game::scene(
        :title("T"),
        game::overlay(:toggle("F1"))
    )
}
"#;
        let program = parse(src).expect("parse");
        assert_eq!(program.games.len(), 1);
        assert_eq!(program.games[0].name, "Foo");
        assert_eq!(program.games[0].root.name, "scene");
        assert_eq!(
            program.games[0].root.prop("title").and_then(|e| e.as_string_literal()),
            Some("T")
        );
        assert_eq!(program.games[0].root.children.len(), 1);
        assert_eq!(program.games[0].root.children[0].name, "overlay");
        program.validate().expect("validate minimal game");
    }
}
