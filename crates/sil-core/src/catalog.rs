//! One node model for every `ns::node(...)` catalog.
//!
//! `ui::`, `game::` / `scene::`, and `loop::` used to each carry a parallel
//! spec family. They now share [`NodeSpec`]: a name, options (with a kind and
//! an optional closed value set), a child policy, surfaces, and doc. Namespace
//! validators keep the rules that are genuinely theirs (loop taint, the
//! `game::mesh` asset/shape exclusion) and call the helpers here for required
//! options, unknown options, closed enums, child policy, and catalog lines.

use crate::expr::Expr;

/// Render or runtime surface a node declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Web,
    Terminal,
    WebGpu,
}

impl Surface {
    pub fn as_str(self) -> &'static str {
        match self {
            Surface::Web => "web",
            Surface::Terminal => "terminal",
            Surface::WebGpu => "webgpu",
        }
    }
}

/// Kind of one `:option(value)`.
///
/// The historical name in code is "prop"; author-facing text says "option".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionKind {
    String,
    Bool,
    Ident,
    StringList,
    Number,
    Flag,
    Expr,
    Node,
    Template,
    Ref,
}

/// One `:option` on a catalog node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionSpec {
    pub name: &'static str,
    pub kind: OptionKind,
    pub required: bool,
    pub description: &'static str,
    pub closed_values: &'static [&'static str],
}

impl OptionSpec {
    pub const fn new(name: &'static str, kind: OptionKind, required: bool) -> Self {
        Self {
            name,
            kind,
            required,
            description: "",
            closed_values: &[],
        }
    }
}

/// Named slot on a UI primitive (`card` actions, and similar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotSpec {
    pub name: &'static str,
    pub component: &'static str,
    pub required: bool,
}

/// Event a node can emit (`:on(click(...))`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSpec {
    pub name: &'static str,
}

/// Which children a node accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildPolicy {
    None,
    AnyOf(&'static [&'static str]),
    Any,
}

/// Role of a loop node. Other namespaces use [`NodeRole::Plain`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    Plain,
    Root,
    Trigger,
    Step,
    Block,
}

/// One catalog node, shared by every namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// Options. The field keeps the historical name `props` so catalog
    /// literals and callers stay stable; author-facing text says "option".
    pub props: &'static [OptionSpec],
    pub children: ChildPolicy,
    pub surfaces: &'static [Surface],
    pub slots: &'static [SlotSpec],
    pub events: &'static [EventSpec],
    pub role: NodeRole,
}

pub const UI_SURFACES: &[Surface] = &[Surface::Web, Surface::Terminal];
pub const SCENE_SURFACES: &[Surface] = &[Surface::WebGpu];
pub const NO_SURFACES: &[Surface] = &[];
pub const NO_SLOTS: &[SlotSpec] = &[];
pub const NO_EVENTS: &[EventSpec] = &[];

impl NodeSpec {
    /// Options on this node. Same slice as [`Self::props`].
    pub fn options(&self) -> &'static [OptionSpec] {
        self.props
    }
}

pub fn is_known_option(props: &[OptionSpec], name: &str) -> bool {
    props.iter().any(|prop| prop.name == name)
}

pub fn format_option_list(props: &[OptionSpec]) -> String {
    if props.is_empty() {
        return "none".to_string();
    }
    props
        .iter()
        .map(|prop| {
            let mut item = if prop.required {
                format!("`{}`", prop.name)
            } else {
                format!("`{}?`", prop.name)
            };
            if matches!(prop.kind, OptionKind::Flag) {
                item.push_str(" (flag)");
            }
            item
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// UI catalog lines mark required options with `(required)` rather than a bare
/// name, matching the AGENTS contract pinned by `docs_conformance`.
pub fn format_ui_option_list(props: &[OptionSpec]) -> String {
    if props.is_empty() {
        return "none".to_string();
    }
    props
        .iter()
        .map(|prop| {
            let mut item = if prop.required {
                format!("`{}` (required)", prop.name)
            } else {
                format!("`{}?`", prop.name)
            };
            if matches!(prop.kind, OptionKind::Flag) {
                item.push_str(" (flag)");
            }
            item
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn format_child_policy(policy: ChildPolicy) -> String {
    match policy {
        ChildPolicy::None => "none".to_string(),
        ChildPolicy::Any => "any".to_string(),
        ChildPolicy::AnyOf(names) => names
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

pub fn child_allowed(policy: ChildPolicy, child: &str) -> bool {
    match policy {
        ChildPolicy::None => false,
        ChildPolicy::Any => true,
        ChildPolicy::AnyOf(names) => names.contains(&child),
    }
}

/// Why a closed-enum value was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedEnumError {
    /// A bare identifier that is not in the closed set.
    NotAllowed(String),
    /// A quoted string. Closed enums are bare identifiers only (decision D5).
    StringForm(String),
}

/// Closed-enum rule, uniform across namespaces: a bare identifier from the
/// set, or a dynamic expression (left unchecked). Quoted strings are rejected
/// with a fix-it.
pub fn check_closed_enum(expr: &Expr, allowed: &[&str]) -> Result<(), ClosedEnumError> {
    match expr {
        Expr::Ident(value) if allowed.iter().any(|item| *item == value) => Ok(()),
        Expr::Ident(value) => Err(ClosedEnumError::NotAllowed(value.clone())),
        Expr::String(value) => Err(ClosedEnumError::StringForm(value.clone())),
        _ => Ok(()),
    }
}

/// Whether an expression satisfies an option kind. `ref_ok` is the caller's
/// `Resource.capability` parse, which only loop options need.
pub fn option_accepts(kind: OptionKind, expr: &Expr, ref_ok: bool) -> bool {
    match kind {
        OptionKind::String | OptionKind::Template => matches!(expr, Expr::String(_)),
        OptionKind::Ident => matches!(expr, Expr::Ident(_)),
        OptionKind::Number => matches!(expr, Expr::Number(n) if n.parse::<u64>().is_ok()),
        OptionKind::Flag => matches!(expr, Expr::Bool(true)),
        OptionKind::Bool => matches!(expr, Expr::Bool(_)),
        OptionKind::Ref => ref_ok,
        OptionKind::Expr | OptionKind::Node | OptionKind::StringList => true,
    }
}

pub fn option_expected(kind: OptionKind) -> &'static str {
    match kind {
        OptionKind::String | OptionKind::Template => "a string",
        OptionKind::Ident => "a bare name",
        OptionKind::Number => "a whole number",
        OptionKind::Flag => "a bare flag",
        OptionKind::Bool => "true or false",
        OptionKind::Ref => "`Resource.capability`",
        OptionKind::Expr => "an expression",
        OptionKind::Node => "a nested node",
        OptionKind::StringList => "a list of strings",
    }
}

pub fn closed_enum_message(
    namespace: &str,
    node: &str,
    option: &str,
    err: &ClosedEnumError,
    allowed: &[&str],
) -> String {
    let set = allowed.join("|");
    match err {
        ClosedEnumError::NotAllowed(value) => format!(
            "{namespace}::{node} `:{option}({value})` must be one of {set}"
        ),
        ClosedEnumError::StringForm(value) => format!(
            "{namespace}::{node} `:{option}` takes a bare name; write `:{option}({value})` instead of `:{option}(\"{value}\")`"
        ),
    }
}
