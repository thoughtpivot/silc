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

## Consequences

- Raku highlighters provide a useful editing baseline.
- The lexer/parser stay deliberately smaller than Rakudo.
- A syntactically valid Raku program can still be outside the Silc grammar.
- Diagnostics must say when a Raku-looking construct is unsupported.
