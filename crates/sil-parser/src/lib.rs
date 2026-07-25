//! Recursive-descent parser for Silc 0.2.0 grammar.

use sil_core::{
    App, CompField, Component, Contract, EmitDecl, EventBinding, Expr, Field, Handler, Method,
    Module, ModuleKind, Param, Pipeline, PipelineStep, Program, QueryBinding, Resource,
    ResourceKind, ResourceMethod, Route, SlotDecl, Span, Subset, TraitArg, TypeExpr, UiNode,
    UiTemplate,
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

enum ClassKind {
    Contract,
    Module(ModuleKind),
    Component,
    Resource,
    App,
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
                Some(Token::Class) => self.parse_class_into(&mut program)?,
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

    fn parse_class_into(&mut self, program: &mut Program) -> Result<(), ParseError> {
        let start = self.current_span();
        self.expect_simple(Token::Class, "`class`")?;
        let name = self.expect_ident("class name")?;
        let mut traits = Vec::new();
        let mut kind = ClassKind::Contract;

        while matches!(self.peek(), Some(Token::Is)) {
            self.advance();
            let trait_name = self.expect_ident("trait name after `is`")?;
            let value = if matches!(self.peek(), Some(Token::LParen)) {
                self.advance();
                self.collect_until_matching_paren()?
            } else {
                String::new()
            };
            match trait_name.as_str() {
                "view" => {
                    return Err(self.error_here(
                        "`is view` was removed in Silc 0.2.0; use `is component` instead",
                    ));
                }
                "component" => kind = ClassKind::Component,
                "resource" => kind = ClassKind::Resource,
                "app" => kind = ClassKind::App,
                other => {
                    let parsed = ModuleKind::parse(other);
                    if parsed != ModuleKind::Unknown {
                        kind = ClassKind::Module(parsed);
                    } else {
                        traits.push(TraitArg {
                            name: trait_name,
                            value,
                        });
                    }
                }
            }
        }

        self.expect_simple(Token::LBrace, "`{` after class declaration")?;

        match kind {
            ClassKind::Component => {
                let component = self.parse_component_body(name, start)?;
                self.expect_simple(Token::RBrace, "`}` after component")?;
                program.components.push(component);
            }
            ClassKind::Resource => {
                let resource = self.parse_resource_body(name, start)?;
                self.expect_simple(Token::RBrace, "`}` after resource")?;
                program.resources.push(resource);
            }
            ClassKind::App => {
                let app = self.parse_app_body(name, start)?;
                self.expect_simple(Token::RBrace, "`}` after app")?;
                program.apps.push(app);
            }
            ClassKind::Contract => {
                let mut fields = Vec::new();
                while !matches!(self.peek(), Some(Token::RBrace)) {
                    match self.peek() {
                        Some(Token::Has) => fields.push(self.parse_contract_field()?),
                        None => return Err(self.error_here("unterminated class body")),
                        _ => return Err(self.error_here("expected `has` or `}` in contract")),
                    }
                }
                self.advance();
                program.contracts.push(Contract {
                    name,
                    fields,
                    span: start,
                });
            }
            ClassKind::Module(module_kind) => {
                let mut fields = Vec::new();
                let mut methods = Vec::new();
                while !matches!(self.peek(), Some(Token::RBrace)) {
                    match self.peek() {
                        Some(Token::Has) => fields.push(self.parse_contract_field()?),
                        Some(Token::Method) => methods.push(self.parse_pipeline_method()?),
                        None => return Err(self.error_here("unterminated class body")),
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
                    span: start,
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
        let name = self.expect_ident("attribute name")?;
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
                    let qname = self.expect_ident("query name")?;
                    self.expect_simple(Token::Eq, "`=` in query binding")?;
                    let resource = self.expect_ident("resource name")?;
                    self.expect_simple(Token::Dot, "`.` before resource method")?;
                    let method = self.expect_ident("resource method")?;
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
            span: start,
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
        let component = self.expect_ident("component name")?;
        self.expect_simple(Token::LParen, "`(` after component")?;
        let (props, events, slots, children) = self.parse_node_args()?;
        Ok(UiNode {
            component,
            props,
            events,
            slots,
            children,
            span: start,
        })
    }

    fn parse_component_call(&mut self) -> Result<UiNode, ParseError> {
        let start = self.current_span();
        let component = self.expect_ident("component name")?;
        self.expect_simple(Token::LParen, "`(` after component")?;
        let (props, events, slots, children) = self.parse_node_args()?;
        Ok(UiNode {
            component,
            props,
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
            Vec<EventBinding>,
            Vec<(String, UiTemplate)>,
            Vec<UiTemplate>,
        ),
        ParseError,
    > {
        let mut props = Vec::new();
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
                let key = self.expect_ident("prop/slot/event name")?;
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
                    } else {
                        let expr = self.parse_expr()?;
                        self.expect_simple(Token::RParen, "`)` after prop")?;
                        props.push((key, expr));
                    }
                } else {
                    // bare flag :submit
                    props.push((key, Expr::Bool(true)));
                }
            } else {
                children.push(self.parse_template_item()?);
            }
        }
        self.expect_simple(Token::RParen, "`)` after component args")?;
        Ok((props, events, slots, children))
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
            span: start,
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

    fn parse_resource_body(&mut self, name: String, start: Span) -> Result<Resource, ParseError> {
        let mut methods = Vec::new();
        let mut table = None;
        let mut contract = None;
        while !matches!(self.peek(), Some(Token::RBrace)) {
            match self.peek() {
                Some(Token::Query) | Some(Token::Mutation) => {
                    methods.push(self.parse_resource_method()?);
                }
                Some(Token::Has) => {
                    // Optional metadata: has Str $.table = "products";
                    let field = self.parse_contract_field()?;
                    if field.name == "table" {
                        table = field.default.map(|d| d.trim_matches('"').to_string());
                    } else if field.name == "contract" {
                        contract = field.default.map(|d| d.trim_matches('"').to_string());
                    }
                }
                _ => return Err(self.error_here("expected `query`, `mutation`, `has`, or `}`")),
            }
        }
        Ok(Resource {
            name,
            methods,
            contract,
            table,
            span: start,
        })
    }

    fn parse_resource_method(&mut self) -> Result<ResourceMethod, ParseError> {
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
        self.expect_simple(Token::LParen, "`(` after resource method")?;
        let param_tokens = self.take_balanced_paren_tokens()?;
        let params = parse_params(&param_tokens);
        let return_ty = if matches!(self.peek(), Some(Token::Arrow)) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_simple(Token::LBrace, "`{` before resource method body")?;
        let method = self.finish_pipeline_method(name.clone(), params.clone(), start)?;
        Ok(ResourceMethod {
            kind,
            name,
            params,
            return_ty,
            pipeline: method.pipeline,
            span: start,
        })
    }

    fn parse_app_body(&mut self, name: String, start: Span) -> Result<App, ParseError> {
        let mut routes = Vec::new();
        let mut serve = None;
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
                    let method = self.parse_pipeline_method()?;
                    if method.name == "serve" {
                        serve = Some(method);
                    }
                }
                _ => return Err(self.error_here("expected `route`, `method serve`, or `}`")),
            }
        }
        Ok(App {
            name,
            routes,
            serve,
            span: start,
        })
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
        match self.advance_token()? {
            Token::Ident(s) => Ok(s),
            _ => Err(self.error_here(&format!("expected {what}"))),
        }
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
            params.push(Param {
                name,
                ty,
                named,
                default,
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
    // ns::name(...)
    if let Some(Token::Ident(ns)) = tokens.first().map(|t| &t.token) {
        if matches!(tokens.get(1).map(|t| &t.token), Some(Token::DoubleColon)) {
            if let Some(Token::Ident(name)) = tokens.get(2).map(|t| &t.token) {
                let args = if matches!(tokens.get(3).map(|t| &t.token), Some(Token::LParen)) {
                    parse_call_args(&tokens[4..])?
                } else {
                    vec![]
                };
                return Ok(PipelineStep::Call {
                    namespace: Some(ns.clone()),
                    name: name.clone(),
                    args,
                });
            }
        }
        // bare name
        if tokens.len() == 1 {
            return Ok(PipelineStep::Name(ns.clone()));
        }
        // name(...) without namespace
        if matches!(tokens.get(1).map(|t| &t.token), Some(Token::LParen)) {
            let args = parse_call_args(&tokens[2..])?;
            return Ok(PipelineStep::Call {
                namespace: None,
                name: ns.clone(),
                args,
            });
        }
        return Ok(PipelineStep::Name(ns.clone()));
    }
    Err(format!(
        "unrecognized pipeline step near `{}`",
        tokens.first().map(|t| t.slice.as_str()).unwrap_or("?")
    ))
}

/// Named call args may reuse reserved words (`:route`, `:method`, `:query`, …).
fn arg_name_from_token(token: &Token) -> Option<String> {
    match token {
        Token::Ident(n) => Some(n.clone()),
        Token::Route => Some("route".into()),
        Token::Method => Some("method".into()),
        Token::Query => Some("query".into()),
        Token::Mutation => Some("mutation".into()),
        Token::State => Some("state".into()),
        Token::Slot => Some("slot".into()),
        Token::Emit => Some("emit".into()),
        Token::Has => Some("has".into()),
        Token::Class => Some("class".into()),
        Token::For => Some("for".into()),
        Token::When => Some("when".into()),
        Token::Else => Some("else".into()),
        Token::Await => Some("await".into()),
        _ => None,
    }
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
@version("0.2.0")
class Product {
    has Str $.name;
    has num64 $.price;
}
class ProductCard is component {
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
class ShopPage is component {
    method render() {
        ui::page(
            ProductCard(:product($.product), :on(add => on_add))
        )
    }
    method on_add(Product $p) {
        navigate("/");
    }
}
class ShopApp is app {
    route "/" => ShopPage;
    method serve() {
        ui::web(:root(ShopApp), :port(18088))
            ==> ui::terminal(:port(18023))
    }
}
"#;
        let program = parse(src).expect("parse");
        assert_eq!(program.contracts.len(), 1);
        assert_eq!(program.components.len(), 2);
        assert_eq!(program.apps.len(), 1);
        assert_eq!(program.apps[0].routes[0].component, "ShopPage");
    }

    #[test]
    fn rejects_legacy_view() {
        let src = r#"
class X is view {
    method render() { ui::page() }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(err.message.contains("is view"));
    }
}
