//! Typed operation registry and the declared compatibility matrix.
//!
//! Routing, docs, and "cannot mix" checks read this table. Adding an
//! operation means adding one [`OperationSpec`]; engines and capability
//! exclusions are not re-listed in the router or in `infer_graph`.

/// Runtime that executes an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Bun,
    Python,
    Go,
    /// Compiler-owned; authors do not write the call.
    Synthesized,
    /// Recognized spelling with no runtime yet.
    Stub,
}

/// Capability an operation contributes to a program. Compatibility rules
/// name these, not individual call spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// `llm::complete` and `loop::ask`: one local model, one provisioning path.
    LocalModel,
    TextScore,
    Embedding,
    Scrape,
    DocExtract,
    HttpApi,
    Ui,
    Scene,
    Loop,
}

/// One executable, synthesized, or stub operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationSpec {
    pub namespace: &'static str,
    pub name: &'static str,
    pub engine: Engine,
    pub capabilities: &'static [Capability],
    /// Author-facing and runnable today. Synthesized and stub entries are false.
    pub executable: bool,
    pub description: &'static str,
}

const LOCAL_MODEL: &[Capability] = &[Capability::LocalModel];
const EMBEDDING: &[Capability] = &[Capability::Embedding];
const SCRAPE: &[Capability] = &[Capability::Scrape];
const DOC: &[Capability] = &[Capability::DocExtract];
const HTTP: &[Capability] = &[Capability::HttpApi];
const UI: &[Capability] = &[Capability::Ui];
const SCENE: &[Capability] = &[Capability::Scene];
const LOOP: &[Capability] = &[Capability::Loop];
const SCORE: &[Capability] = &[Capability::TextScore];
const NONE: &[Capability] = &[];

pub const OPERATION_CATALOG: &[OperationSpec] = &[
    OperationSpec {
        namespace: "service",
        name: "http",
        engine: Engine::Go,
        capabilities: HTTP,
        executable: true,
        description: "HTTP API route served by the Go runtime.",
    },
    OperationSpec {
        namespace: "text",
        name: "score",
        engine: Engine::Python,
        capabilities: SCORE,
        executable: true,
        description: "Local text score. One processor family per program.",
    },
    OperationSpec {
        namespace: "llm",
        name: "complete",
        engine: Engine::Python,
        capabilities: LOCAL_MODEL,
        executable: true,
        description: "Local model completion (silclm). Same capability as loop::ask.",
    },
    OperationSpec {
        namespace: "scrape",
        name: "page",
        engine: Engine::Bun,
        capabilities: SCRAPE,
        executable: true,
        description: "Fetch one page.",
    },
    OperationSpec {
        namespace: "scrape",
        name: "site",
        engine: Engine::Go,
        capabilities: SCRAPE,
        executable: true,
        description: "Crawl a site. A service module hosts it on Bun; the crawl sidecar is Go.",
    },
    OperationSpec {
        namespace: "scrape",
        name: "select",
        engine: Engine::Bun,
        capabilities: SCRAPE,
        executable: true,
        description: "Select fields from a fetched page.",
    },
    OperationSpec {
        namespace: "scrape",
        name: "render",
        engine: Engine::Python,
        capabilities: SCRAPE,
        executable: true,
        description: "Render a page in a browser.",
    },
    OperationSpec {
        namespace: "scrape",
        name: "extract",
        engine: Engine::Python,
        capabilities: SCRAPE,
        executable: true,
        description: "Extract structured fields from a rendered page.",
    },
    OperationSpec {
        namespace: "doc",
        name: "extract",
        engine: Engine::Python,
        capabilities: DOC,
        executable: true,
        description: "Extract a contract from an uploaded document.",
    },
    OperationSpec {
        namespace: "tensor",
        name: "tokenize",
        engine: Engine::Python,
        capabilities: EMBEDDING,
        executable: true,
        description: "Tokenize text for the embedding model.",
    },
    OperationSpec {
        namespace: "tensor",
        name: "infer",
        engine: Engine::Python,
        capabilities: EMBEDDING,
        executable: true,
        description: "Run the embedding model (CPU).",
    },
    OperationSpec {
        namespace: "mcp",
        name: "call",
        engine: Engine::Go,
        capabilities: LOOP,
        executable: true,
        description: "Call one MCP tool. Used from loop::read, not as a pipeline step.",
    },
    // Compiler-synthesized. Present so routing and docs share one list.
    OperationSpec {
        namespace: "ui",
        name: "web",
        engine: Engine::Synthesized,
        capabilities: UI,
        executable: false,
        description: "Synthesized web surface.",
    },
    OperationSpec {
        namespace: "ui",
        name: "terminal",
        engine: Engine::Synthesized,
        capabilities: UI,
        executable: false,
        description: "Synthesized terminal surface.",
    },
    OperationSpec {
        namespace: "scene",
        name: "render",
        engine: Engine::Synthesized,
        capabilities: SCENE,
        executable: false,
        description: "Synthesized WebGPU scene runtime.",
    },
    // Stub namespaces kept so a mixed program can name them.
    OperationSpec {
        namespace: "http",
        name: "get",
        engine: Engine::Stub,
        capabilities: NONE,
        executable: false,
        description: "Removed. Use scrape::page.",
    },
    OperationSpec {
        namespace: "html",
        name: "extract_body",
        engine: Engine::Stub,
        capabilities: NONE,
        executable: false,
        description: "Removed. Use scrape::select.",
    },
    OperationSpec {
        namespace: "numpy",
        name: "array",
        engine: Engine::Stub,
        capabilities: NONE,
        executable: false,
        description: "Stub.",
    },
    OperationSpec {
        namespace: "pandas",
        name: "frame",
        engine: Engine::Stub,
        capabilities: NONE,
        executable: false,
        description: "Stub.",
    },
    OperationSpec {
        namespace: "ws",
        name: "connect",
        engine: Engine::Stub,
        capabilities: NONE,
        executable: false,
        description: "Stub.",
    },
    OperationSpec {
        namespace: "sys",
        name: "env",
        engine: Engine::Stub,
        capabilities: NONE,
        executable: false,
        description: "Stub.",
    },
    OperationSpec {
        namespace: "schema",
        name: "check",
        engine: Engine::Stub,
        capabilities: NONE,
        executable: false,
        description: "Stub.",
    },
    OperationSpec {
        namespace: "payload",
        name: "decode",
        engine: Engine::Stub,
        capabilities: NONE,
        executable: false,
        description: "Stub.",
    },
    OperationSpec {
        namespace: "json",
        name: "parse",
        engine: Engine::Stub,
        capabilities: NONE,
        executable: false,
        description: "Stub.",
    },
];

/// Author-facing operations that run today, derived from [`OPERATION_CATALOG`].
pub fn executable_ops() -> Vec<(&'static str, &'static str)> {
    OPERATION_CATALOG
        .iter()
        .filter(|op| op.executable && op.namespace != "mcp")
        .map(|op| (op.namespace, op.name))
        .collect()
}

pub fn lookup_operation(namespace: &str, name: &str) -> Option<&'static OperationSpec> {
    OPERATION_CATALOG
        .iter()
        .find(|op| op.namespace == namespace && op.name == name)
}

pub fn is_executable(namespace: &str, name: &str) -> bool {
    lookup_operation(namespace, name).is_some_and(|op| op.executable && op.namespace != "mcp")
}

pub fn is_registered_namespace(namespace: &str) -> bool {
    OPERATION_CATALOG.iter().any(|op| op.namespace == namespace)
        || matches!(namespace, "game" | "loop" | "ipc" | "store" | "resource")
}

/// Two capabilities that cannot appear in one program, with the diagnostic
/// authors see. `infer_graph` consults this instead of open-coded pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompatRule {
    pub left: Capability,
    pub right: Capability,
    pub message: &'static str,
}

pub const COMPATIBILITY: &[CompatRule] = &[
    CompatRule {
        left: Capability::Scrape,
        right: Capability::TextScore,
        message: "cannot mix scrape::* with text::score in one program",
    },
    CompatRule {
        left: Capability::DocExtract,
        right: Capability::TextScore,
        message: "cannot mix doc::* with text::score in one program",
    },
    CompatRule {
        left: Capability::TextScore,
        right: Capability::LocalModel,
        message: "cannot mix text::score and the local model (llm::complete or loop::ask) in one program",
    },
    CompatRule {
        left: Capability::TextScore,
        right: Capability::Embedding,
        message: "cannot mix text::score/llm::complete with tensor::infer in one program",
    },
    CompatRule {
        left: Capability::LocalModel,
        right: Capability::Embedding,
        message: "cannot mix text::score/llm::complete with tensor::infer in one program",
    },
    CompatRule {
        left: Capability::Loop,
        right: Capability::Scrape,
        message: "loops cannot be combined with scrape::* or tensor::* pipelines in Silc 0.6.0; use loop::read for pages",
    },
    CompatRule {
        left: Capability::Loop,
        right: Capability::Embedding,
        message: "loops cannot be combined with scrape::* or tensor::* pipelines in Silc 0.6.0; use loop::read for pages",
    },
];

pub fn compatibility_message(present: &[Capability]) -> Option<&'static str> {
    for rule in COMPATIBILITY {
        let has_left = present.contains(&rule.left);
        let has_right = present.contains(&rule.right);
        if has_left && has_right && rule.left != rule.right {
            return Some(rule.message);
        }
    }
    None
}

/// `text::score` and `llm::complete` used to be spelled `text.score`. The
/// colon form is the operation name.
pub fn processor_op_name(kind: &str) -> &'static str {
    match kind {
        "score" => "text::score",
        "llm" => "llm::complete",
        "tensor" => "tensor::infer",
        _ => "none",
    }
}
