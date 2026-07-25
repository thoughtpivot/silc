//! Declarative UI view subject: semantic component trees for `ui::web(:view(...))`.

use crate::contract::Contract;
use crate::types::Span;
use std::collections::{BTreeSet, HashSet};

/// A named, compiler-validated UI tree authored with `class X is view`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiView {
    pub name: String,
    pub root: UiNode,
    pub span: Span,
}

/// One semantic component instance in a view tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNode {
    pub component: String,
    pub props: Vec<(String, UiValue)>,
    pub slots: Vec<(String, UiNode)>,
    pub children: Vec<UiNode>,
    pub span: Span,
}

/// Typed prop values. Framework/CSS strings are not allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiValue {
    String(String),
    Bool(bool),
    Ident(String),
    StringList(Vec<String>),
    Flag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    String,
    Bool,
    Ident,
    StringList,
    Flag,
}

#[derive(Debug, Clone, Copy)]
pub struct PropSpec {
    pub name: &'static str,
    pub kind: PropKind,
    pub required: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct SlotSpec {
    pub name: &'static str,
    pub component: &'static str,
    pub required: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ChildPolicy {
    None,
    AnyOf(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy)]
pub struct ComponentSpec {
    pub name: &'static str,
    pub props: &'static [PropSpec],
    pub slots: &'static [SlotSpec],
    pub children: ChildPolicy,
}

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
    "toolbar",
    "button",
    "chat",
    "chat_history",
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
    "toolbar",
    "button",
];

/// Compiler-owned semantic catalog for the web vertical slice.
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
    },
    ComponentSpec {
        name: "app_bar",
        props: &[PropSpec {
            name: "title",
            kind: PropKind::String,
            required: true,
        }],
        slots: &[],
        children: ChildPolicy::None,
    },
    ComponentSpec {
        name: "side_panel",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(&["nav_item"]),
    },
    ComponentSpec {
        name: "nav_item",
        props: &[
            PropSpec {
                name: "label",
                kind: PropKind::String,
                required: true,
            },
            PropSpec {
                name: "active",
                kind: PropKind::Flag,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
    },
    ComponentSpec {
        name: "toolbar",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(&["button"]),
    },
    ComponentSpec {
        name: "stack",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(LAYOUT_CHILDREN),
    },
    ComponentSpec {
        name: "row",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(LAYOUT_CHILDREN),
    },
    ComponentSpec {
        name: "grid",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(LAYOUT_CHILDREN),
    },
    ComponentSpec {
        name: "card",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(LAYOUT_CHILDREN),
    },
    ComponentSpec {
        name: "heading",
        props: &[
            PropSpec {
                name: "text",
                kind: PropKind::String,
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
    },
    ComponentSpec {
        name: "text",
        props: &[PropSpec {
            name: "text",
            kind: PropKind::String,
            required: true,
        }],
        slots: &[],
        children: ChildPolicy::None,
    },
    ComponentSpec {
        name: "form",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(FORM_CHILDREN),
    },
    ComponentSpec {
        name: "text_input",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: true,
            },
            PropSpec {
                name: "label",
                kind: PropKind::String,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
    },
    ComponentSpec {
        name: "textarea",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: true,
            },
            PropSpec {
                name: "label",
                kind: PropKind::String,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
    },
    ComponentSpec {
        name: "radio_group",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: true,
            },
            PropSpec {
                name: "options",
                kind: PropKind::StringList,
                required: true,
            },
            PropSpec {
                name: "label",
                kind: PropKind::String,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
    },
    ComponentSpec {
        name: "button",
        props: &[
            PropSpec {
                name: "label",
                kind: PropKind::String,
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
    },
    ComponentSpec {
        name: "chat",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: true,
            },
            PropSpec {
                name: "label",
                kind: PropKind::String,
                required: false,
            },
            PropSpec {
                name: "placeholder",
                kind: PropKind::String,
                required: false,
            },
        ],
        slots: &[],
        children: ChildPolicy::None,
    },
    ComponentSpec {
        name: "chat_history",
        props: &[
            PropSpec {
                name: "title",
                kind: PropKind::String,
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
    },
];

pub fn lookup_component(name: &str) -> Option<&'static ComponentSpec> {
    UI_COMPONENT_CATALOG.iter().find(|c| c.name == name)
}

pub fn catalog_component_names() -> Vec<&'static str> {
    UI_COMPONENT_CATALOG.iter().map(|c| c.name).collect()
}

impl UiValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            UiValue::String(s) => Some(s),
            UiValue::Ident(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_string_list(&self) -> Option<&[String]> {
        match self {
            UiValue::StringList(items) => Some(items),
            _ => None,
        }
    }

    pub fn is_flag_or_true(&self) -> bool {
        matches!(self, UiValue::Flag | UiValue::Bool(true))
    }
}

impl UiNode {
    pub fn prop(&self, name: &str) -> Option<&UiValue> {
        self.props
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }

    pub fn has_flag(&self, name: &str) -> bool {
        self.prop(name)
            .map(|value| value.is_flag_or_true())
            .unwrap_or(false)
    }

    pub fn field_bindings(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_fields(&mut out);
        out.sort();
        out.dedup();
        out
    }

    fn collect_fields(&self, out: &mut Vec<String>) {
        if let Some(UiValue::Ident(field)) = self.prop("field") {
            out.push(field.clone());
        }
        for (_, child) in &self.slots {
            child.collect_fields(out);
        }
        for child in &self.children {
            child.collect_fields(out);
        }
    }

    pub fn has_submit_button(&self) -> bool {
        // `ui::chat` embeds its own send button.
        if self.component == "chat" || (self.component == "button" && self.has_flag("submit")) {
            return true;
        }
        self.slots
            .iter()
            .any(|(_, child)| child.has_submit_button())
            || self.children.iter().any(|child| child.has_submit_button())
    }

    pub fn contains_component(&self, name: &str) -> bool {
        self.component == name
            || self
                .slots
                .iter()
                .any(|(_, child)| child.contains_component(name))
            || self
                .children
                .iter()
                .any(|child| child.contains_component(name))
    }
}

/// Validate a view against the component catalog and an optional Contract for `:field` bindings.
pub fn validate_view(view: &UiView, contract: Option<&Contract>) -> Result<(), String> {
    if view.root.component != "page" {
        return Err(format!(
            "view `{}` must root at `ui::page` (found `ui::{}`)",
            view.name, view.root.component
        ));
    }
    validate_node(&view.root, contract)?;
    if let Some(contract) = contract {
        let fields: HashSet<&str> = contract.fields.iter().map(|f| f.name.as_str()).collect();
        for binding in view.root.field_bindings() {
            if !fields.contains(binding.as_str()) {
                return Err(format!(
                    "view `{}` binds unknown Contract field `{binding}` on `{}`",
                    view.name, contract.name
                ));
            }
        }
    }
    if !view.root.has_submit_button() {
        return Err(format!(
            "view `{}` must include a `ui::button` with `:submit`",
            view.name
        ));
    }
    Ok(())
}

fn validate_node(node: &UiNode, contract: Option<&Contract>) -> Result<(), String> {
    let spec = lookup_component(&node.component).ok_or_else(|| {
        let known = catalog_component_names().join(", ");
        format!(
            "unknown UI component `ui::{}` (supported: {known})",
            node.component
        )
    })?;

    let mut seen_props = BTreeSet::new();
    for (name, value) in &node.props {
        if !seen_props.insert(name.clone()) {
            return Err(format!(
                "duplicate prop `:{name}` on `ui::{}`",
                node.component
            ));
        }
        let prop_spec = spec.props.iter().find(|p| p.name == *name).ok_or_else(|| {
            let allowed = spec
                .props
                .iter()
                .map(|p| format!(":{}", p.name))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "unknown prop `:{name}` on `ui::{}` (allowed: {allowed})",
                node.component
            )
        })?;
        check_prop_kind(spec.name, prop_spec, value)?;
    }
    for prop in spec.props {
        if prop.required && !node.props.iter().any(|(name, _)| name == prop.name) {
            return Err(format!(
                "`ui::{}` requires `:{}`",
                node.component, prop.name
            ));
        }
    }

    let mut seen_slots = BTreeSet::new();
    for (name, child) in &node.slots {
        if !seen_slots.insert(name.clone()) {
            return Err(format!(
                "duplicate slot `:{name}` on `ui::{}`",
                node.component
            ));
        }
        let slot_spec = spec.slots.iter().find(|s| s.name == *name).ok_or_else(|| {
            let allowed = spec
                .slots
                .iter()
                .map(|s| format!(":{}", s.name))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "unknown slot `:{name}` on `ui::{}` (allowed: {allowed})",
                node.component
            )
        })?;
        if child.component != slot_spec.component {
            return Err(format!(
                "slot `:{name}` on `ui::{}` must be `ui::{}` (found `ui::{}`)",
                node.component, slot_spec.component, child.component
            ));
        }
        validate_node(child, contract)?;
    }
    for slot in spec.slots {
        if slot.required && !node.slots.iter().any(|(name, _)| name == slot.name) {
            return Err(format!(
                "`ui::{}` requires slot `:{}`",
                node.component, slot.name
            ));
        }
    }

    match spec.children {
        ChildPolicy::None if !node.children.is_empty() => {
            return Err(format!("`ui::{}` does not accept children", node.component));
        }
        ChildPolicy::AnyOf(allowed) => {
            if node.children.is_empty()
                && matches!(
                    node.component.as_str(),
                    "page" | "form" | "stack" | "toolbar"
                )
            {
                return Err(format!(
                    "`ui::{}` requires at least one child",
                    node.component
                ));
            }
            for child in &node.children {
                if lookup_component(&child.component).is_none() {
                    let known = catalog_component_names().join(", ");
                    return Err(format!(
                        "unknown UI component `ui::{}` (supported: {known})",
                        child.component
                    ));
                }
                if !allowed.contains(&child.component.as_str()) {
                    return Err(format!(
                        "`ui::{}` cannot contain `ui::{}`",
                        node.component, child.component
                    ));
                }
                validate_node(child, contract)?;
            }
        }
        ChildPolicy::None => {}
    }

    if node.component == "button" {
        if let Some(UiValue::Ident(variant)) = node.prop("variant") {
            if !matches!(variant.as_str(), "primary" | "secondary" | "destructive") {
                return Err(format!(
                    "`ui::button` :variant({variant}) is unsupported (use primary, secondary, or destructive)"
                ));
            }
        }
        if let Some(UiValue::Ident(size)) = node.prop("size") {
            if !matches!(size.as_str(), "sm" | "md" | "lg") {
                return Err(format!(
                    "`ui::button` :size({size}) is unsupported (use sm, md, or lg)"
                ));
            }
        }
    }
    if node.component == "heading" {
        if let Some(UiValue::Ident(level)) = node.prop("level") {
            if !matches!(level.as_str(), "1" | "2" | "3" | "h1" | "h2" | "h3") {
                return Err(format!(
                    "`ui::heading` :level({level}) is unsupported (use 1, 2, or 3)"
                ));
            }
        }
    }
    if node.component == "radio_group" {
        if let Some(UiValue::StringList(options)) = node.prop("options") {
            if options.is_empty() {
                return Err("`ui::radio_group` :options must not be empty".into());
            }
        }
    }

    let _ = contract;
    Ok(())
}

fn check_prop_kind(component: &str, spec: &PropSpec, value: &UiValue) -> Result<(), String> {
    let ok = match (spec.kind, value) {
        (PropKind::String, UiValue::String(_)) => true,
        (PropKind::Bool, UiValue::Bool(_)) => true,
        (PropKind::Ident, UiValue::Ident(_)) => true,
        (PropKind::StringList, UiValue::StringList(_)) => true,
        (PropKind::Flag, UiValue::Flag | UiValue::Bool(_)) => true,
        // Allow bare `:active` / `:submit` as Flag when Bool was expected.
        (PropKind::Bool, UiValue::Flag) => true,
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "`ui::{component}` :{} has the wrong value kind",
            spec.name
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Field;
    use crate::types::TypeExpr;

    fn sample_view() -> UiView {
        UiView {
            name: "FeedbackView".into(),
            root: UiNode {
                component: "page".into(),
                props: vec![],
                slots: vec![
                    (
                        "app_bar".into(),
                        UiNode {
                            component: "app_bar".into(),
                            props: vec![("title".into(), UiValue::String("Feedback".into()))],
                            slots: vec![],
                            children: vec![],
                            span: Span::default(),
                        },
                    ),
                    (
                        "side_panel".into(),
                        UiNode {
                            component: "side_panel".into(),
                            props: vec![],
                            slots: vec![],
                            children: vec![UiNode {
                                component: "nav_item".into(),
                                props: vec![
                                    ("label".into(), UiValue::String("Inbox".into())),
                                    ("active".into(), UiValue::Flag),
                                ],
                                slots: vec![],
                                children: vec![],
                                span: Span::default(),
                            }],
                            span: Span::default(),
                        },
                    ),
                ],
                children: vec![UiNode {
                    component: "form".into(),
                    props: vec![],
                    slots: vec![],
                    children: vec![UiNode {
                        component: "stack".into(),
                        props: vec![],
                        slots: vec![],
                        children: vec![
                            UiNode {
                                component: "text_input".into(),
                                props: vec![
                                    ("field".into(), UiValue::Ident("author".into())),
                                    ("label".into(), UiValue::String("Author".into())),
                                ],
                                slots: vec![],
                                children: vec![],
                                span: Span::default(),
                            },
                            UiNode {
                                component: "radio_group".into(),
                                props: vec![
                                    ("field".into(), UiValue::Ident("rating".into())),
                                    (
                                        "options".into(),
                                        UiValue::StringList(vec![
                                            "Good".into(),
                                            "Okay".into(),
                                            "Bad".into(),
                                        ]),
                                    ),
                                ],
                                slots: vec![],
                                children: vec![],
                                span: Span::default(),
                            },
                            UiNode {
                                component: "toolbar".into(),
                                props: vec![],
                                slots: vec![],
                                children: vec![UiNode {
                                    component: "button".into(),
                                    props: vec![
                                        ("label".into(), UiValue::String("Submit".into())),
                                        ("variant".into(), UiValue::Ident("primary".into())),
                                        ("submit".into(), UiValue::Flag),
                                    ],
                                    slots: vec![],
                                    children: vec![],
                                    span: Span::default(),
                                }],
                                span: Span::default(),
                            },
                        ],
                        span: Span::default(),
                    }],
                    span: Span::default(),
                }],
                span: Span::default(),
            },
            span: Span::default(),
        }
    }

    #[test]
    fn validates_catalog_view_with_contract_fields() {
        let contract = Contract {
            name: "FeedbackRecord".into(),
            fields: vec![
                Field {
                    name: "author".into(),
                    ty: TypeExpr::Named("Str".into()),
                    default: None,
                },
                Field {
                    name: "rating".into(),
                    ty: TypeExpr::Named("Str".into()),
                    default: None,
                },
            ],
            span: Span::default(),
        };
        validate_view(&sample_view(), Some(&contract)).unwrap();
    }

    #[test]
    fn rejects_unknown_field_binding() {
        let contract = Contract {
            name: "FeedbackRecord".into(),
            fields: vec![Field {
                name: "author".into(),
                ty: TypeExpr::Named("Str".into()),
                default: None,
            }],
            span: Span::default(),
        };
        let err = validate_view(&sample_view(), Some(&contract)).unwrap_err();
        assert!(err.contains("unknown Contract field `rating`"));
    }
}
