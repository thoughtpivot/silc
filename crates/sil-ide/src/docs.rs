//! Documentation catalog for Silc keywords, operators, types, and operations.

use sil_core::{is_executable_op, EXECUTABLE_OPS, UI_COMPONENT_CATALOG};

pub fn keyword_doc(keyword: &str) -> Option<&'static str> {
    Some(match keyword {
        "subset" => {
            "Declares a semantic subtype of a base type, optionally constrained with `where`. \
             Use subsets when you want a named refinement (for example Email of Str) that the \
             type checker and UI bindings can treat as distinct from the base."
        }
        "class" => {
            "Legacy/alias declaration form retained for older samples. Prefer `contract` for \
             data schemas so fields, resources, and synthesized HTTP stay aligned."
        }
        "contract" => {
            "Declares a data schema with typed `has` fields. Contracts are the source of truth \
             for resource rows, form bindings, and the shape of values that flow through queries."
        }
        "component" => {
            "Declares a UI component with props, state, queries, handlers, and a `render` \
             template. Components are the unit of screen composition on both web and terminal."
        }
        "resource" => {
            "Declares a persistent resource bound to a contract (`resource Name for Contract`). \
             Query and mutation methods on the resource become storage operations and, when \
             served, matching HTTP routes."
        }
        "app" => {
            "Declares an application with routes that map URL paths to components. An app is \
             the entry that the runtime serves for both web and terminal surfaces."
        }
        "game" => {
            "Declares a WebGPU-only game scene (`game Name { game::scene(...) }`). Distinct from \
             dual-surface `app` routes; the compiler synthesizes a Babylon/Vite runtime from the \
             closed `game::*` catalog (ADR-012)."
        }
        "service" => {
            "Declares a service module, commonly used with `service::http` to expose an API \
             surface. Keep service bodies focused on transport concerns rather than UI."
        }
        "processor" => {
            "Declares a processor module for pipelines such as scrape, llm, or tensor steps. \
             Processors own transformational work that sits between ingress and persistence."
        }
        "sink" => {
            "Declares a sink module for persistence side-effects. Prefer resource mutations for \
             ordinary CRUD; sinks remain for specialized write paths outside the resource model."
        }
        "task" => {
            "Declares a task module for scheduled or on-demand background work. Use tasks when \
             the work is not driven by a UI handler or HTTP request."
        }
        "has" => {
            "Declares a typed field on a contract, module, or component (`has Type $.name`). \
             On components, pair with `state` when the field is mutable local state rather than \
             an incoming prop."
        }
        "method" => {
            "Declares a method body — a handler, pipeline step, or `render` template. Method \
             names are the stable identifiers parents and routes bind to."
        }
        "is" => {
            "Attaches a trait or modifier to a declaration (`is storage(SQLite)`). Modifiers \
             configure runtime behavior without changing the declaration's primary kind."
        }
        "of" => {
            "Connects a subset to its base type in `subset Name of Base`. The subset inherits \
             the base representation and may further constrain it with `where`."
        }
        "where" => {
            "Constrains a subset with a closed predicate. Only `.contains`, `.starts-with`, and \
             `.ends-with` are legal today, and the constraint is checked at type boundaries \
             rather than as an open expression language."
        }
        "query" => {
            "Binds component state to a resource query (`query $.name = Resource.method();`) \
             or declares a resource query capability (`query list;`). Query bindings are \
             read-only and re-run when their dependencies invalidate."
        }
        "mutation" => {
            "Declares a resource mutation capability (`mutation create;`). Mutations persist \
             changes and typically synthesize matching write HTTP routes when the app is served."
        }
        "seed" => {
            "Declares an idempotent seed row (`seed Contract.new(:field(value), …);`). Seeds \
             run once so local demos and tests start with known data without duplicating rows."
        }
        "slot" => {
            "Declares a named content slot on a component. Parents fill slots to inject chrome \
             such as app bars or action rows without hard-coding child structure."
        }
        "emit" => {
            "Declares a component event that can be raised to parents. Pair declarations with \
             `emit` calls in handlers so parent templates can bind `:on(event(handler))`."
        }
        "state" => {
            "Marks a component field as mutable local state (`has state Type $.name`). Unlike \
             props and query bindings, state is owned by the component and updated from handlers."
        }
        "when" => {
            "Conditional template branch (`when condition { … } else { … }`). Only the matching \
             branch is rendered, on both web and terminal surfaces."
        }
        "for" => {
            "Template loop (`for $.items -> $item { … }`). Each element of the collection is \
             bound to the loop variable for one rendering of the body."
        }
        "else" => {
            "Alternate branch of a `when` template. Rendered only when the `when` condition is \
             false; omit it when the false path should produce nothing."
        }
        "route" => {
            "Maps a URL path to a component inside an `app` (`route \"/\" => HomePage;`). \
             Routes are how the runtime decides which component tree to mount for a request."
        }
        "await" => {
            "Awaits an asynchronous expression inside a handler. Use it for resource mutations, \
             LLM calls, and other ops that must finish before subsequent statements run."
        }
        _ => return None,
    })
}

pub fn operator_doc(op: &str) -> Option<&'static str> {
    Some(match op {
        "==>" => {
            "Pipeline feed operator. Threads the left-hand value into the next declarative \
             step so each stage receives the previous result. The surface looks Raku-inspired, \
             but the semantics are Silc's own pipeline model."
        }
        "=>" => {
            "Fat arrow used in routes (`route \"/\" => Page;`) and some event bindings. It \
             associates a left-hand selector with the target component or handler on the right."
        }
        "->" => {
            "Maps a `for` collection into a loop item (`for $.items -> $item`). The identifier \
             on the right is bound once per element while the body renders."
        }
        "::" => {
            "Namespace qualifier for builtins and ops (`ui::table`, `llm::complete`, \
             `scrape::page`). The left side selects the catalog; the right side is the member."
        }
        "&&" => {
            "Logical AND over boolean expressions. Evaluation follows ordinary short-circuit \
             rules and is common in `when` conditions and handler guards."
        }
        "||" => {
            "Logical OR over boolean expressions. Use it to combine alternate success paths in \
             conditions without nested `when` templates."
        }
        "==" => {
            "Equality comparison between two values. Prefer it in `when` conditions and \
             filters when you need an exact match rather than a subset predicate."
        }
        "!=" => {
            "Inequality comparison between two values. Useful for excluding a sentinel or \
             ensuring a field has changed before running a mutation."
        }
        "<=" => {
            "Less-than-or-equal comparison for ordered values. Typical in numeric guards and \
             pagination-style bounds checks inside handlers."
        }
        ">=" => {
            "Greater-than-or-equal comparison for ordered values. Pair it with `<` / `<=` when \
             expressing inclusive ranges."
        }
        "<" => {
            "Less-than comparison for ordered values. In some type forms it also opens a type \
             argument list, so spacing matters around expressions."
        }
        ">" => {
            "Greater-than comparison for ordered values. Use it in conditions; it is not the \
             pipeline feed operator (`==>`)."
        }
        "+" => {
            "Addition over numeric operands. Keep operands on a shared numeric type so the \
             expression type-checks cleanly."
        }
        "-" => {
            "Subtraction between numerics, or unary negation before a number. In templates, \
             prefer explicit parentheses when mixing unary and binary uses."
        }
        "*" => {
            "Multiplication over numeric operands. Common in scoring and simple derived fields \
             computed inside handlers."
        }
        "/" => {
            "Division over numeric operands. Watch for integer truncation when both sides are \
             integral types rather than `num32` / `num64`."
        }
        "!" => {
            "Logical NOT over a boolean expression. Reach for it in `when` conditions when the \
             negative branch is the one you want to render."
        }
        "." => {
            "Member access into resources, fields, and methods (`Articles.list`, \
             `$article.title`). The left side supplies the receiver; the right side names the \
             member."
        }
        "$" => {
            "Sigil for variables, state, and props (`$article`, `$.title`). A bare `$name` is \
             a local or parameter; `$.name` refers to component-owned state, props, or queries."
        }
        ":" => {
            "Colon-pair / named argument introducer (`:label(\"Save\")`, `:sortable`). In UI \
             trees it marks props and flags; in constructors it names fields."
        }
        "=" => {
            "Assignment or binding. Used for query bindings, state updates, and ordinary \
             initializer forms — the surrounding construct decides mutability."
        }
        _ => return None,
    })
}

pub fn builtin_type_doc(name: &str) -> Option<&'static str> {
    Some(match name {
        "Str" => {
            "Unicode string primitive. Default textual type for labels, bodies, and most \
             form fields unless a tighter subset (such as Email) applies."
        }
        "UUID" => {
            "Universally unique identifier. Prefer it as the default identity type for \
             contracts and resource rows so synthesized HTTP routes can address records stably."
        }
        "Bool" => {
            "Boolean true/false value. Used for flags, checkbox/switch state, and `when` \
             conditions that branch template output."
        }
        "Int" => {
            "Integer numeric type without an explicit bit width. Prefer `int32` / `int64` when \
             storage width or wire format matters."
        }
        "int32" => {
            "32-bit signed integer. Choose it for counters and compact numeric fields that do \
             not need 64-bit range."
        }
        "int64" => {
            "64-bit signed integer. Use it for large counters, timestamps-as-ints, or values \
             that may exceed 32-bit range in storage."
        }
        "num32" => {
            "32-bit floating numeric type. Common for embedding vectors and other ML tensors \
             where memory density matters more than extra precision."
        }
        "num64" => {
            "64-bit floating numeric type. Prefer it for general-purpose fractional values \
             when embedding-style density is not required."
        }
        _ => return None,
    })
}

/// Documentation for a left-hand namespace qualifier (`ui` in `ui::table`).
pub fn namespace_doc(ns: &str) -> Option<String> {
    let text: String = match ns {
        "ui" => format!(
            "Dual-surface UI primitive catalog ({count} builtins). \
             Author `ui::*` nodes inside component `render` templates; they compile to both \
             web (React/Tailwind) and terminal (OpenTUI). Do not author `ui::web` or \
             `ui::terminal` as operations — those surfaces are synthesized from `app` routes.",
            count = UI_COMPONENT_CATALOG.len()
        ),
        "game" => format!(
            "WebGPU game scene catalog ({count} nodes). Author `game::scene` and nested \
             `game::*` nodes inside a `game` declaration; the compiler lowers them to a Babylon \
             runtime manifest. Game programs are web-only (no terminal surface).",
            count = sil_core::GAME_NODE_CATALOG.len()
        ),
        "service" => {
            "Runnable service namespace. Author `service::http` in a service module to expose \
             an HTTP API surface; the compiler wires routes from resources and handlers."
                .into()
        }
        "text" => {
            "Runnable text namespace. Author `text::score` in processor or handler pipelines \
             for local, deterministic relevance scoring without calling an LLM."
                .into()
        }
        "llm" => {
            "Runnable local-LLM namespace. Author `llm::complete` (via silclm) in processors \
             or awaited handlers when you need generated language grounded on app data."
                .into()
        }
        "scrape" => {
            "Runnable scrape namespace. Author `scrape::page`, `site`, `select`, `render`, and \
             `extract` in ingress/processor pipelines to fetch and structure web content."
                .into()
        }
        "doc" => {
            "Runnable document namespace. Author `doc::extract(:into(Contract))` with \
             `ui::file_input` so uploads become structured resource rows (ADR-011)."
                .into()
        }
        "tensor" => {
            "Runnable tensor namespace. Author `tensor::tokenize` then `tensor::infer` for the \
             CPU MiniLM embedding path (exactly 384 `num32` values in Silc 0.4.0)."
                .into()
        }
        "ipc" => {
            "Compiler-owned IPC namespace. Cross-engine shared-buffer traffic is synthesized; \
             do not author `ipc::*` calls in `.silc` sources."
                .into()
        }
        "store" => {
            "Compiler-owned persistence namespace. SQLite wiring (`store::sqlite`, \
             `store::commit`) is synthesized from resources; do not author these ops yourself."
                .into()
        }
        "resource" => {
            "Compiler-owned resource-op namespace. Prefer declaration-style \
             `resource Name for Contract` with `query` / `mutation` capabilities; do not \
             author `resource::list` / `get` / `create` / … as pipeline ops."
                .into()
        }
        "http" => {
            "Stub-only HTTP namespace in Silc 0.4.0. It parses and routes but does not execute; \
             prefer `scrape::*` for fetches and `service::http` for API surfaces."
                .into()
        }
        "html" => {
            "Stub-only HTML namespace in Silc 0.4.0. Prefer `scrape::select` / `scrape::extract` \
             for structured extraction from fetched pages."
                .into()
        }
        "numpy" | "pandas" => format!(
            "Stub-only `{ns}` namespace in Silc 0.4.0. It parses and routes but does not \
             execute; keep numerical / tabular work in typed contracts and runnable ops \
             such as `text::score` or `tensor::*`."
        ),
        "ws" => {
            "Stub-only WebSocket namespace in Silc 0.4.0. It is recognized by the classifier \
             but is not an author-runnable executable op today."
                .into()
        }
        "sys" => {
            "Stub-only system namespace in Silc 0.4.0. Recognized for routing, but not \
             executable; keep side effects in resources, services, and processors."
                .into()
        }
        "schema" => {
            "Stub-only schema namespace in Silc 0.4.0. Prefer `contract` / `subset` \
             declarations for typed shapes rather than `schema::*` pipeline ops."
                .into()
        }
        "payload" => {
            "Stub-only payload namespace in Silc 0.4.0. Cross-engine payloads move through \
             synthesized IPC; do not author `payload::*` calls."
                .into()
        }
        "json" => {
            "Stub-only JSON namespace in Silc 0.4.0. It parses and routes but does not \
             execute; prefer typed contracts and resource/HTTP surfaces for structured data."
                .into()
        }
        _ => return None,
    };
    Some(text)
}

pub fn executable_op_doc(namespace: &str, name: &str) -> Option<String> {
    if !EXECUTABLE_OPS
        .iter()
        .any(|(ns, n)| *ns == namespace && *n == name)
        && !is_executable_op(namespace, name)
    {
        return None;
    }
    let summary = match (namespace, name) {
        ("service", "http") => {
            "Exposes an HTTP API surface for the program. Legal in a service module; the \
             compiler synthesizes routes from resources and handlers rather than requiring \
             hand-written REST boilerplate."
        }
        ("text", "score") => {
            "Scores text locally with a runnable text operation. Use it in processor or \
             handler pipelines when you need a deterministic relevance signal without calling \
             an LLM."
        }
        ("llm", "complete") => {
            "Local LLM completion via silclm. Place it in a processor or awaited handler step; \
             it is heavier than `text::score` and appropriate when you need generated language."
        }
        ("scrape", "page") => {
            "Fetches and optionally renders a single web page. Typical first step in a scrape \
             pipeline before `select` / `extract`; network-bound and not free."
        }
        ("scrape", "site") => {
            "Crawls a site within configured depth and host constraints. More expensive than \
             `scrape::page`; keep depth tight to avoid runaway fetches."
        }
        ("scrape", "select") => {
            "Selects nodes from scraped HTML. Runs after a page/site fetch and feeds structured \
             extract steps without requiring hand-written DOM code."
        }
        ("scrape", "render") => {
            "Renders dynamic page content before scraping. Use it when static HTML is \
             insufficient; rendering is slower than a plain fetch."
        }
        ("scrape", "extract") => {
            "Extracts structured fields into a contract from selected nodes. Bridges raw HTML \
             into typed Silc values that resources and UI can consume."
        }
        ("tensor", "tokenize") => {
            "Tokenizes input for an embedding or inference model. Pair it with `tensor::infer` \
             in a processor pipeline; tokenization alone does not produce embeddings."
        }
        ("tensor", "infer") => {
            "Runs CPU tensor inference such as embeddings. Legal in processor pipelines after \
             tokenization; expect non-trivial CPU cost on larger batches."
        }
        _ => {
            "Runnable Silc 0.4.0 operation. Legal in the module or pipeline contexts documented \
             for its namespace; prefer the executable set over stub-only ops."
        }
    };
    Some(format!("Runnable operation `{namespace}::{name}`.\n\n{summary}"))
}

pub fn stub_op_doc(namespace: &str, name: &str) -> String {
    format!(
        "Namespace operation `{namespace}::{name}`.\n\n\
         This symbol is recognized but is not an author-runnable executable op in Silc 0.4.0. \
         Prefer scrape::*, doc::extract, llm::complete, tensor::*, text::score, or service::http inside \
         processor, service, or awaited handler pipelines."
    )
}

pub fn resource_method_summary(kind: &str, method: &str) -> &'static str {
    match (kind, method) {
        ("query", "list" | "all") => {
            "Lists all rows for the resource contract from persistent storage. When the app \
             is served, this typically maps to a GET collection endpoint over the resource table."
        }
        ("query", "get") => {
            "Fetches a single row by identity from persistent storage. When served, this \
             typically maps to GET on the resource's `/:id` route."
        }
        ("mutation", "create" | "add") => {
            "Creates a new row for the resource contract and persists it. When served, this \
             typically maps to POST on the resource collection endpoint."
        }
        ("mutation", "update") => {
            "Updates an existing row in persistent storage. When served, this typically maps \
             to PUT on the resource's `/:id` route."
        }
        ("mutation", "delete" | "remove") => {
            "Deletes a row from persistent storage. When served, this typically maps to DELETE \
             on the resource's `/:id` route."
        }
        ("query", _) => {
            "Resource query capability that reads from persistent storage without mutating rows. \
             Call it from component query bindings or awaited handlers."
        }
        ("mutation", _) => {
            "Resource mutation capability that persists changes. Call it from handlers; the \
             runtime invalidates related queries after a successful write."
        }
        _ => {
            "Resource method on a persistent store. Queries read rows; mutations write them and \
             may synthesize matching HTTP routes when the app is served."
        }
    }
}

/// Every lexer keyword that should have a hover entry (for conformance tests).
pub const KEYWORD_NAMES: &[&str] = &[
    "subset", "class", "contract", "component", "resource", "app", "game", "service", "processor",
    "sink", "task", "has", "method", "is", "of", "where", "query", "mutation", "seed", "slot",
    "emit", "state", "when", "for", "else", "route", "await",
];

pub const BUILTIN_TYPE_NAMES: &[&str] =
    &["Str", "UUID", "num32", "num64", "int32", "int64", "Bool", "Int"];
