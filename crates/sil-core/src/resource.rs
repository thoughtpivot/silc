//! Persistent resources backed by Contracts and query/mutation pipelines.

use crate::module::Param;
use crate::pipeline::Pipeline;
use crate::types::{Span, TypeExpr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Query,
    Mutation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceMethod {
    pub kind: ResourceKind,
    pub name: String,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeExpr>,
    pub pipeline: Pipeline,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resource {
    pub name: String,
    pub methods: Vec<ResourceMethod>,
    /// Optional Contract this resource primarily stores.
    pub contract: Option<String>,
    /// SQLite table name; defaults to snake_case of resource name.
    pub table: Option<String>,
    pub span: Span,
}

impl Resource {
    pub fn table_name(&self) -> String {
        self.table.clone().unwrap_or_else(|| {
            let mut out = String::new();
            for (i, ch) in self.name.chars().enumerate() {
                if ch.is_uppercase() {
                    if i > 0 {
                        out.push('_');
                    }
                    out.push(ch.to_ascii_lowercase());
                } else {
                    out.push(ch);
                }
            }
            if out.is_empty() {
                "resource".into()
            } else {
                out
            }
        })
    }

    pub fn find_method(&self, name: &str) -> Option<&ResourceMethod> {
        self.methods.iter().find(|m| m.name == name)
    }
}

/// Derived CRUD action exposed on the HTTP/runtime surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDef {
    pub id: String,
    pub resource: String,
    pub method: String,
    pub http_method: String,
    pub path: String,
    pub kind: ResourceKind,
}
