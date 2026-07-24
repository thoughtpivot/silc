//! Contract subject: schemas from `class` / `has` / `subset`.

use crate::types::{Span, TypeExpr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subset {
    pub name: String,
    pub base: TypeExpr,
    pub predicate: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    pub name: String,
    pub fields: Vec<Field>,
    pub span: Span,
}
