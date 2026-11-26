//! Loop subject: scheduled, approval-gated, model-assisted work (ADR-014).
//!
//! Authors declare `loop Name { loop::flow(...) }` with one trigger and a tree
//! of steps from the closed `LOOP_NODE_CATALOG`. The compiler checks bounds,
//! effect keys, fail-closed gates, and that model output is gated or approved
//! before it reaches an effect. A compiler-owned Go kernel runs the lowered
//! plan with receipts, an append-only event log, and replay.

use std::collections::{HashMap, HashSet};

use crate::expr::{BinOp, Expr};
use crate::program::Program;
use crate::types::{Span, TypeExpr};

pub const MAX_LOOP_LIST: u64 = 10_000;
pub const MAX_LOOP_RETRY: u64 = 10;
pub const MAX_LOOP_DURATION_MINUTES: u64 = 30 * 24 * 60;
pub const DEFAULT_ASK_RETRY: u64 = 1;
pub const DEFAULT_READ_RETRY: u64 = 2;
pub const LOOP_READ_OPS: &[&str] = &[
    "scrape::page",
    "scrape::site",
    "scrape::render",
    "scrape::select",
    "mcp::call",
];
/// The embedding step `loop::let` nests (THO-552, ADR-014).
pub const LOOP_EMBED_OP: &str = "tensor::infer";
/// The score step `loop::let` nests (THO-579, ADR-020).
pub const LOOP_SCORE_OP: &str = "text::score";
/// Operations nested in `loop::let`. Neither is recorded; both are
/// recomputed on replay.
pub const LOOP_LET_OPS: &[&str] = &[LOOP_EMBED_OP, LOOP_SCORE_OP];
/// Hard cap on pages one `scrape::site` read may fetch.
pub const MAX_LOOP_SITE_PAGES: u64 = 100;

use crate::catalog::{
    ChildPolicy, NodeRole, NodeSpec, OptionKind, OptionSpec, NO_EVENTS, NO_SLOTS, NO_SURFACES,
};

/// Historical names for the shared catalog types.
pub type LoopPropKind = OptionKind;
pub type LoopPropSpec = OptionSpec;
pub type LoopChildPolicy = ChildPolicy;
pub type LoopNodeRole = NodeRole;
pub type LoopNodeSpec = NodeSpec;

const fn lp(
    name: &'static str,
    kind: LoopPropKind,
    required: bool,
    description: &'static str,
) -> LoopPropSpec {
    LoopPropSpec {
        name,
        kind,
        required,
        description,
        closed_values: &[],
    }
}

const fn lp_closed(
    name: &'static str,
    kind: LoopPropKind,
    required: bool,
    description: &'static str,
    closed_values: &'static [&'static str],
) -> LoopPropSpec {
    LoopPropSpec {
        name,
        kind,
        required,
        description,
        closed_values,
    }
}

pub const LOOP_TRIGGERS: &[&str] = &["schedule", "manual", "on_mutation"];

pub const LOOP_STEPS: &[&str] = &[
    "let", "find", "read", "fallback", "ask", "gate", "branch", "each", "write", "notify",
    "approve", "stop", "fail", "skip",
];

const FLOW_CHILDREN: &[&str] = &[
    "schedule",
    "manual",
    "on_mutation",
    "let",
    "find",
    "read",
    "fallback",
    "ask",
    "gate",
    "branch",
    "each",
    "write",
    "notify",
    "approve",
    "stop",
    "fail",
    "skip",
];

const TERMINATORS: &[&str] = &["stop", "fail", "skip"];

pub const LOOP_NODE_CATALOG: &[LoopNodeSpec] = &[
    LoopNodeSpec {
        name: "flow",
        role: LoopNodeRole::Root,
        description: "Root of a loop. The first child is exactly one trigger; the remaining children are steps that run in order.",
        props: &[],
        children: LoopChildPolicy::AnyOf(FLOW_CHILDREN),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "schedule",
        role: LoopNodeRole::Trigger,
        description: "Starts a run on a 5-field cron schedule in an explicit time zone. A loop never overlaps itself; a firing during an active run is skipped and logged. Run now in /loops also starts a run off schedule; `$event` fields are then unknown.",
        props: &[
            lp("cron", LoopPropKind::String, true, "Five-field cron (minute hour day-of-month month day-of-week), e.g. \"0 8 * * 1-5\"."),
            lp("tz", LoopPropKind::String, true, "IANA time zone the cron is evaluated in, e.g. \"America/New_York\" or \"UTC\"."),
            lp("catch_up", LoopPropKind::String, false, "How far back a missed firing still runs after downtime, e.g. \"4h\" (max 30d). Only the latest missed firing runs."),
        ],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "manual",
        role: LoopNodeRole::Trigger,
        description: "Starts a run when a person asks for one. With an `app` (or any scheduled loop) that is Run now in the synthesized /loops inbox; when every loop is manual and there is no `app`, the program is a command: `silc main.silc` runs each loop once, prints its notices to stdout, and exits.",
        props: &[],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "on_mutation",
        role: LoopNodeRole::Trigger,
        description: "Starts one run per created or updated row of a declared resource. The row is available as `$event`.",
        props: &[
            lp("resource", LoopPropKind::Ident, true, "Resource whose rows start runs, e.g. `Rfis`."),
            lp_closed("mutation", LoopPropKind::Ident, true, "Which change starts a run: `create` or `update`.", &["create", "update"]),
        ],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "let",
        role: LoopNodeRole::Step,
        description: "Binds a name for the rest of the enclosing block. With `:value(expr)` it is deterministic and does no I/O. With a nested `tensor::infer(:model(...), :from(expr))` it embeds a Str through the CPython embed worker and binds a `Vec[num32; N]` of the model's dimension (`Emb384` for the default `minilm-l6-v2`). With a nested `text::score(:text(expr))` it binds a score from 0 to 1 computed on the CPU in the loop kernel. Embeds and scores are recomputed on replay, never recorded, and never model calls.",
        props: &[
            lp("as", LoopPropKind::Ident, true, "Binding name, used later as `$name`."),
            lp("value", LoopPropKind::Expr, false, "Expression over earlier bindings, literals, or `Contract.new(...)`. Required unless the let nests `tensor::infer(...)` or `text::score(...)`."),
            lp_closed("op", LoopPropKind::String, false, "Nested operation, written inside the let: `tensor::infer(...)` or `text::score(...)`.", LOOP_LET_OPS),
            lp("model", LoopPropKind::String, false, "`tensor::infer`: embedding model from the catalog, e.g. \"minilm-l6-v2\" (the default: 384 dims, CPU). Unknown models are rejected."),
            lp("from", LoopPropKind::Expr, false, "`tensor::infer`: the Str to embed, e.g. `$release.text`."),
            lp("prefer", LoopPropKind::Ident, false, "`tensor::infer`: device. Only `CPU` is accepted; `CUDA` is rejected."),
            lp("text", LoopPropKind::Expr, false, "`text::score`: the Str to score, from a read or row; model output from `loop::ask` must pass a gate or approval first."),
        ],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "find",
        role: LoopNodeRole::Step,
        description: "Reads rows from a declared resource with a filter, order, and a hard row limit. Recorded so replays see the same rows. `Resource.nearest(:to($vec))` ranks rows by cosine similarity to an embedding instead, best first.",
        props: &[
            lp("as", LoopPropKind::Ident, true, "Binding name for the rows (a list) or the single row with `:one`."),
            lp("from", LoopPropKind::Ref, true, "Resource query to read, e.g. `Rfis.list`, or `Incidents.nearest(:to($vec))` on a resource with `query nearest;` where `$vec` is an embedding of the model's dimension (e.g. `Emb384`)."),
            lp("where", LoopPropKind::Expr, false, "Row filter; contract fields are `$.field`. Unknown values do not match (fail closed). With `nearest` it filters the `:max` nearest rows, so fewer may come back."),
            lp("order", LoopPropKind::Expr, false, "Row field to sort by, e.g. `$.due`. Not allowed with `nearest`, which is always best first."),
            lp("desc", LoopPropKind::Flag, false, "Sort descending instead of ascending. Not allowed with `nearest`."),
            lp("max", LoopPropKind::Number, false, "Maximum rows returned (required unless `:one`; at most 10000)."),
            lp("one", LoopPropKind::Flag, false, "Return the first matching row instead of a list; empty when nothing matches."),
        ],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "read",
        role: LoopNodeRole::Step,
        description: "Fetches outside data with a bounded retry and records it for replay, so a resumed run never refetches. `scrape::page` is an HTTP GET (title and text), optionally narrowed by a CSS `:select` and bound `:into` a contract; `scrape::render` is the same page rendered in a browser; `scrape::site` crawls from a start page up to a hard `:max` page cap and binds a list of pages; `scrape::select` binds every element matching `:css` as a list; `mcp::call` calls one tool on an MCP server over streamable HTTP.",
        props: &[
            lp("as", LoopPropKind::Ident, true, "Binding name: a page (`url`, `status`, `title`, `text`), a list of pages (`scrape::site`) or matches (`scrape::select`: `url`, `text`, `href`), rows of the `:into` contract, or a tool result (`text`, `data`)."),
            lp("into", LoopPropKind::Ident, false, "`scrape::*`: contract each page or match binds as. Fields named `url`, `status`, `title`, `text` (and `href` for a match) take its values (`text` is the selected text); other fields come from `:fields` or are unknown."),
            lp_closed("op", LoopPropKind::String, true, "Read operation: \"scrape::page\", \"scrape::site\", \"scrape::render\", \"scrape::select\", or \"mcp::call\".", LOOP_READ_OPS),
            lp("url", LoopPropKind::Expr, false, "`scrape::*`: absolute http(s) URL (the start page for `scrape::site`), literal or from a binding."),
            lp("server", LoopPropKind::String, false, "`mcp::call`: MCP server URL, e.g. \"https://example.run.app/mcp\"."),
            lp("tool", LoopPropKind::String, false, "`mcp::call`: tool name on that server."),
            lp("args", LoopPropKind::Expr, false, "`mcp::call`: tool arguments as `Contract.new(...)`; field names are sent as written."),
            lp("auth_env", LoopPropKind::String, false, "`mcp::call`: environment variable holding the bearer token. Tokens never go in source."),
            lp("select", LoopPropKind::String, false, "`mcp::call`: dotted path into the result kept as `data`, e.g. \"records.parsed\" (lists map over items). `scrape::page`, `scrape::render`, `scrape::site`: CSS selector; each page narrows to the first match before `text` is taken, e.g. \"article\"."),
            lp("fields", LoopPropKind::Expr, false, "`scrape::*` with `:into`: per-field CSS selectors inside the selection or match, e.g. `Release.new(:version(\"h2\"))`; the first match's text fills the field."),
            lp("css", LoopPropKind::String, false, "`scrape::select`: CSS selector; every match, in page order up to `:max`, becomes one item."),
            lp("depth", LoopPropKind::Number, false, "`scrape::site`: link depth from the start page, 1 (start page only) to 10 (default 2)."),
            lp("max", LoopPropKind::Number, false, "`scrape::site`: hard cap on pages fetched (1 to 100, required). `scrape::select`: most matches kept (1 to 10000, required)."),
            lp("same_host", LoopPropKind::Bool, false, "`scrape::site`: follow only links on the start page's host (default true)."),
            lp("link_css", LoopPropKind::String, false, "`scrape::site`: CSS selector for the links to follow (default \"a[href]\")."),
            lp("retry", LoopPropKind::Number, false, "Retries after a failed fetch (default 2, max 10). `scrape::site` retries each page."),
        ],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "fallback",
        role: LoopNodeRole::Step,
        description: "Tries two or more `loop::read` attempts in order and binds the first result that satisfies `:accept`. Attempts omit `:as` and use the same read operation. A failed fetch or a result that is false or unknown under `:accept` moves to the next attempt (fail closed). When none is accepted the required `loop::otherwise` runs and must stop, fail, or skip. Each attempt is recorded for replay.",
        props: &[
            lp("as", LoopPropKind::Ident, true, "Binding name for the accepted result, shaped like the attempts' read operation."),
            lp("accept", LoopPropKind::Expr, true, "Condition over the attempt's result, written with the `:as` binding (e.g. `$page.status == 200`); false or unknown rejects the attempt."),
        ],
        children: LoopChildPolicy::AnyOf(&["read", "otherwise"]),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "ask",
        role: LoopNodeRole::Step,
        description: "Asks the local silclm model for one value of a contract. The answer is checked against the contract and recorded; it must pass a gate or approval before any effect uses it.",
        props: &[
            lp("as", LoopPropKind::Ident, true, "Binding name for the typed answer."),
            lp("into", LoopPropKind::Ident, true, "Contract the answer must match, e.g. `Reminder`."),
            lp("prompt", LoopPropKind::Template, true, "Instruction for the model; may use `{$name.field}` placeholders."),
            lp("from", LoopPropKind::Expr, false, "Context given to the model, e.g. `$rfi`. Several values: `:from($release, $seen)` (same as `:from([$release, $seen])`); the model sees one JSON object labelled by binding path, e.g. `release` and `seen`."),
            lp("retry", LoopPropKind::Number, false, "Retries when the answer does not match the contract (default 1, max 10)."),
        ],
        children: LoopChildPolicy::AnyOf(&["otherwise"]),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "gate",
        role: LoopNodeRole::Step,
        description: "Continues only when the condition is true. False or unknown runs `loop::otherwise` (which must stop, fail, or skip) or fails the run.",
        props: &[
            lp("that", LoopPropKind::Expr, true, "Condition; missing fields make it unknown, and unknown blocks."),
            lp("reason", LoopPropKind::String, true, "Why the gate exists; recorded when it blocks."),
        ],
        children: LoopChildPolicy::AnyOf(&["otherwise"]),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "branch",
        role: LoopNodeRole::Step,
        description: "Runs the first `loop::when` whose condition is true, else the required `loop::otherwise`. Unknown conditions are not true.",
        props: &[],
        children: LoopChildPolicy::AnyOf(&["when", "otherwise"]),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "when",
        role: LoopNodeRole::Block,
        description: "One arm of a `loop::branch`. Its steps run when the condition is true.",
        props: &[lp("that", LoopPropKind::Expr, true, "Condition for this arm; unknown counts as false.")],
        children: LoopChildPolicy::AnyOf(LOOP_STEPS),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "otherwise",
        role: LoopNodeRole::Block,
        description: "Fallback block for `branch`, `gate`, `ask`, or `fallback`. Under gate, ask, and fallback it must end in stop, fail, or skip.",
        props: &[],
        children: LoopChildPolicy::AnyOf(LOOP_STEPS),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "each",
        role: LoopNodeRole::Step,
        description: "Runs its steps once per item of a bounded list. A failed item does not stop the others; the run ends partial.",
        props: &[
            lp("in", LoopPropKind::Expr, true, "List to iterate, e.g. `$overdue`."),
            lp("as", LoopPropKind::Ident, true, "Binding name for the current item."),
            lp("max", LoopPropKind::Number, true, "Maximum items processed (at most 10000); extra items fail the run before it starts the loop."),
        ],
        children: LoopChildPolicy::AnyOf(LOOP_STEPS),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "write",
        role: LoopNodeRole::Step,
        description: "Creates or updates a resource row exactly once per key. A receipt is reserved before and committed with the row in one transaction.",
        props: &[
            lp("to", LoopPropKind::Ref, true, "Resource mutation, e.g. `Reminders.create` or `Rfis.update` (update merges fields)."),
            lp("value", LoopPropKind::Expr, true, "Row value: `Contract.new(...)` or a binding of the resource's contract."),
            lp("key", LoopPropKind::Template, true, "Idempotency key with at least one placeholder, e.g. \"{$rfi.id}:remind:{$today}\"."),
            lp("unchecked", LoopPropKind::String, false, "Reason to let unreviewed model output reach this effect; shown in the build report."),
        ],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "notify",
        role: LoopNodeRole::Step,
        description: "Posts an in-app notice to the synthesized /loops inbox exactly once per key. v1 has no external email or chat delivery.",
        props: &[
            lp("to", LoopPropKind::Template, true, "Recipient label, e.g. \"{$rfi.pm}\"."),
            lp("text", LoopPropKind::Template, true, "Notice text with `{$name.field}` placeholders."),
            lp("key", LoopPropKind::Template, true, "Idempotency key with at least one placeholder."),
            lp("unchecked", LoopPropKind::String, false, "Reason to let unreviewed model output reach this effect."),
        ],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "approve",
        role: LoopNodeRole::Step,
        description: "Pauses until the named person approves in the /loops inbox, or the deadline passes. Requires `loop::declined` and `loop::timed_out` blocks.",
        props: &[
            lp("by", LoopPropKind::Template, true, "Who must decide; the inbox requires this exact name."),
            lp("message", LoopPropKind::Template, true, "Question shown to the approver."),
            lp("show", LoopPropKind::Expr, false, "Value shown with the request, e.g. the model's `$draft`."),
            lp("within", LoopPropKind::String, true, "Deadline such as \"2d\" or \"4h\" (max 30d)."),
            lp("as", LoopPropKind::Ident, false, "Binding for the decision (`outcome`, `by`, `decided_at`, `note`)."),
        ],
        children: LoopChildPolicy::AnyOf(&["declined", "timed_out"]),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "declined",
        role: LoopNodeRole::Block,
        description: "Runs when the approver declines; must end in stop, fail, or skip.",
        props: &[],
        children: LoopChildPolicy::AnyOf(LOOP_STEPS),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "timed_out",
        role: LoopNodeRole::Block,
        description: "Runs when nobody decides before `:within`; must end in stop, fail, or skip.",
        props: &[],
        children: LoopChildPolicy::AnyOf(LOOP_STEPS),

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "stop",
        role: LoopNodeRole::Step,
        description: "Ends the run successfully. Inside `each` it ends the whole run, not just the item.",
        props: &[lp("reason", LoopPropKind::Template, false, "Why the run stopped; recorded on the run.")],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "fail",
        role: LoopNodeRole::Step,
        description: "Fails the current item inside `each`, or the run outside it, with a recorded reason.",
        props: &[lp("reason", LoopPropKind::Template, true, "Failure reason shown in the runs table.")],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
    LoopNodeSpec {
        name: "skip",
        role: LoopNodeRole::Step,
        description: "Skips the rest of the current `each` item; only valid inside `each`.",
        props: &[lp("reason", LoopPropKind::Template, true, "Why the item was skipped; recorded in the event log.")],
        children: LoopChildPolicy::None,

        surfaces: NO_SURFACES,
        slots: NO_SLOTS,
        events: NO_EVENTS,},
];

pub fn lookup_loop_node(name: &str) -> Option<&'static LoopNodeSpec> {
    LOOP_NODE_CATALOG.iter().find(|n| n.name == name)
}

pub fn catalog_loop_node_names() -> Vec<&'static str> {
    LOOP_NODE_CATALOG.iter().map(|n| n.name).collect()
}

pub fn loop_prop_doc(node: &str, prop: &str) -> Option<&'static str> {
    lookup_loop_node(node).and_then(|spec| {
        spec.props
            .iter()
            .find(|p| p.name == prop)
            .map(|p| p.description)
    })
}

/// One-line AGENTS-style catalog entry for a loop node.
pub fn format_loop_catalog_line(spec: &LoopNodeSpec) -> String {
    let props: Vec<String> = spec
        .props
        .iter()
        .map(|p| {
            let mut item = if p.required {
                format!("`{}`", p.name)
            } else {
                format!("`{}?`", p.name)
            };
            if matches!(p.kind, LoopPropKind::Flag) {
                item.push_str(" (flag)");
            }
            item
        })
        .collect();
    let children = match spec.children {
        LoopChildPolicy::None => "none".to_string(),
        LoopChildPolicy::AnyOf(allowed) if allowed == LOOP_STEPS => "steps".to_string(),
        LoopChildPolicy::AnyOf(allowed) if allowed == FLOW_CHILDREN => {
            "one trigger, then steps".to_string()
        }
        LoopChildPolicy::AnyOf(allowed) => allowed
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", "),
        LoopChildPolicy::Any => "any".to_string(),
    };
    format!(
        "- `loop::{}` — options: {}; children: {}",
        spec.name,
        if props.is_empty() {
            "none".into()
        } else {
            props.join(", ")
        },
        children
    )
}

/// Markdown digest of the closed `loop::*` catalog for assist / docs.
pub fn format_loop_catalog_md() -> String {
    let mut out = String::from(
        "# loop::* catalog (ADR-014)\n\n\
         Declare `loop Name { loop::flow(trigger, steps...) }`. One trigger per loop. \
         Every find/each has `:max`, every write/notify a `:key`, every approve a `:within`. \
         Model output from `loop::ask` must pass a gate or approval before an effect.\n\n",
    );
    for node in LOOP_NODE_CATALOG {
        out.push_str(&format_loop_catalog_line(node));
        out.push('\n');
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoopNode {
    pub name: String,
    pub name_span: Span,
    pub props: Vec<(String, Expr)>,
    pub prop_spans: Vec<Span>,
    pub children: Vec<LoopNode>,
    pub span: Span,
}

impl LoopNode {
    pub fn prop(&self, name: &str) -> Option<&Expr> {
        self.props.iter().find(|(n, _)| n == name).map(|(_, e)| e)
    }

    pub fn has_flag(&self, name: &str) -> bool {
        matches!(self.prop(name), Some(Expr::Bool(true)))
    }

    pub fn ident_prop(&self, name: &str) -> Option<&str> {
        match self.prop(name) {
            Some(Expr::Ident(s)) | Some(Expr::Var(s)) => Some(s),
            _ => None,
        }
    }

    pub fn string_prop(&self, name: &str) -> Option<&str> {
        self.prop(name).and_then(|e| e.as_string_literal())
    }

    pub fn number_prop(&self, name: &str) -> Option<u64> {
        match self.prop(name) {
            Some(Expr::Number(n)) => n.parse::<u64>().ok(),
            _ => None,
        }
    }

    /// `Resource.capability` for `:from` / `:to`, with or without options:
    /// `Incidents.nearest(:to($vec))` gives `("Incidents", "nearest")`.
    pub fn ref_prop(&self, name: &str) -> Option<(&str, &str)> {
        let expr = match self.prop(name) {
            Some(Expr::Call { callee, .. }) => callee.as_ref(),
            Some(other) => other,
            None => return None,
        };
        match expr {
            Expr::Member { base, field } => match base.as_ref() {
                Expr::Ident(b) => Some((b.as_str(), field.as_str())),
                _ => None,
            },
            _ => None,
        }
    }

    /// The `:option(expr)` arguments of `Resource.capability(...)`, in order.
    /// `None` when the reference has no parentheses. An argument that is not
    /// an option comes back with an empty name.
    pub fn ref_options(&self, name: &str) -> Option<Vec<(&str, &Expr)>> {
        let Some(Expr::Call { args, .. }) = self.prop(name) else {
            return None;
        };
        Some(
            args.iter()
                .map(|a| match a {
                    Expr::Assign { target, value } => match target.as_ref() {
                        Expr::Ident(n) => (n.as_str(), value.as_ref()),
                        _ => ("", a),
                    },
                    other => ("", other),
                })
                .collect(),
        )
    }

    /// `:to(expr)` of `loop::find :from(Resource.nearest(:to(expr)))`.
    pub fn nearest_to(&self) -> Option<&Expr> {
        if self.ref_prop("from")?.1 != crate::resource::NEAREST_QUERY {
            return None;
        }
        self.ref_options("from")?
            .into_iter()
            .find(|(n, _)| *n == "to")
            .map(|(_, e)| e)
    }

    pub fn child(&self, name: &str) -> Option<&LoopNode> {
        self.children.iter().find(|c| c.name == name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Loop {
    pub name: String,
    pub root: LoopNode,
    pub span: Span,
}

impl Loop {
    pub fn trigger(&self) -> Option<&LoopNode> {
        self.root
            .children
            .first()
            .filter(|c| LOOP_TRIGGERS.contains(&c.name.as_str()))
    }

    pub fn steps(&self) -> &[LoopNode] {
        if self.trigger().is_some() {
            &self.root.children[1..]
        } else {
            &self.root.children
        }
    }

    pub fn contains_node(&self, name: &str) -> bool {
        fn walk(node: &LoopNode, name: &str) -> bool {
            node.name == name || node.children.iter().any(|c| walk(c, name))
        }
        walk(&self.root, name)
    }

    /// Time zone the loop's `$today` is computed in.
    pub fn time_zone(&self) -> String {
        self.trigger()
            .filter(|t| t.name == "schedule")
            .and_then(|t| t.string_prop("tz"))
            .unwrap_or("UTC")
            .to_string()
    }
}

/// Whether `step` is an embedding step, `loop::let(:as(x), tensor::infer(...))`.
pub fn is_loop_embed(step: &LoopNode) -> bool {
    step.name == "let" && step.string_prop("op") == Some(LOOP_EMBED_OP)
}

/// The catalog model an embedding step names with `:model`, or the default
/// `minilm-l6-v2`. Unknown ids are an error.
pub fn loop_embed_model(
    step: &LoopNode,
) -> Result<&'static crate::model_catalog::EmbeddingModelCatalogEntry, String> {
    let id = step
        .string_prop("model")
        .unwrap_or(crate::model_catalog::DEFAULT_EMBEDDING_MODEL_ID);
    crate::model_catalog::validate_embedding_model_id(id)
}

/// Static type of an embedding step's binding: `Vec[num32; N]`, N being the
/// model's declared dimension. A contract field takes it only when its type
/// resolves to the same vector, e.g. `Emb384` for `minilm-l6-v2`.
pub fn loop_embed_type(model: &crate::model_catalog::EmbeddingModelCatalogEntry) -> TypeExpr {
    TypeExpr::Vec {
        elem: "num32".into(),
        len: Some(model.dimension.into()),
    }
}

/// Every embedding step in `lp`, in source order.
pub fn loop_embed_steps(lp: &Loop) -> Vec<&LoopNode> {
    fn walk<'a>(node: &'a LoopNode, out: &mut Vec<&'a LoopNode>) {
        if is_loop_embed(node) {
            out.push(node);
        }
        for child in &node.children {
            walk(child, out);
        }
    }
    let mut out = Vec::new();
    walk(&lp.root, &mut out);
    out
}

/// Parse `30m`, `4h`, `2d`, `1w` into minutes.
pub fn parse_loop_duration(text: &str) -> Result<u64, String> {
    let text = text.trim();
    let (num, unit) = text.split_at(
        text.find(|c: char| !c.is_ascii_digit())
            .unwrap_or(text.len()),
    );
    let n: u64 = num.parse().map_err(|_| {
        format!("invalid duration \"{text}\"; use forms like \"30m\", \"4h\", \"2d\"")
    })?;
    let minutes = match unit {
        "m" => n,
        "h" => n * 60,
        "d" => n * 60 * 24,
        "w" => n * 60 * 24 * 7,
        _ => return Err(format!("invalid duration \"{text}\"; units are m, h, d, w")),
    };
    if minutes == 0 {
        return Err(format!("duration \"{text}\" must be greater than zero"));
    }
    if minutes > MAX_LOOP_DURATION_MINUTES {
        return Err(format!("duration \"{text}\" exceeds the 30d limit"));
    }
    Ok(minutes)
}

/// Validate a 5-field cron expression (numbers, `*`, ranges, steps, lists).
pub fn validate_cron(cron: &str) -> Result<(), String> {
    let fields: Vec<&str> = cron.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(format!(
            "cron \"{cron}\" must have 5 fields (minute hour day-of-month month day-of-week)"
        ));
    }
    let ranges = [
        (0, 59, "minute"),
        (0, 23, "hour"),
        (1, 31, "day-of-month"),
        (1, 12, "month"),
        (0, 7, "day-of-week"),
    ];
    for (field, (lo, hi, label)) in fields.iter().zip(ranges) {
        for part in field.split(',') {
            let (range, step) = match part.split_once('/') {
                Some((r, s)) => (r, Some(s)),
                None => (part, None),
            };
            if let Some(step) = step {
                let s: u32 = step
                    .parse()
                    .map_err(|_| format!("cron \"{cron}\": bad step `{step}` in {label}"))?;
                if s == 0 {
                    return Err(format!("cron \"{cron}\": step must be positive in {label}"));
                }
            }
            if range == "*" {
                continue;
            }
            let (a, b) = match range.split_once('-') {
                Some((a, b)) => (a, b),
                None => (range, range),
            };
            let parse = |v: &str| -> Result<u32, String> {
                v.parse::<u32>()
                    .map_err(|_| format!("cron \"{cron}\": `{v}` is not a number in {label}"))
            };
            let (a, b) = (parse(a)?, parse(b)?);
            if a < lo || b > hi || a > b {
                return Err(format!("cron \"{cron}\": {label} must be within {lo}-{hi}"));
            }
        }
    }
    Ok(())
}

fn validate_tz(tz: &str) -> Result<(), String> {
    let ok = tz == "UTC"
        || (tz.contains('/')
            && tz.split('/').all(|seg| {
                !seg.is_empty()
                    && seg
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_+-".contains(c))
            }));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "time zone \"{tz}\" must be an IANA name such as \"America/New_York\" or \"UTC\""
        ))
    }
}

/// Placeholder paths (`$rfi.id` → ["rfi", "id"]) inside a template string.
pub fn template_placeholders(template: &str) -> Result<Vec<Vec<String>>, String> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let end = after
            .find('}')
            .ok_or_else(|| format!("unclosed `{{` in template \"{template}\""))?;
        let inner = after[..end].trim();
        let path = inner.strip_prefix('$').unwrap_or(inner);
        let parts: Vec<String> = path.split('.').map(|s| s.trim().to_string()).collect();
        if parts.is_empty()
            || parts.iter().any(|p| {
                p.is_empty()
                    || !p
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            })
        {
            return Err(format!(
                "template placeholder `{{{inner}}}` must be a path like `{{$rfi.id}}`"
            ));
        }
        out.push(parts);
        rest = &after[end + 1..];
    }
    Ok(out)
}

/// Check a `scrape::page(:select(...))` selector against the closed CSS subset
/// the Go loop kernel runs (cascadia): type or `*`, `.class`, `#id`,
/// `[attr]` / `[attr=value]` (also `~=`, `^=`, `$=`, `*=`), the combinators
/// descendant, `>`, `+`, `~`, and `,` lists. Pseudo-classes are not accepted.
pub fn validate_css_selector(sel: &str) -> Result<(), String> {
    fn ident(chars: &[char], i: &mut usize) -> bool {
        let start = *i;
        while *i < chars.len()
            && (chars[*i].is_ascii_alphanumeric() || chars[*i] == '-' || chars[*i] == '_')
        {
            *i += 1;
        }
        *i > start && !chars[start].is_ascii_digit()
    }
    fn skip_ws(chars: &[char], i: &mut usize) -> bool {
        let start = *i;
        while *i < chars.len() && chars[*i].is_whitespace() {
            *i += 1;
        }
        *i > start
    }
    fn attr(chars: &[char], i: &mut usize) -> Result<(), String> {
        *i += 1;
        skip_ws(chars, i);
        if !ident(chars, i) {
            return Err("`[` must be followed by an attribute name".into());
        }
        skip_ws(chars, i);
        if *i < chars.len() && chars[*i] != ']' {
            if "~^$*".contains(chars[*i]) {
                *i += 1;
            }
            if *i >= chars.len() || chars[*i] != '=' {
                return Err("attribute selectors are `[name]` or `[name=value]`".into());
            }
            *i += 1;
            skip_ws(chars, i);
            match chars.get(*i) {
                Some(&q) if q == '"' || q == '\'' => {
                    *i += 1;
                    while *i < chars.len() && chars[*i] != q {
                        if chars[*i] == '\\' {
                            return Err("escapes are not supported in attribute values".into());
                        }
                        *i += 1;
                    }
                    if *i >= chars.len() {
                        return Err("unterminated quoted attribute value".into());
                    }
                    *i += 1;
                }
                _ => {
                    if !ident(chars, i) {
                        return Err("attribute value must be a name or a quoted string".into());
                    }
                }
            }
            skip_ws(chars, i);
        }
        if chars.get(*i) != Some(&']') {
            return Err("unclosed `[`".into());
        }
        *i += 1;
        Ok(())
    }
    // Returns false when no compound selector starts at `i`.
    fn compound(chars: &[char], i: &mut usize) -> Result<bool, String> {
        let start = *i;
        if chars.get(*i) == Some(&'*') {
            *i += 1;
        } else if chars.get(*i).is_some_and(|c| c.is_ascii_alphabetic()) {
            ident(chars, i);
        }
        loop {
            match chars.get(*i) {
                Some('.') | Some('#') => {
                    let mark = chars[*i];
                    *i += 1;
                    if !ident(chars, i) {
                        return Err(format!("`{mark}` must be followed by a name"));
                    }
                }
                Some('[') => attr(chars, i)?,
                Some(':') => {
                    return Err("pseudo-classes such as `:first-child` are not supported".into())
                }
                _ => break,
            }
        }
        Ok(*i > start)
    }

    if sel.trim().is_empty() {
        return Err("the selector is empty".into());
    }
    let chars: Vec<char> = sel.chars().collect();
    let mut i = 0;
    loop {
        skip_ws(&chars, &mut i);
        if !compound(&chars, &mut i)? {
            return Err(match chars.get(i) {
                Some(c) => format!("unexpected `{c}`"),
                None => "a selector is missing after `,` or a combinator".into(),
            });
        }
        loop {
            let ws = skip_ws(&chars, &mut i);
            match chars.get(i) {
                None => return Ok(()),
                Some(',') => {
                    i += 1;
                    break;
                }
                Some('>') | Some('+') | Some('~') => {
                    i += 1;
                    skip_ws(&chars, &mut i);
                }
                Some(c) if !ws => return Err(format!("unexpected `{c}`")),
                Some(_) => {}
            }
            if !compound(&chars, &mut i)? {
                return Err(match chars.get(i) {
                    Some(c) => format!("unexpected `{c}`"),
                    None => "a selector is missing after a combinator".into(),
                });
            }
        }
    }
}

/// Root binding names referenced by an expression.
pub fn expr_roots(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Var(name) => out.push(name.clone()),
        Expr::Member { base, .. } => expr_roots(base, out),
        Expr::Call { callee, args } => {
            expr_roots(callee, out);
            for a in args {
                expr_roots(a, out);
            }
        }
        Expr::BinOp { left, right, .. } => {
            expr_roots(left, out);
            expr_roots(right, out);
        }
        Expr::Unary { expr, .. } | Expr::Await(expr) => expr_roots(expr, out),
        Expr::List(items) => items.iter().for_each(|i| expr_roots(i, out)),
        Expr::New { fields, .. } => fields.iter().for_each(|(_, v)| expr_roots(v, out)),
        _ => {}
    }
}

/// Worst-case counts for one run, derived from static bounds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoopBounds {
    pub model_calls: u64,
    pub writes: u64,
    pub notices: u64,
    pub approvals: u64,
    pub reads: u64,
    /// `text::score` steps: listed in the report, never model calls.
    pub scores: u64,
    pub rows_scanned: u64,
    pub unchecked_effects: u64,
    /// Embedding steps (CPython embed worker). Never model calls.
    pub embeds: u64,
}

impl LoopBounds {
    pub fn effects(&self) -> u64 {
        self.writes + self.notices
    }

    fn add(&mut self, other: &LoopBounds) {
        self.model_calls = self.model_calls.saturating_add(other.model_calls);
        self.writes = self.writes.saturating_add(other.writes);
        self.notices = self.notices.saturating_add(other.notices);
        self.approvals = self.approvals.saturating_add(other.approvals);
        self.reads = self.reads.saturating_add(other.reads);
        self.scores = self.scores.saturating_add(other.scores);
        self.rows_scanned = self.rows_scanned.saturating_add(other.rows_scanned);
        self.unchecked_effects = self
            .unchecked_effects
            .saturating_add(other.unchecked_effects);
        self.embeds = self.embeds.saturating_add(other.embeds);
    }

    fn max(&mut self, other: &LoopBounds) {
        self.model_calls = self.model_calls.max(other.model_calls);
        self.writes = self.writes.max(other.writes);
        self.notices = self.notices.max(other.notices);
        self.approvals = self.approvals.max(other.approvals);
        self.reads = self.reads.max(other.reads);
        self.scores = self.scores.max(other.scores);
        self.rows_scanned = self.rows_scanned.max(other.rows_scanned);
        self.unchecked_effects = self.unchecked_effects.max(other.unchecked_effects);
        self.embeds = self.embeds.max(other.embeds);
    }

    fn scale(&self, n: u64) -> LoopBounds {
        LoopBounds {
            model_calls: self.model_calls.saturating_mul(n),
            writes: self.writes.saturating_mul(n),
            notices: self.notices.saturating_mul(n),
            approvals: self.approvals.saturating_mul(n),
            reads: self.reads.saturating_mul(n),
            scores: self.scores.saturating_mul(n),
            rows_scanned: self.rows_scanned.saturating_mul(n),
            unchecked_effects: self.unchecked_effects.saturating_mul(n),
            embeds: self.embeds.saturating_mul(n),
        }
    }
}

/// Worst-case fetches for one `loop::read`: each try of a read, and for
/// `scrape::site` each try of every page up to its `:max` cap.
pub fn read_fetch_bound(read: &LoopNode) -> u64 {
    let tries = 1 + read.number_prop("retry").unwrap_or(DEFAULT_READ_RETRY);
    if read.string_prop("op") == Some("scrape::site") {
        tries.saturating_mul(read.number_prop("max").unwrap_or(0))
    } else {
        tries
    }
}

/// Upper bound on model calls, effects, approvals, and reads for one run.
pub fn loop_bounds(lp: &Loop) -> LoopBounds {
    fn block(steps: &[LoopNode]) -> LoopBounds {
        let mut total = LoopBounds::default();
        for step in steps {
            total.add(&node(step));
        }
        total
    }
    fn node(step: &LoopNode) -> LoopBounds {
        let mut b = LoopBounds::default();
        match step.name.as_str() {
            "let" if is_loop_embed(step) => b.embeds = 1,
            "find" => {
                b.rows_scanned = if step.has_flag("one") {
                    1
                } else {
                    step.number_prop("max").unwrap_or(0)
                };
            }
            "let" if step.string_prop("op") == Some(LOOP_SCORE_OP) => b.scores = 1,
            "read" => b.reads = read_fetch_bound(step),
            "fallback" => {
                for attempt in step.children.iter().filter(|c| c.name == "read") {
                    b.reads = b.reads.saturating_add(read_fetch_bound(attempt));
                }
                if let Some(o) = step.child("otherwise") {
                    b.add(&block(&o.children));
                }
            }
            "ask" => {
                b.model_calls = 1 + step.number_prop("retry").unwrap_or(DEFAULT_ASK_RETRY);
                if let Some(o) = step.child("otherwise") {
                    b.add(&block(&o.children));
                }
            }
            "write" => {
                b.writes = 1;
                if step.prop("unchecked").is_some() {
                    b.unchecked_effects = 1;
                }
            }
            "notify" => {
                b.notices = 1;
                if step.prop("unchecked").is_some() {
                    b.unchecked_effects = 1;
                }
            }
            "approve" => {
                b.approvals = 1;
                let mut arms = LoopBounds::default();
                for c in &step.children {
                    arms.max(&block(&c.children));
                }
                b.add(&arms);
            }
            "gate" => {
                if let Some(o) = step.child("otherwise") {
                    b.add(&block(&o.children));
                }
            }
            "branch" => {
                for c in &step.children {
                    b.max(&block(&c.children));
                }
            }
            "each" => {
                let n = step.number_prop("max").unwrap_or(0);
                b = block(&step.children).scale(n);
            }
            _ => {}
        }
        b
    }
    block(lp.steps())
}

fn render_expr(expr: &Expr) -> String {
    fn path(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Var(n) => Some(format!("${n}")),
            Expr::Member { base, field } => path(base).map(|b| format!("{b}.{field}")),
            _ => None,
        }
    }
    match expr {
        Expr::String(s)
            if template_placeholders(s)
                .map(|p| p.is_empty())
                .unwrap_or(true) =>
        {
            format!("\"{s}\"")
        }
        Expr::String(_) => "this text template".into(),
        Expr::Number(n) => n.clone(),
        Expr::Bool(b) => b.to_string(),
        other => path(other)
            .map(|p| format!("`{p}`"))
            .unwrap_or_else(|| "this expression".into()),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum BindTy {
    Value,
    Row(String),
    List(String),
    Approval,
    Page,
    Fields(&'static [&'static str]),
    Typed(ValTy),
    /// A bounded list from `scrape::site` or `scrape::select`; `loop::each`
    /// binds one item of the inner shape.
    Items(Box<BindTy>),
}

/// Static type of a loop value, used to compare `Contract.new(:field(expr))`
/// arguments with the field's declared type.
#[derive(Debug, Clone, PartialEq)]
enum ValTy {
    Str,
    Int,
    Num,
    Bool,
    Vector { elem: String, len: Option<u64> },
    Row(String),
    List,
    Unknown,
}

impl ValTy {
    fn display(&self) -> String {
        match self {
            ValTy::Str => "Str".into(),
            ValTy::Int => "Int".into(),
            ValTy::Num => "Num".into(),
            ValTy::Bool => "Bool".into(),
            ValTy::Vector { elem, len: Some(n) } => format!("Vec[{elem}; {n}]"),
            ValTy::Vector { elem, len: None } => format!("Vec[{elem}]"),
            ValTy::Row(c) => c.clone(),
            ValTy::List => "a list".into(),
            ValTy::Unknown => "an unknown type".into(),
        }
    }

    /// Whether a value of type `value` may be stored in a field of this type.
    /// Nothing coerces: `Str` never becomes `Int` or a vector. `Int` widens to `Num`.
    fn accepts(&self, value: &ValTy) -> bool {
        match (self, value) {
            (ValTy::Num, ValTy::Int) => true,
            (ValTy::Unknown, _) => true,
            (a, b) => a == b && *b != ValTy::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
struct Binding {
    ty: BindTy,
    tainted: bool,
}

type Scope = HashMap<String, Binding>;

struct Checker<'a> {
    program: &'a Program,
    lp: &'a Loop,
    names: HashSet<String>,
    trigger_resource: Option<String>,
}

const APPROVAL_FIELDS: &[&str] = &["outcome", "by", "decided_at", "note"];
const PAGE_FIELDS: &[&str] = &["url", "status", "title", "text"];
const CALL_FIELDS: &[&str] = &["text", "data"];
const MATCH_FIELDS: &[&str] = &["url", "text", "href"];
/// Values a `scrape::page`, `scrape::render`, or `scrape::site` page gives
/// same-named `:into` fields, with the type each field must have.
const PAGE_INTO: &[(&str, &str)] = &[
    ("url", "Str"),
    ("status", "Int"),
    ("title", "Str"),
    ("text", "Str"),
];
/// Values a `scrape::select` match gives same-named `:into` fields.
const MATCH_INTO: &[(&str, &str)] = &[("url", "Str"), ("text", "Str"), ("href", "Str")];
/// Options that apply only to some `scrape::*` reads, and the reads they apply to.
const SCRAPE_READ_OPTIONS: &[(&str, &[&str])] = &[
    (
        "select",
        &["scrape::page", "scrape::render", "scrape::site"],
    ),
    (
        "into",
        &[
            "scrape::page",
            "scrape::render",
            "scrape::site",
            "scrape::select",
        ],
    ),
    (
        "fields",
        &[
            "scrape::page",
            "scrape::render",
            "scrape::site",
            "scrape::select",
        ],
    ),
    ("css", &["scrape::select"]),
    ("depth", &["scrape::site"]),
    ("max", &["scrape::site", "scrape::select"]),
    ("same_host", &["scrape::site"]),
    ("link_css", &["scrape::site"]),
];
/// Date window fields on the reserved `$calendar` binding, in the loop's time zone.
pub const CALENDAR_FIELDS: &[&str] = &["today", "weekday", "last7_start", "last7_end", "next7_end"];
const RESERVED_BINDINGS: &[&str] = &["today", "now", "event", "calendar"];
pub const SCHEDULE_EVENT_FIELDS: &[&str] = &["scheduled_for", "fired_at", "late"];
pub const MANUAL_EVENT_FIELDS: &[&str] = &["requested_by", "requested_at"];

impl<'a> Checker<'a> {
    fn err(&self, node: &LoopNode, msg: impl AsRef<str>) -> String {
        format!(
            "loop `{}`: loop::{}: {}",
            self.lp.name,
            node.name,
            msg.as_ref()
        )
    }

    fn contract_fields(&self, contract: &str) -> Option<Vec<(&'a str, &'a TypeExpr)>> {
        self.program
            .contracts
            .iter()
            .find(|c| c.name == contract)
            .map(|c| c.fields.iter().map(|f| (f.name.as_str(), &f.ty)).collect())
    }

    fn resource_contract(&self, resource: &str) -> Option<String> {
        self.program
            .resources
            .iter()
            .find(|r| r.name == resource)
            .and_then(|r| r.contract.clone())
    }

    fn bind(
        &mut self,
        node: &LoopNode,
        scope: &mut Scope,
        name: &str,
        b: Binding,
    ) -> Result<(), String> {
        if RESERVED_BINDINGS.contains(&name) {
            return Err(self.err(
                node,
                format!("`{name}` is reserved; choose another `:as` name"),
            ));
        }
        if !self.names.insert(name.to_string()) {
            return Err(self.err(
                node,
                format!("binding `{name}` is already declared in this loop"),
            ));
        }
        scope.insert(name.to_string(), b);
        Ok(())
    }

    /// Check a path `root.field...` resolves; returns its taint.
    fn check_path(
        &self,
        node: &LoopNode,
        scope: &Scope,
        row: Option<&str>,
        path: &[String],
    ) -> Result<bool, String> {
        let root = path[0].as_str();
        if let Some(contract) = row {
            if let Some(fields) = self.contract_fields(contract) {
                if fields.iter().any(|(f, _)| *f == root) {
                    if scope.contains_key(root) {
                        return Err(self.err(
                            node,
                            format!(
                                "`${root}` is both a row field and a binding; rename the binding"
                            ),
                        ));
                    }
                    return Ok(false);
                }
            }
        }
        let Some(binding) = scope.get(root) else {
            return Err(self.err(node, format!("unknown binding `${root}`")));
        };
        if let Some(field) = path.get(1) {
            let ok = match &binding.ty {
                BindTy::Row(c) => self
                    .contract_fields(c)
                    .map(|fs| fs.iter().any(|(f, _)| f == field))
                    .unwrap_or(true),
                BindTy::List(_) => field == "count",
                BindTy::Approval => APPROVAL_FIELDS.contains(&field.as_str()),
                BindTy::Page => PAGE_FIELDS.contains(&field.as_str()),
                BindTy::Fields(fields) => fields.contains(&field.as_str()),
                BindTy::Items(_) => field == "count",
                BindTy::Typed(_) => false,
                BindTy::Value => true,
            };
            if !ok {
                return Err(self.err(node, format!("`${root}` has no field `{field}`")));
            }
        }
        Ok(binding.tainted)
    }

    /// Validate a value expression; returns whether it carries model output.
    fn check_expr(
        &self,
        node: &LoopNode,
        scope: &Scope,
        row: Option<&str>,
        expr: &Expr,
    ) -> Result<bool, String> {
        match expr {
            Expr::String(_) | Expr::Number(_) | Expr::Bool(_) => Ok(false),
            Expr::Ident(name) => Err(self.err(
                node,
                format!("bare `{name}` is not a value; use `${name}` for a binding or \"{name}\" for text"),
            )),
            Expr::Var(name) => self.check_path(node, scope, row, std::slice::from_ref(name)),
            Expr::Member { .. } => {
                let mut path = Vec::new();
                let mut cur = expr;
                loop {
                    match cur {
                        Expr::Member { base, field } => {
                            path.push(field.clone());
                            cur = base;
                        }
                        Expr::Var(name) => {
                            path.push(name.clone());
                            break;
                        }
                        _ => return Err(self.err(node, "member access must start from a `$binding`")),
                    }
                }
                path.reverse();
                self.check_path(node, scope, row, &path)
            }
            Expr::BinOp { left, right, .. } => {
                Ok(self.check_expr(node, scope, row, left)? | self.check_expr(node, scope, row, right)?)
            }
            Expr::Unary { expr, .. } => self.check_expr(node, scope, row, expr),
            Expr::List(items) => {
                let mut t = false;
                for i in items {
                    t |= self.check_expr(node, scope, row, i)?;
                }
                Ok(t)
            }
            Expr::New { ty, fields } => {
                let Some(cfields) = self.contract_fields(ty) else {
                    return Err(self.err(node, format!("unknown contract `{ty}` in `{ty}.new(...)`")));
                };
                let mut t = false;
                let mut type_errors = Vec::new();
                for (fname, value) in fields {
                    let Some((_, fty)) = cfields.iter().find(|(f, _)| f == fname) else {
                        return Err(self.err(node, format!("contract `{ty}` has no field `{fname}`")));
                    };
                    t |= self.check_expr(node, scope, row, value)?;
                    if let Err(e) = self.check_field_type(node, scope, row, ty, fname, fty, value) {
                        type_errors.push(e);
                    }
                }
                if !type_errors.is_empty() {
                    return Err(type_errors.join("\n"));
                }
                Ok(t)
            }
            Expr::Call { .. } => Err(self.err(node, "function and method calls are not allowed in loop expressions")),
            _ => Err(self.err(node, "this expression form is not allowed in a loop")),
        }
    }

    fn check_template(
        &self,
        node: &LoopNode,
        scope: &Scope,
        prop: &str,
    ) -> Result<(bool, usize), String> {
        let Some(expr) = node.prop(prop) else {
            return Ok((false, 0));
        };
        let Some(text) = expr.as_string_literal() else {
            return Err(self.err(node, format!(":{prop} must be a string template")));
        };
        let placeholders = template_placeholders(text).map_err(|e| self.err(node, e))?;
        let mut tainted = false;
        for path in &placeholders {
            tainted |= self.check_path(node, scope, None, path)?;
            let expr = path[1..]
                .iter()
                .fold(Expr::Var(path[0].clone()), |base, field| Expr::Member {
                    base: Box::new(base),
                    field: field.clone(),
                });
            if let ty @ ValTy::Vector { .. } = self.infer_val_ty(scope, None, &expr) {
                return Err(self.err(
                    node,
                    format!(
                        ":{prop}: `{{${}}}` is a vector ({}) and cannot be interpolated into text",
                        path.join("."),
                        ty.display()
                    ),
                ));
            }
        }
        Ok((tainted, placeholders.len()))
    }

    /// An embedding `loop::let`: checks the nested `tensor::infer(...)` and
    /// returns the bound vector type and whether `:from` carries model output.
    /// `None` for a plain `:value` let.
    fn check_let_embed(
        &self,
        step: &LoopNode,
        scope: &Scope,
    ) -> Result<Option<(ValTy, bool)>, String> {
        let Some(op) = step.string_prop("op") else {
            if let Some(extra) = ["model", "from", "prefer"]
                .iter()
                .find(|p| step.prop(p).is_some())
            {
                return Err(self.err(
                    step,
                    format!(":{extra} belongs to a nested `tensor::infer(...)`, e.g. `loop::let(:as(vec), tensor::infer(:from($doc.text)))`"),
                ));
            }
            if step.prop("value").is_none() {
                return Err(self.err(
                    step,
                    "requires `:value(...)` or a nested `tensor::infer(...)`",
                ));
            }
            return Ok(None);
        };
        debug_assert_eq!(op, LOOP_EMBED_OP, "checked in check_step");
        if step.prop("value").is_some() {
            return Err(self.err(
                step,
                "takes either `:value(...)` or a nested `tensor::infer(...)`, not both",
            ));
        }
        let model = loop_embed_model(step).map_err(|e| self.err(step, e))?;
        if let Some(prefer) = step.ident_prop("prefer") {
            if prefer.eq_ignore_ascii_case("CUDA") {
                return Err(self.err(
                    step,
                    format!(
                        "tensor::infer is CPU-only in {}; remove :prefer(CUDA) (default/CPU accepted)",
                        crate::silc_release()
                    ),
                ));
            }
            if !prefer.eq_ignore_ascii_case("CPU") {
                return Err(self.err(
                    step,
                    format!("unsupported tensor::infer :prefer({prefer}); only CPU is accepted"),
                ));
            }
        }
        let Some(from) = step.prop("from") else {
            return Err(self.err(
                step,
                "`tensor::infer` requires `:from(...)`, the text to embed, e.g. `:from($release.text)`",
            ));
        };
        let tainted = self.check_expr(step, scope, None, from)?;
        let what = render_expr(from);
        match self.infer_val_ty(scope, None, from) {
            ValTy::Str => {}
            ValTy::Unknown => {
                return Err(self.err(
                    step,
                    format!("`tensor::infer` embeds a Str, but the type of {what} is not known; embed a typed field such as `$release.text`"),
                ));
            }
            other => {
                return Err(self.err(
                    step,
                    format!(
                        "`tensor::infer` embeds a Str, but {what} is {}",
                        other.display()
                    ),
                ));
            }
        }
        let ty = self.resolve_val_ty(&loop_embed_type(model), 0);
        Ok(Some((ty, tainted)))
    }

    fn expr_ty(&self, scope: &Scope, expr: &Expr) -> BindTy {
        match expr {
            Expr::Var(name) => scope
                .get(name)
                .map(|b| b.ty.clone())
                .unwrap_or(BindTy::Value),
            Expr::New { ty, .. } => BindTy::Row(ty.clone()),
            Expr::Member { base, field } => {
                if let Expr::Var(root) = base.as_ref() {
                    if let Some(BindTy::Row(c)) = scope.get(root).map(|b| &b.ty) {
                        if let Some(fields) = self.contract_fields(c) {
                            if let Some((_, TypeExpr::Array(inner))) =
                                fields.iter().find(|(f, _)| f == field)
                            {
                                if let TypeExpr::Named(n) = inner.as_ref() {
                                    if self.contract_fields(n).is_some() {
                                        return BindTy::List(n.clone());
                                    }
                                }
                                return BindTy::List(String::new());
                            }
                        }
                    }
                }
                BindTy::Value
            }
            _ => BindTy::Value,
        }
    }

    fn resolve_val_ty(&self, ty: &TypeExpr, depth: usize) -> ValTy {
        match ty {
            TypeExpr::Optional(inner) => self.resolve_val_ty(inner, depth),
            TypeExpr::Array(_) => ValTy::List,
            TypeExpr::Vec { elem, len } => ValTy::Vector {
                elem: elem.clone(),
                len: *len,
            },
            TypeExpr::Named(name) => match name.as_str() {
                "Str" | "UUID" => ValTy::Str,
                "Int" | "int32" | "int64" => ValTy::Int,
                "num32" | "num64" => ValTy::Num,
                "Bool" => ValTy::Bool,
                _ => {
                    if let Some(subset) = self.program.subsets.iter().find(|s| s.name == *name) {
                        if depth < 8 {
                            return self.resolve_val_ty(&subset.base, depth + 1);
                        }
                        ValTy::Unknown
                    } else if self.contract_fields(name).is_some() {
                        ValTy::Row(name.clone())
                    } else {
                        ValTy::Unknown
                    }
                }
            },
        }
    }

    fn infer_val_ty(&self, scope: &Scope, row: Option<&str>, expr: &Expr) -> ValTy {
        match expr {
            Expr::String(_) => ValTy::Str,
            Expr::Number(n) => {
                if n.contains('.') || n.contains('e') || n.contains('E') {
                    ValTy::Num
                } else {
                    ValTy::Int
                }
            }
            Expr::Bool(_) => ValTy::Bool,
            Expr::Var(name) => {
                if let Some(contract) = row {
                    if let Some(fields) = self.contract_fields(contract) {
                        if let Some((_, fty)) = fields.iter().find(|(f, _)| f == name) {
                            return self.resolve_val_ty(fty, 0);
                        }
                    }
                }
                match name.as_str() {
                    "today" | "now" => return ValTy::Str,
                    _ => {}
                }
                match scope.get(name).map(|b| &b.ty) {
                    Some(BindTy::Row(c)) => ValTy::Row(c.clone()),
                    Some(BindTy::Typed(t)) => t.clone(),
                    Some(BindTy::List(_)) | Some(BindTy::Items(_)) => ValTy::List,
                    _ => ValTy::Unknown,
                }
            }
            Expr::Member { base, field } => {
                let base_ty = match base.as_ref() {
                    Expr::Var(root) => scope.get(root).map(|b| b.ty.clone()),
                    other => match self.infer_val_ty(scope, row, other) {
                        ValTy::Row(c) => Some(BindTy::Row(c)),
                        _ => None,
                    },
                };
                match base_ty {
                    Some(BindTy::Row(c)) => self
                        .contract_fields(&c)
                        .and_then(|fs| {
                            fs.iter()
                                .find(|(f, _)| f == field)
                                .map(|(_, t)| self.resolve_val_ty(t, 0))
                        })
                        .unwrap_or(ValTy::Unknown),
                    Some(BindTy::List(_)) | Some(BindTy::Items(_)) if field == "count" => {
                        ValTy::Int
                    }
                    Some(BindTy::Page) => match field.as_str() {
                        "status" => ValTy::Int,
                        _ => ValTy::Str,
                    },
                    Some(BindTy::Approval) => ValTy::Str,
                    Some(BindTy::Fields(_)) => match field.as_str() {
                        "data" => ValTy::Unknown,
                        "late" => ValTy::Bool,
                        _ => ValTy::Str,
                    },
                    _ => ValTy::Unknown,
                }
            }
            Expr::BinOp { op, left, right } => match op {
                BinOp::Eq
                | BinOp::Ne
                | BinOp::Lt
                | BinOp::Le
                | BinOp::Gt
                | BinOp::Ge
                | BinOp::And
                | BinOp::Or => ValTy::Bool,
                _ => {
                    let l = self.infer_val_ty(scope, row, left);
                    let r = self.infer_val_ty(scope, row, right);
                    match (l, r) {
                        (ValTy::Int, ValTy::Int) => ValTy::Int,
                        (ValTy::Int | ValTy::Num, ValTy::Int | ValTy::Num) => ValTy::Num,
                        _ => ValTy::Unknown,
                    }
                }
            },
            Expr::Unary { op, expr } => match op {
                crate::expr::UnaryOp::Not => ValTy::Bool,
                crate::expr::UnaryOp::Neg => self.infer_val_ty(scope, row, expr),
            },
            Expr::List(_) => ValTy::List,
            Expr::New { ty, .. } => ValTy::Row(ty.clone()),
            _ => ValTy::Unknown,
        }
    }

    /// Reject a `Contract.new(:field(expr))` argument whose type does not match
    /// the field's declared type. Unknown value types fail closed.
    #[allow(clippy::too_many_arguments)]
    fn check_field_type(
        &self,
        node: &LoopNode,
        scope: &Scope,
        row: Option<&str>,
        contract: &str,
        field: &str,
        declared: &TypeExpr,
        value: &Expr,
    ) -> Result<(), String> {
        let expected = self.resolve_val_ty(declared, 0);
        if expected == ValTy::Unknown {
            return Ok(());
        }
        let found = self.infer_val_ty(scope, row, value);
        if expected.accepts(&found) {
            return Ok(());
        }
        let declared_name = declared.display();
        let expected_name = match &expected {
            ValTy::Vector { .. } if declared_name != expected.display() => {
                format!("{} ({declared_name})", expected.display())
            }
            _ if declared_name != expected.display() => {
                format!("{declared_name} (a {})", expected.display())
            }
            _ => expected.display(),
        };
        let what = render_expr(value);
        if found == ValTy::Unknown {
            return Err(self.err(
                node,
                format!(
                    "field `{field}` of `{contract}.new(...)` expects {expected_name}, but the type of {what} is not known; only a value with a declared type can be written"
                ),
            ));
        }
        Err(self.err(
            node,
            format!(
                "field `{field}` of `{contract}.new(...)` expects {expected_name}, but {what} is {}; nothing coerces into a typed field, so convert explicitly",
                found.display()
            ),
        ))
    }

    fn clear_taint(scope: &mut Scope, expr: Option<&Expr>, template: Option<&str>) {
        let mut roots = Vec::new();
        if let Some(e) = expr {
            expr_roots(e, &mut roots);
        }
        if let Some(t) = template {
            if let Ok(paths) = template_placeholders(t) {
                roots.extend(paths.into_iter().map(|p| p[0].clone()));
            }
        }
        for r in roots {
            if let Some(b) = scope.get_mut(&r) {
                b.tainted = false;
            }
        }
    }

    fn check_props(&self, node: &LoopNode) -> Result<&'static LoopNodeSpec, String> {
        self.check_props_except(node, &[])
    }

    /// Like `check_props`, but the named options need not be present.
    fn check_props_except(
        &self,
        node: &LoopNode,
        not_required: &[&str],
    ) -> Result<&'static LoopNodeSpec, String> {
        let spec = lookup_loop_node(&node.name).ok_or_else(|| {
            format!(
                "loop `{}`: unknown node `loop::{}`; known: {}",
                self.lp.name,
                node.name,
                catalog_loop_node_names().join(", ")
            )
        })?;
        if let Some(op) = node.string_prop("op") {
            if let Some(message) = misplaced_op(&node.name, op) {
                return Err(self.err(node, message));
            }
        }
        for prop in spec
            .props
            .iter()
            .filter(|p| p.required && !not_required.contains(&p.name))
        {
            if node.prop(prop.name).is_none() {
                return Err(self.err(node, format!("requires option `:{}`", prop.name)));
            }
        }
        let mut seen = HashSet::new();
        for (pname, value) in &node.props {
            let Some(pspec) = spec.props.iter().find(|p| p.name == *pname) else {
                return Err(self.err(node, format!("unknown option `:{pname}`")));
            };
            if !seen.insert(pname.as_str()) {
                return Err(self.err(node, format!("option `:{pname}` appears twice")));
            }
            if !pspec.closed_values.is_empty() && matches!(pspec.kind, LoopPropKind::String) {
                // `:op("mcp::call")` stays a string for one release. The nested
                // form `mcp::call(...)` is parsed into the same string.
                let text = match value {
                    Expr::String(s) | Expr::Ident(s) => s.as_str(),
                    _ => "",
                };
                if !pspec.closed_values.contains(&text) {
                    return Err(self.err(
                        node,
                        format!(":{pname} must be one of {}", pspec.closed_values.join("|")),
                    ));
                }
            } else if !pspec.closed_values.is_empty() {
                if let Err(err) = crate::catalog::check_closed_enum(value, pspec.closed_values) {
                    let message = match err {
                        crate::catalog::ClosedEnumError::NotAllowed(_) => format!(
                            ":{pname} must be one of {}",
                            pspec.closed_values.join("|")
                        ),
                        crate::catalog::ClosedEnumError::StringForm(value) => format!(
                            ":{pname} takes a bare name; write `:{pname}({value})` instead of `:{pname}(\"{value}\")`"
                        ),
                    };
                    return Err(self.err(node, message));
                }
            }
            let ok =
                crate::catalog::option_accepts(pspec.kind, value, node.ref_prop(pname).is_some());
            if !ok {
                let want = crate::catalog::option_expected(pspec.kind);
                return Err(self.err(node, format!(":{pname} must be {want}")));
            }
        }
        match spec.children {
            LoopChildPolicy::None if !node.children.is_empty() => {
                return Err(self.err(node, "does not accept child nodes"));
            }
            LoopChildPolicy::AnyOf(allowed) => {
                for child in &node.children {
                    if lookup_loop_node(&child.name).is_none() {
                        return Err(format!(
                            "loop `{}`: unknown node `loop::{}`; known: {}",
                            self.lp.name,
                            child.name,
                            catalog_loop_node_names().join(", ")
                        ));
                    }
                    if !allowed.contains(&child.name.as_str()) {
                        return Err(self.err(
                            node,
                            format!(
                                "cannot contain loop::{}; allowed: {}",
                                child.name,
                                allowed.join(", ")
                            ),
                        ));
                    }
                }
            }
            LoopChildPolicy::None | LoopChildPolicy::Any => {}
        }
        Ok(spec)
    }

    fn check_max(&self, node: &LoopNode, prop: &str, limit: u64) -> Result<u64, String> {
        let n = node.number_prop(prop).unwrap_or(0);
        if n == 0 || n > limit {
            return Err(self.err(node, format!(":{prop} must be between 1 and {limit}")));
        }
        Ok(n)
    }

    fn check_retry(&self, node: &LoopNode) -> Result<(), String> {
        if let Some(n) = node.number_prop("retry") {
            if n > MAX_LOOP_RETRY {
                return Err(self.err(node, format!(":retry must be at most {MAX_LOOP_RETRY}")));
            }
        }
        Ok(())
    }

    /// `:into(Contract)` and `:fields(Contract.new(...))` on a `scrape::*`
    /// read. `auto` lists the fields the page or match fills by name; `what`
    /// names it in messages. Returns the contract, or `None` without `:into`.
    fn check_page_into(
        &self,
        step: &LoopNode,
        auto: &[(&str, &str)],
        what: &str,
    ) -> Result<Option<String>, String> {
        let Some(into) = step.ident_prop("into") else {
            if step.prop("fields").is_some() {
                return Err(self.err(
                    step,
                    ":fields needs :into(Contract) to name the row it fills",
                ));
            }
            return Ok(None);
        };
        let Some(cfields) = self.contract_fields(into) else {
            return Err(self.err(step, format!("unknown contract `{into}` in :into")));
        };
        let is_named = |ty: &TypeExpr, want: &str| match ty {
            TypeExpr::Named(n) => n == want,
            TypeExpr::Optional(inner) => matches!(inner.as_ref(), TypeExpr::Named(n) if n == want),
            _ => false,
        };
        let mut selected: Vec<&str> = Vec::new();
        if let Some(fields) = step.prop("fields") {
            let Expr::New { ty, fields } = fields else {
                return Err(self.err(
                    step,
                    format!(":fields must be `{into}.new(:field(\"css\"), ...)`"),
                ));
            };
            if ty != into {
                return Err(self.err(step, format!(":fields must be `{into}.new(...)` to match :into({into}), not `{ty}.new(...)`")));
            }
            for (fname, value) in fields {
                let Some((_, fty)) = cfields.iter().find(|(f, _)| f == fname) else {
                    return Err(self.err(step, format!("contract `{into}` has no field `{fname}`")));
                };
                if !is_named(fty, "Str") {
                    return Err(self.err(step, format!(":fields fills `{fname}` with {what} text, so `{into}.{fname}` must be Str")));
                }
                let Some(css) = value.as_string_literal() else {
                    return Err(self.err(
                        step,
                        format!(":fields `{fname}` must be a CSS selector string"),
                    ));
                };
                validate_css_selector(css).map_err(|e| {
                    self.err(
                        step,
                        format!(
                            ":fields `{fname}`(\"{css}\") is not a supported CSS selector: {e}"
                        ),
                    )
                })?;
                selected.push(fname.as_str());
            }
        }
        for (fname, fty) in &cfields {
            if selected.contains(fname) {
                continue;
            }
            let Some((_, want)) = auto.iter().find(|(f, _)| f == fname) else {
                continue;
            };
            if !is_named(fty, want) {
                return Err(self.err(
                    step,
                    format!("`{into}.{fname}` takes the {what}'s `{fname}`, so it must be {want}"),
                ));
            }
        }
        Ok(Some(into.to_string()))
    }

    /// `loop::let(:as(x), text::score(:text(...)))`: a pure CPU score of one
    /// Str. Unchecked model output is refused, so a score never carries an
    /// ask's answer past its gate; the score itself is untainted.
    fn check_score(&self, step: &LoopNode, scope: &Scope) -> Result<BindTy, String> {
        if let Some(extra) = ["model", "from", "prefer"]
            .iter()
            .find(|p| step.prop(p).is_some())
        {
            return Err(self.err(
                step,
                format!(":{extra} belongs to `tensor::infer(...)`; `text::score` takes :text"),
            ));
        }
        if step.prop("value").is_some() {
            return Err(self.err(
                step,
                "takes :value or a nested `text::score(...)`, not both",
            ));
        }
        let Some(text) = step.prop("text") else {
            return Err(self.err(step, "`text::score` requires :text(...), the Str to score, e.g. `text::score(:text($page.text))`"));
        };
        let tainted = self.check_expr(step, scope, None, text)?;
        let what = render_expr(text);
        if tainted {
            return Err(self.err(
                step,
                format!("`text::score` cannot score {what}: it is model output from loop::ask that has not passed a loop::gate or loop::approve; gate it first, or score what the loop read"),
            ));
        }
        if let Expr::Var(name) = text {
            if scope
                .get(name)
                .is_some_and(|b| !matches!(b.ty, BindTy::Typed(_) | BindTy::Value))
            {
                return Err(self.err(
                    step,
                    format!("`text::score` scores text, and `${name}` is a whole value; score one of its Str fields, e.g. `${name}.text`"),
                ));
            }
        }
        match self.infer_val_ty(scope, None, text) {
            ValTy::Str => Ok(BindTy::Typed(ValTy::Num)),
            ValTy::Unknown => Err(self.err(
                step,
                format!("`text::score` scores text, but the type of {what} is not known; pass a Str such as `$page.text`"),
            )),
            other => Err(self.err(
                step,
                format!("`text::score` scores text, but {what} is {}; pass a Str such as `$page.text`", other.display()),
            )),
        }
    }

    /// `:from(Resource.nearest(:to($vec)))`: exactly one `:to`, a vector of the
    /// dimension the resource stores, and no `:order` or `:desc` (rows come
    /// back best first by cosine).
    fn check_nearest(
        &self,
        step: &LoopNode,
        scope: &Scope,
        res: &str,
        contract: &str,
    ) -> Result<(), String> {
        let usage = format!("write `:from({res}.nearest(:to($vec)))` with an embedding binding");
        let options = step.ref_options("from").unwrap_or_default();
        if options.is_empty() {
            return Err(self.err(step, format!("`{res}.nearest` needs `:to(...)`; {usage}")));
        }
        if let Some((name, _)) = options.iter().find(|(n, _)| *n != "to") {
            let what = if name.is_empty() {
                "a positional argument".to_string()
            } else {
                format!("unknown option `:{name}`")
            };
            return Err(self.err(
                step,
                format!("`{res}.nearest` takes only `:to(...)`, found {what}; {usage}"),
            ));
        }
        if options.len() > 1 {
            return Err(self.err(step, format!("`{res}.nearest` takes `:to(...)` once")));
        }
        for prop in ["order", "desc"] {
            if step.prop(prop).is_some() {
                return Err(self.err(
                    step,
                    format!(":{prop} does not apply to `{res}.nearest`; nearest rows always come back best first by cosine similarity"),
                ));
            }
        }
        let field = crate::resource::NEAREST_VECTOR_FIELD;
        let stored = self
            .contract_fields(contract)
            .and_then(|fs| {
                fs.iter()
                    .find(|(f, _)| *f == field)
                    .map(|(_, t)| self.resolve_val_ty(t, 0))
            })
            .and_then(|t| match t {
                ValTy::Vector { len: Some(n), .. } => Some(n),
                _ => None,
            });
        let Some(dim) = stored else {
            return Err(self.err(
                step,
                format!("`{res}.nearest` ranks on `{contract}.{field}`, which must be a fixed-length vector such as `Emb384`"),
            ));
        };
        let to = options[0].1;
        self.check_expr(step, scope, None, to)?;
        match self.infer_val_ty(scope, None, to) {
            ValTy::Vector { len: Some(n), .. } if n == dim => Ok(()),
            ValTy::Vector { len, .. } => Err(self.err(
                step,
                format!(
                    "`:to` must be a {dim}-float embedding to match `{contract}.{field}`, but {} has {}",
                    render_expr(to),
                    len.map_or("no fixed length".to_string(), |n| format!("{n} floats"))
                ),
            )),
            ValTy::Unknown => Err(self.err(
                step,
                format!(
                    "`:to` must be a {dim}-float embedding (from the loop embedding step), but the type of {} is not known",
                    render_expr(to)
                ),
            )),
            other => Err(self.err(
                step,
                format!(
                    "`:to` must be a {dim}-float embedding (from the loop embedding step), but {} is {}",
                    render_expr(to),
                    other.display()
                ),
            )),
        }
    }

    fn block_terminates(steps: &[LoopNode]) -> bool {
        match steps.last() {
            Some(last) if TERMINATORS.contains(&last.name.as_str()) => true,
            Some(last) if last.name == "branch" => last
                .children
                .iter()
                .all(|arm| Self::block_terminates(&arm.children)),
            _ => false,
        }
    }

    fn check_terminating(&self, owner: &LoopNode, block: &LoopNode) -> Result<(), String> {
        if !Self::block_terminates(&block.children) {
            return Err(self.err(
                owner,
                format!("loop::{} must end in loop::stop, loop::fail, or loop::skip so it cannot fall through", block.name),
            ));
        }
        Ok(())
    }

    fn check_block(
        &mut self,
        steps: &[LoopNode],
        scope: &mut Scope,
        in_each: bool,
    ) -> Result<(), String> {
        if steps.is_empty() {
            return Err(format!(
                "loop `{}`: a block must contain at least one step",
                self.lp.name
            ));
        }
        for (i, step) in steps.iter().enumerate() {
            if LOOP_TRIGGERS.contains(&step.name.as_str()) {
                return Err(self.err(
                    step,
                    "a trigger must be the first child of loop::flow, and only one is allowed",
                ));
            }
            if !LOOP_STEPS.contains(&step.name.as_str()) {
                return Err(self.err(step, "is not a step here"));
            }
            if i + 1 < steps.len() && Self::block_terminates(&steps[..=i]) {
                return Err(self.err(
                    &steps[i + 1],
                    "is unreachable; the previous step always ends the block",
                ));
            }
            self.check_step(step, scope, in_each)?;
        }
        Ok(())
    }

    /// Check one read operation's options; returns the shape of its result.
    fn check_read(&self, step: &LoopNode, scope: &Scope) -> Result<BindTy, String> {
        let op = step.string_prop("op").unwrap_or_default();
        let mcp_props = ["server", "tool", "args", "auth_env"];
        let ty = if op == "mcp::call" {
            if step.prop("url").is_some() {
                return Err(self.err(step, "`mcp::call` takes :server and :tool, not :url"));
            }
            if let Some((extra, ops)) = SCRAPE_READ_OPTIONS
                .iter()
                .find(|(p, _)| *p != "select" && step.prop(p).is_some())
            {
                return Err(self.err(
                    step,
                    format!(
                        ":{extra} only applies to {}; an `mcp::call` result binds `text` and `data`",
                        ops.iter().map(|o| format!("`{o}`")).collect::<Vec<_>>().join(", ")
                    ),
                ));
            }
            let Some(server) = step.string_prop("server") else {
                return Err(self.err(step, "`mcp::call` requires :server(\"https://.../mcp\")"));
            };
            if !(server.starts_with("http://") || server.starts_with("https://")) {
                return Err(self.err(step, ":server must be an absolute http(s) URL"));
            }
            if step.string_prop("tool").is_none_or(|t| t.trim().is_empty()) {
                return Err(self.err(step, "`mcp::call` requires :tool(\"name\")"));
            }
            if let Some(args) = step.prop("args") {
                if !matches!(args, Expr::New { .. }) {
                    return Err(self.err(step, ":args must be `Contract.new(...)`"));
                }
                self.check_expr(step, scope, None, args)?;
            }
            if let Some(env) = step.string_prop("auth_env") {
                let valid = !env.is_empty()
                    && env
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                    && !env.starts_with(|c: char| c.is_ascii_digit());
                if !valid {
                    return Err(self.err(
                        step,
                        ":auth_env must be an environment variable name like API_TOKEN",
                    ));
                }
            }
            if let Some(sel) = step.string_prop("select") {
                let valid = sel.split('.').all(|seg| {
                    !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                });
                if !valid {
                    return Err(self.err(
                        step,
                        ":select must be a dotted field path like \"records.parsed\"; CSS selectors apply to `scrape::page`",
                    ));
                }
            }
            BindTy::Fields(CALL_FIELDS)
        } else {
            if let Some(extra) = mcp_props.iter().find(|p| step.prop(p).is_some()) {
                return Err(self.err(step, format!(":{extra} only applies to `mcp::call`")));
            }
            for (option, ops) in SCRAPE_READ_OPTIONS {
                if step.prop(option).is_some() && !ops.contains(&op) {
                    let hint = if *option == "select" && op == "scrape::select" {
                        "; `scrape::select` takes its selector as :css(\"...\")".to_string()
                    } else {
                        format!(
                            "; it applies to {}",
                            ops.iter()
                                .map(|o| format!("`{o}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    return Err(self.err(step, format!(":{option} does not apply to `{op}`{hint}")));
                }
            }
            let Some(url) = step.prop("url") else {
                return Err(self.err(step, format!("`{op}` requires :url(...)")));
            };
            self.check_expr(step, scope, None, url)?;
            if let Expr::String(u) = url {
                if !(u.starts_with("http://") || u.starts_with("https://")) {
                    return Err(self.err(step, ":url must be an absolute http(s) URL"));
                }
            }
            for option in ["select", "css", "link_css"] {
                if let Some(sel) = step.string_prop(option) {
                    validate_css_selector(sel).map_err(|e| {
                        let hint = if option == "select" {
                            "; dotted JSON paths apply to `mcp::call`"
                        } else {
                            ""
                        };
                        self.err(
                            step,
                            format!(
                                ":{option}(\"{sel}\") is not a supported CSS selector: {e}{hint}"
                            ),
                        )
                    })?;
                }
            }
            match op {
                "scrape::site" => {
                    if step.prop("max").is_none() {
                        return Err(self.err(
                            step,
                            format!("`scrape::site` requires :max(N), a hard cap on pages fetched (at most {MAX_LOOP_SITE_PAGES}), so the crawl is bounded and `silc build` can count it"),
                        ));
                    }
                    self.check_max(step, "max", MAX_LOOP_SITE_PAGES)?;
                    if step.prop("depth").is_some() {
                        self.check_max(
                            step,
                            "depth",
                            u64::from(crate::scrape_catalog::MAX_SITE_DEPTH),
                        )?;
                    }
                    let item = match self.check_page_into(step, PAGE_INTO, "page")? {
                        Some(c) => BindTy::Row(c),
                        None => BindTy::Page,
                    };
                    BindTy::Items(Box::new(item))
                }
                "scrape::select" => {
                    if step.string_prop("css").is_none() {
                        return Err(self.err(step, "`scrape::select` requires :css(\"selector\")"));
                    }
                    if step.prop("max").is_none() {
                        return Err(self.err(
                            step,
                            format!("`scrape::select` requires :max(N) (at most {MAX_LOOP_LIST}) so the list of matches is bounded"),
                        ));
                    }
                    self.check_max(step, "max", MAX_LOOP_LIST)?;
                    let item = match self.check_page_into(step, MATCH_INTO, "match")? {
                        Some(c) => BindTy::Row(c),
                        None => BindTy::Fields(MATCH_FIELDS),
                    };
                    BindTy::Items(Box::new(item))
                }
                _ => match self.check_page_into(step, PAGE_INTO, "page")? {
                    Some(c) => BindTy::Row(c),
                    None => BindTy::Page,
                },
            }
        };

        self.check_retry(step)?;
        Ok(ty)
    }

    fn check_fallback(
        &mut self,
        step: &LoopNode,
        scope: &mut Scope,
        in_each: bool,
    ) -> Result<(), String> {
        let attempts: Vec<&LoopNode> = step.children.iter().filter(|c| c.name == "read").collect();
        let otherwise_count = step
            .children
            .iter()
            .filter(|c| c.name == "otherwise")
            .count();
        if attempts.len() < 2 {
            return Err(self.err(
                step,
                "needs two or more loop::read attempts; with one attempt use loop::read",
            ));
        }
        if otherwise_count != 1 || step.children.last().is_none_or(|c| c.name != "otherwise") {
            return Err(self.err(
                step,
                "needs the loop::read attempts followed by exactly one loop::otherwise",
            ));
        }
        let mut result_ty: Option<(BindTy, String)> = None;
        for attempt in &attempts {
            self.check_props_except(attempt, &["as"])?;
            if attempt.prop("as").is_some() {
                return Err(self.err(
                    attempt,
                    "inside loop::fallback a read has no :as; the accepted result binds to the fallback's :as",
                ));
            }
            let ty = self.check_read(attempt, scope)?;
            let op = attempt.string_prop("op").unwrap_or_default().to_string();
            match &result_ty {
                Some((_, first_op)) if *first_op != op => {
                    return Err(self.err(
                        step,
                        format!(
                            "attempts must use one read operation so the result has one shape; found `{first_op}` and `{op}`"
                        ),
                    ));
                }
                Some((first_ty, _)) if *first_ty != ty => {
                    return Err(self.err(
                        step,
                        "attempts must bind one shape; give every `scrape::*` attempt the same :into (or none)",
                    ));
                }
                Some(_) => {}
                None => result_ty = Some((ty, op)),
            }
        }
        let ty = result_ty.map(|(t, _)| t).unwrap_or(BindTy::Value);
        let name = step.ident_prop("as").unwrap().to_string();
        let accept = step.prop("accept").unwrap();
        let mut accept_scope = scope.clone();
        self.bind(
            step,
            &mut accept_scope,
            &name,
            Binding {
                ty: ty.clone(),
                tainted: false,
            },
        )?;
        let mut roots = Vec::new();
        expr_roots(accept, &mut roots);
        if !roots.contains(&name) {
            return Err(self.err(
                step,
                format!(":accept must test the attempt's result through `${name}`"),
            ));
        }
        self.check_expr(step, &accept_scope, None, accept)?;
        let otherwise = step.child("otherwise").unwrap();
        self.check_terminating(step, otherwise)?;
        let mut inner = scope.clone();
        self.check_block(&otherwise.children, &mut inner, in_each)?;
        scope.insert(name, Binding { ty, tainted: false });
        Ok(())
    }

    fn check_step(
        &mut self,
        step: &LoopNode,
        scope: &mut Scope,
        in_each: bool,
    ) -> Result<(), String> {
        self.check_props(step)?;
        match step.name.as_str() {
            "let" if step.string_prop("op") == Some(LOOP_SCORE_OP) => {
                let ty = self.check_score(step, scope)?;
                let name = step.ident_prop("as").unwrap().to_string();
                self.bind(step, scope, &name, Binding { ty, tainted: false })?;
            }
            "let" => {
                if step.prop("text").is_some() {
                    return Err(self.err(
                        step,
                        ":text belongs to a nested `text::score(:text(...))`; a plain let takes :value",
                    ));
                }
                if let Some((ty, tainted)) = self.check_let_embed(step, scope)? {
                    let name = step.ident_prop("as").unwrap().to_string();
                    return self.bind(
                        step,
                        scope,
                        &name,
                        Binding {
                            ty: BindTy::Typed(ty),
                            tainted,
                        },
                    );
                }
                let value = step.prop("value").unwrap();
                let tainted = self.check_expr(step, scope, None, value)?;
                let mut ty = self.expr_ty(scope, value);
                if ty == BindTy::Value {
                    let inferred = self.infer_val_ty(scope, None, value);
                    if inferred != ValTy::Unknown {
                        ty = BindTy::Typed(inferred);
                    }
                }
                let name = step.ident_prop("as").unwrap().to_string();
                self.bind(step, scope, &name, Binding { ty, tainted })?;
            }
            "find" => {
                let (res, cap) = step.ref_prop("from").unwrap();
                let Some(resource) = self.program.resources.iter().find(|r| r.name == res) else {
                    return Err(self.err(step, format!("unknown resource `{res}`")));
                };
                let contract = resource.contract.clone().unwrap_or_default();
                if cap == crate::resource::NEAREST_QUERY {
                    if resource
                        .find_method(cap)
                        .is_none_or(|m| m.kind != crate::ResourceKind::Query)
                    {
                        return Err(self.err(step, format!("`{res}` does not declare `query nearest;`; add it (with `vector_embedding` on `{contract}`) to rank rows by cosine similarity")));
                    }
                    self.check_nearest(step, scope, res, &contract)?;
                } else {
                    match resource.find_method(cap) {
                        Some(m)
                            if m.kind == crate::ResourceKind::Query
                                && (cap == "list" || cap == "all") => {}
                        _ => {
                            return Err(self.err(step, format!("`{res}.{cap}` is not a list query; declare `query list;` and use `{res}.list`")));
                        }
                    }
                    if step.ref_options("from").is_some() {
                        return Err(self.err(
                            step,
                            format!("`{res}.{cap}` takes no options; write `:from({res}.{cap})`"),
                        ));
                    }
                }
                let one = step.has_flag("one");
                if !one {
                    if step.prop("max").is_none() {
                        return Err(self.err(
                            step,
                            "requires `:max(N)` (or `:one`) so the read is bounded",
                        ));
                    }
                    self.check_max(step, "max", MAX_LOOP_LIST)?;
                } else if step.prop("max").is_some() {
                    return Err(self.err(
                        step,
                        "`:one` already limits the result to one row; remove `:max`",
                    ));
                }
                if let Some(w) = step.prop("where") {
                    self.check_expr(step, scope, Some(&contract), w)?;
                }
                if let Some(o) = step.prop("order") {
                    let field = match o {
                        Expr::Var(f) => Some(f.as_str()),
                        _ => None,
                    };
                    let fields = self.contract_fields(&contract).unwrap_or_default();
                    if !field.is_some_and(|f| fields.iter().any(|(n, _)| *n == f)) {
                        return Err(self.err(
                            step,
                            format!(":order must name a `{contract}` field, e.g. `$.due`"),
                        ));
                    }
                }
                let name = step.ident_prop("as").unwrap().to_string();
                let ty = if one {
                    BindTy::Row(contract)
                } else {
                    BindTy::List(contract)
                };
                self.bind(step, scope, &name, Binding { ty, tainted: false })?;
            }
            "read" => {
                let ty = self.check_read(step, scope)?;
                let name = step.ident_prop("as").unwrap().to_string();
                self.bind(step, scope, &name, Binding { ty, tainted: false })?;
            }
            "fallback" => self.check_fallback(step, scope, in_each)?,
            "ask" => {
                let into = step.ident_prop("into").unwrap().to_string();
                if self.contract_fields(&into).is_none() {
                    return Err(self.err(step, format!("unknown contract `{into}` in :into")));
                }
                if let Some(from) = step.prop("from") {
                    self.check_expr(step, scope, None, from)?;
                }
                self.check_template(step, scope, "prompt")?;
                self.check_retry(step)?;
                if let Some(o) = step.child("otherwise") {
                    if step.children.len() > 1 {
                        return Err(self.err(step, "accepts at most one loop::otherwise"));
                    }
                    self.check_terminating(step, o)?;
                    let mut inner = scope.clone();
                    self.check_block(&o.children, &mut inner, in_each)?;
                }
                let name = step.ident_prop("as").unwrap().to_string();
                self.bind(
                    step,
                    scope,
                    &name,
                    Binding {
                        ty: BindTy::Row(into),
                        tainted: true,
                    },
                )?;
            }
            "gate" => {
                let that = step.prop("that").unwrap();
                self.check_expr(step, scope, None, that)?;
                if step.children.len() > 1 {
                    return Err(self.err(step, "accepts at most one loop::otherwise"));
                }
                if let Some(o) = step.child("otherwise") {
                    self.check_terminating(step, o)?;
                    let mut inner = scope.clone();
                    self.check_block(&o.children, &mut inner, in_each)?;
                }
                Self::clear_taint(scope, Some(that), None);
            }
            "branch" => {
                let whens = step.children.iter().filter(|c| c.name == "when").count();
                let last_is_otherwise = step.children.last().is_some_and(|c| c.name == "otherwise");
                let otherwise_count = step
                    .children
                    .iter()
                    .filter(|c| c.name == "otherwise")
                    .count();
                if whens == 0 || otherwise_count != 1 || !last_is_otherwise {
                    return Err(self.err(
                        step,
                        "needs one or more loop::when arms followed by exactly one loop::otherwise",
                    ));
                }
                for arm in &step.children {
                    self.check_props(arm)?;
                    let mut inner = scope.clone();
                    if let Some(that) = arm.prop("that") {
                        self.check_expr(arm, scope, None, that)?;
                        Self::clear_taint(&mut inner, Some(that), None);
                    }
                    self.check_block(&arm.children, &mut inner, in_each)?;
                }
            }
            "each" => {
                let list = step.prop("in").unwrap();
                let tainted = self.check_expr(step, scope, None, list)?;
                let item_ty = match self.expr_ty(scope, list) {
                    BindTy::List(c) if !c.is_empty() => BindTy::Row(c),
                    BindTy::List(_) => BindTy::Value,
                    BindTy::Items(item) => *item,
                    _ => {
                        return Err(self.err(
                            step,
                            ":in must be a list binding (from loop::find, scrape::site, scrape::select, or a list field)",
                        ));
                    }
                };
                self.check_max(step, "max", MAX_LOOP_LIST)?;
                let mut inner = scope.clone();
                let name = step.ident_prop("as").unwrap().to_string();
                self.bind(
                    step,
                    &mut inner,
                    &name,
                    Binding {
                        ty: item_ty,
                        tainted,
                    },
                )?;
                self.check_block(&step.children, &mut inner, true)?;
            }
            "write" => {
                let (res, cap) = step.ref_prop("to").unwrap();
                let Some(resource) = self.program.resources.iter().find(|r| r.name == res) else {
                    return Err(self.err(step, format!("unknown resource `{res}`")));
                };
                if step.ref_options("to").is_some() {
                    return Err(self.err(
                        step,
                        format!("`{res}.{cap}` takes no options; write `:to({res}.{cap})`"),
                    ));
                }
                match resource.find_method(cap) {
                    Some(m)
                        if m.kind == crate::ResourceKind::Mutation
                            && (cap == "create" || cap == "update") => {}
                    _ => {
                        return Err(self.err(step, format!("`{res}.{cap}` is not a create/update mutation; declare `mutation {cap};`")));
                    }
                }
                if self.trigger_resource.as_deref() == Some(res) {
                    return Err(self.err(step, format!("writes to `{res}`, the resource that triggers this loop; that would retrigger it")));
                }
                let contract = self.resource_contract(res).unwrap_or_default();
                let value = step.prop("value").unwrap();
                let mut tainted = self.check_expr(step, scope, None, value)?;
                match self.expr_ty(scope, value) {
                    BindTy::Row(c) if c == contract => {}
                    _ => {
                        return Err(self.err(
                            step,
                            format!(
                                ":value must be `{contract}.new(...)` or a `{contract}` binding"
                            ),
                        ));
                    }
                }
                if cap == "update" {
                    let has_id = match value {
                        Expr::New { fields, .. } => fields.iter().any(|(f, _)| f == "id"),
                        _ => true,
                    };
                    if !has_id {
                        return Err(self.err(step, "an update must set `:id(...)` on the value"));
                    }
                }
                tainted |= self.check_key(step, scope)?;
                self.check_effect_taint(step, tainted)?;
            }
            "notify" => {
                let (t1, _) = self.check_template(step, scope, "to")?;
                let (t2, _) = self.check_template(step, scope, "text")?;
                let t3 = self.check_key(step, scope)?;
                self.check_effect_taint(step, t1 | t2 | t3)?;
            }
            "approve" => {
                self.check_template(step, scope, "by")?;
                self.check_template(step, scope, "message")?;
                if let Some(show) = step.prop("show") {
                    self.check_expr(step, scope, None, show)?;
                }
                parse_loop_duration(step.string_prop("within").unwrap())
                    .map_err(|e| self.err(step, e))?;
                let declined = step
                    .children
                    .iter()
                    .filter(|c| c.name == "declined")
                    .count();
                let timed_out = step
                    .children
                    .iter()
                    .filter(|c| c.name == "timed_out")
                    .count();
                if declined != 1 || timed_out != 1 {
                    return Err(self.err(
                        step,
                        "needs exactly one loop::declined and one loop::timed_out block",
                    ));
                }
                for arm in &step.children {
                    self.check_props(arm)?;
                    self.check_terminating(step, arm)?;
                    let mut inner = scope.clone();
                    self.check_block(&arm.children, &mut inner, in_each)?;
                }
                Self::clear_taint(scope, step.prop("show"), step.string_prop("message"));
                if let Some(name) = step.ident_prop("as").map(str::to_string) {
                    self.bind(
                        step,
                        scope,
                        &name,
                        Binding {
                            ty: BindTy::Approval,
                            tainted: false,
                        },
                    )?;
                }
            }
            "skip" => {
                if !in_each {
                    return Err(self.err(step, "is only valid inside loop::each"));
                }
                self.check_template(step, scope, "reason")?;
            }
            "stop" | "fail" => {
                self.check_template(step, scope, "reason")?;
            }
            _ => {}
        }
        Ok(())
    }

    fn check_key(&self, step: &LoopNode, scope: &Scope) -> Result<bool, String> {
        let (tainted, placeholders) = self.check_template(step, scope, "key")?;
        if placeholders == 0 {
            return Err(self.err(
                step,
                ":key needs at least one `{$binding}` placeholder so each effect has its own receipt",
            ));
        }
        Ok(tainted)
    }

    fn check_effect_taint(&self, step: &LoopNode, tainted: bool) -> Result<(), String> {
        if tainted && step.prop("unchecked").is_none() {
            return Err(self.err(
                step,
                "uses model output from loop::ask that has not passed a loop::gate or loop::approve; check it first or add :unchecked(\"reason\")",
            ));
        }
        if let Some(reason) = step.string_prop("unchecked") {
            if reason.trim().is_empty() {
                return Err(self.err(step, ":unchecked needs a reason"));
            }
        }
        Ok(())
    }
}

/// Why a nested operation cannot sit in this loop node, or `None` when it may
/// (or when the node's closed `:op` list reports it instead).
fn misplaced_op(node: &str, op: &str) -> Option<String> {
    let in_read = LOOP_READ_OPS.contains(&op);
    let in_let = LOOP_LET_OPS.contains(&op);
    match node {
        "read" if in_read => None,
        "let" if in_let => None,
        _ if in_read => Some(format!(
            "`{op}` is a read; inside a loop it must sit in loop::read, e.g. `loop::read(:as(x), {op}(...))`, so it is bounded and recorded for replay"
        )),
        _ if op == LOOP_SCORE_OP => Some(format!(
            "`{op}` is not a read; bind it with `loop::let(:as(score), {op}(:text($page.text)))`"
        )),
        _ if in_let => Some(format!(
            "`{op}` is not a read; bind it with `loop::let(:as(vec), {op}(:from($page.text)))`"
        )),
        _ if op.starts_with("scrape::") => Some(format!(
            "`{op}` runs in pipelines only; inside a loop, read with scrape::page, scrape::site, scrape::render, or scrape::select in loop::read"
        )),
        "read" => None,
        _ => Some(format!(
            "`{op}` cannot be nested in loop::{node}; loop::read nests {} and loop::let nests {}",
            LOOP_READ_OPS.join(", "),
            LOOP_LET_OPS.join(", ")
        )),
    }
}

/// Validate one loop against the program's contracts and resources.
pub fn validate_loop(program: &Program, lp: &Loop) -> Result<(), String> {
    if lp.root.name != "flow" {
        return Err(format!(
            "loop `{}` root must be `loop::flow`, found `loop::{}`",
            lp.name, lp.root.name
        ));
    }
    let mut checker = Checker {
        program,
        lp,
        names: HashSet::new(),
        trigger_resource: None,
    };
    checker.check_props(&lp.root)?;
    let Some(trigger) = lp.trigger() else {
        return Err(format!(
            "loop `{}`: the first child of loop::flow must be a trigger (loop::schedule, loop::manual, or loop::on_mutation)",
            lp.name
        ));
    };
    checker.check_props(trigger)?;
    let mut scope = Scope::new();
    scope.insert(
        "today".into(),
        Binding {
            ty: BindTy::Value,
            tainted: false,
        },
    );
    scope.insert(
        "now".into(),
        Binding {
            ty: BindTy::Value,
            tainted: false,
        },
    );
    scope.insert(
        "calendar".into(),
        Binding {
            ty: BindTy::Fields(CALENDAR_FIELDS),
            tainted: false,
        },
    );
    match trigger.name.as_str() {
        "schedule" => {
            validate_cron(trigger.string_prop("cron").unwrap())
                .map_err(|e| checker.err(trigger, e))?;
            validate_tz(trigger.string_prop("tz").unwrap()).map_err(|e| checker.err(trigger, e))?;
            if let Some(c) = trigger.string_prop("catch_up") {
                parse_loop_duration(c).map_err(|e| checker.err(trigger, e))?;
            }
            scope.insert(
                "event".into(),
                Binding {
                    ty: BindTy::Fields(SCHEDULE_EVENT_FIELDS),
                    tainted: false,
                },
            );
        }
        "manual" => {
            scope.insert(
                "event".into(),
                Binding {
                    ty: BindTy::Fields(MANUAL_EVENT_FIELDS),
                    tainted: false,
                },
            );
        }
        "on_mutation" => {
            let res = trigger.ident_prop("resource").unwrap().to_string();
            let mutation = trigger.ident_prop("mutation").unwrap();
            let Some(resource) = program.resources.iter().find(|r| r.name == res) else {
                return Err(checker.err(trigger, format!("unknown resource `{res}`")));
            };
            if resource
                .find_method(mutation)
                .is_none_or(|m| m.kind != crate::ResourceKind::Mutation)
            {
                return Err(checker.err(
                    trigger,
                    format!("`{res}` does not declare `mutation {mutation};`"),
                ));
            }
            let contract = resource.contract.clone().unwrap_or_default();
            scope.insert(
                "event".into(),
                Binding {
                    ty: BindTy::Row(contract),
                    tainted: false,
                },
            );
            checker.trigger_resource = Some(res);
        }
        _ => unreachable!(),
    }
    let steps = lp.steps();
    if steps.is_empty() {
        return Err(format!(
            "loop `{}`: loop::flow needs at least one step after the trigger",
            lp.name
        ));
    }
    checker.check_block(steps, &mut scope, false)?;
    Ok(())
}

/// Program-level loop rules: names, coexistence, and per-loop validation.
pub fn validate_loops(program: &Program) -> Result<(), String> {
    if program.loops.is_empty() {
        return Ok(());
    }
    if !program.games.is_empty() {
        return Err(
            "cannot mix `loop` and `game` in one program; game programs are WebGPU-only".into(),
        );
    }
    for lp in &program.loops {
        if lp.name.starts_with("Loop") {
            return Err(format!(
                "loop name `{}` is reserved; names starting with `Loop` belong to the synthesized inbox",
                lp.name
            ));
        }
        validate_loop(program, lp)?;
    }
    if loops_are_command(program) {
        for lp in &program.loops {
            if lp.contains_node("approve") {
                return Err(format!(
                    "loop `{}` uses loop::approve, but this program is a command (manual loops, no `app`) so nobody can approve it; add an `app` with a route, or replace the approval with loop::gate",
                    lp.name
                ));
            }
        }
    }
    Ok(())
}

/// A command program: every loop is `loop::manual` and nothing declares a
/// surface. `silc main.silc` runs each loop once, prints notices, and exits
/// instead of serving the `/loops` inbox.
pub fn loops_are_command(program: &Program) -> bool {
    !program.loops.is_empty()
        && program.apps.is_empty()
        && program.games.is_empty()
        && program
            .loops
            .iter()
            .all(|l| l.trigger().is_some_and(|t| t.name == "manual"))
}

/// Human-readable trigger summary, e.g. `schedule "0 8 * * 1-5" America/New_York`.
pub fn describe_loop_trigger(lp: &Loop) -> String {
    let Some(t) = lp.trigger() else {
        return "no trigger".into();
    };
    match t.name.as_str() {
        "schedule" => {
            let mut s = format!(
                "schedule \"{}\" {}",
                t.string_prop("cron").unwrap_or_default(),
                t.string_prop("tz").unwrap_or_default()
            );
            if let Some(c) = t.string_prop("catch_up") {
                s.push_str(&format!(", catch up {c}"));
            }
            s
        }
        "manual" => "manual".into(),
        "on_mutation" => format!(
            "on {} of {}",
            t.ident_prop("mutation").unwrap_or_default(),
            t.ident_prop("resource").unwrap_or_default()
        ),
        other => other.to_string(),
    }
}

/// The `silc build` cost report: worst case per run for every loop.
pub fn format_loop_cost_report(program: &Program) -> Option<String> {
    if program.loops.is_empty() {
        return None;
    }
    let width = program
        .loops
        .iter()
        .map(|l| l.name.len())
        .max()
        .unwrap_or(0)
        .max(8);
    let mut out = String::from("loops (worst case per run, from static bounds):\n");
    for lp in &program.loops {
        let b = loop_bounds(lp);
        out.push_str(&format!(
            "  {:<width$}  {}\n",
            lp.name,
            describe_loop_trigger(lp)
        ));
        out.push_str(&format!(
            "  {:<width$}  model calls {} · effects {} (writes {}, notices {}) · approvals {} · reads {} · scores {} · rows scanned {}\n",
            "",
            b.model_calls,
            b.effects(),
            b.writes,
            b.notices,
            b.approvals,
            b.reads,
            b.scores,
            b.rows_scanned,
        ));
        if b.embeds > 0 {
            let mut models: Vec<&str> = loop_embed_steps(lp)
                .into_iter()
                .filter_map(|s| loop_embed_model(s).ok().map(|m| m.id))
                .collect();
            models.sort_unstable();
            models.dedup();
            out.push_str(&format!(
                "  {:<width$}  embeds {} ({}, CPython embed worker; not model calls)\n",
                "",
                b.embeds,
                models.join(", "),
            ));
        }
        let mut unchecked = Vec::new();
        collect_unchecked(&lp.root, &mut unchecked);
        for reason in unchecked {
            out.push_str(&format!("  {:<width$}  unchecked effect: {reason}\n", ""));
        }
    }
    Some(out)
}

fn collect_unchecked(node: &LoopNode, out: &mut Vec<String>) {
    if let Some(reason) = node.string_prop("unchecked") {
        out.push(format!("loop::{} — {reason}", node.name));
    }
    for c in &node.children {
        collect_unchecked(c, out);
    }
}

pub fn loops_use_ask(program: &Program) -> bool {
    program.loops.iter().any(|l| l.contains_node("ask"))
}

/// Any loop has a `loop::read` of this operation, including fallback attempts.
pub fn loops_use_read_op(program: &Program, op: &str) -> bool {
    fn walk(node: &LoopNode, op: &str) -> bool {
        (node.name == "read" && node.string_prop("op") == Some(op))
            || node.children.iter().any(|c| walk(c, op))
    }
    program.loops.iter().any(|l| walk(&l.root, op))
}

/// Comparison operators the kernel evaluates with three-valued logic.
pub fn loop_binop_symbol(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_flow_root_and_triggers() {
        assert_eq!(lookup_loop_node("flow").unwrap().role, LoopNodeRole::Root);
        for t in LOOP_TRIGGERS {
            assert_eq!(lookup_loop_node(t).unwrap().role, LoopNodeRole::Trigger);
        }
        for s in LOOP_STEPS {
            assert_eq!(lookup_loop_node(s).unwrap().role, LoopNodeRole::Step, "{s}");
        }
    }

    #[test]
    fn every_loop_node_and_prop_has_description() {
        for node in LOOP_NODE_CATALOG {
            assert!(
                node.description.len() > 40,
                "loop::{} description too short",
                node.name
            );
            for prop in node.props {
                assert!(
                    prop.description.len() > 20,
                    "loop::{} :{} description too short",
                    node.name,
                    prop.name
                );
            }
        }
    }

    #[test]
    fn durations_parse_and_bound() {
        assert_eq!(parse_loop_duration("30m").unwrap(), 30);
        assert_eq!(parse_loop_duration("4h").unwrap(), 240);
        assert_eq!(parse_loop_duration("2d").unwrap(), 2880);
        assert!(parse_loop_duration("31d").is_err());
        assert!(parse_loop_duration("0d").is_err());
        assert!(parse_loop_duration("3 days").is_err());
    }

    #[test]
    fn cron_validation() {
        validate_cron("0 8 * * 1-5").unwrap();
        validate_cron("*/15 * * * *").unwrap();
        validate_cron("0,30 9-17 1 1-12/2 0").unwrap();
        assert!(validate_cron("0 8 * *").is_err());
        assert!(validate_cron("60 8 * * *").is_err());
        assert!(validate_cron("0 8 * * MON").is_err());
    }

    #[test]
    fn template_placeholders_parse() {
        let p = template_placeholders("{$rfi.id}:remind:{$today}").unwrap();
        assert_eq!(
            p,
            vec![vec!["rfi".to_string(), "id".into()], vec!["today".into()]]
        );
        assert!(template_placeholders("{$rfi.id").is_err());
        assert!(template_placeholders("{1 + 2}").is_err());
    }

    #[test]
    fn catalog_md_lists_every_node() {
        let md = format_loop_catalog_md();
        for name in catalog_loop_node_names() {
            assert!(md.contains(&format!("`loop::{name}`")), "{name}");
        }
    }
}
