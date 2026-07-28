//! Executable operation registry for Silc 0.4.0 runnable programs.

use crate::app::App;
use crate::component::Component;
use crate::model_catalog::{
    validate_embedding_model_id, validate_model_id, DEFAULT_EMBEDDING_MODEL_ID, DEFAULT_MODEL_ID,
    DEFAULT_TENSOR_INPUT_FIELD, DEFAULT_TENSOR_OUTPUT_FIELD, MINILM_EMBEDDING_DIM,
};
use crate::module::{Module, ModuleKind};
use crate::pipeline::PipelineStep;
use crate::program::Program;
use crate::resource::{sink_table_for_contract, ActionDef, Resource, ResourceKind};
use crate::scrape_catalog::{
    parse_js_mode, parse_same_host, parse_site_depth, JsMode, DEFAULT_JS_MODE, DEFAULT_SITE_DEPTH,
};
use crate::types::TypeExpr;

/// Author-facing operations Silc 0.4.0 can lower and run.
/// Runtime-owned surfaces (`ui::web`/`ui::terminal`), IPC/store, and resource CRUD
/// pipelines are synthesized by the compiler and must not appear in source.
pub const EXECUTABLE_OPS: &[(&str, &str)] = &[
    ("service", "http"),
    ("text", "score"),
    ("llm", "complete"),
    ("scrape", "page"),
    ("scrape", "site"),
    ("scrape", "select"),
    ("scrape", "render"),
    ("scrape", "extract"),
    ("doc", "extract"),
    ("tensor", "tokenize"),
    ("tensor", "infer"),
];

const SUPPORTED_OPS_HELP: &str =
    "`app` routes (dual-surface UI synthesized), `resource Name for Contract` capabilities, optional text::score or llm::complete, scrape::*, doc::extract, tensor::tokenize/infer pipeline, or service::http API-only";

const TENSOR_CPU_ONLY: &str =
    "tensor::infer is CPU-only in Silc 0.4.0; remove :prefer(CUDA) (default/CPU accepted)";

const SCRAPE_MIGRATE_HINT: &str =
    "use scrape::page / scrape::site / scrape::select instead of http::get / html::* (see ADR-006)";

pub const DEFAULT_TERMINAL_PORT: u16 = 18023;
pub const DEFAULT_API_PORT: u16 = 8080;
pub const DEFAULT_WEB_PORT: u16 = 18088;

fn env_u16(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

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
    TensorInfer,
}

impl ProcessorOp {
    pub fn as_str(self) -> &'static str {
        match self {
            ProcessorOp::None => "none",
            ProcessorOp::Score => "text.score",
            ProcessorOp::LlmComplete => "llm.complete",
            ProcessorOp::TensorInfer => "tensor.infer",
        }
    }

    pub fn needs_llm(self) -> bool {
        matches!(self, ProcessorOp::LlmComplete)
    }

    pub fn needs_tensor(self) -> bool {
        matches!(self, ProcessorOp::TensorInfer)
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
    pub doc: bool,
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

/// Derived document-extract capabilities (ADR-011).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocCapabilities {
    pub extract: bool,
    pub extract_into: Option<String>,
    /// Resource table that receives extracted rows (first resource for extract_into).
    pub table: Option<String>,
}

impl DocCapabilities {
    pub fn active(&self) -> bool {
        self.extract
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

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutableGraph {
    pub mode: ExecutionMode,
    pub processor_op: ProcessorOp,
    pub capabilities: UiCapabilities,
    pub scrape: ScrapeCapabilities,
    pub doc: DocCapabilities,
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
    /// Closed embedding output dimension when `processor_op` is `TensorInfer`.
    pub embedding_dim: Option<u32>,
    /// Tensor runtime device (`CPU` only in Silc 0.4.0).
    pub tensor_device: Option<String>,
    /// Contract field read by the tensor pipeline (default `raw_content`).
    pub tensor_input_field: Option<String>,
    /// Contract field written by `tensor::infer` (default `vector_embedding`).
    pub tensor_output_field: Option<String>,
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

    pub fn has_doc(&self) -> bool {
        self.doc.active()
    }

    pub fn is_scrape_only(&self) -> bool {
        self.has_scrape() && !self.has_ui() && !self.has_api() && !self.needs_tensor()
    }

    /// No UI/API surface — scrape ingest + tensor processor + sink pipeline.
    pub fn is_pipeline_only(&self) -> bool {
        !self.has_ui() && !self.has_api() && self.needs_tensor()
    }

    pub fn api_port(&self) -> Option<u16> {
        self.api_routes.first().map(|r| r.port)
    }

    pub fn needs_llm(&self) -> bool {
        self.processor_op.needs_llm() || self.capabilities.llm
    }

    pub fn needs_tensor(&self) -> bool {
        self.processor_op.needs_tensor()
    }

    pub fn needs_doc_extract(&self) -> bool {
        self.has_doc()
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

/// Every namespace recognized by the classifier (runnable, compiler-owned, or stub).
pub const KNOWN_NAMESPACES: &[&str] = &[
    "ui", "http", "html", "service", "text", "llm", "ipc", "store", "resource", "scrape", "doc",
    "tensor", "numpy", "pandas", "ws", "sys", "schema", "payload", "json",
];

fn is_known_namespace(ns: &str) -> bool {
    KNOWN_NAMESPACES.contains(&ns)
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
            | "doc"
            | "tensor"
    )
}

fn is_legacy_http_html_stub(ns: &str, name: &str) -> bool {
    matches!(
        (ns, name),
        ("http", "get") | ("html", "extract_body") | ("html", "extract") | ("html", "select")
    )
}

/// Scan author-written pipelines (modules, optional legacy serve, resources).
pub fn scan_author_calls(program: &Program, mut f: impl FnMut(&str, &str)) {
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
    scan_author_calls(program, |ns, name| {
        if is_executable_op(ns, name) {
            saw_exec = true;
        } else if is_known_namespace(ns) {
            saw_unknown_ns = true;
            if is_legacy_http_html_stub(ns, name) {
                saw_legacy_http_html = true;
            }
        }
    });

    let declaration_runnable = !program.apps.is_empty() || !program.resources.is_empty();

    if (saw_exec || declaration_runnable) && saw_unknown_ns {
        if saw_legacy_http_html {
            return Err(format!(
                "cannot mix stub-only and executable operations; {SCRAPE_MIGRATE_HINT}"
            ));
        }
        return Err(format!(
            "cannot mix stub-only and executable operations; supported runnable ops: {SUPPORTED_OPS_HELP}"
        ));
    }
    if saw_exec || declaration_runnable {
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
                                "operation `{ns}::{name}` is not executable in Silc 0.4.0"
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

fn resolve_embedding_model_ref(
    model_from_op: Option<String>,
) -> Result<&'static crate::model_catalog::EmbeddingModelCatalogEntry, String> {
    let id = model_from_op.unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL_ID.into());
    validate_embedding_model_id(&id)
}

fn embedding_dim_of_type(program: &Program, ty: &TypeExpr) -> Option<u32> {
    match ty {
        TypeExpr::Vec {
            elem,
            len: Some(len),
        } if elem == "num32" || elem == "Num32" => Some(*len as u32),
        TypeExpr::Named(name) => {
            program
                .subsets
                .iter()
                .find(|s| s.name == *name)
                .and_then(|subset| match &subset.base {
                    TypeExpr::Vec {
                        elem,
                        len: Some(len),
                    } if elem == "num32" || elem == "Num32" => Some(*len as u32),
                    _ => None,
                })
        }
        _ => None,
    }
}

fn validate_tensor_contract(program: &Program, scrape: &ScrapeCapabilities) -> Result<(), String> {
    let contract_name = scrape
        .extract_into
        .as_deref()
        .or_else(|| program.contracts.first().map(|c| c.name.as_str()))
        .ok_or_else(|| {
            "tensor pipeline requires a Contract with raw_content and vector_embedding".to_string()
        })?;
    let contract = program
        .contracts
        .iter()
        .find(|c| c.name == contract_name)
        .ok_or_else(|| format!("tensor pipeline references unknown contract `{contract_name}`"))?;

    let has_input = contract
        .fields
        .iter()
        .any(|f| f.name == DEFAULT_TENSOR_INPUT_FIELD);
    if !has_input {
        return Err(format!(
            "tensor pipeline contract `{contract_name}` must declare `{DEFAULT_TENSOR_INPUT_FIELD}`"
        ));
    }

    let output = contract
        .fields
        .iter()
        .find(|f| f.name == DEFAULT_TENSOR_OUTPUT_FIELD)
        .ok_or_else(|| {
            format!(
                "tensor pipeline contract `{contract_name}` must declare `{DEFAULT_TENSOR_OUTPUT_FIELD}`"
            )
        })?;

    if let Some(dim) = embedding_dim_of_type(program, &output.ty) {
        if dim != MINILM_EMBEDDING_DIM {
            return Err(format!(
                "tensor pipeline embedding dimension must be {MINILM_EMBEDDING_DIM} (got {dim} on `{DEFAULT_TENSOR_OUTPUT_FIELD}`)"
            ));
        }
    }

    Ok(())
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
    let mut saw_tokenize = false;
    let mut saw_infer = false;
    let mut saw_publish = false;
    let mut saw_sqlite = false;
    let mut saw_commit = false;
    let mut http_port = DEFAULT_WEB_PORT;
    let mut http_route = "/".to_string();
    let mut terminal_port: Option<u16> = None;
    let mut sqlite_table = "app_data".to_string();
    let mut model_from_op: Option<String> = None;
    let mut embedding_model_from_op: Option<String> = None;
    let mut tensor_prefer: Option<String> = None;
    let mut tokenize_before_infer = false;
    let mut app_root: Option<String> = None;
    let mut api_routes: Vec<ApiRoute> = Vec::new();
    let mut last_contract: Option<String> = None;
    let mut scrape = ScrapeCapabilities {
        js: parse_js_mode(DEFAULT_JS_MODE).unwrap_or(JsMode::Auto),
        depth: DEFAULT_SITE_DEPTH,
        same_host: true,
        ..Default::default()
    };
    let mut doc = DocCapabilities::default();

    let mut scan_pipeline = |steps: &[PipelineStep]| -> Result<(), String> {
        let mut saw_tokenize_in_pipeline = false;
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
                    ..
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
                    ("tensor", "tokenize") => {
                        saw_tokenize = true;
                        saw_tokenize_in_pipeline = true;
                        for arg in args {
                            if arg.name == "model" {
                                let model = normalize_ident(&arg.value);
                                if let Some(existing) = &embedding_model_from_op {
                                    if existing != &model {
                                        return Err(format!(
                                            "tensor::tokenize and tensor::infer must use the same model (got `{existing}` and `{model}`)"
                                        ));
                                    }
                                }
                                embedding_model_from_op = Some(model);
                            }
                        }
                    }
                    ("tensor", "infer") => {
                        saw_infer = true;
                        if saw_tokenize_in_pipeline {
                            tokenize_before_infer = true;
                        }
                        for arg in args {
                            match arg.name.as_str() {
                                "model" => {
                                    let model = normalize_ident(&arg.value);
                                    if let Some(existing) = &embedding_model_from_op {
                                        if existing != &model {
                                            return Err(format!(
                                                "tensor::tokenize and tensor::infer must use the same model (got `{existing}` and `{model}`)"
                                            ));
                                        }
                                    }
                                    embedding_model_from_op = Some(model);
                                }
                                "prefer" => {
                                    tensor_prefer = Some(normalize_ident(&arg.value));
                                }
                                _ => {}
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
                        let css =
                            css.ok_or_else(|| "scrape::select requires :css(...)".to_string())?;
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
                    ("doc", "extract") => {
                        doc.extract = true;
                        let mut into: Option<String> = None;
                        for arg in args {
                            if arg.name == "into" {
                                into = Some(normalize_ident(&arg.value));
                            }
                        }
                        let into = into
                            .ok_or_else(|| "doc::extract requires :into(Contract)".to_string())?;
                        if !program.contracts.iter().any(|c| c.name == into) {
                            return Err(format!(
                                "doc::extract references unknown contract `{into}`"
                            ));
                        }
                        doc.extract_into = Some(into);
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

    // Apps imply dual-surface UI; ports/surfaces are compiler-owned defaults.
    if !program.apps.is_empty() {
        saw_ui_web = true;
        saw_terminal = true;
        http_port = env_u16("SILC_HTTP_PORT", DEFAULT_WEB_PORT);
        terminal_port = Some(env_u16("SILC_TERMINAL_PORT", DEFAULT_TERMINAL_PORT));
        if app_root.is_none() {
            app_root = program.apps.first().map(|a| a.name.clone());
        }
        if let Some(default_app) = program.apps.first() {
            if let Some(route) = default_app.default_route() {
                http_route = route.path.clone();
            }
        }
    }

    let has_ui = saw_ui_web || saw_terminal;
    let has_api = !api_routes.is_empty();
    let has_scrape = scrape.active();
    let has_doc = doc.active();
    let has_tensor = saw_tokenize || saw_infer;
    if !has_ui && !has_api && !has_scrape && !has_doc && !has_tensor {
        // Resource-only or processor pipelines without surface — still runnable if we have resources + app.
        if program.apps.is_empty() && program.resources.is_empty() {
            return Err(format!(
                "runnable program must declare an `app`, scrape::*, doc::*, tensor::*, or service::http; {SUPPORTED_OPS_HELP}"
            ));
        }
    }

    if has_scrape && saw_score {
        return Err("cannot mix scrape::* with text::score in one program".into());
    }
    if has_doc && saw_score {
        return Err("cannot mix doc::* with text::score in one program".into());
    }
    if has_doc {
        let into = doc
            .extract_into
            .as_deref()
            .ok_or_else(|| "doc::extract requires :into(Contract)".to_string())?;
        let resource = program
            .resources
            .iter()
            .find(|r| r.contract.as_deref() == Some(into))
            .ok_or_else(|| {
                format!(
                    "doc::extract(:into({into})) needs a `resource … for {into}` to store extracted rows"
                )
            })?;
        doc.table = Some(resource.table_name());
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

    if has_ui {
        let app = app.clone().ok_or_else(|| {
            "UI programs require an `app` declaration with at least one route".to_string()
        })?;
        if app.routes.is_empty() {
            return Err(format!(
                "app `{}` must declare at least one route",
                app.name
            ));
        }
        if let (Some(tp), hp) = (terminal_port, http_port) {
            if tp == hp {
                return Err("terminal port must differ from web port".into());
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

    if saw_tokenize != saw_infer {
        return Err(
            "tensor pipelines require tensor::tokenize ==> tensor::infer in one processor method"
                .into(),
        );
    }
    if saw_infer && !tokenize_before_infer {
        return Err(
            "tensor pipelines require tensor::tokenize before tensor::infer in the same method"
                .into(),
        );
    }
    if let Some(prefer) = &tensor_prefer {
        if prefer.eq_ignore_ascii_case("CUDA") {
            return Err(TENSOR_CPU_ONLY.into());
        }
        if !prefer.eq_ignore_ascii_case("CPU") {
            return Err(format!(
                "unsupported tensor::infer :prefer({prefer}); Silc 0.4.0 accepts CPU only"
            ));
        }
    }

    let processor_op = if saw_score && saw_llm {
        return Err("cannot mix text::score and llm::complete in one program".into());
    } else if (saw_score || saw_llm) && saw_infer {
        return Err(
            "cannot mix text::score/llm::complete with tensor::infer in one program".into(),
        );
    } else if saw_score {
        ProcessorOp::Score
    } else if saw_llm {
        ProcessorOp::LlmComplete
    } else if saw_infer {
        ProcessorOp::TensorInfer
    } else {
        ProcessorOp::None
    };

    let (model_ref, embedding_dim, tensor_device, tensor_input_field, tensor_output_field) =
        if processor_op.needs_llm() {
            (
                Some(resolve_model_ref(&processors, model_from_op)?),
                None,
                None,
                None,
                None,
            )
        } else if processor_op.needs_tensor() {
            let entry = resolve_embedding_model_ref(embedding_model_from_op)?;
            validate_tensor_contract(program, &scrape)?;
            (
                Some(entry.id.to_string()),
                Some(entry.dimension),
                Some("CPU".into()),
                Some(DEFAULT_TENSOR_INPUT_FIELD.into()),
                Some(DEFAULT_TENSOR_OUTPUT_FIELD.into()),
            )
        } else {
            (None, None, None, None, None)
        };

    // Persistence is synthesized from the processor — authors never declare sinks.
    let mut synthesized_sink = String::new();
    if processor_op != ProcessorOp::None {
        if processors.len() != 1 {
            return Err(
                "programs using text::score, llm::complete, or tensor::infer need exactly one processor"
                    .into(),
            );
        }
        if !sinks.is_empty() {
            return Err(
                "author `sink` modules are not supported in Silc 0.4.0; remove them — the compiler synthesizes SQLite persistence"
                    .into(),
            );
        }
        let processor = processors[0];
        let contract = processor
            .methods
            .iter()
            .find_map(|m| {
                m.params.iter().find_map(|p| match &p.ty {
                    Some(TypeExpr::Named(name))
                        if program.contracts.iter().any(|c| c.name == *name) =>
                    {
                        Some(name.clone())
                    }
                    _ => None,
                })
            })
            .or_else(|| program.contracts.first().map(|c| c.name.clone()))
            .ok_or_else(|| {
                "processor programs need a Contract parameter so persistence can be synthesized"
                    .to_string()
            })?;
        sqlite_table = sink_table_for_contract(&contract);
        synthesized_sink = format!("{}Db", contract);
        let _ = (saw_publish, saw_sqlite, saw_commit); // legacy scan flags unused in 0.4.0
    }

    if processor_op.needs_tensor() {
        if !has_scrape || !scrape.page || !scrape.extract {
            return Err(
                "tensor pipeline programs require a scrape service with scrape::page and scrape::extract"
                    .into(),
            );
        }
        if services.len() != 1 {
            return Err("tensor pipeline programs need exactly one service module".into());
        }
        if has_ui || has_api {
            return Err(
                "tensor pipeline programs are pipeline-only (no UI or service::http)".into(),
            );
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
        doc: has_doc,
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
        doc,
        app_name: app.as_ref().map(|a| a.name.clone()),
        app,
        service: service_name,
        processor: processors
            .first()
            .map(|m| m.name.clone())
            .unwrap_or_default(),
        sink: if synthesized_sink.is_empty() {
            sinks.first().map(|m| m.name.clone()).unwrap_or_default()
        } else {
            synthesized_sink
        },
        http_port: if has_ui || saw_ui_web {
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
        embedding_dim,
        tensor_device,
        tensor_input_field,
        tensor_output_field,
        actions,
        resource_tables,
        root_component,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::TraitArg;
    use crate::contract::{Contract, Field, Subset, SubsetPredicate};
    use crate::module::{Method, Module, ModuleKind, Param};
    use crate::pipeline::{Pipeline, PipelineStep};
    use crate::program::Program;
    use crate::types::{Span, TypeExpr};

    fn runnable_tensor_pipeline() -> Program {
        Program {
            version: Some("0.4.0".into()),
            subsets: vec![
                Subset {
                    name: "Uri".into(),
                    base: TypeExpr::Named("Str".into()),
                    predicate: Some(SubsetPredicate::Contains("://".into())),
                    span: Span::default(),
                },
                Subset {
                    name: "Emb384".into(),
                    base: TypeExpr::Vec {
                        elem: "num32".into(),
                        len: Some(384),
                    },
                    predicate: None,
                    span: Span::default(),
                },
            ],
            contracts: vec![Contract {
                name: "ArticlePayload".into(),
                fields: vec![
                    Field::new("id", TypeExpr::Named("UUID".into())),
                    Field::new("url", TypeExpr::Named("Uri".into())),
                    Field::new("raw_content", TypeExpr::Named("Str".into())),
                    Field::new("vector_embedding", TypeExpr::Named("Emb384".into())),
                ],
                span: Span::default(),
            }],
            modules: vec![
                Module {
                    name: "NetworkIngress".into(),
                    kind: ModuleKind::Service,
                    traits: vec![],
                    fields: vec![],
                    methods: vec![Method {
                        name: "fetch_article".into(),
                        params: vec![],
                        pipeline: Pipeline {
                            steps: vec![
                                PipelineStep::Name("target_url".into()),
                                PipelineStep::Call {
                                    namespace: Some("scrape".into()),
                                    name: "page".into(),
                                    args: vec![TraitArg {
                                        name: "js".into(),
                                        value: "false".into(),
                                    }],
                                    span: Default::default(),
                                },
                                PipelineStep::Call {
                                    namespace: Some("scrape".into()),
                                    name: "extract".into(),
                                    args: vec![TraitArg {
                                        name: "into".into(),
                                        value: "ArticlePayload".into(),
                                    }],
                                    span: Default::default(),
                                },
                            ],
                        },
                    }],
                    span: Span::default(),
                },
                Module {
                    name: "EmbeddingEngine".into(),
                    kind: ModuleKind::Processor,
                    traits: vec![],
                    fields: vec![],
                    methods: vec![Method {
                        name: "generate_vectors".into(),
                        params: vec![Param {
                            name: "article".into(),
                            ty: Some(TypeExpr::Named("ArticlePayload".into())),
                            named: false,
                            default: None,
                            span: Default::default(),
                        }],
                        pipeline: Pipeline {
                            steps: vec![
                                PipelineStep::FieldAccess {
                                    base: "article".into(),
                                    field: "raw_content".into(),
                                },
                                PipelineStep::Call {
                                    namespace: Some("tensor".into()),
                                    name: "tokenize".into(),
                                    args: vec![],
                                    span: Default::default(),
                                },
                                PipelineStep::Call {
                                    namespace: Some("tensor".into()),
                                    name: "infer".into(),
                                    args: vec![TraitArg {
                                        name: "model".into(),
                                        value: "minilm-l6-v2".into(),
                                    }],
                                    span: Default::default(),
                                },
                            ],
                        },
                    }],
                    span: Span::default(),
                },
            ],
            components: vec![],
            resources: vec![],
            apps: vec![],
        }
    }

    fn legacy_stub_pipeline() -> Program {
        Program {
            version: Some("0.4.0".into()),
            subsets: vec![],
            contracts: vec![Contract {
                name: "ArticlePayload".into(),
                fields: vec![
                    Field::new("raw_content", TypeExpr::Named("Str".into())),
                    Field::new(
                        "vector_embedding",
                        TypeExpr::Vec {
                            elem: "num32".into(),
                            len: Some(768),
                        },
                    ),
                ],
                span: Span::default(),
            }],
            modules: vec![
                Module {
                    name: "NetworkIngress".into(),
                    kind: ModuleKind::Service,
                    traits: vec![],
                    fields: vec![],
                    methods: vec![Method {
                        name: "fetch".into(),
                        params: vec![],
                        pipeline: Pipeline {
                            steps: vec![
                                PipelineStep::Call {
                                    namespace: Some("http".into()),
                                    name: "get".into(),
                                    args: vec![],
                                    span: Default::default(),
                                },
                                PipelineStep::Call {
                                    namespace: Some("html".into()),
                                    name: "extract_body".into(),
                                    args: vec![],
                                    span: Default::default(),
                                },
                            ],
                        },
                    }],
                    span: Span::default(),
                },
                Module {
                    name: "EmbeddingEngine".into(),
                    kind: ModuleKind::Processor,
                    traits: vec![],
                    fields: vec![],
                    methods: vec![Method {
                        name: "run".into(),
                        params: vec![],
                        pipeline: Pipeline {
                            steps: vec![
                                PipelineStep::Call {
                                    namespace: Some("tensor".into()),
                                    name: "tokenize".into(),
                                    args: vec![],
                                    span: Default::default(),
                                },
                                PipelineStep::Call {
                                    namespace: Some("tensor".into()),
                                    name: "infer".into(),
                                    args: vec![],
                                    span: Default::default(),
                                },
                            ],
                        },
                    }],
                    span: Span::default(),
                },
            ],
            components: vec![],
            resources: vec![],
            apps: vec![],
        }
    }

    #[test]
    fn executable_ops_are_author_facing_workflows() {
        assert!(is_executable_op("llm", "complete"));
        assert!(is_executable_op("text", "score"));
        assert!(is_executable_op("scrape", "page"));
        assert!(is_executable_op("scrape", "site"));
        assert!(is_executable_op("scrape", "select"));
        assert!(is_executable_op("scrape", "render"));
        assert!(is_executable_op("scrape", "extract"));
        assert!(is_executable_op("tensor", "tokenize"));
        assert!(is_executable_op("tensor", "infer"));
        assert!(!is_executable_op("ui", "web"));
        assert!(!is_executable_op("resource", "list"));
        assert!(!is_executable_op("ipc", "publish"));
        assert!(!is_executable_op("store", "sqlite"));
        assert!(!is_executable_op("http", "get"));
        assert!(!is_executable_op("html", "extract_body"));
    }

    #[test]
    fn processor_op_tensor_infer_helpers() {
        assert_eq!(ProcessorOp::TensorInfer.as_str(), "tensor.infer");
        assert!(ProcessorOp::TensorInfer.needs_tensor());
        assert!(!ProcessorOp::TensorInfer.needs_llm());
        assert!(!ProcessorOp::Score.needs_tensor());
        assert!(!ProcessorOp::LlmComplete.needs_tensor());
        assert!(!ProcessorOp::None.needs_tensor());
    }

    #[test]
    fn classifies_runnable_tensor_pipeline() {
        let program = runnable_tensor_pipeline();
        assert_eq!(classify_program(&program).unwrap(), ExecutionMode::Runnable);
        let graph = infer_graph(&program).unwrap().expect("graph");
        assert!(graph.is_pipeline_only());
        assert!(graph.needs_tensor());
        assert!(!graph.has_ui());
        assert!(!graph.has_api());
        assert!(graph.has_scrape());
        assert!(graph.scrape.page);
        assert!(graph.scrape.extract);
        assert_eq!(graph.processor_op, ProcessorOp::TensorInfer);
        assert_eq!(graph.model_ref.as_deref(), Some("minilm-l6-v2"));
        assert_eq!(graph.embedding_dim, Some(384));
        assert_eq!(graph.tensor_device.as_deref(), Some("CPU"));
        assert_eq!(graph.tensor_input_field.as_deref(), Some("raw_content"));
        assert_eq!(
            graph.tensor_output_field.as_deref(),
            Some("vector_embedding")
        );
        assert_eq!(graph.service, "NetworkIngress");
        assert_eq!(graph.processor, "EmbeddingEngine");
        assert_eq!(graph.sink, "ArticlePayloadDb");
        assert_eq!(graph.sqlite_table, "article_payloads");
    }

    #[test]
    fn rejects_cuda_prefer_with_cpu_only_diagnostic() {
        let mut program = runnable_tensor_pipeline();
        if let PipelineStep::Call { args, .. } =
            &mut program.modules[1].methods[0].pipeline.steps[2]
        {
            args.push(TraitArg {
                name: "prefer".into(),
                value: "CUDA".into(),
            });
        }
        let err = infer_graph(&program).unwrap_err();
        assert!(
            err.contains("CPU-only") && err.contains("CUDA"),
            "expected CPU-only diagnostic, got {err}"
        );
    }

    #[test]
    fn accepts_explicit_cpu_prefer() {
        let mut program = runnable_tensor_pipeline();
        if let PipelineStep::Call { args, .. } =
            &mut program.modules[1].methods[0].pipeline.steps[2]
        {
            args.push(TraitArg {
                name: "prefer".into(),
                value: "CPU".into(),
            });
        }
        let graph = infer_graph(&program).unwrap().expect("graph");
        assert_eq!(graph.tensor_device.as_deref(), Some("CPU"));
    }

    #[test]
    fn rejects_author_sink_modules() {
        let mut program = runnable_tensor_pipeline();
        program.modules.push(Module {
            name: "EmbeddingDb".into(),
            kind: ModuleKind::Sink,
            traits: vec![TraitArg {
                name: "storage".into(),
                value: "SQLite".into(),
            }],
            fields: vec![],
            methods: vec![],
            span: Span::default(),
        });
        let err = infer_graph(&program).unwrap_err();
        assert!(
            err.contains("sink") && err.contains("synthesizes"),
            "expected author sink rejection, got {err}"
        );
    }

    #[test]
    fn rejects_legacy_http_html_mix_with_adr006() {
        let program = legacy_stub_pipeline();
        let err = classify_program(&program).unwrap_err();
        assert!(
            err.contains("scrape::page") || err.contains("ADR-006"),
            "expected ADR-006 migration diagnostic, got {err}"
        );
        assert!(!is_executable_op("http", "get"));
        assert!(!is_executable_op("html", "extract"));
    }

    #[test]
    fn rejects_unknown_embedding_model() {
        let mut program = runnable_tensor_pipeline();
        if let PipelineStep::Call { args, .. } =
            &mut program.modules[1].methods[0].pipeline.steps[2]
        {
            args[0].value = "not-a-model".into();
        }
        let err = infer_graph(&program).unwrap_err();
        assert!(
            err.contains("unknown embedding model") || err.contains("not-a-model"),
            "expected model catalog rejection, got {err}"
        );
    }

    #[test]
    fn rejects_wrong_embedding_dimension() {
        let mut program = runnable_tensor_pipeline();
        program.subsets[1].name = "Emb768".into();
        program.subsets[1].base = TypeExpr::Vec {
            elem: "num32".into(),
            len: Some(768),
        };
        program.contracts[0].fields[3].ty = TypeExpr::Named("Emb768".into());
        let err = infer_graph(&program).unwrap_err();
        assert!(
            err.contains("384") && err.contains("768"),
            "expected dimension diagnostic, got {err}"
        );
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
                    shorthand: false,
                },
                crate::resource::ResourceMethod {
                    kind: ResourceKind::Mutation,
                    name: "create".into(),
                    params: vec![],
                    return_ty: None,
                    pipeline: crate::pipeline::Pipeline { steps: vec![] },
                    span: Span::default(),
                    shorthand: false,
                },
            ],
            contract: Some("Product".into()),
            table: Some("products".into()),
            seeds: vec![],
            span: Span::default(),
        };
        let actions = derive_actions(&[resource]);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].path, "/api/products");
        assert_eq!(actions[0].http_method, "GET");
        assert_eq!(actions[1].http_method, "POST");
    }
}
