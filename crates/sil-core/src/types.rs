#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    Named(String),
    Vec { elem: String, len: Option<u64> },
}

impl TypeExpr {
    pub fn name(&self) -> &str {
        match self {
            TypeExpr::Named(n) => n,
            TypeExpr::Vec { elem, .. } => elem,
        }
    }
}
