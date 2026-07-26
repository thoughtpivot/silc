//! Executable operation registry for Silc 0.2.0 runnable programs.

use crate::app::App;
use crate::component::Component;
use crate::model_catalog::{validate_model_id, DEFAULT_MODEL_ID};
use crate::module::{Module, ModuleKind};
use crate::pipeline::PipelineStep;
use crate::program::Program;
use crate::resource::{ActionDef, Resource, ResourceKind};
use crate::scrape_catalog::{
    parse_js_mode, parse_same_host, parse_site_depth, JsMode, DEFAULT_JS_MODE, DEFAULT_SITE_DEPTH,
};

/// Operations Silc 0.2.0 can lower and run.
pub const EXECUTABLE_OPS: &[(&str, &str)] = &[
    ("ui", "web"),
    ("ui", "terminal"),
    ("service", "http"),
    ("text", "score"),
    ("llm", "complete"),
    ("ipc", "publish"),
    ("store", "sqlite"),
    ("store", "commit"),
    ("resource", "list"),
    ("resource", "get"),
    ("resource", "create"),
    ("resource", "update"),
    ("resource", "delete"),
    ("scrape", "page"),
    ("scrape", "site"),
    ("scrape", "select"),
    ("scrape", "render"),
    ("scrape", "extract"),
];

const SUPPORTED_OPS_HELP: &str =
    "ui::web + ui::terminal with an `is app` root, resources/actions, optional text::score or llm::complete, scrape::*, or service::http API-only";

const SCRAPE_MIGRATE_HINT: &str =
    "use scrape::page / scrape::site / scrape::select instead of http::get / html::* (see ADR-006)";

pub const DEFAULT_TERMINAL_PORT: u16 = 18023;
pub const DEFAULT_API_PORT: u16 = 8080;
pub const DEFAULT_WEB_PORT: u16 = 18088;

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
pub enum ProcessorOp {
    None,
    Score,
    LlmComplete,
}

impl ProcessorOp {
    pub fn as_str(self) -> &'static str {
        match self {
            ProcessorOp::None => "none",
            ProcessorOp::Score => "text.score",
            ProcessorOp::LlmComplete => "llm.complete",
        }
    }

    pub fn needs_llm(self) -> bool {
        matches!(self, ProcessorOp::LlmComplete)
    }
}

/// Derived UI/runtime capabilities (never authored as a portal profile).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UiCapabilities {
    pub web: bool,
    pub terminal: bool,
    pub score: bool,
    pub llm: bool,
    pub history: bool,
    pub resources: bool,
    pub scrape: bool,
}

/// Derived scrape capabilities (ADR-006).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrapeCapabilities {
    pub page: bool,
    pub site: bool,
    pub select: bool,
    pub render: bool,
    pub extract: bool,
    pub js: JsMode,
    pub depth: u32,
    pub same_host: bool,
    pub link_css: Option<String>,
    pub extract_into: Option<String>,
    pub selects: Vec<ScrapeSelect>,
}

impl Default for ScrapeCapabilities {
    fn default() -> Self {
        Self {
            page: false,
            site: false,
            select: false,
            render: false,
            extract: false,
            js: JsMode::Auto,
            depth: DEFAULT_SITE_DEPTH,
            same_host: true,
            link_css: None,
            extract_into: None,
            selects: Vec::new(),
        }
    }
}

impl ScrapeCapabilities {
    pub fn active(&self) -> bool {
        self.page || self.site || self.select || self.render || self.extract
    }

    pub fn needs_crawl(&self) -> bool {
        self.active() && self.site
    }

    pub fn needs_browser(&self) -> bool {
        self.active() && (self.render || self.extract || self.js.needs_browser())
    }
}

/// One `scrape::select(:css(...), :as(...))` projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrapeSelect {
    pub css: String,
    pub as_field: Option<String>,
}

/// One declarative HTTP API route bound to a Contract (`service::http`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiRoute {
    pub port: u16,
    pub path: String,
    pub method: String,
    pub contract: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutableGraph {
    pub mode: ExecutionMode,
    pub processor_op: ProcessorOp,
    pub capabilities: UiCapabilities,
    pub scrape: ScrapeCapabilities,
    pub app: Option<App>,
    pub app_name: Option<String>,
    pub service: String,
    pub processor: String,
    pub sink: String,
    pub http_port: u16,
    pub http_route: String,
    pub sqlite_table: String,
    pub terminal_port: Option<u16>,
    pub api_routes: Vec<ApiRoute>,
    pub model_ref: Option<String>,
    pub actions: Vec<ActionDef>,
    pub resource_tables: Vec<(String, String)>, // (resource_name, table)
    pub root_component: Option<String>,
}

impl ExecutableGraph {
    pub fn has_ui(&self) -> bool {
        self.capabilities.web || self.capabilities.terminal
    }

    pub fn has_api(&self) -> bool {
        !self.api_routes.is_empty()
    }

    pub fn is_api_only(&self) -> bool {
        self.has_api() && !self.has_ui()
    }

    pub fn has_scrape(&self) -> bool {
        self.scrape.active()
    }

    pub fn is_scrape_only(&self) -> bool {
        self.has_scrape() && !self.has_ui() && !self.has_api()
    }

    pub fn api_port(&self) -> Option<u16> {
        self.api_routes.first().map(|r| r.port)
    }

    pub fn needs_llm(&self) -> bool {
        self.processor_op.needs_llm() || self.capabilities.llm
    }

    pub fn needs_scrape_browser(&self) -> bool {
        self.has_scrape() && self.scrape.needs_browser()
    }

    pub fn needs_scrape_crawl(&self) -> bool {
        self.has_scrape() && self.scrape.needs_crawl()
    }
}

pub fn is_executable_op(namespace: &str, name: &str) -> bool {
    EXECUTABLE_OPS
        .iter()
        .any(|(ns, n)| *ns == namespace && *n == name)
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
            | "resource"
            | "scrape"
            | "tensor"
            | "numpy"
            | "pandas"
            | "ws"
            | "sys"
            | "schema"
            | "payload"
            | "json"
    )
}

fn is_v1_exec_namespace(ns: &str) -> bool {
    matches!(
        ns,
        "ui" | "http"
            | "html"
            | "service"
            | "text"
            | "llm"
            | "ipc"
            | "store"
            | "resource"
            | "scrape"
    )
}

fn is_legacy_http_html_stub(ns: &str, name: &str) -> bool {
    matches!(
        (ns, name),
        ("http", "get")
            | ("html", "extract_body")
            | ("html", "extract")
            | ("html", "select")
    )
}

fn scan_calls(program: &Program, mut f: impl FnMut(&str, &str)) {
    for module in &program.modules {
        for method in &module.methods {
            for step in &method.pipeline.steps {
                if let PipelineStep::Call {
                    namespace: Some(ns),
                    name,
                    ..
                } = step
                {
                    f(ns, name);
                }
            }
        }
    }
    for app in &program.apps {
        if let Some(serve) = &app.serve {
            for step in &serve.pipeline.steps {
                if let PipelineStep::Call {
                    namespace: Some(ns),
                    name,
                    ..
                } = step
                {
                    f(ns, name);
                }
            }
        }
    }
    for resource in &program.resources {
        for method in &resource.methods {
            for step in &method.pipeline.steps {
                if let PipelineStep::Call {
                    namespace: Some(ns),
                    name,
                    ..
                } = step
                {
                    f(ns, name);
                }
            }
        }
    }
}

pub fn classify_program(program: &Program) -> Result<ExecutionMode, String> {
    let mut saw_exec = false;
    let mut saw_unknown_ns = false;
    let mut saw_legacy_http_html = false;
    scan_calls(program, |ns, name| {
        if is_executable_op(ns, name) {
            saw_exec = true;
        } else if is_known_namespace(ns) {
            saw_unknown_ns = true;
            if is_legacy_http_html_stub(ns, name) {
                saw_legacy_http_html = true;
            }
        }
    });

    if saw_exec && saw_unknown_ns {
        if saw_legacy_http_html {
            return Err(format!(
                "cannot mix stub-only and executable operations; {SCRAPE_MIGRATE_HINT}"
            ));
        }
        return Err(format!(
            "cannot mix stub-only and executable operations; supported runnable ops: {SUPPORTED_OPS_HELP}"
        ));
    }
    if saw_exec {
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
                                "operation `{ns}::{name}` is not executable in Silc 0.2.0"
                            ));
                        }
                    }
                }
            }
        }
        Ok(ExecutionMode::Runnable)
    } else {
        Ok(ExecutionMode::Stub)
    }
}

fn normalize_route(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"');
    if trimmed.is_empty() {
        "/".into()
    } else if trimmed.starts_with('/') {
        trimmed.into()
    } else {
        format!("/{trimmed}")
    }
}

fn normalize_ident(raw: &str) -> String {
    raw.trim().trim_matches('"').to_string()
}

fn normalize_http_method(raw: &str) -> Result<String, String> {
    let m = raw.trim().trim_matches('"').to_ascii_uppercase();
    match m.as_str() {
        "GET" | "POST" | "PUT" | "DELETE" | "PATCH" => Ok(m),
        _ => Err(format!("unsupported HTTP method `{raw}`")),
    }
}

fn resolve_model_ref(
    processors: &[&Module],
    model_from_op: Option<String>,
) -> Result<String, String> {
    if let Some(m) = model_from_op {
        validate_model_id(&m)?;
        return Ok(m);
    }
    for processor in processors {
        for field in &processor.fields {
            if field.name == "model_ref" {
                if let Some(default) = &field.default {
                    let id = normalize_ident(default);
                    validate_model_id(&id)?;
                    return Ok(id);
                }
            }
        }
    }
    Ok(DEFAULT_MODEL_ID.into())
}

fn derive_actions(resources: &[Resource]) -> Vec<ActionDef> {
    let mut actions = Vec::new();
    for resource in resources {
        let table = resource.table_name();
        for method in &resource.methods {
            let (http_method, path) = match (&method.kind, method.name.as_str()) {
                (ResourceKind::Query, "list" | "all") => ("GET".into(), format!("/api/{table}")),
                (ResourceKind::Query, name) => ("GET".into(), format!("/api/{table}/{name}")),
                (ResourceKind::Mutation, "create" | "add") => {
                    ("POST".into(), format!("/api/{table}"))
                }
                (ResourceKind::Mutation, "update") => ("PUT".into(), format!("/api/{table}/:id")),
                (ResourceKind::Mutation, "delete" | "remove") => {
                    ("DELETE".into(), format!("/api/{table}/:id"))
                }
                (ResourceKind::Mutation, name) => ("POST".into(), format!("/api/{table}/{name}")),
            };
            actions.push(ActionDef {
                id: format!("{}.{}", resource.name, method.name),
                resource: resource.name.clone(),
                method: method.name.clone(),
                http_method,
                path,
                kind: method.kind,
            });
        }
    }
    actions
}

fn component_uses_chat(components: &[Component], name: &str) -> bool {
    components
        .iter()
        .find(|c| c.name == name)
        .map(|c| c.render.contains_component("chat") || c.render.contains_component("chat_history"))
        .unwrap_or(false)
}

pub fn infer_graph(program: &Program) -> Result<Option<ExecutableGraph>, String> {
    match classify_program(program)? {
        ExecutionMode::Stub => return Ok(None),
        ExecutionMode::Runnable => {}
    }

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

    let mut saw_ui_web = false;
    let mut saw_terminal = false;
    let mut saw_score = false;
    let mut saw_llm = false;
    let mut saw_publish = false;
    let mut saw_sqlite = false;
    let mut saw_commit = false;
    let mut http_port = DEFAULT_WEB_PORT;
    let mut http_route = "/".to_string();
    let mut terminal_port: Option<u16> = None;
    let mut sqlite_table = "app_data".to_string();
    let mut model_from_op: Option<String> = None;
    let mut app_root: Option<String> = None;
    let mut api_routes: Vec<ApiRoute> = Vec::new();
    let mut last_contract: Option<String> = None;
    let mut scrape = ScrapeCapabilities {
        js: parse_js_mode(DEFAULT_JS_MODE).unwrap_or(JsMode::Auto),
        depth: DEFAULT_SITE_DEPTH,
        same_host: true,
        ..Default::default()
    };

    let mut scan_pipeline = |steps: &[PipelineStep]| -> Result<(), String> {
        for step in steps {
            match step {
                PipelineStep::Name(name) => {
                    if program.contracts.iter().any(|c| c.name == *name) {
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
                        for arg in args {
                            match arg.name.as_str() {
                                "port" => {
                                    http_port = arg.value.parse().unwrap_or(DEFAULT_WEB_PORT);
                                }
                                "route" => http_route = normalize_route(&arg.value),
                                "root" => app_root = Some(normalize_ident(&arg.value)),
                                _ => {}
                            }
                        }
                    }
                    ("ui", "terminal") => {
                        saw_terminal = true;
                        let mut port = DEFAULT_TERMINAL_PORT;
                        for arg in args {
                            if arg.name == "port" {
                                port = arg.value.parse().unwrap_or(DEFAULT_TERMINAL_PORT);
                            }
                        }
                        terminal_port = Some(port);
                    }
                    ("service", "http") => {
                        let contract = last_contract.clone().ok_or_else(|| {
                            "service::http requires a Contract on the left of `==>`".to_string()
                        })?;
                        let mut port = DEFAULT_API_PORT;
                        let mut path = "/".to_string();
                        let mut method = "GET".to_string();
                        for arg in args {
                            match arg.name.as_str() {
                                "port" => {
                                    port = arg.value.parse().unwrap_or(DEFAULT_API_PORT);
                                }
                                "route" => path = normalize_route(&arg.value),
                                "method" => method = normalize_http_method(&arg.value)?,
                                _ => {}
                            }
                        }
                        api_routes.push(ApiRoute {
                            port,
                            path,
                            method,
                            contract,
                        });
                    }
                    ("text", "score") => saw_score = true,
                    ("llm", "complete") => {
                        saw_llm = true;
                        for arg in args {
                            if arg.name == "model" {
                                model_from_op = Some(normalize_ident(&arg.value));
                            }
                        }
                    }
                    ("ipc", "publish") => saw_publish = true,
                    ("store", "sqlite") => {
                        saw_sqlite = true;
                        for arg in args {
                            if arg.name == "table" {
                                sqlite_table = normalize_ident(&arg.value);
                            }
                        }
                    }
                    ("store", "commit") => saw_commit = true,
                    ("scrape", "page") => {
                        scrape.page = true;
                        for arg in args {
                            match arg.name.as_str() {
                                "js" => scrape.js = parse_js_mode(&arg.value)?,
                                "timeout_ms" => {
                                    let _ = normalize_ident(&arg.value);
                                }
                                _ => {}
                            }
                        }
                    }
                    ("scrape", "site") => {
                        scrape.site = true;
                        scrape.page = true;
                        for arg in args {
                            match arg.name.as_str() {
                                "depth" => scrape.depth = parse_site_depth(&arg.value)?,
                                "same_host" => scrape.same_host = parse_same_host(&arg.value)?,
                                "link_css" => {
                                    scrape.link_css = Some(normalize_ident(&arg.value));
                                }
                                "js" => scrape.js = parse_js_mode(&arg.value)?,
                                _ => {}
                            }
                        }
                    }
                    ("scrape", "select") => {
                        scrape.select = true;
                        let mut css: Option<String> = None;
                        let mut as_field: Option<String> = None;
                        for arg in args {
                            match arg.name.as_str() {
                                "css" => css = Some(normalize_ident(&arg.value)),
                                "as" => as_field = Some(normalize_ident(&arg.value)),
                                _ => {}
                            }
                        }
                        let css = css.ok_or_else(|| {
                            "scrape::select requires :css(...)".to_string()
                        })?;
                        scrape.selects.push(ScrapeSelect { css, as_field });
                    }
                    ("scrape", "render") => {
                        scrape.render = true;
                        scrape.js = JsMode::True;
                    }
                    ("scrape", "extract") => {
                        scrape.extract = true;
                        let mut into: Option<String> = None;
                        for arg in args {
                            if arg.name == "into" {
                                into = Some(normalize_ident(&arg.value));
                            }
                        }
                        let into = into.ok_or_else(|| {
                            "scrape::extract requires :into(Contract)".to_string()
                        })?;
                        if !program.contracts.iter().any(|c| c.name == into) {
                            return Err(format!(
                                "scrape::extract references unknown contract `{into}`"
                            ));
                        }
                        scrape.extract_into = Some(into);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(())
    };

    for module in &program.modules {
        for method in &module.methods {
            scan_pipeline(&method.pipeline.steps)?;
        }
    }
    for app in &program.apps {
        if let Some(serve) = &app.serve {
            scan_pipeline(&serve.pipeline.steps)?;
        }
    }
    for resource in &program.resources {
        for method in &resource.methods {
            scan_pipeline(&method.pipeline.steps)?;
        }
    }

    let has_ui = saw_ui_web || saw_terminal;
    let has_api = !api_routes.is_empty();
    let has_scrape = scrape.active();
    if !has_ui && !has_api && !has_scrape {
        // Resource-only or processor pipelines without surface — still runnable if we have resources + app.
        if program.apps.is_empty() && program.resources.is_empty() {
            return Err(format!(
                "runnable program must declare ui::web/ui::terminal, an `is app`, scrape::*, or service::http; {SUPPORTED_OPS_HELP}"
            ));
        }
    }

    if has_scrape && saw_score {
        return Err("cannot mix scrape::* with text::score in one program".into());
    }
    if has_scrape && !scrape.select && !scrape.extract && !scrape.site && !scrape.page {
        return Err("scrape programs need scrape::page, scrape::site, or scrape::extract".into());
    }

    if has_api {
        let port = api_routes[0].port;
        for route in &api_routes {
            if route.port != port {
                return Err("all service::http routes must share one port".into());
            }
            if !program.contracts.iter().any(|c| c.name == route.contract) {
                return Err(format!(
                    "service::http route references unknown contract `{}`",
                    route.contract
                ));
            }
        }
    }

    let app = if let Some(root) = &app_root {
        program
            .apps
            .iter()
            .find(|a| a.name == *root)
            .cloned()
            .or_else(|| program.apps.first().cloned())
    } else {
        program.apps.first().cloned()
    };

    if has_ui || !program.apps.is_empty() {
        let app = app.clone().ok_or_else(|| {
            "UI programs require an `is app` class with routes and `method serve()`".to_string()
        })?;
        if app.routes.is_empty() {
            return Err(format!(
                "app `{}` must declare at least one route",
                app.name
            ));
        }
        if app.serve.is_none() {
            return Err(format!("app `{}` must declare `method serve()`", app.name));
        }
        if !saw_ui_web {
            return Err("UI apps must call `ui::web(:root(...))` in `serve()`".into());
        }
        if !saw_terminal {
            return Err(
                "UI apps must also call `ui::terminal(:port(...))` — dual-surface is required"
                    .into(),
            );
        }
        if let (Some(tp), hp) = (terminal_port, http_port) {
            if tp == hp {
                return Err("ui::terminal port must differ from ui::web port".into());
            }
        }
        for route in &app.routes {
            let known = program.components.iter().any(|c| c.name == route.component);
            if !known {
                return Err(format!(
                    "route `{}` references unknown component `{}`",
                    route.path, route.component
                ));
            }
        }
    }

    let processor_op = if saw_score && saw_llm {
        return Err("cannot mix text::score and llm::complete in one program".into());
    } else if saw_score {
        ProcessorOp::Score
    } else if saw_llm {
        ProcessorOp::LlmComplete
    } else {
        ProcessorOp::None
    };

    let model_ref = if processor_op.needs_llm() {
        Some(resolve_model_ref(&processors, model_from_op)?)
    } else {
        None
    };

    // Optional classic processor/sink — not required when resources handle persistence.
    if processor_op != ProcessorOp::None {
        if processors.len() != 1 {
            return Err(
                "programs using text::score or llm::complete need exactly one processor".into(),
            );
        }
        if sinks.len() != 1 {
            return Err("programs using text::score or llm::complete need exactly one sink".into());
        }
        if !(saw_publish && saw_sqlite && saw_commit) {
            return Err(
                "processor pipelines require ipc::publish ==> store::sqlite ==> store::commit"
                    .into(),
            );
        }
        let sink = sinks[0];
        let has_sqlite_trait = sink.traits.iter().any(|t| {
            t.name.eq_ignore_ascii_case("storage")
                && t.value.to_ascii_lowercase().contains("sqlite")
        });
        if !has_sqlite_trait {
            return Err(format!(
                "sink `{}` must declare `is storage(SQLite)`",
                sink.name
            ));
        }
    }

    if has_ui && services.len() > 1 {
        return Err("UI programs may declare at most one service module".into());
    }
    if has_api && !has_ui && (processors.len() + sinks.len()) > 0 {
        return Err("API-only programs cannot declare processor or sink modules".into());
    }

    let actions = derive_actions(&program.resources);
    let resource_tables = program
        .resources
        .iter()
        .map(|r| (r.name.clone(), r.table_name()))
        .collect();

    let root_component = app
        .as_ref()
        .and_then(|a| a.default_route().map(|r| r.component.clone()));

    let uses_chat = root_component
        .as_ref()
        .map(|name| component_uses_chat(&program.components, name))
        .unwrap_or(false);

    if uses_chat && !processor_op.needs_llm() && !program.resources.iter().any(|r| {
        r.methods
            .iter()
            .any(|m| m.pipeline.steps.iter().any(|s| matches!(s, PipelineStep::Call { namespace: Some(ns), name, .. } if ns == "llm" && name == "complete")))
    }) {
        // Chat UI is allowed when an LLM resource/processor exists; soft check via processor_op.
        if processor_op == ProcessorOp::None {
            // Allow chat UI without processor if a resource mutation invokes llm — already scanned.
        }
    }

    let capabilities = UiCapabilities {
        web: saw_ui_web,
        terminal: saw_terminal,
        score: saw_score,
        llm: saw_llm || uses_chat,
        history: uses_chat,
        resources: !program.resources.is_empty(),
        scrape: has_scrape,
    };

    let service_name = services
        .first()
        .map(|m| m.name.clone())
        .or_else(|| app.as_ref().map(|a| a.name.clone()))
        .unwrap_or_default();

    Ok(Some(ExecutableGraph {
        mode: ExecutionMode::Runnable,
        processor_op,
        capabilities,
        scrape,
        app_name: app.as_ref().map(|a| a.name.clone()),
        app,
        service: service_name,
        processor: processors
            .first()
            .map(|m| m.name.clone())
            .unwrap_or_default(),
        sink: sinks.first().map(|m| m.name.clone()).unwrap_or_default(),
        http_port: if has_ui {
            http_port
        } else {
            api_routes
                .first()
                .map(|r| r.port)
                .unwrap_or(DEFAULT_API_PORT)
        },
        http_route,
        sqlite_table,
        terminal_port,
        api_routes,
        model_ref,
        actions,
        resource_tables,
        root_component,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Span;

    #[test]
    fn executable_ops_include_resources() {
        assert!(is_executable_op("resource", "list"));
        assert!(is_executable_op("ui", "web"));
        assert!(is_executable_op("scrape", "page"));
        assert!(is_executable_op("scrape", "site"));
        assert!(is_executable_op("scrape", "select"));
        assert!(is_executable_op("scrape", "render"));
        assert!(is_executable_op("scrape", "extract"));
        assert!(!is_executable_op("tensor", "infer"));
    }

    #[test]
    fn derive_actions_builds_crud_paths() {
        let resource = Resource {
            name: "Products".into(),
            methods: vec![
                crate::resource::ResourceMethod {
                    kind: ResourceKind::Query,
                    name: "list".into(),
                    params: vec![],
                    return_ty: None,
                    pipeline: crate::pipeline::Pipeline { steps: vec![] },
                    span: Span::default(),
                },
                crate::resource::ResourceMethod {
                    kind: ResourceKind::Mutation,
                    name: "create".into(),
                    params: vec![],
                    return_ty: None,
                    pipeline: crate::pipeline::Pipeline { steps: vec![] },
                    span: Span::default(),
                },
            ],
            contract: Some("Product".into()),
            table: Some("products".into()),
            span: Span::default(),
        };
        let actions = derive_actions(&[resource]);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].path, "/api/products");
        assert_eq!(actions[0].http_method, "GET");
        assert_eq!(actions[1].http_method, "POST");
    }
}
