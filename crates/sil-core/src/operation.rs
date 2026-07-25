//! Executable operation registry for Silc v1 runnable programs.

use crate::model_catalog::{validate_model_id, DEFAULT_MODEL_ID};
use crate::module::{Module, ModuleKind};
use crate::pipeline::PipelineStep;
use crate::program::Program;
use crate::ui::UiView;

/// Operations that Silc can actually lower and run in v1.
///
/// `ui::web` is the canonical web UI op. `html::form` and `http::serve` remain
/// executable compatibility aliases that lower to the same web profile.
/// `service::http` is the canonical declarative HTTP API op (Go/Gin substrate).
/// `llm::complete` is the local Llama completion op (Python / llama.cpp).
pub const EXECUTABLE_OPS: &[(&str, &str)] = &[
    ("ui", "web"),
    ("ui", "terminal"),
    ("html", "form"),
    ("http", "serve"),
    ("service", "http"),
    ("text", "score"),
    ("llm", "complete"),
    ("ipc", "publish"),
    ("store", "sqlite"),
    ("store", "commit"),
];

const SUPPORTED_OPS_HELP: &str =
    "service::http, or ui::web (or html::form + http::serve) with optional ui::terminal, plus either text::score or llm::complete, with ipc::publish, store::sqlite, store::commit";

/// Default TCP port for `ui::terminal` (telnet-friendly; mnemonic for historic 23).
pub const DEFAULT_TERMINAL_PORT: u16 = 18023;

/// Default TCP port for `service::http` when `:port` is omitted.
pub const DEFAULT_API_PORT: u16 = 8080;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Stub,
    Runnable,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Stub => write!(f, "stub"),
            ExecutionMode::Runnable => write!(f, "runnable"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiSurface {
    /// Canonical `ui::web(...)`.
    Web,
    /// Legacy `html::form() ==> http::serve(...)`.
    LegacyHtmlHttp,
}

impl UiSurface {
    pub fn as_str(self) -> &'static str {
        match self {
            UiSurface::Web => "web",
            UiSurface::LegacyHtmlHttp => "legacy_html_http",
        }
    }

    pub fn substrate(self) -> &'static str {
        "react"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalKind {
    Feedback,
    LlmChat,
    None,
}

impl PortalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PortalKind::Feedback => "feedback",
            PortalKind::LlmChat => "llm_chat",
            PortalKind::None => "none",
        }
    }

    pub fn needs_llm(self) -> bool {
        self == PortalKind::LlmChat
    }
}

/// One declarative HTTP API route bound to a Contract (`service::http`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiRoute {
    pub port: u16,
    pub path: String,
    pub method: String,
    pub contract: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableGraph {
    pub mode: ExecutionMode,
    pub portal_kind: PortalKind,
    pub service: String,
    /// Empty for API-only programs (no processor module).
    pub processor: String,
    /// Empty for API-only programs (no sink module).
    pub sink: String,
    pub http_port: u16,
    pub http_route: String,
    pub sqlite_table: String,
    /// `None` for API-only programs (no browser UI).
    pub ui_surface: Option<UiSurface>,
    /// When set, Bun also listens for line-oriented telnet/TCP sessions.
    pub terminal_port: Option<u16>,
    /// Declarative Go/Gin HTTP API routes (`service::http`).
    pub api_routes: Vec<ApiRoute>,
    /// Catalog model id for `PortalKind::LlmChat`.
    pub model_ref: Option<String>,
    /// Optional author-declared view referenced by `ui::web(:view(...))`.
    pub ui_view: Option<UiView>,
    /// Contract bound to the left of `ui::web` (used for `:field` validation).
    pub ui_contract: Option<String>,
}

impl ExecutableGraph {
    pub fn has_ui(&self) -> bool {
        self.ui_surface.is_some()
    }

    pub fn has_api(&self) -> bool {
        !self.api_routes.is_empty()
    }

    pub fn is_api_only(&self) -> bool {
        self.has_api() && !self.has_ui()
    }

    /// Primary API listen port (first `service::http` route).
    pub fn api_port(&self) -> Option<u16> {
        self.api_routes.first().map(|r| r.port)
    }
}

pub fn is_executable_op(namespace: &str, name: &str) -> bool {
    EXECUTABLE_OPS
        .iter()
        .any(|(ns, op)| *ns == namespace && *op == name)
}

fn is_known_namespace(ns: &str) -> bool {
    matches!(
        ns,
        "ui" | "http"
            | "html"
            | "service"
            | "text"
            | "llm"
            | "ipc"
            | "store"
            | "tensor"
            | "numpy"
            | "pandas"
            | "ws"
            | "sys"
            | "schema"
            | "payload"
    )
}

fn is_v1_exec_namespace(ns: &str) -> bool {
    matches!(
        ns,
        "ui" | "http" | "html" | "service" | "text" | "llm" | "ipc" | "store"
    )
}

fn normalize_model_token(raw: &str) -> String {
    raw.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_start_matches("$.")
        .to_string()
}

fn resolve_model_ref(
    processors: &[&Module],
    model_from_op: Option<String>,
) -> Result<String, String> {
    let field_default = || {
        processors.iter().find_map(|module| {
            module
                .fields
                .iter()
                .find(|field| field.name == "model_ref")
                .and_then(|field| field.default.as_deref())
                .map(normalize_model_token)
        })
    };
    let id = match model_from_op {
        Some(raw) if raw.contains("model_ref") => field_default(),
        Some(raw) => Some(normalize_model_token(&raw)),
        None => field_default(),
    }
    .unwrap_or_else(|| DEFAULT_MODEL_ID.to_string());
    validate_model_id(&id)?;
    Ok(id)
}

pub fn classify_program(program: &Program) -> Result<ExecutionMode, String> {
    let mut saw_exec = false;
    let mut saw_unknown_ns = false;
    for module in &program.modules {
        for method in &module.methods {
            for step in &method.pipeline.steps {
                if let PipelineStep::Call {
                    namespace: Some(ns),
                    name,
                    ..
                } = step
                {
                    if is_executable_op(ns, name) {
                        saw_exec = true;
                    } else if is_known_namespace(ns) {
                        // Known namespace but not in the executable v1 set → stub program.
                        saw_unknown_ns = true;
                    }
                }
            }
        }
    }
    if saw_exec && !saw_unknown_ns {
        // Ensure we didn't mix stub-only ops with executable ones.
        for module in &program.modules {
            for method in &module.methods {
                for step in &method.pipeline.steps {
                    if let PipelineStep::Call {
                        namespace: Some(ns),
                        name,
                        ..
                    } = step
                    {
                        if is_v1_exec_namespace(ns) && !is_executable_op(ns, name) {
                            return Err(format!(
                                "operation `{ns}::{name}` is not executable in Silc v1 (supported: {SUPPORTED_OPS_HELP})"
                            ));
                        }
                    }
                }
            }
        }
        Ok(ExecutionMode::Runnable)
    } else if saw_exec && saw_unknown_ns {
        Err("cannot mix executable v1 operations with stub-only operations in one program".into())
    } else {
        Ok(ExecutionMode::Stub)
    }
}

pub fn infer_graph(program: &Program) -> Result<Option<ExecutableGraph>, String> {
    match classify_program(program)? {
        ExecutionMode::Stub => Ok(None),
        ExecutionMode::Runnable => {
            let services: Vec<&Module> = program
                .modules
                .iter()
                .filter(|m| m.kind == ModuleKind::Service)
                .collect();
            let processors: Vec<&Module> = program
                .modules
                .iter()
                .filter(|m| m.kind == ModuleKind::Processor)
                .collect();
            let sinks: Vec<&Module> = program
                .modules
                .iter()
                .filter(|m| m.kind == ModuleKind::Sink)
                .collect();
            if services.len() != 1 {
                return Err("runnable Silc v1 programs require exactly one service module".into());
            }

            let mut http_port = 8080u16;
            let mut http_route = String::from("/");
            let mut terminal_port: Option<u16> = None;
            let mut saw_ui_web = false;
            let mut saw_terminal = false;
            let mut saw_form = false;
            let mut saw_serve = false;
            let mut saw_score = false;
            let mut saw_llm_complete = false;
            let mut model_from_op: Option<String> = None;
            let mut saw_publish = false;
            let mut saw_sqlite = false;
            let mut saw_commit = false;
            let mut sqlite_table = String::from("feedback");
            let mut api_routes: Vec<ApiRoute> = Vec::new();
            let mut view_name: Option<String> = None;
            let mut ui_contract: Option<String> = None;
            let contract_names: Vec<&str> =
                program.contracts.iter().map(|c| c.name.as_str()).collect();

            for module in &program.modules {
                for method in &module.methods {
                    let mut last_contract: Option<String> = None;
                    for step in &method.pipeline.steps {
                        match step {
                            PipelineStep::Name(name) => {
                                if contract_names.contains(&name.as_str()) {
                                    last_contract = Some(name.clone());
                                }
                            }
                            PipelineStep::Call {
                                namespace: Some(ns),
                                name,
                                args,
                            } => match (ns.as_str(), name.as_str()) {
                                ("ui", "web") => {
                                    saw_ui_web = true;
                                    if let Some(port) = args.iter().find(|a| a.name == "port") {
                                        http_port = port.value.parse().map_err(|_| {
                                            format!("invalid :port({})", port.value)
                                        })?;
                                    }
                                    if let Some(route) = args.iter().find(|a| a.name == "route") {
                                        http_route = normalize_route(&route.value);
                                    }
                                    if let Some(view) = args.iter().find(|a| a.name == "view") {
                                        let name = normalize_ident(&view.value);
                                        if name.is_empty() {
                                            return Err(
                                                "ui::web :view() must name a view class".into()
                                            );
                                        }
                                        view_name = Some(name);
                                    }
                                    if ui_contract.is_none() {
                                        ui_contract = last_contract.clone();
                                    }
                                }
                                ("ui", "terminal") => {
                                    saw_terminal = true;
                                    let mut port = DEFAULT_TERMINAL_PORT;
                                    if let Some(p) = args.iter().find(|a| a.name == "port") {
                                        port = p
                                            .value
                                            .parse()
                                            .map_err(|_| format!("invalid :port({})", p.value))?;
                                    }
                                    terminal_port = Some(port);
                                }
                                ("html", "form") => saw_form = true,
                                ("http", "serve") => {
                                    saw_serve = true;
                                    if let Some(port) = args.iter().find(|a| a.name == "port") {
                                        http_port = port.value.parse().map_err(|_| {
                                            format!("invalid :port({})", port.value)
                                        })?;
                                    }
                                    if let Some(route) = args.iter().find(|a| a.name == "route") {
                                        http_route = normalize_route(&route.value);
                                    }
                                }
                                ("service", "http") => {
                                    let contract = last_contract.clone().ok_or_else(|| {
                                        "service::http requires a Contract on the left of `==>`"
                                            .to_string()
                                    })?;
                                    let mut port = DEFAULT_API_PORT;
                                    if let Some(p) = args.iter().find(|a| a.name == "port") {
                                        port = p
                                            .value
                                            .parse()
                                            .map_err(|_| format!("invalid :port({})", p.value))?;
                                    }
                                    let path = args
                                        .iter()
                                        .find(|a| a.name == "route")
                                        .map(|a| normalize_route(&a.value))
                                        .unwrap_or_else(|| "/".into());
                                    let method = args
                                        .iter()
                                        .find(|a| a.name == "method")
                                        .map(|a| normalize_http_method(&a.value))
                                        .transpose()?
                                        .unwrap_or_else(|| "GET".into());
                                    api_routes.push(ApiRoute {
                                        port,
                                        path,
                                        method,
                                        contract,
                                    });
                                }
                                ("text", "score") => saw_score = true,
                                ("llm", "complete") => {
                                    saw_llm_complete = true;
                                    if let Some(model) = args.iter().find(|arg| arg.name == "model")
                                    {
                                        model_from_op = Some(model.value.clone());
                                    }
                                }
                                ("ipc", "publish") => saw_publish = true,
                                ("store", "sqlite") => {
                                    saw_sqlite = true;
                                    if let Some(table) = args.iter().find(|a| a.name == "table") {
                                        sqlite_table = table.value.clone();
                                    }
                                }
                                ("store", "commit") => saw_commit = true,
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                }
            }

            let has_ui = saw_ui_web || (saw_form && saw_serve);
            let has_api = !api_routes.is_empty();

            if !has_ui && !has_api {
                if saw_terminal {
                    return Err(
                        "runnable feedback portal requires `ui::web` (or html::form + http::serve); add `ui::terminal` alongside it for telnet"
                            .into(),
                    );
                }
                return Err(format!("runnable program requires {SUPPORTED_OPS_HELP}"));
            }

            if has_api {
                let ports: Vec<u16> = api_routes.iter().map(|r| r.port).collect();
                let first = ports[0];
                if ports.iter().any(|p| *p != first) {
                    return Err(
                        "service::http v1 requires all routes to share one :port (one Go/Gin process)"
                            .into(),
                    );
                }
                for route in &api_routes {
                    if !matches!(route.method.as_str(), "GET" | "POST") {
                        return Err(format!(
                            "service::http :method({}) is not supported in v1 (use GET or POST)",
                            route.method
                        ));
                    }
                    if !contract_names.contains(&route.contract.as_str()) {
                        return Err(format!(
                            "service::http references unknown Contract `{}`",
                            route.contract
                        ));
                    }
                }
            }

            let mut portal_kind = PortalKind::None;
            let mut model_ref = None;
            let ui_surface = if has_ui {
                if processors.len() != 1 || sinks.len() != 1 {
                    return Err(
                        "runnable UI programs require exactly one service, one processor, and one sink"
                            .into(),
                    );
                }
                let surface = if saw_ui_web {
                    if saw_form || saw_serve {
                        return Err(
                            "use either `ui::web` or the legacy `html::form` + `http::serve` alias, not both"
                                .into(),
                        );
                    }
                    UiSurface::Web
                } else {
                    UiSurface::LegacyHtmlHttp
                };
                if saw_score && saw_llm_complete {
                    return Err(
                        "cannot mix `text::score` and `llm::complete` in one runnable program"
                            .into(),
                    );
                }
                if saw_llm_complete {
                    if !(saw_publish && saw_sqlite && saw_commit) {
                        return Err(format!(
                            "runnable LLM chat portal requires {SUPPORTED_OPS_HELP}"
                        ));
                    }
                    portal_kind = PortalKind::LlmChat;
                    model_ref = Some(resolve_model_ref(&processors, model_from_op)?);
                    if sqlite_table == "feedback" {
                        sqlite_table = "chat_turns".into();
                    }
                } else if saw_score {
                    if !(saw_publish && saw_sqlite && saw_commit) {
                        return Err(format!(
                            "runnable feedback portal requires {SUPPORTED_OPS_HELP}"
                        ));
                    }
                    portal_kind = PortalKind::Feedback;
                } else {
                    return Err(format!(
                        "runnable UI portal requires text::score or llm::complete ({SUPPORTED_OPS_HELP})"
                    ));
                }
                if let Some(tp) = terminal_port {
                    if tp == http_port {
                        return Err(format!(
                            "ui::terminal :port({tp}) must differ from ui::web :port({http_port})"
                        ));
                    }
                }
                if has_api {
                    if let Some(api_port) = api_routes.first().map(|r| r.port) {
                        if api_port == http_port {
                            return Err(format!(
                                "service::http :port({api_port}) must differ from ui::web :port({http_port})"
                            ));
                        }
                        if terminal_port == Some(api_port) {
                            return Err(format!(
                                "service::http :port({api_port}) must differ from ui::terminal :port({api_port})"
                            ));
                        }
                    }
                }
                let storage_ok = sinks[0]
                    .traits
                    .iter()
                    .any(|t| t.name == "storage" && t.value.eq_ignore_ascii_case("SQLite"));
                if !storage_ok {
                    return Err("runnable sink must declare `is storage(SQLite)`".into());
                }
                Some(surface)
            } else {
                // API-only: service module alone is enough.
                if !processors.is_empty() || !sinks.is_empty() {
                    return Err(
                        "API-only `service::http` programs must not declare processor or sink modules (add ui::web for the feedback-portal shape)"
                            .into(),
                    );
                }
                if view_name.is_some() {
                    return Err(
                        "ui::web(:view(...)) requires a runnable UI portal (service + processor + sink)"
                            .into(),
                    );
                }
                None
            };

            let ui_view = if let Some(name) = &view_name {
                let view = program
                    .views
                    .iter()
                    .find(|view| view.name == *name)
                    .cloned()
                    .ok_or_else(|| format!("ui::web references unknown view `{name}`"))?;
                if ui_contract.is_none() {
                    return Err(
                        "ui::web(:view(...)) requires a Contract on the left of `==>`".into(),
                    );
                }
                Some(view)
            } else {
                None
            };

            let api_http_port = api_routes
                .first()
                .map(|r| r.port)
                .unwrap_or(DEFAULT_API_PORT);

            Ok(Some(ExecutableGraph {
                mode: ExecutionMode::Runnable,
                portal_kind,
                service: services[0].name.clone(),
                processor: processors
                    .first()
                    .map(|m| m.name.clone())
                    .unwrap_or_default(),
                sink: sinks.first().map(|m| m.name.clone()).unwrap_or_default(),
                http_port: if has_ui { http_port } else { api_http_port },
                http_route,
                sqlite_table,
                ui_surface,
                terminal_port,
                api_routes,
                model_ref,
                ui_view,
                ui_contract,
            }))
        }
    }
}

fn normalize_route(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        "/".into()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn normalize_ident(raw: &str) -> String {
    raw.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_start_matches("$.")
        .to_string()
}

fn normalize_http_method(raw: &str) -> Result<String, String> {
    let trimmed = raw
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_uppercase();
    if trimmed.is_empty() {
        return Err("service::http :method() must not be empty".into());
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::TraitArg;
    use crate::contract::{Contract, Field};
    use crate::module::{Method, Module, ModuleKind, Param};
    use crate::pipeline::Pipeline;
    use crate::types::{Span, TypeExpr};

    fn feedback_modules(
        service_steps: Vec<PipelineStep>,
    ) -> (Vec<crate::contract::Contract>, Vec<Module>) {
        (
            vec![Contract {
                name: "FeedbackRecord".into(),
                fields: vec![
                    Field {
                        name: "id".into(),
                        ty: TypeExpr::Named("UUID".into()),
                        default: None,
                    },
                    Field {
                        name: "author".into(),
                        ty: TypeExpr::Named("Str".into()),
                        default: None,
                    },
                    Field {
                        name: "text".into(),
                        ty: TypeExpr::Named("Str".into()),
                        default: None,
                    },
                    Field {
                        name: "summary".into(),
                        ty: TypeExpr::Named("Str".into()),
                        default: None,
                    },
                    Field {
                        name: "score".into(),
                        ty: TypeExpr::Named("num64".into()),
                        default: None,
                    },
                ],
                span: Span::default(),
            }],
            vec![
                Module {
                    name: "WebPortal".into(),
                    kind: ModuleKind::Service,
                    traits: vec![],
                    fields: vec![],
                    methods: vec![Method {
                        name: "listen".into(),
                        params: vec![Param {
                            name: "port".into(),
                            ty: None,
                            named: true,
                            default: Some("8080".into()),
                        }],
                        pipeline: Pipeline {
                            steps: service_steps,
                        },
                    }],
                    span: Span::default(),
                },
                Module {
                    name: "TextAnalyzer".into(),
                    kind: ModuleKind::Processor,
                    traits: vec![],
                    fields: vec![],
                    methods: vec![Method {
                        name: "analyze".into(),
                        params: vec![],
                        pipeline: Pipeline {
                            steps: vec![PipelineStep::Call {
                                namespace: Some("text".into()),
                                name: "score".into(),
                                args: vec![],
                            }],
                        },
                    }],
                    span: Span::default(),
                },
                Module {
                    name: "FeedbackDb".into(),
                    kind: ModuleKind::Sink,
                    traits: vec![
                        TraitArg {
                            name: "latency".into(),
                            value: "5ms".into(),
                        },
                        TraitArg {
                            name: "storage".into(),
                            value: "SQLite".into(),
                        },
                    ],
                    fields: vec![],
                    methods: vec![Method {
                        name: "persist".into(),
                        params: vec![],
                        pipeline: Pipeline {
                            steps: vec![
                                PipelineStep::Call {
                                    namespace: Some("ipc".into()),
                                    name: "publish".into(),
                                    args: vec![],
                                },
                                PipelineStep::Call {
                                    namespace: Some("store".into()),
                                    name: "sqlite".into(),
                                    args: vec![TraitArg {
                                        name: "table".into(),
                                        value: "feedback".into(),
                                    }],
                                },
                                PipelineStep::Call {
                                    namespace: Some("store".into()),
                                    name: "commit".into(),
                                    args: vec![],
                                },
                            ],
                        },
                    }],
                    span: Span::default(),
                },
            ],
        )
    }

    fn feedback_like_legacy() -> Program {
        let (contracts, modules) = feedback_modules(vec![
            PipelineStep::Call {
                namespace: Some("html".into()),
                name: "form".into(),
                args: vec![],
            },
            PipelineStep::Call {
                namespace: Some("http".into()),
                name: "serve".into(),
                args: vec![TraitArg {
                    name: "port".into(),
                    value: "18080".into(),
                }],
            },
        ]);
        Program {
            version: Some("1.0".into()),
            subsets: vec![],
            contracts,
            modules,
            views: vec![],
        }
    }

    fn feedback_like_ui_web() -> Program {
        let (contracts, modules) = feedback_modules(vec![PipelineStep::Call {
            namespace: Some("ui".into()),
            name: "web".into(),
            args: vec![
                TraitArg {
                    name: "port".into(),
                    value: "18080".into(),
                },
                TraitArg {
                    name: "route".into(),
                    value: "/".into(),
                },
            ],
        }]);
        Program {
            version: Some("1.0".into()),
            subsets: vec![],
            contracts,
            modules,
            views: vec![],
        }
    }

    #[test]
    fn classifies_runnable_legacy_feedback() {
        let program = feedback_like_legacy();
        assert_eq!(classify_program(&program).unwrap(), ExecutionMode::Runnable);
        let graph = infer_graph(&program).unwrap().unwrap();
        assert_eq!(graph.http_port, 18080);
        assert_eq!(graph.sqlite_table, "feedback");
        assert_eq!(graph.ui_surface, Some(UiSurface::LegacyHtmlHttp));
        assert_eq!(graph.portal_kind, PortalKind::Feedback);
        assert!(graph.api_routes.is_empty());
    }

    #[test]
    fn classifies_runnable_ui_web() {
        let program = feedback_like_ui_web();
        assert_eq!(classify_program(&program).unwrap(), ExecutionMode::Runnable);
        let graph = infer_graph(&program).unwrap().unwrap();
        assert_eq!(graph.http_port, 18080);
        assert_eq!(graph.http_route, "/");
        assert_eq!(graph.ui_surface, Some(UiSurface::Web));
        assert_eq!(graph.ui_surface.unwrap().substrate(), "react");
        assert_eq!(graph.portal_kind, PortalKind::Feedback);
        assert!(!graph.has_api());
    }

    #[test]
    fn classifies_runnable_llm_chat_and_validates_model() {
        let mut program = feedback_like_ui_web();
        program.modules[1].fields.push(Field {
            name: "model_ref".into(),
            ty: TypeExpr::Named("Str".into()),
            default: Some("\"llama3.2-1b\"".into()),
        });
        program.modules[1].methods[0].pipeline.steps = vec![PipelineStep::Call {
            namespace: Some("llm".into()),
            name: "complete".into(),
            args: vec![TraitArg {
                name: "model".into(),
                value: "$.model_ref".into(),
            }],
        }];
        let graph = infer_graph(&program).unwrap().unwrap();
        assert_eq!(graph.portal_kind, PortalKind::LlmChat);
        assert_eq!(graph.model_ref.as_deref(), Some("llama3.2-1b"));
    }

    #[test]
    fn rejects_unknown_llm_model() {
        let mut program = feedback_like_ui_web();
        program.modules[1].methods[0].pipeline.steps = vec![PipelineStep::Call {
            namespace: Some("llm".into()),
            name: "complete".into(),
            args: vec![TraitArg {
                name: "model".into(),
                value: "not-a-model".into(),
            }],
        }];
        assert!(infer_graph(&program).unwrap_err().contains("unknown model"));
    }

    #[test]
    fn ui_terminal_is_executable_but_requires_runnable_graph() {
        let program = Program {
            version: Some("1.0".into()),
            subsets: vec![],
            contracts: vec![],
            modules: vec![Module {
                name: "TermUi".into(),
                kind: ModuleKind::Service,
                traits: vec![],
                fields: vec![],
                methods: vec![Method {
                    name: "run".into(),
                    params: vec![],
                    pipeline: Pipeline {
                        steps: vec![PipelineStep::Call {
                            namespace: Some("ui".into()),
                            name: "terminal".into(),
                            args: vec![],
                        }],
                    },
                }],
                span: Span::default(),
            }],
            views: vec![],
        };
        assert_eq!(classify_program(&program).unwrap(), ExecutionMode::Runnable);
        assert!(infer_graph(&program).unwrap_err().contains("ui::web"));
    }

    #[test]
    fn supports_ui_web_and_terminal_together() {
        let (contracts, modules) = feedback_modules(vec![
            PipelineStep::Call {
                namespace: Some("ui".into()),
                name: "web".into(),
                args: vec![TraitArg {
                    name: "port".into(),
                    value: "18080".into(),
                }],
            },
            PipelineStep::Call {
                namespace: Some("ui".into()),
                name: "terminal".into(),
                args: vec![TraitArg {
                    name: "port".into(),
                    value: "18023".into(),
                }],
            },
        ]);
        let program = Program {
            version: Some("1.0".into()),
            subsets: vec![],
            contracts,
            modules,
            views: vec![],
        };
        assert_eq!(classify_program(&program).unwrap(), ExecutionMode::Runnable);
        let graph = infer_graph(&program).unwrap().unwrap();
        assert_eq!(graph.terminal_port, Some(18023));
        assert_eq!(graph.http_port, 18080);
    }

    #[test]
    fn classifies_runnable_service_http_api_only() {
        let program = Program {
            version: Some("1.0".into()),
            subsets: vec![],
            contracts: vec![Contract {
                name: "FeedbackRecord".into(),
                fields: vec![Field {
                    name: "author".into(),
                    ty: TypeExpr::Named("Str".into()),
                    default: None,
                }],
                span: Span::default(),
            }],
            modules: vec![Module {
                name: "FeedbackApi".into(),
                kind: ModuleKind::Service,
                traits: vec![],
                fields: vec![],
                methods: vec![
                    Method {
                        name: "list".into(),
                        params: vec![],
                        pipeline: Pipeline {
                            steps: vec![
                                PipelineStep::Name("FeedbackRecord".into()),
                                PipelineStep::Call {
                                    namespace: Some("service".into()),
                                    name: "http".into(),
                                    args: vec![
                                        TraitArg {
                                            name: "port".into(),
                                            value: "18081".into(),
                                        },
                                        TraitArg {
                                            name: "route".into(),
                                            value: "/api/feedback".into(),
                                        },
                                        TraitArg {
                                            name: "method".into(),
                                            value: "GET".into(),
                                        },
                                    ],
                                },
                            ],
                        },
                    },
                    Method {
                        name: "create".into(),
                        params: vec![],
                        pipeline: Pipeline {
                            steps: vec![
                                PipelineStep::Name("FeedbackRecord".into()),
                                PipelineStep::Call {
                                    namespace: Some("service".into()),
                                    name: "http".into(),
                                    args: vec![
                                        TraitArg {
                                            name: "port".into(),
                                            value: "18081".into(),
                                        },
                                        TraitArg {
                                            name: "route".into(),
                                            value: "/api/feedback".into(),
                                        },
                                        TraitArg {
                                            name: "method".into(),
                                            value: "POST".into(),
                                        },
                                    ],
                                },
                            ],
                        },
                    },
                ],
                span: Span::default(),
            }],
            views: vec![],
        };
        assert_eq!(classify_program(&program).unwrap(), ExecutionMode::Runnable);
        let graph = infer_graph(&program).unwrap().unwrap();
        assert!(graph.is_api_only());
        assert_eq!(graph.api_routes.len(), 2);
        assert_eq!(graph.api_port(), Some(18081));
        assert_eq!(graph.http_port, 18081);
        assert_eq!(graph.api_routes[0].method, "GET");
        assert_eq!(graph.api_routes[1].method, "POST");
        assert_eq!(graph.api_routes[0].contract, "FeedbackRecord");
        assert!(graph.processor.is_empty());
        assert!(graph.sink.is_empty());
        assert!(graph.ui_surface.is_none());
    }
}
