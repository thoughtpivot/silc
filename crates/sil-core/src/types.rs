#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    Named(String),
    /// Fixed-length vector `Vec[num32; 768]`.
    Vec {
        elem: String,
        len: Option<u64>,
    },
    /// Open array `[Product]` / `[Str]`.
    Array(Box<TypeExpr>),
    Optional(Box<TypeExpr>),
}

impl TypeExpr {
    pub fn name(&self) -> &str {
        match self {
            TypeExpr::Named(n) => n,
            TypeExpr::Vec { elem, .. } => elem,
            TypeExpr::Array(inner) => inner.name(),
            TypeExpr::Optional(inner) => inner.name(),
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, TypeExpr::Array(_))
    }

    pub fn elem_type(&self) -> Option<&TypeExpr> {
        match self {
            TypeExpr::Array(inner) | TypeExpr::Optional(inner) => Some(inner),
            _ => None,
        }
    }
}
