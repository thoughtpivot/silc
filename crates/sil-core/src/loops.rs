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
pub const LOOP_READ_OPS: &[&str] = &["scrape::page", "mcp::call"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopPropKind {
    /// Literal string.
    String,
    /// Literal string with `{$name.field}` placeholders filled at run time.
    Template,
    /// Bare identifier (binding name, contract, resource, closed token).
    Ident,
    /// Non-negative integer literal.
    Number,
    /// Bare flag (`:desc`).
    Flag,
    /// Expression over bindings (`$rfi.due < $today`).
    Expr,
    /// `Resource.capability` reference.
    Ref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopPropSpec {
    pub name: &'static str,
    pub kind: LoopPropKind,
    pub required: bool,
    pub description: &'static str,
    pub closed_values: &'static [&'static str],
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopChildPolicy {
    None,
    AnyOf(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopNodeRole {
    Root,
    Trigger,
    Step,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopNodeSpec {
    pub name: &'static str,
    pub role: LoopNodeRole,
    pub description: &'static str,
    pub props: &'static [LoopPropSpec],
    pub children: LoopChildPolicy,
}

pub const LOOP_TRIGGERS: &[&str] = &["schedule", "manual", "on_mutation"];

pub const LOOP_STEPS: &[&str] = &[
    "let", "find", "read", "ask", "gate", "branch", "each", "write", "notify", "approve", "stop",
    "fail", "skip",
];

const FLOW_CHILDREN: &[&str] = &[
    "schedule", "manual", "on_mutation", "let", "find", "read", "ask", "gate", "branch", "each",
    "write", "notify", "approve", "stop", "fail", "skip",
];

const TERMINATORS: &[&str] = &["stop", "fail", "skip"];

pub const LOOP_NODE_CATALOG: &[LoopNodeSpec] = &[
    LoopNodeSpec {
        name: "flow",
        role: LoopNodeRole::Root,
        description: "Root of a loop. The first child is exactly one trigger; the remaining children are steps that run in order.",
        props: &[],
        children: LoopChildPolicy::AnyOf(FLOW_CHILDREN),
    },
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
    },
    LoopNodeSpec {
        name: "manual",
        role: LoopNodeRole::Trigger,
        description: "Starts a run when a person asks for one. With an `app` (or any scheduled loop) that is Run now in the synthesized /loops inbox; when every loop is manual and there is no `app`, the program is a command: `silc main.silc` runs each loop once, prints its notices to stdout, and exits.",
        props: &[],
        children: LoopChildPolicy::None,
    },
    LoopNodeSpec {
        name: "on_mutation",
        role: LoopNodeRole::Trigger,
        description: "Starts one run per created or updated row of a declared resource. The row is available as `$event`.",
        props: &[
            lp("resource", LoopPropKind::Ident, true, "Resource whose rows start runs, e.g. `Rfis`."),
            lp_closed("mutation", LoopPropKind::Ident, true, "Which change starts a run: `create` or `update`.", &["create", "update"]),
        ],
        children: LoopChildPolicy::None,
    },
    LoopNodeSpec {
        name: "let",
        role: LoopNodeRole::Step,
        description: "Binds a name to a computed value for the rest of the enclosing block. Deterministic; no I/O.",
        props: &[
            lp("as", LoopPropKind::Ident, true, "Binding name, used later as `$name`."),
            lp("value", LoopPropKind::Expr, true, "Expression over earlier bindings, literals, or `Contract.new(...)`."),
        ],
        children: LoopChildPolicy::None,
    },
    LoopNodeSpec {
        name: "find",
        role: LoopNodeRole::Step,
        description: "Reads rows from a declared resource with a filter, order, and a hard row limit. Recorded so replays see the same rows.",
        props: &[
            lp("as", LoopPropKind::Ident, true, "Binding name for the rows (a list) or the single row with `:one`."),
            lp("from", LoopPropKind::Ref, true, "Resource query to read, e.g. `Rfis.list`."),
            lp("where", LoopPropKind::Expr, false, "Row filter; contract fields are `$.field`. Unknown values do not match (fail closed)."),
            lp("order", LoopPropKind::Expr, false, "Row field to sort by, e.g. `$.due`."),
            lp("desc", LoopPropKind::Flag, false, "Sort descending instead of ascending."),
            lp("max", LoopPropKind::Number, false, "Maximum rows returned (required unless `:one`; at most 10000)."),
            lp("one", LoopPropKind::Flag, false, "Return the first matching row instead of a list; empty when nothing matches."),
        ],
        children: LoopChildPolicy::None,
    },
    LoopNodeSpec {
        name: "read",
        role: LoopNodeRole::Step,
        description: "Fetches outside data with a bounded retry and records it for replay. `scrape::page` is an HTTP GET (title and text); `mcp::call` calls one tool on an MCP server over streamable HTTP.",
        props: &[
            lp("as", LoopPropKind::Ident, true, "Binding name: a page (`url`, `status`, `title`, `text`) or a tool result (`text`, `data`)."),
            lp_closed("op", LoopPropKind::String, true, "Read operation: \"scrape::page\" or \"mcp::call\".", LOOP_READ_OPS),
            lp("url", LoopPropKind::Expr, false, "`scrape::page`: absolute http(s) URL, literal or from a binding."),
            lp("server", LoopPropKind::String, false, "`mcp::call`: MCP server URL, e.g. \"https://example.run.app/mcp\"."),
            lp("tool", LoopPropKind::String, false, "`mcp::call`: tool name on that server."),
            lp("args", LoopPropKind::Expr, false, "`mcp::call`: tool arguments as `Contract.new(...)`; field names are sent as written."),
            lp("auth_env", LoopPropKind::String, false, "`mcp::call`: environment variable holding the bearer token. Tokens never go in source."),
            lp("select", LoopPropKind::String, false, "`mcp::call`: dotted path into the result kept as `data`, e.g. \"records.parsed\" (lists map over items)."),
            lp("retry", LoopPropKind::Number, false, "Retries after a failed fetch (default 2, max 10)."),
        ],
        children: LoopChildPolicy::None,
    },
    LoopNodeSpec {
        name: "ask",
        role: LoopNodeRole::Step,
        description: "Asks the local silclm model for one value of a contract. The answer is checked against the contract and recorded; it must pass a gate or approval before any effect uses it.",
        props: &[
            lp("as", LoopPropKind::Ident, true, "Binding name for the typed answer."),
            lp("into", LoopPropKind::Ident, true, "Contract the answer must match, e.g. `Reminder`."),
            lp("prompt", LoopPropKind::Template, true, "Instruction for the model; may use `{$name.field}` placeholders."),
            lp("from", LoopPropKind::Expr, false, "Context value given to the model, e.g. `$rfi`."),
            lp("retry", LoopPropKind::Number, false, "Retries when the answer does not match the contract (default 1, max 10)."),
        ],
        children: LoopChildPolicy::AnyOf(&["otherwise"]),
    },
    LoopNodeSpec {
        name: "gate",
        role: LoopNodeRole::Step,
        description: "Continues only when the condition is true. False or unknown runs `loop::otherwise` (which must stop, fail, or skip) or fails the run.",
        props: &[
            lp("that", LoopPropKind::Expr, true, "Condition; missing fields make it unknown, and unknown blocks."),
            lp("reason", LoopPropKind::String, true, "Why the gate exists; recorded when it blocks."),
        ],
        children: LoopChildPolicy::AnyOf(&["otherwise"]),
    },
    LoopNodeSpec {
        name: "branch",
        role: LoopNodeRole::Step,
        description: "Runs the first `loop::when` whose condition is true, else the required `loop::otherwise`. Unknown conditions are not true.",
        props: &[],
        children: LoopChildPolicy::AnyOf(&["when", "otherwise"]),
    },
    LoopNodeSpec {
        name: "when",
        role: LoopNodeRole::Block,
        description: "One arm of a `loop::branch`. Its steps run when the condition is true.",
        props: &[lp("that", LoopPropKind::Expr, true, "Condition for this arm; unknown counts as false.")],
        children: LoopChildPolicy::AnyOf(LOOP_STEPS),
    },
    LoopNodeSpec {
        name: "otherwise",
        role: LoopNodeRole::Block,
        description: "Fallback block for `branch`, `gate`, or `ask`. Under gate and ask it must end in stop, fail, or skip.",
        props: &[],
        children: LoopChildPolicy::AnyOf(LOOP_STEPS),
    },
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
    },
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
    },
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
    },
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
    },
    LoopNodeSpec {
        name: "declined",
        role: LoopNodeRole::Block,
        description: "Runs when the approver declines; must end in stop, fail, or skip.",
        props: &[],
        children: LoopChildPolicy::AnyOf(LOOP_STEPS),
    },
    LoopNodeSpec {
        name: "timed_out",
        role: LoopNodeRole::Block,
        description: "Runs when nobody decides before `:within`; must end in stop, fail, or skip.",
        props: &[],
        children: LoopChildPolicy::AnyOf(LOOP_STEPS),
    },
    LoopNodeSpec {
        name: "stop",
        role: LoopNodeRole::Step,
        description: "Ends the run successfully. Inside `each` it ends the whole run, not just the item.",
        props: &[lp("reason", LoopPropKind::Template, false, "Why the run stopped; recorded on the run.")],
        children: LoopChildPolicy::None,
    },
    LoopNodeSpec {
        name: "fail",
        role: LoopNodeRole::Step,
        description: "Fails the current item inside `each`, or the run outside it, with a recorded reason.",
        props: &[lp("reason", LoopPropKind::Template, true, "Failure reason shown in the runs table.")],
        children: LoopChildPolicy::None,
    },
    LoopNodeSpec {
        name: "skip",
        role: LoopNodeRole::Step,
        description: "Skips the rest of the current `each` item; only valid inside `each`.",
        props: &[lp("reason", LoopPropKind::Template, true, "Why the item was skipped; recorded in the event log.")],
        children: LoopChildPolicy::None,
    },
];

pub fn lookup_loop_node(name: &str) -> Option<&'static LoopNodeSpec> {
    LOOP_NODE_CATALOG.iter().find(|n| n.name == name)
}

pub fn catalog_loop_node_names() -> Vec<&'static str> {
    LOOP_NODE_CATALOG.iter().map(|n| n.name).collect()
}

pub fn loop_prop_doc(node: &str, prop: &str) -> Option<&'static str> {
    lookup_loop_node(node)
        .and_then(|spec| spec.props.iter().find(|p| p.name == prop).map(|p| p.description))
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
    };
    format!(
        "- `loop::{}` — props: {}; children: {}",
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

    /// `Resource.capability` for `:from` / `:to`.
    pub fn ref_prop(&self, name: &str) -> Option<(&str, &str)> {
        match self.prop(name) {
            Some(Expr::Member { base, field }) => match base.as_ref() {
                Expr::Ident(b) => Some((b.as_str(), field.as_str())),
                _ => None,
            },
            _ => None,
        }
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

/// Parse `30m`, `4h`, `2d`, `1w` into minutes.
pub fn parse_loop_duration(text: &str) -> Result<u64, String> {
    let text = text.trim();
    let (num, unit) = text.split_at(text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len()));
    let n: u64 = num
        .parse()
        .map_err(|_| format!("invalid duration \"{text}\"; use forms like \"30m\", \"4h\", \"2d\""))?;
    let minutes = match unit {
        "m" => n,
        "h" => n * 60,
        "d" => n * 60 * 24,
        "w" => n * 60 * 24 * 7,
        _ => {
            return Err(format!(
                "invalid duration \"{text}\"; units are m, h, d, w"
            ))
        }
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
    let ranges = [(0, 59, "minute"), (0, 23, "hour"), (1, 31, "day-of-month"), (1, 12, "month"), (0, 7, "day-of-week")];
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
                return Err(format!(
                    "cron \"{cron}\": {label} must be within {lo}-{hi}"
                ));
            }
        }
    }
    Ok(())
}

fn validate_tz(tz: &str) -> Result<(), String> {
    let ok = tz == "UTC"
        || (tz.contains('/')
            && tz
                .split('/')
                .all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || "_+-".contains(c))));
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
                p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
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
    pub rows_scanned: u64,
    pub unchecked_effects: u64,
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
        self.rows_scanned = self.rows_scanned.saturating_add(other.rows_scanned);
        self.unchecked_effects = self.unchecked_effects.saturating_add(other.unchecked_effects);
    }

    fn max(&mut self, other: &LoopBounds) {
        self.model_calls = self.model_calls.max(other.model_calls);
        self.writes = self.writes.max(other.writes);
        self.notices = self.notices.max(other.notices);
        self.approvals = self.approvals.max(other.approvals);
        self.reads = self.reads.max(other.reads);
        self.rows_scanned = self.rows_scanned.max(other.rows_scanned);
        self.unchecked_effects = self.unchecked_effects.max(other.unchecked_effects);
    }

    fn scale(&self, n: u64) -> LoopBounds {
        LoopBounds {
            model_calls: self.model_calls.saturating_mul(n),
            writes: self.writes.saturating_mul(n),
            notices: self.notices.saturating_mul(n),
            approvals: self.approvals.saturating_mul(n),
            reads: self.reads.saturating_mul(n),
            rows_scanned: self.rows_scanned.saturating_mul(n),
            unchecked_effects: self.unchecked_effects.saturating_mul(n),
        }
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
            "find" => {
                b.rows_scanned = if step.has_flag("one") {
                    1
                } else {
                    step.number_prop("max").unwrap_or(0)
                };
            }
            "read" => b.reads = 1 + step.number_prop("retry").unwrap_or(DEFAULT_READ_RETRY),
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

#[derive(Debug, Clone, PartialEq)]
enum BindTy {
    Value,
    Row(String),
    List(String),
    Approval,
    Page,
    Fields(&'static [&'static str]),
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
/// Date window fields on the reserved `$calendar` binding, in the loop's time zone.
pub const CALENDAR_FIELDS: &[&str] = &["today", "weekday", "last7_start", "last7_end", "next7_end"];
const RESERVED_BINDINGS: &[&str] = &["today", "now", "event", "calendar"];
pub const SCHEDULE_EVENT_FIELDS: &[&str] = &["scheduled_for", "fired_at", "late"];
pub const MANUAL_EVENT_FIELDS: &[&str] = &["requested_by", "requested_at"];

impl<'a> Checker<'a> {
    fn err(&self, node: &LoopNode, msg: impl AsRef<str>) -> String {
        format!("loop `{}`: loop::{}: {}", self.lp.name, node.name, msg.as_ref())
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

    fn bind(&mut self, node: &LoopNode, scope: &mut Scope, name: &str, b: Binding) -> Result<(), String> {
        if RESERVED_BINDINGS.contains(&name) {
            return Err(self.err(node, format!("`{name}` is reserved; choose another `:as` name")));
        }
        if !self.names.insert(name.to_string()) {
            return Err(self.err(node, format!("binding `{name}` is already declared in this loop")));
        }
        scope.insert(name.to_string(), b);
        Ok(())
    }

    /// Check a path `root.field...` resolves; returns its taint.
    fn check_path(&self, node: &LoopNode, scope: &Scope, row: Option<&str>, path: &[String]) -> Result<bool, String> {
        let root = path[0].as_str();
        if let Some(contract) = row {
            if let Some(fields) = self.contract_fields(contract) {
                if fields.iter().any(|(f, _)| *f == root) {
                    if scope.contains_key(root) {
                        return Err(self.err(node, format!("`${root}` is both a row field and a binding; rename the binding")));
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
                BindTy::Value => true,
            };
            if !ok {
                return Err(self.err(node, format!("`${root}` has no field `{field}`")));
            }
        }
        Ok(binding.tainted)
    }

    /// Validate a value expression; returns whether it carries model output.
    fn check_expr(&self, node: &LoopNode, scope: &Scope, row: Option<&str>, expr: &Expr) -> Result<bool, String> {
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
                for (fname, value) in fields {
                    if !cfields.iter().any(|(f, _)| f == fname) {
                        return Err(self.err(node, format!("contract `{ty}` has no field `{fname}`")));
                    }
                    t |= self.check_expr(node, scope, row, value)?;
                }
                Ok(t)
            }
            Expr::Call { .. } => Err(self.err(node, "function and method calls are not allowed in loop expressions")),
            _ => Err(self.err(node, "this expression form is not allowed in a loop")),
        }
    }

    fn check_template(&self, node: &LoopNode, scope: &Scope, prop: &str) -> Result<(bool, usize), String> {
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
        }
        Ok((tainted, placeholders.len()))
    }

    fn expr_ty(&self, scope: &Scope, expr: &Expr) -> BindTy {
        match expr {
            Expr::Var(name) => scope.get(name).map(|b| b.ty.clone()).unwrap_or(BindTy::Value),
            Expr::New { ty, .. } => BindTy::Row(ty.clone()),
            Expr::Member { base, field } => {
                if let Expr::Var(root) = base.as_ref() {
                    if let Some(BindTy::Row(c)) = scope.get(root).map(|b| &b.ty) {
                        if let Some(fields) = self.contract_fields(c) {
                            if let Some((_, TypeExpr::Array(inner))) = fields.iter().find(|(f, _)| f == field) {
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
        let spec = lookup_loop_node(&node.name).ok_or_else(|| {
            format!(
                "loop `{}`: unknown node `loop::{}`; known: {}",
                self.lp.name,
                node.name,
                catalog_loop_node_names().join(", ")
            )
        })?;
        for prop in spec.props.iter().filter(|p| p.required) {
            if node.prop(prop.name).is_none() {
                return Err(self.err(node, format!("requires prop `:{}`", prop.name)));
            }
        }
        let mut seen = HashSet::new();
        for (pname, value) in &node.props {
            let Some(pspec) = spec.props.iter().find(|p| p.name == *pname) else {
                return Err(self.err(node, format!("unknown prop `:{pname}`")));
            };
            if !seen.insert(pname.as_str()) {
                return Err(self.err(node, format!("prop `:{pname}` appears twice")));
            }
            let ok = match pspec.kind {
                LoopPropKind::String | LoopPropKind::Template => matches!(value, Expr::String(_)),
                LoopPropKind::Ident => matches!(value, Expr::Ident(_)),
                LoopPropKind::Number => matches!(value, Expr::Number(n) if n.parse::<u64>().is_ok()),
                LoopPropKind::Flag => matches!(value, Expr::Bool(true)),
                LoopPropKind::Ref => node.ref_prop(pname).is_some(),
                LoopPropKind::Expr => true,
            };
            if !ok {
                let want = match pspec.kind {
                    LoopPropKind::String | LoopPropKind::Template => "a string",
                    LoopPropKind::Ident => "a bare name",
                    LoopPropKind::Number => "a whole number",
                    LoopPropKind::Flag => "a bare flag",
                    LoopPropKind::Ref => "`Resource.capability`",
                    LoopPropKind::Expr => "an expression",
                };
                return Err(self.err(node, format!(":{pname} must be {want}")));
            }
            if !pspec.closed_values.is_empty() {
                let v = value.as_ident().unwrap_or_default();
                if !pspec.closed_values.contains(&v) {
                    return Err(self.err(node, format!(":{pname} must be one of {}", pspec.closed_values.join("|"))));
                }
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
                            format!("cannot contain loop::{}; allowed: {}", child.name, allowed.join(", ")),
                        ));
                    }
                }
            }
            LoopChildPolicy::None => {}
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

    fn check_block(&mut self, steps: &[LoopNode], scope: &mut Scope, in_each: bool) -> Result<(), String> {
        if steps.is_empty() {
            return Err(format!("loop `{}`: a block must contain at least one step", self.lp.name));
        }
        for (i, step) in steps.iter().enumerate() {
            if LOOP_TRIGGERS.contains(&step.name.as_str()) {
                return Err(self.err(step, "a trigger must be the first child of loop::flow, and only one is allowed"));
            }
            if !LOOP_STEPS.contains(&step.name.as_str()) {
                return Err(self.err(step, "is not a step here"));
            }
            if i + 1 < steps.len() && Self::block_terminates(&steps[..=i]) {
                return Err(self.err(&steps[i + 1], "is unreachable; the previous step always ends the block"));
            }
            self.check_step(step, scope, in_each)?;
        }
        Ok(())
    }

    fn check_step(&mut self, step: &LoopNode, scope: &mut Scope, in_each: bool) -> Result<(), String> {
        self.check_props(step)?;
        match step.name.as_str() {
            "let" => {
                let value = step.prop("value").unwrap();
                let tainted = self.check_expr(step, scope, None, value)?;
                let ty = self.expr_ty(scope, value);
                let name = step.ident_prop("as").unwrap().to_string();
                self.bind(step, scope, &name, Binding { ty, tainted })?;
            }
            "find" => {
                let (res, cap) = step.ref_prop("from").unwrap();
                let Some(resource) = self.program.resources.iter().find(|r| r.name == res) else {
                    return Err(self.err(step, format!("unknown resource `{res}`")));
                };
                match resource.find_method(cap) {
                    Some(m) if m.kind == crate::ResourceKind::Query && (cap == "list" || cap == "all") => {}
                    _ => {
                        return Err(self.err(step, format!("`{res}.{cap}` is not a list query; declare `query list;` and use `{res}.list`")));
                    }
                }
                let contract = resource.contract.clone().unwrap_or_default();
                let one = step.has_flag("one");
                if !one {
                    if step.prop("max").is_none() {
                        return Err(self.err(step, "requires `:max(N)` (or `:one`) so the read is bounded"));
                    }
                    self.check_max(step, "max", MAX_LOOP_LIST)?;
                } else if step.prop("max").is_some() {
                    return Err(self.err(step, "`:one` already limits the result to one row; remove `:max`"));
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
                        return Err(self.err(step, format!(":order must name a `{contract}` field, e.g. `$.due`")));
                    }
                }
                let name = step.ident_prop("as").unwrap().to_string();
                let ty = if one { BindTy::Row(contract) } else { BindTy::List(contract) };
                self.bind(step, scope, &name, Binding { ty, tainted: false })?;
            }
            "read" => {
                let op = step.string_prop("op").unwrap_or_default();
                let mcp_props = ["server", "tool", "args", "auth_env", "select"];
                let ty = if op == "mcp::call" {
                    if step.prop("url").is_some() {
                        return Err(self.err(step, "`mcp::call` takes :server and :tool, not :url"));
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
                            && env.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                            && !env.starts_with(|c: char| c.is_ascii_digit());
                        if !valid {
                            return Err(self.err(step, ":auth_env must be an environment variable name like API_TOKEN"));
                        }
                    }
                    if let Some(sel) = step.string_prop("select") {
                        let valid = sel.split('.').all(|seg| {
                            !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        });
                        if !valid {
                            return Err(self.err(step, ":select must be a dotted field path like \"records.parsed\""));
                        }
                    }
                    BindTy::Fields(CALL_FIELDS)
                } else {
                    if let Some(extra) = mcp_props.iter().find(|p| step.prop(p).is_some()) {
                        return Err(self.err(step, format!(":{extra} only applies to `mcp::call`")));
                    }
                    let Some(url) = step.prop("url") else {
                        return Err(self.err(step, "`scrape::page` requires :url(...)"));
                    };
                    self.check_expr(step, scope, None, url)?;
                    if let Expr::String(u) = url {
                        if !(u.starts_with("http://") || u.starts_with("https://")) {
                            return Err(self.err(step, ":url must be an absolute http(s) URL"));
                        }
                    }
                    BindTy::Page
                };
                self.check_retry(step)?;
                let name = step.ident_prop("as").unwrap().to_string();
                self.bind(step, scope, &name, Binding { ty, tainted: false })?;
            }
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
                self.bind(step, scope, &name, Binding { ty: BindTy::Row(into), tainted: true })?;
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
                let otherwise_count = step.children.iter().filter(|c| c.name == "otherwise").count();
                if whens == 0 || otherwise_count != 1 || !last_is_otherwise {
                    return Err(self.err(step, "needs one or more loop::when arms followed by exactly one loop::otherwise"));
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
                    _ => {
                        return Err(self.err(step, ":in must be a list binding (from loop::find or a list field)"));
                    }
                };
                self.check_max(step, "max", MAX_LOOP_LIST)?;
                let mut inner = scope.clone();
                let name = step.ident_prop("as").unwrap().to_string();
                self.bind(step, &mut inner, &name, Binding { ty: item_ty, tainted })?;
                self.check_block(&step.children, &mut inner, true)?;
            }
            "write" => {
                let (res, cap) = step.ref_prop("to").unwrap();
                let Some(resource) = self.program.resources.iter().find(|r| r.name == res) else {
                    return Err(self.err(step, format!("unknown resource `{res}`")));
                };
                match resource.find_method(cap) {
                    Some(m) if m.kind == crate::ResourceKind::Mutation && (cap == "create" || cap == "update") => {}
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
                        return Err(self.err(step, format!(":value must be `{contract}.new(...)` or a `{contract}` binding")));
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
                parse_loop_duration(step.string_prop("within").unwrap()).map_err(|e| self.err(step, e))?;
                let declined = step.children.iter().filter(|c| c.name == "declined").count();
                let timed_out = step.children.iter().filter(|c| c.name == "timed_out").count();
                if declined != 1 || timed_out != 1 {
                    return Err(self.err(step, "needs exactly one loop::declined and one loop::timed_out block"));
                }
                for arm in &step.children {
                    self.check_props(arm)?;
                    self.check_terminating(step, arm)?;
                    let mut inner = scope.clone();
                    self.check_block(&arm.children, &mut inner, in_each)?;
                }
                Self::clear_taint(scope, step.prop("show"), step.string_prop("message"));
                if let Some(name) = step.ident_prop("as").map(str::to_string) {
                    self.bind(step, scope, &name, Binding { ty: BindTy::Approval, tainted: false })?;
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
    scope.insert("today".into(), Binding { ty: BindTy::Value, tainted: false });
    scope.insert("now".into(), Binding { ty: BindTy::Value, tainted: false });
    scope.insert("calendar".into(), Binding { ty: BindTy::Fields(CALENDAR_FIELDS), tainted: false });
    match trigger.name.as_str() {
        "schedule" => {
            validate_cron(trigger.string_prop("cron").unwrap()).map_err(|e| checker.err(trigger, e))?;
            validate_tz(trigger.string_prop("tz").unwrap()).map_err(|e| checker.err(trigger, e))?;
            if let Some(c) = trigger.string_prop("catch_up") {
                parse_loop_duration(c).map_err(|e| checker.err(trigger, e))?;
            }
            scope.insert("event".into(), Binding { ty: BindTy::Fields(SCHEDULE_EVENT_FIELDS), tainted: false });
        }
        "manual" => {
            scope.insert("event".into(), Binding { ty: BindTy::Fields(MANUAL_EVENT_FIELDS), tainted: false });
        }
        "on_mutation" => {
            let res = trigger.ident_prop("resource").unwrap().to_string();
            let mutation = trigger.ident_prop("mutation").unwrap();
            let Some(resource) = program.resources.iter().find(|r| r.name == res) else {
                return Err(checker.err(trigger, format!("unknown resource `{res}`")));
            };
            if resource.find_method(mutation).is_none_or(|m| m.kind != crate::ResourceKind::Mutation) {
                return Err(checker.err(trigger, format!("`{res}` does not declare `mutation {mutation};`")));
            }
            let contract = resource.contract.clone().unwrap_or_default();
            scope.insert("event".into(), Binding { ty: BindTy::Row(contract), tainted: false });
            checker.trigger_resource = Some(res);
        }
        _ => unreachable!(),
    }
    let steps = lp.steps();
    if steps.is_empty() {
        return Err(format!("loop `{}`: loop::flow needs at least one step after the trigger", lp.name));
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
        return Err("cannot mix `loop` and `game` in one program; game programs are WebGPU-only".into());
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
    let width = program.loops.iter().map(|l| l.name.len()).max().unwrap_or(0).max(8);
    let mut out = String::from("loops (worst case per run, from static bounds):\n");
    for lp in &program.loops {
        let b = loop_bounds(lp);
        out.push_str(&format!("  {:<width$}  {}\n", lp.name, describe_loop_trigger(lp)));
        out.push_str(&format!(
            "  {:<width$}  model calls {} · effects {} (writes {}, notices {}) · approvals {} · reads {} · rows scanned {}\n",
            "",
            b.model_calls,
            b.effects(),
            b.writes,
            b.notices,
            b.approvals,
            b.reads,
            b.rows_scanned,
        ));
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
            assert!(node.description.len() > 40, "loop::{} description too short", node.name);
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
        assert_eq!(p, vec![vec!["rfi".to_string(), "id".into()], vec!["today".into()]]);
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
