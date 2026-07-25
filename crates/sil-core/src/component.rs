//! Author-defined and standard-library UI components.

use crate::expr::Expr;
use crate::module::{Method, Param};
use crate::types::{Span, TypeExpr};

#[derive(Debug, Clone, PartialEq)]
pub struct SlotDecl {
    pub name: String,
    pub required: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmitDecl {
    pub name: String,
    pub payload: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryBinding {
    pub name: String,
    pub resource: String,
    pub method: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompField {
    pub name: String,
    pub ty: TypeExpr,
    pub default: Option<Expr>,
    pub is_state: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiTemplate {
    Node(UiNode),
    When {
        condition: Expr,
        body: Box<UiTemplate>,
        else_body: Option<Box<UiTemplate>>,
    },
    For {
        items: Expr,
        item_name: String,
        body: Box<UiTemplate>,
    },
    Block(Vec<UiTemplate>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventBinding {
    pub event: String,
    pub handler: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiNode {
    /// Builtin `page`/`button` or author component name `ProductCard`.
    pub component: String,
    pub props: Vec<(String, Expr)>,
    pub events: Vec<EventBinding>,
    pub slots: Vec<(String, UiTemplate)>,
    pub children: Vec<UiTemplate>,
    pub span: Span,
}

impl UiNode {
    pub fn contains_component(&self, name: &str) -> bool {
        if self.component == name {
            return true;
        }
        self.children.iter().any(|c| c.contains_component(name))
            || self.slots.iter().any(|(_, t)| t.contains_component(name))
    }

    pub fn prop(&self, name: &str) -> Option<&Expr> {
        self.props.iter().find(|(n, _)| n == name).map(|(_, e)| e)
    }
}

impl UiTemplate {
    pub fn contains_component(&self, name: &str) -> bool {
        match self {
            UiTemplate::Node(n) => n.contains_component(name),
            UiTemplate::When {
                body, else_body, ..
            } => {
                body.contains_component(name)
                    || else_body
                        .as_ref()
                        .map(|b| b.contains_component(name))
                        .unwrap_or(false)
            }
            UiTemplate::For { body, .. } => body.contains_component(name),
            UiTemplate::Block(items) => items.iter().any(|i| i.contains_component(name)),
        }
    }

    pub fn as_node(&self) -> Option<&UiNode> {
        match self {
            UiTemplate::Node(n) => Some(n),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub props: Vec<CompField>,
    pub state: Vec<CompField>,
    pub slots: Vec<SlotDecl>,
    pub emits: Vec<EmitDecl>,
    pub queries: Vec<QueryBinding>,
    pub methods: Vec<Method>,
    /// Handler methods that are not `render` — may use expression bodies.
    pub handlers: Vec<Handler>,
    pub render: UiTemplate,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Handler {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Expr>,
    pub span: Span,
}

impl Component {
    pub fn all_fields(&self) -> impl Iterator<Item = &CompField> {
        self.props.iter().chain(self.state.iter())
    }
}
