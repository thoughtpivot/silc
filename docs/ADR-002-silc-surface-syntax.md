# ADR-002: Raku-Inspired Silc Surface Syntax

- **Status:** Accepted
- **Date:** 2026-07-25
- **Updated:** 2026-07-27
- **Related:** [ADR-007](ADR-007-pipeline-feeds.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [intent-vs-subjects.md](intent-vs-subjects.md),
  [subject-first-decision.md](subject-first-decision.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)
- **Superseded by (partial):** [ADR-009](ADR-009-compiler-synthesized-runtime.md)
  for author `serve()` / `sink` / IPC-store resource pipeline examples from
  0.2.0–0.3.0.

## Context

Silc is an AI-native language: models emit it and humans supervise it. The
surface needs semantic density, predictable parsing, and useful editor support.
Raku provides mature declaration forms—subsets, traits, signatures, adverbials,
and feed operators—that Silc borrows as **surface vocabulary** without
implementing the Raku language or Rakudo runtime.

## Decision

**Silc is an independent intent language with a Raku-inspired surface.**

- `.silc` is the only accepted source extension.
- `.raku` and `.sil` are not supported (rename to `.silc`).
- `silc` defines semantics; Rakudo is not a Silc runtime.
- Silc is **not** a Raku subset, dialect, or roast-compatible profile. Shared
  glyphs do not imply shared meaning.

### Surface mapping

| Surface form | Compiler concept (internal) |
| --- | --- |
| `subset` / `where`, `contract` / `has` | Contract |
| `service …` / `processor …` / `task …` | Module |
| `component …` | Component (props, state, slots, emit, render) |
| `resource … for …` | Resource (capability query / mutation; optional `seed`) |
| `app …` | App (routes; dual-surface serving synthesized) |
| traits, units, colon-pair adverbials | Constraint |
| `==>` feeds | Pipeline (see [ADR-007](ADR-007-pipeline-feeds.md)) |
| `ui::…` catalog nodes in `render()` | UiTemplate / UiNode (template vocabulary) |
| inferred Go / Python / Bun assignment | Target |

### Direct declarations (policy)

`is view` was removed in 0.2.0. Legacy `class … is …` declarators were removed
in 0.3.0 in favor of direct declaration keywords (`contract`, `component`,
`resource`, `app`, `service`, `processor`, `task`). That product choice is an
**owner override** of the July 2026 familiarity benchmark no-go; measured
evidence remains in
[subject-first-decision.md](subject-first-decision.md) (historical appendix).

From 0.4.0, author-facing docs prefer **direct declarations** /
**declaration-based surface**. The word *subject* names the compiler-internal
ownership model — see [intent-vs-subjects.md](intent-vs-subjects.md).

Author `sink`, `method serve()`, and IPC/store/resource pipelines were removed
in 0.4.0 — see [ADR-009](ADR-009-compiler-synthesized-runtime.md).

`@domain` is not part of the grammar. Routing derives from module kinds, hard
constraints, and operation namespaces. Source must declare exact
`@version("0.4.0")` matching the compiler package version.

## Selected Raku ideas (retained)

Silc keeps these compact, model-friendly forms:

- `subset` + closed `where` predicates for semantic types
- `contract` + `has` for schemas
- `is` traits for module constraints such as `storage(SQLite)` (compiler use;
  authors do not declare sinks)
- `method` signatures with unit literals
- colon-pair options such as `:batch(64)` and `:prefer(CPU)`
- `$name` / `$.field` sigils where they distinguish bindings
- `==>` for dataflow (Silc Pipeline IR — not Rakudo call-threading)
- `when` / `for` in UI templates
- namespace-qualified operations (`ns::op`)

## Non-goals (do not inherit from Raku)

- Full Raku OO (roles, mixins, inheritance as OO, BUILD, …)
- Junctions, hyperoperators, topicalizers
- Custom operators and slangs
- Runtime metaprogramming / `EVAL`
- Multiple synonymous forms for the same intent
- Multiple dispatch
- Dynamic mixins
- Roast / Rakudo compatibility

## Subset `where` predicates (v1)

`subset Name of Base where { … }` uses a **closed** predicate language. Unsupported
bodies are compile errors.

v1 (base must resolve to `Str`):

| Predicate | Meaning |
| --- | --- |
| `.contains("lit")` | value contains the literal |
| `.starts-with("lit")` | value starts with the literal |
| `.ends-with("lit")` | value ends with the literal |

Predicates are enforced at compile time for known string literals and at
resource/API ingress for subset-typed `Str` fields.

## Component / resource / app surface (0.4.0)

### Components

```silc
component ItemCard {
    has Item $.item;                 # prop
    has state Str $.draft = "";      # local state
    slot actions;                    # named slot (optional)
    emit remove(Item);               # emitted event
    query $.items = Inventory.list(); # resource query binding

    method render() {
        ui::card(
            ui::heading(:text($.item.name)),
            ui::button(:label("Delete"), :on(click(on_remove)))
        )
    }

    method on_remove() { emit remove($.item); }
}
```

Template / handler control flow:

- Assignment: `$.field = expr;`
- Conditionals: `when expr { … } else { … }`
- Iteration: `for expr -> $item { … }`
- Navigation: `navigate("/path")`
- Async: `await expr`
- Event wiring: `:on(click(handler))`, `:on(submit(handler))`,
  `:on(select(handler))` on `ui::table` (row payload),
  `:on(remove => on_delete)` (forward author events)

### Resources

```silc
resource InventoryItems for InventoryItem {
    query list;
    mutation create;
}

resource Articles for Article {
    query list;
    mutation create;
    mutation update;
    mutation delete;
    seed Article.new(:id("article-001"), :title("Hello"));
}
```

Capability declarations expand to conventional CRUD signatures. Derived HTTP:
`GET/POST /api/{table}`, `GET/PUT/DELETE /api/{table}/:id` (table from the
resource name). Optional `seed Contract.new(...)` rows are compiler-owned
idempotent inserts (`INSERT OR IGNORE`) and require a stable `:id("…")`.

### Apps

```silc
app MyApp {
    route "/" => HomePage;
    route "/admin" => AdminPage;
}
```

UI apps synthesize both web and terminal surfaces ([ADR-009](ADR-009-compiler-synthesized-runtime.md)).
Override ports with `SILC_HTTP_PORT` / `SILC_TERMINAL_PORT`. Optional workflow
modules use `service` / `processor` / `task` with `==>` pipelines; persistence
is synthesized for processor programs.

### Historical (0.3.0)

Pre-0.4.0 programs authored `method serve() { ui::web … ==> ui::terminal … }`,
`sink` modules, and `resource::*` / `ipc::*` / `store::*` pipelines. Those
forms are rejected by the 0.4.0 parser; see ADR-009.

The exhaustive agent-facing contract (types, ops, 38-primitive UI catalog) lives
in [`crates/silc/templates/AGENTS.md`](../crates/silc/templates/AGENTS.md) and is
mirrored in the root [README](../README.md).

Pipeline feed semantics: [ADR-007](ADR-007-pipeline-feeds.md).

## Consequences

- Temporary editor association may map `*.silc` to a Raku highlighter for
  baseline coloring; that is ergonomics only, not language compatibility.
  A native `.silc` grammar is the intended long-term tooling path.
- The lexer/parser stay deliberately smaller than Rakudo.
- A syntactically valid Raku program is almost always outside the Silc grammar.
- Diagnostics must say when a Raku-looking construct is unsupported.
- Direct declarations are the product surface despite the historical benchmark
  no-go; model training should use the preserved evidence appendix.
