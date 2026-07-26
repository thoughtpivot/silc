# ADR-002: Raku-Inspired Silc Surface Syntax

- **Status:** Accepted
- **Date:** 2026-07-25
- **Updated:** 2026-07-26

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

| Surface form | Silc subject |
| --- | --- |
| `subset` / `where`, `class` / `has` | Contract |
| `class … is service|processor|sink` | Module |
| `class … is component` | Component (props, state, slots, emit, render) |
| `class … is resource` | Resource (query / mutation) |
| `class … is app` | App (routes + serve) |
| traits, units, colon-pair adverbials | Constraint |
| `==>` feeds | Pipeline (see [ADR-007](ADR-007-pipeline-feeds.md)) |
| `ui::…` / author components in `render()` | UiTemplate / UiNode |
| inferred Go / Python / Bun assignment | Target |

`is view` was removed in 0.2.0.

`@domain` is not part of the grammar. Routing derives from module kinds, hard
constraints, and operation namespaces.

## Selected Raku ideas (retained)

Silc keeps these compact, model-friendly forms:

- `subset` + closed `where` predicates for semantic types
- `class` + `has` for schemas
- `is` traits for module kinds and constraints
- `method` signatures with unit literals
- colon-pair options such as `:batch(64)` and `:prefer<CUDA>`
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

## Component / resource / app surface (0.2.0)

### Components

```silc
class ItemCard is component {
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
  `:on(remove => on_delete)` (forward author events)

### Resources

```silc
class Inventory is resource {
    has Str $.table = "inventory_items";
    query list() -> [InventoryItem] {
        InventoryItem ==> resource::list(:table(inventory_items))
    }
    mutation create(InventoryItem $item) {
        $item ==> resource::create(:table(inventory_items))
    }
}
```

CRUD ops: `resource::list|get|create|update|delete`. Derived HTTP:
`GET/POST /api/{table}`, `GET/PUT/DELETE /api/{table}/:id`.

### Apps

```silc
class MyApp is app {
    route "/" => HomePage;
    route "/admin" => AdminPage;

    method serve() {
        ui::web(:root(MyApp), :port(18080), :route("/"))
            ==> ui::terminal(:port(18023))
    }
}
```

UI apps require both surfaces with distinct ports. Optional backend modules use
`is service` / `is processor` / `is sink` with `==>` pipelines.

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
