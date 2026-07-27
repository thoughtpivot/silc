//! Deterministic Tier 1 + Tier 2 routing for Silc modules.

use sil_core::{Module, ModuleKind, Program, Target};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDecision {
    pub module: String,
    pub target: Target,
    pub provenance: String,
}

pub fn route_program(program: &Program) -> Vec<RouteDecision> {
    program.modules.iter().map(route_module).collect()
}

fn scrape_ops(module: &Module) -> Vec<&str> {
    module
        .methods
        .iter()
        .flat_map(|method| method.pipeline.steps.iter())
        .filter_map(|step| match step {
            sil_core::PipelineStep::Call {
                namespace: Some(ns),
                name,
                ..
            } if ns == "scrape" => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

pub fn route_module(module: &Module) -> RouteDecision {
    let namespaces: Vec<&str> = module
        .methods
        .iter()
        .flat_map(|method| method.pipeline.namespaces())
        .collect();
    let has = |candidates: &[&str]| {
        namespaces
            .iter()
            .any(|namespace| candidates.contains(namespace))
    };
    let scrape = scrape_ops(module);
    let has_scrape_op = |names: &[&str]| scrape.iter().any(|op| names.contains(op));
    let cuda = module.methods.iter().any(|method| {
        method.pipeline.steps.iter().any(|step| match step {
            sil_core::PipelineStep::Call { args, .. } => args
                .iter()
                .any(|arg| arg.name == "prefer" && arg.value.eq_ignore_ascii_case("CUDA")),
            _ => false,
        })
    });
    let low_latency = module.traits.iter().any(|t| {
        t.name == "latency"
            && t.value
                .trim_end_matches("ms")
                .parse::<u64>()
                .is_ok_and(|value| value <= 10)
    });

    let sqlite_storage = module
        .traits
        .iter()
        .any(|t| t.name == "storage" && t.value.eq_ignore_ascii_case("SQLite"));

    // Provenance cites ADR-004 / ADR-006 runtime strength catalogs.
    let (target, provenance) = if module.kind == ModuleKind::Sink && (low_latency || sqlite_storage)
    {
        (
            Target::Go,
            if sqlite_storage {
                "tier1: sink+SQLite → Go (durable low-latency storage)".to_string()
            } else {
                "tier1: sink+latency≤10ms → Go (predictable low-latency systems path)".to_string()
            },
        )
    } else if module.kind == ModuleKind::Processor
        && (has(&["tensor", "numpy", "pandas", "text", "llm"]) || cuda)
    {
        (
            Target::Python,
            if has(&["llm"]) {
                "tier1: processor+llm → Python (local LLM / llama.cpp)".to_string()
            } else if has(&["tensor"]) {
                "tier1: processor+tensor → Python (ONNX Runtime CPU embedding)".to_string()
            } else {
                "tier1: processor+data/ML/text → Python (scientific/ML and text scoring)"
                    .to_string()
            },
        )
    } else if module.kind == ModuleKind::Processor && has_scrape_op(&["render", "extract"]) {
        (
            Target::Python,
            "tier1: processor+scrape render/extract → Python (Playwright browser, ADR-006)"
                .to_string(),
        )
    } else if module.kind == ModuleKind::Service
        && has(&["service"])
        && !has(&["ui", "html", "scrape"])
    {
        (
            Target::Go,
            "tier1: service+service::http → Go (Gin HTTP API)".to_string(),
        )
    } else if module.kind == ModuleKind::Service {
        // Scrape services stay on Bun (UI/ingest). Colly/Playwright are
        // compiler-owned sidecars selected by ScrapeCapabilities (ADR-006).
        (
            Target::Bun,
            if has(&["scrape"]) {
                "tier1: service+scrape → Bun (UI ingress + bun-fetch-v1; crawl/browser sidecars, ADR-006)"
                    .to_string()
            } else {
                "tier1: service → Bun (async I/O and web UI protocols)".to_string()
            },
        )
    } else if has(&["service"]) && !has(&["ui", "html", "scrape"]) {
        (
            Target::Go,
            format!(
                "tier2: namespaces [{}] → Go (declarative HTTP API)",
                namespaces.join(", ")
            ),
        )
    } else if has_scrape_op(&["site"]) {
        (
            Target::Go,
            format!(
                "tier2: scrape::site → Go (Colly concurrency, ADR-006); namespaces [{}]",
                namespaces.join(", ")
            ),
        )
    } else if has_scrape_op(&["render", "extract"]) {
        (
            Target::Python,
            format!(
                "tier2: scrape render/extract → Python (Playwright, ADR-006); namespaces [{}]",
                namespaces.join(", ")
            ),
        )
    } else if has(&["http", "html", "ws", "ui", "scrape"]) {
        (
            Target::Bun,
            format!(
                "tier2: namespaces [{}] → Bun (async I/O / UI / static scrape, ADR-006)",
                namespaces.join(", ")
            ),
        )
    } else if has(&["tensor", "numpy", "pandas", "text", "llm"]) {
        (
            Target::Python,
            if has(&["tensor"]) {
                format!(
                    "tier2: namespaces [{}] → Python (ONNX Runtime CPU embedding)",
                    namespaces.join(", ")
                )
            } else {
                format!(
                    "tier2: namespaces [{}] → Python (scientific/ML/text/llm)",
                    namespaces.join(", ")
                )
            },
        )
    } else if has(&["store", "ipc", "sys"]) {
        (
            Target::Go,
            format!(
                "tier2: namespaces [{}] → Go (systems IPC and storage)",
                namespaces.join(", ")
            ),
        )
    } else {
        match module.kind {
            ModuleKind::Processor => (
                Target::Python,
                "fallback: processor → Python (domain analysis glue)".to_string(),
            ),
            ModuleKind::Sink => (
                Target::Go,
                "fallback: sink → Go (systems and storage paths)".to_string(),
            ),
            _ => (
                Target::Bun,
                "fallback: module → Bun (async I/O default)".to_string(),
            ),
        }
    };

    RouteDecision {
        module: module.name.clone(),
        target,
        provenance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sil_core::{Method, Module, ModuleKind, Pipeline, PipelineStep, Program, Span};

    #[test]
    fn routes_article_pipeline_service_and_processor() {
        let source = include_str!("../tests/fixtures/data_pipeline.silc");
        let program = sil_parser::parse(source).expect("parse example");
        let decisions = route_program(&program);
        assert_eq!(decisions.len(), 2);
        assert_eq!(decisions[0].target, Target::Bun);
        assert_eq!(decisions[1].target, Target::Python);
        assert!(decisions
            .iter()
            .all(|decision| !decision.provenance.is_empty()));
    }

    #[test]
    fn routes_runnable_tensor_pipeline_without_author_sink() {
        let source = include_str!("../tests/fixtures/data_pipeline_runnable.silc");
        let program = sil_parser::parse(source).expect("parse runnable pipeline");
        let decisions = route_program(&program);
        assert_eq!(decisions.len(), 2);
        assert_eq!(decisions[0].module, "NetworkIngress");
        assert_eq!(decisions[0].target, Target::Bun);
        assert!(
            decisions[0].provenance.contains("scrape") || decisions[0].provenance.contains("Bun"),
            "expected scrape→Bun provenance, got {}",
            decisions[0].provenance
        );
        assert_eq!(decisions[1].module, "EmbeddingEngine");
        assert_eq!(decisions[1].target, Target::Python);
        assert!(
            decisions[1].provenance.contains("ONNX") && decisions[1].provenance.contains("CPU"),
            "expected ONNX CPU provenance, got {}",
            decisions[1].provenance
        );

        // Fixture must also classify as the closed runnable tensor graph with synthesized sink.
        let graph = sil_core::infer_graph(&program)
            .expect("infer")
            .expect("runnable graph");
        assert!(graph.is_pipeline_only());
        assert_eq!(graph.processor_op, sil_core::ProcessorOp::TensorInfer);
        assert_eq!(graph.model_ref.as_deref(), Some("minilm-l6-v2"));
        assert_eq!(graph.embedding_dim, Some(384));
        assert_eq!(graph.sink, "ArticlePayloadDb");
        assert_eq!(graph.sqlite_table, "article_payloads");
    }

    #[test]
    fn routes_ui_namespace_to_bun() {
        let program = Program {
            version: Some("1.0".into()),
            subsets: vec![],
            contracts: vec![],
            modules: vec![Module {
                name: "TermOnly".into(),
                kind: ModuleKind::Processor,
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
            components: vec![],
            resources: vec![],
            apps: vec![],
        };
        let decisions = route_program(&program);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].target, Target::Bun);
        assert!(decisions[0].provenance.contains("ui"));
    }

    #[test]
    fn routes_score_processor_to_python() {
        let source = r#"
@version("0.4.0")
contract FeedbackRecord { has Str $.author; has Str $.text; }
component Page {
    method render() { ui::page(ui::text(:text("x"))) }
}
app FeedbackApp {
    route "/" => Page;
}
processor TextAnalyzer {
    method analyze(FeedbackRecord $record) { $record.text ==> text::score() }
}
"#;
        let program = sil_parser::parse(source).expect("parse");
        let decisions = route_program(&program);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].target, Target::Python);
        let graph = sil_core::infer_graph(&program)
            .expect("infer")
            .expect("runnable graph");
        assert_eq!(graph.sink, "FeedbackRecordDb");
    }

    #[test]
    fn routes_llm_processor_to_python() {
        let source = r#"
@version("0.4.0")
contract ChatRecord { has Str $.prompt; has Str $.reply; }
component ChatPage {
    has state Str $.prompt = "";
    method render() {
        ui::page(ui::chat(:value($.prompt), :on(send(on_send))))
    }
    method on_send() { Assistant.complete(); }
}
app ChatApp {
    route "/" => ChatPage;
}
processor Assistant {
    method complete(ChatRecord $record) {
        $record.prompt ==> llm::complete(:model("silclm"))
    }
}
"#;
        let program = sil_parser::parse(source).expect("parse chat app");
        let decisions = route_program(&program);
        let python = decisions.iter().find(|d| d.target == Target::Python);
        assert!(python.is_some(), "expected python processor route");
        assert!(python.unwrap().provenance.contains("llm"));
        let graph = sil_core::infer_graph(&program)
            .expect("infer")
            .expect("runnable graph");
        assert_eq!(graph.sink, "ChatRecordDb");
        assert!(graph.capabilities.web && graph.capabilities.terminal);
    }

    #[test]
    fn routes_service_http_to_go() {
        let source = r#"
@version("1.0")
contract FeedbackRecord { has Str $.author; has Str $.text; }
service FeedbackApi {
    method list(:$port = 18081) {
        FeedbackRecord ==> service::http(:port(18081), :route("/api/feedback"), :method(GET))
    }
}
"#;
        let program = sil_parser::parse(source).expect("parse");
        let decisions = route_program(&program);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].target, Target::Go);
        assert!(decisions[0].provenance.contains("Gin") || decisions[0].provenance.contains("Go"));
    }

    #[test]
    fn routes_scrape_site_service_to_bun() {
        let source = r#"
@version("0.4.0")
service Crawler {
    method run() {
        seed_url ==> scrape::site(:depth(2), :same_host(true)) ==> scrape::select(:css("title"), :as(title))
    }
}
"#;
        let program = sil_parser::parse(source).expect("parse scrape site");
        let decisions = route_program(&program);
        assert_eq!(decisions[0].target, Target::Bun);
        assert!(
            decisions[0].provenance.contains("scrape") || decisions[0].provenance.contains("Bun")
        );
    }

    #[test]
    fn routes_scrape_site_task_to_go() {
        let source = r#"
@version("0.4.0")
task Crawler {
    method run() {
        seed_url ==> scrape::site(:depth(2), :same_host(true))
    }
}
"#;
        let program = sil_parser::parse(source).expect("parse scrape site task");
        let decisions = route_program(&program);
        assert_eq!(decisions[0].target, Target::Go);
        assert!(
            decisions[0].provenance.contains("Colly") || decisions[0].provenance.contains("scrape")
        );
    }

    #[test]
    fn routes_scrape_page_with_ui_service_to_bun() {
        let source = r#"
@version("0.4.0")
component Page {
    method render() { ui::page(ui::text(:text("x"))) }
}
app ScraperApp {
    route "/" => Page;
}
service Ingest {
    method fetch() {
        url ==> scrape::page(:js(false)) ==> scrape::select(:css("title"), :as(title))
    }
}
"#;
        let program = sil_parser::parse(source).expect("parse scrape page ui");
        let decisions = route_program(&program);
        let bun = decisions.iter().find(|d| d.module == "Ingest").unwrap();
        assert_eq!(bun.target, Target::Bun);
    }

    #[test]
    fn routes_scrape_render_processor_to_python() {
        let source = r#"
@version("0.4.0")
processor Browser {
    method run() {
        url ==> scrape::render() ==> scrape::extract(:into(Article))
    }
}
contract Article { has Str $.title; }
"#;
        let program = sil_parser::parse(source).expect("parse scrape render");
        let decisions = route_program(&program);
        assert_eq!(decisions[0].target, Target::Python);
        assert!(
            decisions[0].provenance.contains("Playwright")
                || decisions[0].provenance.contains("scrape")
        );
    }
}
