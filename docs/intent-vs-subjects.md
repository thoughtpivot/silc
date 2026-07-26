# Intent language vs subject architecture

Silc has two complementary vocabularies. Conflating them made the product
identity unclear in 0.3.x.

## Author surface (product identity)

**.silc source is intent-oriented and declaration-based.**

Authors declare:

- domain data (`contract`, `subset`)
- interaction (`component`)
- durable collections (`resource Name for Contract` with capabilities)
- entry routes (`app` with `route`s)
- workflows (`service`, `processor`, `task`, `==>` feeds)

Authors do **not** declare runtime substrates: React/OpenTUI, Bun/CPython/Go,
mmap/UDS, SQLite wiring, dual-surface `serve()` pipelines, or IPC/store
staging. Those are synthesized by the compiler.

## Compiler interior (implementation architecture)

**Subjects are an internal `sil-core` ownership model**, not the language’s
product identity. A subject is a durable semantic concept with shared types and
invariants (Contract, Component, Resource, App, Module, Pipeline, Target, …).
Boundary crates (`sil-parser`, `sil-router`, `sil-codegen`, `sil-ipc`) translate
across edges; they do not redefine subject meaning.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the subject/boundary map.

## Historical note

The phrase **subject-first** named the 0.3.0 migration from `class … is …` to
direct declarators (`component X`, `resource X`, …). That migration evidence
remains in [subject-first-decision.md](subject-first-decision.md). From 0.4.0
onward, prefer **direct declarations** / **declaration-based surface** in
author-facing docs, and reserve **subject** for compiler architecture.
