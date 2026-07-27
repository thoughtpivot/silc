//! Persistent resources backed by Contracts and capability declarations.

use crate::expr::Expr;
use crate::module::Param;
use crate::pipeline::Pipeline;
use crate::types::{Span, TypeExpr};

/// A declarative seed row authored as `seed Contract.new(:field(value), …);`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceSeed {
    pub contract: String,
    pub fields: Vec<(String, Expr)>,
    pub span: Span,
}

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
    /// True when authored as a bodyless capability (`query list;`).
    pub shorthand: bool,
}

impl ResourceMethod {
    /// Expand a bodyless capability into a conventional CRUD signature.
    pub fn expand_with_contract(&mut self, contract: &str) -> Result<(), String> {
        if !self.shorthand {
            return Ok(());
        }
        let (params, return_ty) = synthesize_capability(&self.kind, &self.name, contract)?;
        self.params = params;
        self.return_ty = return_ty;
        self.pipeline = Pipeline { steps: vec![] };
        self.shorthand = false;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resource {
    pub name: String,
    pub methods: Vec<ResourceMethod>,
    /// Contract this resource stores (`resource Name for Contract`).
    pub contract: Option<String>,
    /// SQLite table name; defaults to snake_case of resource name.
    pub table: Option<String>,
    /// Idempotent initial rows (`seed Contract.new(…);`).
    pub seeds: Vec<ResourceSeed>,
    pub span: Span,
}

impl Resource {
    pub fn table_name(&self) -> String {
        self.table.clone().unwrap_or_else(|| snake_case(&self.name))
    }

    pub fn find_method(&self, name: &str) -> Option<&ResourceMethod> {
        self.methods.iter().find(|m| m.name == name)
    }

    /// Expand bodyless capabilities into conventional CRUD signatures.
    pub fn expand_shorthand_methods(&mut self) -> Result<(), String> {
        let contract = self.contract.clone().ok_or_else(|| {
            format!(
                "resource `{}` must declare `for Contract` (e.g. `resource {} for Item {{ … }}`)",
                self.name, self.name
            )
        })?;
        for method in &mut self.methods {
            method.expand_with_contract(&contract)?;
        }
        Ok(())
    }
}

fn synthesize_capability(
    kind: &ResourceKind,
    name: &str,
    contract: &str,
) -> Result<(Vec<Param>, Option<TypeExpr>), String> {
    let item = Param {
        name: "item".into(),
        ty: Some(TypeExpr::Named(contract.to_string())),
        named: false,
        default: None,
    };
    match (kind, name) {
        (ResourceKind::Query, "list" | "all") => Ok((
            vec![],
            Some(TypeExpr::Array(Box::new(TypeExpr::Named(
                contract.to_string(),
            )))),
        )),
        (ResourceKind::Query, "get") => {
            Ok((vec![item], Some(TypeExpr::Named(contract.to_string()))))
        }
        (ResourceKind::Mutation, "create" | "add" | "update" | "delete" | "remove") => {
            Ok((vec![item], None))
        }
        (ResourceKind::Query, other) => Err(format!(
            "unknown resource query capability `{other}`; supported: list, get"
        )),
        (ResourceKind::Mutation, other) => Err(format!(
            "unknown resource mutation capability `{other}`; supported: create, update, delete"
        )),
    }
}

pub fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
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
}

/// Deterministic persistence collection for a synthesized processor sink.
pub fn sink_table_for_contract(contract: &str) -> String {
    let snake = snake_case(contract);
    if snake.ends_with('s') {
        snake
    } else {
        format!("{snake}s")
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
