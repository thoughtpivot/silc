//! Subject-oriented semantic core of ThoughtPivot Silc 0.2.0.
//!
//! Surface mapping (Raku-inspired Silc, ADR-002):
//! `class`/`has`/`subset` → Contract, `is service` → Module,
//! `is component` → Component, `is resource` → Resource, `is app` → App,
//! traits/units/adverbials → Constraint, `==>` → Pipeline.

pub mod app;
pub mod component;
pub mod constraint;
pub mod contract;
pub mod expr;
pub mod model_catalog;
pub mod module;
pub mod operation;
pub mod pipeline;
pub mod program;
pub mod resource;
pub mod target;
pub mod types;
pub mod ui;

pub use app::{App, Route};
pub use component::{
    CompField, Component, EmitDecl, EventBinding, Handler, QueryBinding, SlotDecl, UiNode,
    UiTemplate,
};
pub use constraint::TraitArg;
pub use contract::{Contract, Field, Subset};
pub use expr::{BinOp, Expr, InterpPart, UnaryOp};
pub use model_catalog::{
    is_known_model_id, lookup_model, validate_model_id, ModelCatalogEntry, DEFAULT_LLM_N_CTX,
    DEFAULT_MODEL_ID, LEGACY_MODEL_ID, MODEL_CATALOG,
};
pub use module::{Method, Module, ModuleKind, Param};
pub use operation::{
    classify_program, infer_graph, is_executable_op, ApiRoute, ExecutableGraph, ExecutionMode,
    ProcessorOp, UiCapabilities, DEFAULT_API_PORT, DEFAULT_TERMINAL_PORT, DEFAULT_WEB_PORT,
    EXECUTABLE_OPS,
};
pub use pipeline::{Pipeline, PipelineStep};
pub use program::Program;
pub use resource::{ActionDef, Resource, ResourceKind, ResourceMethod};
pub use target::Target;
pub use types::{Span, TypeExpr};
pub use ui::{
    catalog_component_names, format_component_catalog_line, lookup_component,
    validate_builtin_node, validate_template, ChildPolicy, ComponentSpec, EventSpec, PropKind,
    PropSpec, SlotSpec, Surface, UI_COMPONENT_CATALOG,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_built_article_pipeline_subjects() {
        let program = sample_article_pipeline();
        assert_eq!(program.contracts.len(), 1);
        assert_eq!(program.modules.len(), 3);
        assert_eq!(program.modules[0].kind, ModuleKind::Service);
        assert_eq!(program.modules[1].kind, ModuleKind::Processor);
        assert_eq!(program.modules[2].kind, ModuleKind::Sink);
        assert!(program.validate().is_ok());
    }

    #[test]
    fn validation_rejects_unknown_types() {
        let mut program = sample_article_pipeline();
        program.contracts[0].fields[0].ty = TypeExpr::Named("MissingType".into());
        assert_eq!(
            program.validate().unwrap_err(),
            "unknown type `MissingType`"
        );
    }

    pub fn sample_article_pipeline() -> Program {
        Program {
            version: Some("0.2.0".into()),
            subsets: vec![
                Subset {
                    name: "Uri".into(),
                    base: TypeExpr::Named("Str".into()),
                    predicate: Some(".contains(\"://\")".into()),
                    span: Span::default(),
                },
                Subset {
                    name: "Emb768".into(),
                    base: TypeExpr::Vec {
                        elem: "num32".into(),
                        len: Some(768),
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
                    Field::new("vector_embedding", TypeExpr::Named("Emb768".into())),
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
                                    namespace: Some("http".into()),
                                    name: "get".into(),
                                    args: vec![],
                                },
                                PipelineStep::Call {
                                    namespace: Some("html".into()),
                                    name: "extract_body".into(),
                                    args: vec![],
                                },
                                PipelineStep::Name("ArticlePayload".into()),
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
                        params: vec![],
                        pipeline: Pipeline {
                            steps: vec![
                                PipelineStep::Call {
                                    namespace: Some("tensor".into()),
                                    name: "tokenize".into(),
                                    args: vec![],
                                },
                                PipelineStep::Call {
                                    namespace: Some("tensor".into()),
                                    name: "infer".into(),
                                    args: vec![TraitArg {
                                        name: "prefer".into(),
                                        value: "CUDA".into(),
                                    }],
                                },
                            ],
                        },
                    }],
                    span: Span::default(),
                },
                Module {
                    name: "RealtimeCache".into(),
                    kind: ModuleKind::Sink,
                    traits: vec![
                        TraitArg {
                            name: "latency".into(),
                            value: "2ms".into(),
                        },
                        TraitArg {
                            name: "storage".into(),
                            value: "MemoryMapped".into(),
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
                                    name: "share_buffer".into(),
                                    args: vec![],
                                },
                                PipelineStep::Call {
                                    namespace: Some("store".into()),
                                    name: "upsert_primary".into(),
                                    args: vec![],
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
}
