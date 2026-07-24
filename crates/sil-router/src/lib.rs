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

    let (target, provenance) = if module.kind == ModuleKind::Sink && low_latency {
        (
            Target::Go,
            "tier1: sink with latency <= 10ms requires Go".to_string(),
        )
    } else if module.kind == ModuleKind::Processor && (has(&["tensor", "numpy", "pandas"]) || cuda)
    {
        (
            Target::Python,
            "tier1: processor with data/ML evidence requires Python".to_string(),
        )
    } else if module.kind == ModuleKind::Service {
        (
            Target::Bun,
            "tier1: service prefers Bun for async I/O".to_string(),
        )
    } else if has(&["http", "html", "ws"]) {
        (
            Target::Bun,
            format!(
                "tier2: namespace evidence [{}] selects Bun",
                namespaces.join(", ")
            ),
        )
    } else if has(&["tensor", "numpy", "pandas"]) {
        (
            Target::Python,
            format!(
                "tier2: namespace evidence [{}] selects Python",
                namespaces.join(", ")
            ),
        )
    } else if has(&["store", "ipc", "sys"]) {
        (
            Target::Go,
            format!(
                "tier2: namespace evidence [{}] selects Go",
                namespaces.join(", ")
            ),
        )
    } else {
        match module.kind {
            ModuleKind::Processor => (
                Target::Python,
                "fallback: processor defaults to Python".to_string(),
            ),
            ModuleKind::Sink => (Target::Go, "fallback: sink defaults to Go".to_string()),
            _ => (Target::Bun, "fallback: module defaults to Bun".to_string()),
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

    #[test]
    fn routes_article_pipeline_three_ways() {
        let source = include_str!("../../../examples/article_pipeline.silc");
        let program = sil_parser::parse(source).expect("parse example");
        let decisions = route_program(&program);
        assert_eq!(decisions[0].target, Target::Bun);
        assert_eq!(decisions[1].target, Target::Python);
        assert_eq!(decisions[2].target, Target::Go);
        assert!(decisions
            .iter()
            .all(|decision| !decision.provenance.is_empty()));
    }
}
