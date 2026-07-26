# ADR-002: Raku-Inspired Silc Surface Syntax

- **Status:** Accepted
- **Date:** 2026-07-25

## Context

Silc is an AI-native language: models emit it and humans supervise it. The
surface needs semantic density, predictable parsing, and useful editor support.
Raku provides mature declaration forms—subsets, traits, signatures, adverbials,
and feed operators—without requiring Silc to implement the Raku language.

## Decision

Silc is its own language with a **Raku-inspired authoring surface**.

- `.silc` is the primary extension.
- `.raku` is accepted when the program conforms to Silc's supported grammar.
- `.sil` is not supported.
- `silc` defines semantics; Rakudo is not a Silc runtime.

### Surface mapping

| Surface form | Silc subject |
| --- | --- |
| `subset` / `where`, `class` / `has` | Contract |
| `class … is service|processor|sink` | Module |
| `class … is component` | Component (props, state, slots, emit, render) |
| `class … is resource` | Resource (query / mutation) |
| `class … is app` | App (routes + serve) |
| traits, units, colon-pair adverbials | Constraint |
| `==>` feeds | Pipeline |
| `ui::…` / author components in `render()` | UiTemplate / UiNode |
| inferred Go / Python / Bun assignment | Target |

`is view` was removed in 0.2.0.

`@domain` is not part of the grammar. Routing derives from module kinds, hard
constraints, and operation namespaces.

## Selected Raku ideas

- `subset` + `where` for semantic types
- `class` + `has` for schemas
- `is` traits for module kinds and constraints
- `method` signatures with unit literals
- colon-pair options such as `:batch(64)` and `:prefer<CUDA>`
- `==>` for dataflow

Silc does not adopt full Raku OO, junctions, hyperoperators, topicalizers,
runtime `EVAL`, or multiple synonymous forms for the same intent.

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

## Consequences

- Raku highlighters provide a useful editing baseline.
- The lexer/parser stay deliberately smaller than Rakudo.
- A syntactically valid Raku program can still be outside the Silc grammar.
- Diagnostics must say when a Raku-looking construct is unsupported.
