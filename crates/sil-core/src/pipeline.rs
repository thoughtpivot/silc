//! Pipeline subject: `==>` feed steps.

use crate::constraint::TraitArg;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineStep {
    Name(String),
    FieldAccess {
        base: String,
        field: String,
    },
    Call {
        namespace: Option<String>,
        name: String,
        args: Vec<TraitArg>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pipeline {
    pub steps: Vec<PipelineStep>,
}

impl Pipeline {
    pub fn namespaces(&self) -> Vec<&str> {
        self.steps
            .iter()
            .filter_map(|s| match s {
                PipelineStep::Call {
                    namespace: Some(ns),
                    ..
                } => Some(ns.as_str()),
                _ => None,
            })
            .collect()
    }
}
