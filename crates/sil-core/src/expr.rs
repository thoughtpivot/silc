//! Expression AST for component templates, bindings, and handlers.

use crate::types::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    String(String),
    Number(String),
    Bool(bool),
    Ident(String),
    /// `$name` or `$.field` local/state access
    Var(String),
    Member {
        base: Box<Expr>,
        field: String,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    BinOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    List(Vec<Expr>),
    New {
        ty: String,
        fields: Vec<(String, Expr)>,
    },
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
    },
    Emit {
        event: String,
        payload: Option<Box<Expr>>,
    },
    Navigate {
        path: String,
    },
    Await(Box<Expr>),
    Interpolated(Vec<InterpPart>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterpPart {
    Lit(String),
    Expr(Expr),
}

impl Expr {
    pub fn as_ident(&self) -> Option<&str> {
        match self {
            Expr::Ident(s) | Expr::Var(s) | Expr::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_string_literal(&self) -> Option<&str> {
        match self {
            Expr::String(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedExpr {
    pub expr: Expr,
    pub span: Span,
}

impl SpannedExpr {
    pub fn new(expr: Expr, span: Span) -> Self {
        Self { expr, span }
    }
}
