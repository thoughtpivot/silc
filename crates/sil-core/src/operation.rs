//! Executable operation registry for Silc v1 runnable programs.

use crate::module::{Module, ModuleKind};
use crate::pipeline::PipelineStep;
use crate::program::Program;

/// Operations that Silc can actually lower and run in v1.
///
/// `ui::web` is the canonical web UI op. `html::form` and `http::serve` remain
/// executable compatibility aliases that lower to the same web profile.
pub const EXECUTABLE_OPS: &[(&str, &str)] = &[
    ("ui", "web"),
    ("ui", "terminal"),
    ("html", "form"),
    ("http", "serve"),
    ("text", "score"),
    ("ipc", "publish"),
    ("store", "sqlite"),
    ("store", "commit"),
];

const SUPPORTED_OPS_HELP: &str =
    "ui::web (or html::form + http::serve), optional ui::terminal, text::score, ipc::publish, store::sqlite, store::commit";

/// Default TCP port for `ui::terminal` (telnet-friendly; mnemonic for historic 23).
pub const DEFAULT_TERMINAL_PORT: u16 = 18023;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableGraph {
    pub mode: ExecutionMode,
    pub service: String,
    pub processor: String,
    pub sink: String,
    pub http_port: u16,
    pub http_route: String,
    pub sqlite_table: String,
    pub ui_surface: UiSurface,
    /// When set, Bun also listens for line-oriented telnet/TCP sessions.
    pub terminal_port: Option<u16>,
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
            | "text"
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
    matches!(ns, "ui" | "http" | "html" | "text" | "ipc" | "store")
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
            if services.len() != 1 || processors.len() != 1 || sinks.len() != 1 {
                return Err(
                    "runnable Silc v1 programs require exactly one service, one processor, and one sink"
                        .into(),
                );
            }

            let mut http_port = 8080u16;
            let mut http_route = String::from("/");
            let mut terminal_port: Option<u16> = None;
            let mut saw_ui_web = false;
            let mut saw_terminal = false;
            let mut saw_form = false;
            let mut saw_serve = false;
            let mut saw_score = false;
            let mut saw_publish = false;
            let mut saw_sqlite = false;
            let mut saw_commit = false;
            let mut sqlite_table = String::from("feedback");

            for module in &program.modules {
                for method in &module.methods {
                    for step in &method.pipeline.steps {
                        if let PipelineStep::Call {
                            namespace: Some(ns),
                            name,
                            args,
                        } = step
                        {
                            match (ns.as_str(), name.as_str()) {
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
                                ("text", "score") => saw_score = true,
                                ("ipc", "publish") => saw_publish = true,
                                ("store", "sqlite") => {
                                    saw_sqlite = true;
                                    if let Some(table) = args.iter().find(|a| a.name == "table") {
                                        sqlite_table = table.value.clone();
                                    }
                                }
                                ("store", "commit") => saw_commit = true,
                                _ => {}
                            }
                        }
                    }
                }
            }

            let ui_surface = if saw_ui_web {
                if saw_form || saw_serve {
                    return Err(
                        "use either `ui::web` or the legacy `html::form` + `http::serve` alias, not both"
                            .into(),
                    );
                }
                UiSurface::Web
            } else if saw_form && saw_serve {
                UiSurface::LegacyHtmlHttp
            } else if saw_terminal {
                // Terminal alone is not enough for the feedback portal shape —
                // browser (or legacy HTML) remains required so React/HTTP health stays available.
                return Err(
                    "runnable feedback portal requires `ui::web` (or html::form + http::serve); add `ui::terminal` alongside it for telnet"
                        .into(),
                );
            } else {
                return Err(format!(
                    "runnable feedback portal requires {SUPPORTED_OPS_HELP}"
                ));
            };

            if !(saw_score && saw_publish && saw_sqlite && saw_commit) {
                return Err(format!(
                    "runnable feedback portal requires {SUPPORTED_OPS_HELP}"
                ));
            }

            if let Some(tp) = terminal_port {
                if tp == http_port {
                    return Err(format!(
                        "ui::terminal :port({tp}) must differ from ui::web :port({http_port})"
                    ));
                }
            }

            let storage_ok = sinks[0]
                .traits
                .iter()
                .any(|t| t.name == "storage" && t.value.eq_ignore_ascii_case("SQLite"));
            if !storage_ok {
                return Err("runnable sink must declare `is storage(SQLite)`".into());
            }

            Ok(Some(ExecutableGraph {
                mode: ExecutionMode::Runnable,
                service: services[0].name.clone(),
                processor: processors[0].name.clone(),
                sink: sinks[0].name.clone(),
                http_port,
                http_route,
                sqlite_table,
                ui_surface,
                terminal_port,
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
        }
    }

    #[test]
    fn classifies_runnable_legacy_feedback() {
        let program = feedback_like_legacy();
        assert_eq!(classify_program(&program).unwrap(), ExecutionMode::Runnable);
        let graph = infer_graph(&program).unwrap().unwrap();
        assert_eq!(graph.http_port, 18080);
        assert_eq!(graph.sqlite_table, "feedback");
        assert_eq!(graph.ui_surface, UiSurface::LegacyHtmlHttp);
    }

    #[test]
    fn classifies_runnable_ui_web() {
        let program = feedback_like_ui_web();
        assert_eq!(classify_program(&program).unwrap(), ExecutionMode::Runnable);
        let graph = infer_graph(&program).unwrap().unwrap();
        assert_eq!(graph.http_port, 18080);
        assert_eq!(graph.http_route, "/");
        assert_eq!(graph.ui_surface, UiSurface::Web);
        assert_eq!(graph.ui_surface.substrate(), "react");
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
        };
        assert_eq!(classify_program(&program).unwrap(), ExecutionMode::Runnable);
        assert!(infer_graph(&program)
            .unwrap_err()
            .contains("exactly one service, one processor, and one sink"));
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
        };
        assert_eq!(classify_program(&program).unwrap(), ExecutionMode::Runnable);
        let graph = infer_graph(&program).unwrap().unwrap();
        assert_eq!(graph.terminal_port, Some(18023));
        assert_eq!(graph.http_port, 18080);
    }
}
