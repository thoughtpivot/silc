//! Built-in UI primitive catalog with dual-surface (web + terminal) contracts.

use crate::component::{UiNode, UiTemplate};
use crate::contract::Contract;
use crate::expr::Expr;

/// Render surface every primitive and author component must support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Web,
    Terminal,
}

impl Surface {
    pub fn as_str(self) -> &'static str {
        match self {
            Surface::Web => "web",
            Surface::Terminal => "terminal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    String,
    Bool,
    Ident,
    StringList,
    Expr,
    Flag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropSpec {
    pub name: &'static str,
    pub kind: PropKind,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotSpec {
    pub name: &'static str,
    pub component: &'static str,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildPolicy {
    None,
    AnyOf(&'static [&'static str]),
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSpec {
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentSpec {
    pub name: &'static str,
    pub props: &'static [PropSpec],
    pub slots: &'static [SlotSpec],
    pub children: ChildPolicy,
    pub events: &'static [EventSpec],
    /// Every builtin must declare both surfaces.
    pub surfaces: &'static [Surface],
}

const BOTH: &[Surface] = &[Surface::Web, Surface::Terminal];

const LAYOUT_CHILDREN: &[&str] = &[
    "stack",
    "row",
    "grid",
    "card",
    "heading",
    "text",
    "form",
    "text_input",
    "textarea",
    "radio_group",
    "select",
    "checkbox",
    "switch",
    "field",
    "button",
    "toolbar",
    "chat",
    "chat_history",
    "search_input",
    "filter_bar",
    "collection",
    "list",
    "table",
    "badge",
    "alert",
    "divider",
    "section",
    "description_list",
    "tabs",
    "dialog",
    "loading",
    "empty",
    "nav_item",
];

const FORM_CHILDREN: &[&str] = &[
    "stack",
    "row",
    "grid",
    "card",
    "heading",
    "text",
    "text_input",
    "textarea",
    "radio_group",
    "select",
    "checkbox",
    "switch",
    "field",
    "button",
    "toolbar",
    "badge",
    "alert",
    "divider",
    "section",
    "loading",
    "empty",
];

pub const UI_COMPONENT_CATALOG: &[ComponentSpec] = &[
    ComponentSpec {
        name: "page",
        props: &[],
        slots: &[
            SlotSpec {
                name: "app_bar",
                component: "app_bar",
                required: false,
            },
            SlotSpec {
                name: "side_panel",
                component: "side_panel",
                required: false,
            },
            SlotSpec {
                name: "footer",
                component: "footer",
                required: false,
            },
        ],
        children: ChildPolicy::AnyOf(LAYOUT_CHILDREN),
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "app_bar",
        props: &[PropSpec {
            name: "title",
            kind: PropKind::Expr,
            required: true,
        }],
        slots: &[],
        children: ChildPolicy::None,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "side_panel",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(&["nav_item"]),
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "nav_item",
        props: &[
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "to",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "active",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "click" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "toolbar",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(&["button"]),
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "stack",
        props: &[],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "row",
        props: &[],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "grid",
        props: &[],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "card",
        props: &[],
        slots: &[SlotSpec {
            name: "actions",
            component: "row",
            required: false,
        }],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "heading",
        props: &[
            PropSpec {
                name: "text",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "level",
                kind: PropKind::Ident,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "text",
        props: &[PropSpec {
            name: "text",
            kind: PropKind::Expr,
            required: true,
        }],
        slots: &[],
        children: ChildPolicy::None,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "form",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(FORM_CHILDREN),
        events: &[EventSpec { name: "submit" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "text_input",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "value",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "placeholder",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "disabled",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "input" }, EventSpec { name: "change" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "textarea",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "value",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "disabled",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "input" }, EventSpec { name: "change" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "radio_group",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "value",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "options",
                kind: PropKind::StringList,
                required: true,
            },
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "disabled",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "change" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "select",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "value",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "options",
                kind: PropKind::StringList,
                required: true,
            },
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "placeholder",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "disabled",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "change" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "checkbox",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "checked",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "disabled",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "change" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "switch",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "checked",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "disabled",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "change" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "field",
        props: &[
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "hint",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "error",
                kind: PropKind::Expr,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::AnyOf(FORM_CHILDREN),
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "button",
        props: &[
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "variant",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "size",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "submit",
                kind: PropKind::Flag,
                required: false,
            },
            PropSpec {
                name: "active",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "disabled",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "click" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "chat",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "value",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "placeholder",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "session",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "loading",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "error",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "context",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "persona",
                kind: PropKind::Expr,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "send" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "chat_history",
        props: &[
            PropSpec {
                name: "title",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "items",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "collapsible",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "search_input",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "value",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "placeholder",
                kind: PropKind::Expr,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "input" }, EventSpec { name: "submit" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "filter_bar",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(&["search_input", "button", "text_input"]),
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "collection",
        props: &[
            PropSpec {
                name: "items",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "empty_text",
                kind: PropKind::Expr,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "list",
        props: &[PropSpec {
            name: "items",
            kind: PropKind::Expr,
            required: false,
        }],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "table",
        props: &[
            PropSpec {
                name: "rows",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "columns",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "empty_text",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "filter_field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "filter_column",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "filter_all",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "sortable",
                kind: PropKind::Flag,
                required: false,
            },
            PropSpec {
                name: "searchable",
                kind: PropKind::Flag,
                required: false,
            },
            PropSpec {
                name: "selectable",
                kind: PropKind::Flag,
                required: false,
            },
            PropSpec {
                name: "dense",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "badge",
        props: &[
            PropSpec {
                name: "text",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "tone",
                kind: PropKind::Ident,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "alert",
        props: &[
            PropSpec {
                name: "text",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "title",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "tone",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "dismissible",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "dismiss" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "divider",
        props: &[PropSpec {
            name: "label",
            kind: PropKind::Expr,
            required: false,
        }],
        slots: &[],
        children: ChildPolicy::None,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "section",
        props: &[
            PropSpec {
                name: "title",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "description",
                kind: PropKind::Expr,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "footer",
        props: &[],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "description_list",
        props: &[PropSpec {
            name: "items",
            kind: PropKind::Expr,
            required: true,
        }],
        slots: &[],
        children: ChildPolicy::None,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "tabs",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "value",
                kind: PropKind::Expr,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::AnyOf(&["tab"]),
        events: &[EventSpec { name: "change" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "tab",
        props: &[
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "value",
                kind: PropKind::Expr,
                required: true,
            },
        ],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "dialog",
        props: &[
            PropSpec {
                name: "open",
                kind: PropKind::Expr,
                required: true,
            },
            PropSpec {
                name: "title",
                kind: PropKind::Expr,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[EventSpec { name: "confirm" }, EventSpec { name: "cancel" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "loading",
        props: &[PropSpec {
            name: "text",
            kind: PropKind::Expr,
            required: false,
        }],
        slots: &[],
        children: ChildPolicy::None,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "empty",
        props: &[PropSpec {
            name: "text",
            kind: PropKind::Expr,
            required: false,
        }],
        slots: &[],
        children: ChildPolicy::None,
        events: &[],
        surfaces: BOTH,
    },
];

pub fn lookup_component(name: &str) -> Option<&'static ComponentSpec> {
    UI_COMPONENT_CATALOG.iter().find(|c| c.name == name)
}

pub fn catalog_component_names() -> Vec<&'static str> {
    UI_COMPONENT_CATALOG.iter().map(|c| c.name).collect()
}

/// Canonical one-line API reference for docs / AGENTS.md (kept in sync by tests).
pub fn format_component_catalog_line(spec: &ComponentSpec) -> String {
    let props = if spec.props.is_empty() {
        "none".to_string()
    } else {
        spec.props
            .iter()
            .map(|prop| {
                let mut item = if prop.required {
                    format!("`{}` (required)", prop.name)
                } else {
                    format!("`{}?`", prop.name)
                };
                if matches!(prop.kind, PropKind::Flag) {
                    item.push_str(" (flag)");
                }
                item
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let events = if spec.events.is_empty() {
        "none".to_string()
    } else {
        spec.events
            .iter()
            .map(|event| format!("`{}`", event.name))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let slots = if spec.slots.is_empty() {
        "none".to_string()
    } else {
        spec.slots
            .iter()
            .map(|slot| format!("`{}`→`{}`", slot.name, slot.component))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let children = match spec.children {
        ChildPolicy::None => "none".to_string(),
        ChildPolicy::Any => "any".to_string(),
        ChildPolicy::AnyOf(names) => format!(
            "anyOf({})",
            names
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    format!(
        "- `ui::{}` — props: {}; events: {}; slots: {}; children: {}; surfaces: web+terminal",
        spec.name, props, events, slots, children
    )
}

const VARIANT_VALUES: &[&str] = &["primary", "secondary", "destructive", "ghost"];
const TONE_VALUES: &[&str] = &["default", "muted", "info", "success", "warning", "danger"];
const SIZE_VALUES: &[&str] = &["sm", "md", "lg"];

fn validate_closed_ident_prop(node: &UiNode, prop: &str, allowed: &[&str]) -> Result<(), String> {
    let Some(expr) = node.prop(prop) else {
        return Ok(());
    };
    let value = match expr {
        Expr::Ident(s) | Expr::String(s) => s.as_str(),
        // Dynamic expressions are allowed; only closed tokens are validated.
        _ => return Ok(()),
    };
    if allowed.iter().any(|item| *item == value) {
        return Ok(());
    }
    Err(format!(
        "invalid `:{}({})` on `ui::{}`; expected one of: {}",
        prop,
        value,
        node.component,
        allowed.join(", ")
    ))
}

pub fn validate_builtin_node(node: &UiNode) -> Result<(), String> {
    let Some(spec) = lookup_component(&node.component) else {
        // Author component — validated against Program components elsewhere.
        return Ok(());
    };
    if !spec.surfaces.contains(&Surface::Web) || !spec.surfaces.contains(&Surface::Terminal) {
        return Err(format!(
            "builtin `ui::{}` must support both web and terminal surfaces",
            node.component
        ));
    }
    for prop_spec in spec.props {
        if prop_spec.required && node.prop(prop_spec.name).is_none() {
            return Err(format!(
                "`ui::{}` missing required prop `:{}`",
                node.component, prop_spec.name
            ));
        }
    }
    for (name, _) in &node.props {
        if !spec.props.iter().any(|p| p.name == *name) {
            return Err(format!(
                "unknown prop `:{}` on `ui::{}`",
                name, node.component
            ));
        }
    }
    for binding in &node.events {
        if !spec.events.iter().any(|e| e.name == binding.event) {
            return Err(format!(
                "unknown event `:on({})` on `ui::{}`",
                binding.event, node.component
            ));
        }
    }
    validate_closed_ident_prop(node, "variant", VARIANT_VALUES)?;
    validate_closed_ident_prop(node, "tone", TONE_VALUES)?;
    validate_closed_ident_prop(node, "size", SIZE_VALUES)?;
    Ok(())
}

pub fn validate_template(template: &UiTemplate) -> Result<(), String> {
    match template {
        UiTemplate::Node(node) => {
            validate_builtin_node(node)?;
            for child in &node.children {
                validate_template(child)?;
            }
            for (_, slot) in &node.slots {
                validate_template(slot)?;
            }
            Ok(())
        }
        UiTemplate::When {
            body, else_body, ..
        } => {
            validate_template(body)?;
            if let Some(else_body) = else_body {
                validate_template(else_body)?;
            }
            Ok(())
        }
        UiTemplate::For { body, .. } => validate_template(body),
        UiTemplate::Block(items) => {
            for item in items {
                validate_template(item)?;
            }
            Ok(())
        }
    }
}

/// Legacy helper kept for transitional tests — validates a root page node.
pub fn validate_view_root(root: &UiNode, _contract: Option<&Contract>) -> Result<(), String> {
    if root.component != "page" {
        return Err("component root must be `ui::page`".into());
    }
    validate_builtin_node(root)?;
    for child in &root.children {
        validate_template(child)?;
    }
    for (_, slot) in &root.slots {
        validate_template(slot)?;
    }
    Ok(())
}

pub fn expr_as_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::String(s) => Some(s.clone()),
        Expr::Ident(s) | Expr::Var(s) => Some(s.clone()),
        Expr::Number(n) => Some(n.clone()),
        Expr::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::UiNode;
    use crate::types::Span;

    fn node(component: &str, props: Vec<(&str, Expr)>) -> UiNode {
        UiNode {
            component: component.into(),
            props: props
                .into_iter()
                .map(|(name, expr)| (name.to_string(), expr))
                .collect(),
            events: vec![],
            slots: vec![],
            children: vec![],
            span: Span::default(),
        }
    }

    #[test]
    fn rejects_unknown_variant_tone_and_size() {
        let bad_variant = node(
            "button",
            vec![
                ("label", Expr::String("Go".into())),
                ("variant", Expr::Ident("neon".into())),
            ],
        );
        assert!(validate_builtin_node(&bad_variant)
            .unwrap_err()
            .contains("invalid `:variant(neon)`"));

        let bad_tone = node(
            "alert",
            vec![
                ("text", Expr::String("Hi".into())),
                ("tone", Expr::Ident("purple".into())),
            ],
        );
        assert!(validate_builtin_node(&bad_tone)
            .unwrap_err()
            .contains("invalid `:tone(purple)`"));

        let bad_size = node(
            "button",
            vec![
                ("label", Expr::String("Go".into())),
                ("size", Expr::Ident("xl".into())),
            ],
        );
        assert!(validate_builtin_node(&bad_size)
            .unwrap_err()
            .contains("invalid `:size(xl)`"));
    }

    #[test]
    fn catalog_doc_lines_are_stable() {
        let button = lookup_component("button").unwrap();
        assert_eq!(
            format_component_catalog_line(button),
            "- `ui::button` — props: `label` (required), `variant?`, `size?`, `submit?` (flag), `active?`, `disabled?` (flag); events: `click`; slots: none; children: none; surfaces: web+terminal"
        );
        assert_eq!(UI_COMPONENT_CATALOG.len(), 38);
        for spec in UI_COMPONENT_CATALOG {
            let line = format_component_catalog_line(spec);
            assert!(line.starts_with(&format!("- `ui::{}` — ", spec.name)));
            assert!(line.contains("surfaces: web+terminal"));
        }
    }

    #[test]
    fn accepts_closed_variant_and_tone_tokens() {
        let button = node(
            "button",
            vec![
                ("label", Expr::String("Save".into())),
                ("variant", Expr::Ident("primary".into())),
                ("size", Expr::Ident("sm".into())),
            ],
        );
        assert!(validate_builtin_node(&button).is_ok());

        let alert = node(
            "alert",
            vec![
                ("text", Expr::String("Ready".into())),
                ("tone", Expr::Ident("info".into())),
            ],
        );
        assert!(validate_builtin_node(&alert).is_ok());
    }
}
