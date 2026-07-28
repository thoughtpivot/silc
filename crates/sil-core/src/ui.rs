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
    /// One or two sentences: what the primitive renders and when to use it.
    pub description: &'static str,
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
    "file_input",
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
    "file_input",
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
        description: "Root layout shell for a screen. Hosts optional `app_bar`, `side_panel`, and `footer` slots around the main content tree, and is the required root of a component `render` template.",
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
        description: "Top application bar that shows a required title. Use it for the screen heading and brand strip that stays visible across the page.",
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
        description: "Vertical navigation rail that accepts only `nav_item` children. Reach for it when a page needs persistent secondary navigation beside the main content.",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(&["nav_item"]),
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "nav_item",
        description: "A single navigation entry with a label and optional route target. Mark it `:active` for the current location, or bind `:on(click)` for custom navigation handlers.",
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
        description: "Horizontal action strip that holds `button` children. Use it above tables, forms, or collections for primary and secondary actions.",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(&["button"]),
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "stack",
        description: "Vertical layout container that stacks children top-to-bottom with consistent spacing. Prefer it for most form and content hierarchies on both web and terminal surfaces.",
        props: &[],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "row",
        description: "Horizontal layout container that places children side-by-side. Use it for toolbars, button groups, and compact label/value pairs.",
        props: &[],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "grid",
        description: "Two-dimensional layout container for children arranged in a responsive grid. Reach for it when a stack or row alone cannot express the desired alignment.",
        props: &[],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "card",
        description: "Surfaced content panel with optional `actions` slot. Use it to group a related block of UI so it reads as one unit on web and terminal.",
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
        description: "Semantic title text with a required `:text` and optional heading `:level`. Use it to introduce sections without inventing custom typography primitives.",
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
        description: "Plain body copy bound through the required `:text` prop. Prefer it for paragraphs, captions, and any non-interactive string display.",
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
        description: "Form container that groups inputs and emits a `submit` event. Place fields and action buttons inside it so enter-to-submit and validation wiring stay consistent.",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(FORM_CHILDREN),
        events: &[EventSpec { name: "submit" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "text_input",
        description: "Single-line text field for short values. Bind `:field` or `:value`, optionally label and placeholder it, and listen for `input` or `change` as the user types.",
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
        description: "Multi-line text field for longer content such as article bodies or notes. Same binding model as `text_input`, but sized for paragraphs instead of a single line.",
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
        name: "file_input",
        description: "File picker for document upload (PDF, DOCX, ODT, Markdown, HTML, plain text). Bind `:field` for the staged upload handle, optionally constrain with `:accept`, and use inside a form that posts multipart to the synthesized `/upload` route when `doc::extract` is present.",
        props: &[
            PropSpec {
                name: "field",
                kind: PropKind::Ident,
                required: false,
            },
            PropSpec {
                name: "label",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "accept",
                kind: PropKind::Expr,
                required: false,
            },
            PropSpec {
                name: "multiple",
                kind: PropKind::Flag,
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
        name: "radio_group",
        description: "Exclusive choice control over a closed `:options` list. Bind the selected value through `:field` or `:value` when the user must pick exactly one option.",
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
        description: "Dropdown choice control over a closed `:options` list. Prefer it over radio groups when the option set is long or screen space is tight.",
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
        description: "Boolean toggle with a required visible label. Use it for independent on/off settings that are not mutually exclusive with siblings.",
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
        description: "Boolean on/off control styled as a switch, with a required label. Prefer it for settings that feel like enabling a mode rather than checking a box.",
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
        description: "Labeled wrapper that can show hint and error text around nested form children. Use it when an input needs surrounding help or validation chrome.",
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
        description: "Clickable action control with a required label and optional variant/size. Set `:submit` inside forms, or bind `:on(click)` for ordinary handler dispatch.",
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
        description: "Conversational composer that collects a message and emits `send`. Optional session, persona, loading, and error props wire it into LLM or agent flows.",
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
        description: "Scrollable transcript of prior chat turns. Bind `:items` to message history and optionally make the panel `:collapsible` when space is scarce.",
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
        description: "Search-oriented text field that emits `input` while typing and `submit` on enter. Optional persona/context props support scored or agent-assisted search UIs.",
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
        events: &[EventSpec { name: "input" }, EventSpec { name: "submit" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "filter_bar",
        description: "Horizontal filter strip that hosts search inputs, buttons, and text fields. Place it above collections or tables when users need to narrow a result set.",
        props: &[],
        slots: &[],
        children: ChildPolicy::AnyOf(&["search_input", "button", "text_input"]),
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "collection",
        description: "Repeating container driven by a required `:items` expression. Render children once per item, and supply `:empty_text` for the zero-results state.",
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
        description: "Simple vertical list of items, optionally bound through `:items`. Prefer it for lightweight enumerations where a full table would be overkill.",
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
        description: "Renders a collection of records as rows and columns, one column per `:columns` entry. Use it for tabular data where users scan and compare across fields; it degrades to an aligned text grid on the terminal surface.",
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
        events: &[EventSpec { name: "select" }],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "badge",
        description: "Compact status chip with required text and optional closed `:tone`. Use it to tag rows, cards, or headings with a short categorical label.",
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
        description: "Prominent inline notice with required text and optional title, tone, dismissibility, and auto-dismiss. Reach for it for success, warning, or error feedback; use `:auto_dismiss_ms` for transient toasts that fade away.",
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
            PropSpec {
                name: "auto_dismiss_ms",
                kind: PropKind::Expr,
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
        description: "Visual separator between sections, optionally labeled. Use it to break dense stacks without introducing a full new section heading.",
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
        description: "Titled content region with optional description and arbitrary children. Prefer it when a card is too heavy but the block still needs a clear heading.",
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
        description: "Bottom-of-page region for secondary links, legal copy, or actions. Usually hosted in the page `footer` slot rather than inline in the main stack.",
        props: &[],
        slots: &[],
        children: ChildPolicy::Any,
        events: &[],
        surfaces: BOTH,
    },
    ComponentSpec {
        name: "description_list",
        description: "Key/value presentation driven by a required `:items` expression. Use it for read-only detail views such as record summaries.",
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
        description: "Tabbed container whose selected tab is bound through `:field` or `:value`. Children must be `tab` nodes; a `change` event fires when the selection moves.",
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
        description: "One pane inside a `tabs` parent, identified by required `:label` and `:value`. The value is the tab identity compared against the parent selection, not a form input.",
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
        description: "Modal overlay controlled by a required `:open` expression. Use `confirm` and `cancel` events for affirmative and dismissive actions around focused tasks.",
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
        description: "In-progress placeholder with optional status text. Show it while a query, mutation, or agent call is outstanding.",
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
        description: "Zero-results placeholder with optional explanatory text. Pair it with collections, lists, and tables when there is nothing to render yet.",
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

/// Documentation for a prop on a builtin UI primitive.
///
/// Checks `(component, prop)` overrides first, then a shared vocabulary keyed by
/// prop name. Returns `None` only when the prop is unknown to the catalog.
pub fn prop_doc(component: &str, prop: &str) -> Option<&'static str> {
    for (comp, name, doc) in PROP_DOC_OVERRIDES {
        if *comp == component && *name == prop {
            return Some(doc);
        }
    }
    if let Some(doc) = shared_prop_doc(prop) {
        return Some(doc);
    }
    // Unknown to catalog — still allow a soft fallback for hover UX.
    if lookup_component(component)
        .map(|spec| spec.props.iter().any(|p| p.name == prop))
        .unwrap_or(false)
    {
        return Some("Prop on this UI primitive. Consult the component catalog line for type and requiredness.");
    }
    None
}

/// Documentation for an event on a builtin UI primitive.
pub fn event_doc(component: &str, event: &str) -> Option<&'static str> {
    for (comp, name, doc) in EVENT_DOC_OVERRIDES {
        if *comp == component && *name == event {
            return Some(doc);
        }
    }
    if let Some(doc) = shared_event_doc(event) {
        return Some(doc);
    }
    if lookup_component(component)
        .map(|spec| spec.events.iter().any(|e| e.name == event))
        .unwrap_or(false)
    {
        return Some("Event emitted by this UI primitive. Bind it with `:on(event(handler))`.");
    }
    None
}

fn shared_prop_doc(prop: &str) -> Option<&'static str> {
    Some(match prop {
        "label" => "Visible label shown beside or above the control. Prefer a short noun phrase that names the value the user is editing or the action they will take.",
        "field" => "Binds this control to a named form or state field. The runtime keeps the control's value in sync with that field across renders.",
        "value" => "Controlled value for the control. Pass an expression when you want to drive the displayed value from component state or a query result.",
        "text" => "Display text rendered by the primitive. This is content, not a form binding — use `:field` or `:value` when the user can edit the string.",
        "title" => "Short heading text for the chrome around this primitive. Keep it brief so it remains readable on both web and terminal surfaces.",
        "placeholder" => "Hint text shown when the control is empty. It disappears as soon as the user enters a value and is not submitted with the form.",
        "disabled" => "When set, the control ignores interaction and typically renders with muted chrome. Use it while a mutation is in flight or the action is unavailable.",
        "variant" => "Closed visual style for the control. Allowed tokens are `primary`, `secondary`, `destructive`, and `ghost`.",
        "tone" => "Closed semantic color token for status chrome. Allowed values are `default`, `muted`, `info`, `success`, `warning`, and `danger`.",
        "size" => "Closed size token for the control. Allowed values are `sm`, `md`, and `lg`.",
        "checked" => "Boolean expression for whether the control is currently on. Bind it to state when the toggle should round-trip through handlers.",
        "active" => "Marks this item as the current or selected entry. Use it for navigation highlighting or pressed-button state.",
        "options" => "Closed list of string choices the user may pick from. Prefer a stable order so web and terminal renders stay aligned.",
        "items" => "Collection expression that drives repeated or list content. Each element becomes one rendered row, message, or description entry.",
        "empty_text" => "Message shown when the bound collection has no rows. Use it so empty states stay intentional instead of rendering a blank panel.",
        "rows" => "Row data for a table. Usually an array of contract values from a resource query or local collection.",
        "columns" => "Column descriptors that name which fields appear and in what order. One column entry becomes one table heading and cell binding.",
        "sortable" => "When set, column headers become clickable sort controls. Sorting is client-side over the currently loaded rows.",
        "searchable" => "When set, the table exposes a search box that filters visible rows. Pair with `:filter_field` when the filter should bind to named state.",
        "selectable" => "When set, rows can be selected and the table emits a `select` event. Use it for bulk actions or detail drill-down.",
        "dense" => "Tightens row and cell spacing for information-dense tables. Prefer the default spacing when the table is the page's primary focus.",
        "filter_field" => "Named state or form field that holds the active table filter string. The table reads this field when `:searchable` filtering is enabled.",
        "filter_column" => "Optional column key that scopes the table filter to a single field. Omit it to search across all columns.",
        "filter_all" => "Expression that forces the table to show every row regardless of the current filter. Useful for an explicit \"clear filters\" control.",
        "submit" => "Marks the button as the form's submit action. Pressing it (or Enter inside the form) fires the surrounding `form`'s `submit` event.",
        "to" => "Route or path target for navigation. When set, activating the item navigates without needing a custom click handler.",
        "level" => "Heading rank such as `h1`–`h3`. Choose the level that matches document structure rather than visual size alone.",
        "hint" => "Secondary help text shown under a field label. Use it for format examples or soft constraints that are not validation errors.",
        "error" => "Validation or failure message associated with this control. Prefer binding it only when an error is present so the chrome stays quiet otherwise.",
        "dismissible" => "When set, the alert shows a dismiss control and can emit `dismiss`. Use it for transient notices the user may clear.",
        "auto_dismiss_ms" => "Milliseconds before the alert fades and emits `dismiss` (e.g. `5000`). Omit for a sticky notice; pair with `:dismissible` so the user can clear it early.",
        "description" => "Supporting copy under a section title. Keep it to one or two sentences that explain why the section exists.",
        "open" => "Boolean expression that controls whether the dialog is visible. Drive it from component state so confirm/cancel handlers can close it.",
        "collapsible" => "When set, the panel can collapse to reclaim vertical space. Useful for secondary history or detail panes.",
        "session" => "Opaque session identifier for a conversational flow. Pass it so successive sends stay attached to the same chat context.",
        "loading" => "Expression that is truthy while an async operation is in flight. The control can show busy chrome until it clears.",
        "context" => "Extra context blob supplied to scored or agent-assisted controls. Use it for embeddings, filters, or prior turns the model should see.",
        "persona" => "Named persona or agent identity that shapes how an assisted control behaves. Keep personas stable so scores and completions stay comparable.",
        _ => return None,
    })
}

fn shared_event_doc(event: &str) -> Option<&'static str> {
    Some(match event {
        "click" => "Fires when the user activates the control. Bind a handler with `:on(click(handler))` for navigation or mutations.",
        "change" => "Fires when the committed value changes (blur, toggle, or selection). Prefer `change` over `input` when you only care about finalized edits.",
        "input" => "Fires on each keystroke or incremental edit. Use it for live filtering and search-as-you-type; prefer `change` for commits.",
        "submit" => "Fires when the user submits the form or search. Typically bound to a mutation or navigation handler.",
        "send" => "Fires when the user sends a chat message. The handler usually clears the composer and appends to history.",
        "select" => "Fires when the user selects a row or item. The handler receives enough identity to open a detail view or stage a bulk action.",
        "dismiss" => "Fires when the user dismisses an alert or notice. Clear the backing state so the alert does not immediately reappear.",
        "confirm" => "Fires when the user confirms a dialog. Perform the destructive or committing action, then set `:open` to false.",
        "cancel" => "Fires when the user cancels a dialog. Leave data unchanged and set `:open` to false.",
        _ => return None,
    })
}

/// Props whose meaning diverges from the shared vocabulary on a specific component.
const PROP_DOC_OVERRIDES: &[(&str, &str, &str)] = &[
    (
        "tab",
        "value",
        "Stable identity for this tab pane. Compared against the parent `tabs` selection; it is not a form field value.",
    ),
    (
        "tabs",
        "value",
        "Currently selected tab identity. Bind it to state so changing tabs round-trips through a `change` handler.",
    ),
    (
        "tabs",
        "field",
        "Named state field that stores the selected tab identity. Prefer this over `:value` when the selection should participate in form-like state.",
    ),
    (
        "chat_history",
        "items",
        "Ordered transcript of chat turns to render. Each item is typically a message contract with role and text fields.",
    ),
    (
        "list",
        "items",
        "Optional collection driving the list rows. When omitted, children alone define the list content.",
    ),
    (
        "collection",
        "items",
        "Required collection expression. The collection repeats its child template once per element.",
    ),
    (
        "description_list",
        "items",
        "Required key/value pairs to render as a description list. Use contract fields or tuples the runtime can label.",
    ),
    (
        "button",
        "active",
        "Expression that renders the button in a pressed/selected visual state. Distinct from `:disabled`, which blocks interaction entirely.",
    ),
    (
        "nav_item",
        "active",
        "Flag marking this nav entry as the current location. Keeps the side panel highlight in sync with the active route.",
    ),
    (
        "chat",
        "value",
        "Controlled composer text for the chat input. Bind it to state when the handler needs to clear or rewrite the draft after `send`.",
    ),
    (
        "chat",
        "loading",
        "Truthy while a reply is outstanding. The composer typically disables send and shows busy chrome until it clears.",
    ),
    (
        "loading",
        "text",
        "Optional status copy shown beside the busy indicator. Prefer a short phrase such as \"Loading articles…\".",
    ),
    (
        "empty",
        "text",
        "Optional explanation shown in the empty state. Tell the user what is missing and, when useful, what action fills it.",
    ),
];

/// Events whose meaning is specialized on a specific component.
const EVENT_DOC_OVERRIDES: &[(&str, &str, &str)] = &[
    (
        "form",
        "submit",
        "Fires when the form is submitted via a `:submit` button or Enter in a field. Run validation and mutations here.",
    ),
    (
        "search_input",
        "submit",
        "Fires when the user presses Enter in the search field. Use it to commit the query; use `input` for live filtering.",
    ),
    (
        "table",
        "select",
        "Fires when a selectable row is chosen. Handlers typically open a detail route or stage the row for a mutation.",
    ),
    (
        "dialog",
        "confirm",
        "Fires on the affirmative dialog action. Perform the work, then close by setting the bound `:open` expression false.",
    ),
    (
        "dialog",
        "cancel",
        "Fires when the dialog is dismissed without confirming. Leave underlying data unchanged and close the dialog.",
    ),
    (
        "chat",
        "send",
        "Fires when the user sends the current composer value. Append to history, call the agent/LLM path, and clear the draft.",
    ),
    (
        "tabs",
        "change",
        "Fires when the selected tab identity changes. Update the bound `:field`/`:value` state so the active pane stays in sync.",
    ),
];

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
            component_span: Default::default(),
            prop_spans: vec![],
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
        assert_eq!(UI_COMPONENT_CATALOG.len(), 39);
        for spec in UI_COMPONENT_CATALOG {
            let line = format_component_catalog_line(spec);
            assert!(line.starts_with(&format!("- `ui::{}` — ", spec.name)));
            assert!(line.contains("surfaces: web+terminal"));
        }
    }

    #[test]
    fn every_catalog_entry_has_nonempty_description() {
        assert_eq!(UI_COMPONENT_CATALOG.len(), 39);
        for spec in UI_COMPONENT_CATALOG {
            assert!(
                !spec.description.trim().is_empty(),
                "ui::{} missing description",
                spec.name
            );
            assert!(
                spec.description.contains('.') || spec.description.len() > 40,
                "ui::{} description looks too short: {}",
                spec.name,
                spec.description
            );
        }
    }

    #[test]
    fn every_catalog_prop_and_event_has_docs() {
        for spec in UI_COMPONENT_CATALOG {
            for prop in spec.props {
                let doc = prop_doc(spec.name, prop.name)
                    .unwrap_or_else(|| panic!("missing prop_doc for ui::{}:{}", spec.name, prop.name));
                assert!(
                    doc.len() > 20,
                    "prop_doc for ui::{}:{} too short",
                    spec.name,
                    prop.name
                );
            }
            for event in spec.events {
                let doc = event_doc(spec.name, event.name).unwrap_or_else(|| {
                    panic!("missing event_doc for ui::{}:{}", spec.name, event.name)
                });
                assert!(
                    doc.len() > 20,
                    "event_doc for ui::{}:{} too short",
                    spec.name,
                    event.name
                );
            }
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
