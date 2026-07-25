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
    "button",
    "toolbar",
    "chat",
    "chat_history",
    "search_input",
    "filter_bar",
    "collection",
    "list",
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
    "button",
    "toolbar",
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
        ],
        slots: &[],
        children: ChildPolicy::None,
        events: &[EventSpec { name: "change" }],
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
