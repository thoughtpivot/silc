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
            } else {
                "tier1: processor+data/ML/text → Python (scientific/ML and text scoring)"
                    .to_string()
            },
        )
    } else if module.kind == ModuleKind::Processor
        && has_scrape_op(&["render", "extract"])
    {
        (
            Target::Python,
            "tier1: processor+scrape render/extract → Python (Playwright browser, ADR-006)"
                .to_string(),
        )
    } else if module.kind == ModuleKind::Service && has(&["service"]) && !has(&["ui", "html", "scrape"])
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
            format!(
                "tier2: namespaces [{}] → Python (scientific/ML/text/llm)",
                namespaces.join(", ")
            ),
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
    fn routes_article_pipeline_three_ways() {
        let source = include_str!("../tests/fixtures/data_pipeline.silc");
        let program = sil_parser::parse(source).expect("parse example");
        let decisions = route_program(&program);
        assert_eq!(decisions[0].target, Target::Bun);
        assert_eq!(decisions[1].target, Target::Python);
        assert_eq!(decisions[2].target, Target::Go);
        assert!(decisions
            .iter()
            .all(|decision| !decision.provenance.is_empty()));
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
    fn routes_ui_web_service_to_bun() {
        let source = r#"
@version("1.0")
class FeedbackRecord { has Str $.author; has Str $.text; }
class WebPortal is service {
    method listen(:$port = 18080) {
        FeedbackRecord ==> ui::web(:port(18080), :route("/"))
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
        let program = sil_parser::parse(source).expect("parse");
        let decisions = route_program(&program);
        assert_eq!(decisions[0].target, Target::Bun);
        assert_eq!(decisions[1].target, Target::Python);
        assert_eq!(decisions[2].target, Target::Go);
    }

    #[test]
    fn routes_llm_processor_to_python() {
        let source = r#"
@version("0.2.0")
class ChatRecord { has Str $.prompt; has Str $.reply; }
class ChatPage is component {
    has state Str $.prompt = "";
    method render() {
        ui::page(ui::chat(:value($.prompt), :on(send(on_send))))
    }
    method on_send() { Assistant.complete(); }
}
class ChatApp is app {
    route "/" => ChatPage;
    method serve() {
        ui::web(:root(ChatApp), :port(18090)) ==> ui::terminal(:port(18091))
    }
}
class Assistant is processor {
    method complete(ChatRecord $record) {
        $record.prompt ==> llm::complete(:model("silclm"))
    }
}
class ChatDb is sink is storage(SQLite) {
    method persist(ChatRecord $record) {
        $record ==> ipc::publish() ==> store::sqlite(:table(chats)) ==> store::commit()
    }
}
"#;
        let program = sil_parser::parse(source).expect("parse chat app");
        let decisions = route_program(&program);
        let python = decisions.iter().find(|d| d.target == Target::Python);
        assert!(python.is_some(), "expected python processor route");
        assert!(python.unwrap().provenance.contains("llm"));
        let go = decisions.iter().find(|d| d.target == Target::Go);
        assert!(go.is_some(), "expected go sink route");
    }

    #[test]
    fn routes_service_http_to_go() {
        let source = r#"
@version("1.0")
class FeedbackRecord { has Str $.author; has Str $.text; }
class FeedbackApi is service {
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
@version("0.2.0")
class Crawler is service {
    method run() {
        seed_url ==> scrape::site(:depth(2), :same_host(true)) ==> scrape::select(:css("title"), :as(title))
    }
}
"#;
        let program = sil_parser::parse(source).expect("parse scrape site");
        let decisions = route_program(&program);
        assert_eq!(decisions[0].target, Target::Bun);
        assert!(decisions[0].provenance.contains("scrape") || decisions[0].provenance.contains("Bun"));
    }

    #[test]
    fn routes_scrape_site_task_to_go() {
        let source = r#"
@version("0.2.0")
class Crawler is task {
    method run() {
        seed_url ==> scrape::site(:depth(2), :same_host(true))
    }
}
"#;
        let program = sil_parser::parse(source).expect("parse scrape site task");
        let decisions = route_program(&program);
        assert_eq!(decisions[0].target, Target::Go);
        assert!(decisions[0].provenance.contains("Colly") || decisions[0].provenance.contains("scrape"));
    }

    #[test]
    fn routes_scrape_page_with_ui_service_to_bun() {
        let source = r#"
@version("0.2.0")
class Page is component {
    method render() { ui::page(ui::text(:text("x"))) }
}
class ScraperApp is app {
    route "/" => Page;
    method serve() {
        ui::web(:root(ScraperApp), :port(18100)) ==> ui::terminal(:port(18101))
    }
}
class Ingest is service {
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
@version("0.2.0")
class Browser is processor {
    method run() {
        url ==> scrape::render() ==> scrape::extract(:into(Article))
    }
}
class Article { has Str $.title; }
"#;
        let program = sil_parser::parse(source).expect("parse scrape render");
        let decisions = route_program(&program);
        assert_eq!(decisions[0].target, Target::Python);
        assert!(decisions[0].provenance.contains("Playwright") || decisions[0].provenance.contains("scrape"));
    }
}
