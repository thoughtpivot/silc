//! Module subject: `class … is service|processor|sink` + methods.

use crate::constraint::TraitArg;
use crate::contract::Field;
use crate::pipeline::Pipeline;
use crate::types::{Span, TypeExpr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    Service,
    Processor,
    Sink,
    Task,
    Unknown,
}

impl ModuleKind {
    pub fn parse(name: &str) -> Self {
        match name {
            "service" => ModuleKind::Service,
            "processor" => ModuleKind::Processor,
            "sink" => ModuleKind::Sink,
            "task" => ModuleKind::Task,
            _ => ModuleKind::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ModuleKind::Service => "service",
            ModuleKind::Processor => "processor",
            ModuleKind::Sink => "sink",
            ModuleKind::Task => "task",
            ModuleKind::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub named: bool,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    pub name: String,
    pub params: Vec<Param>,
    pub pipeline: Pipeline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    pub kind: ModuleKind,
    pub traits: Vec<TraitArg>,
    pub fields: Vec<Field>,
    pub methods: Vec<Method>,
    pub span: Span,
}
