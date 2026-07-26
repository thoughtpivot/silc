# ADR-004: Runtime Engine Strength Catalog

- **Status:** Accepted
- **Date:** 2026-07-25
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-003](ADR-003-declarative-ui.md), [ARCHITECTURE.md](ARCHITECTURE.md)

## Context

Silc routes modules to **Bun** (TypeScript), **CPython**, or **Go**. Authors and
agents never choose engines. Routing must stay explainable: every `Target`
decision cites why that engine is the right tool for the job.

Personal preference is irrelevant. Silc picks engines from complementary
strengths—the same principle that blesses React for `ui::web` (ecosystem
default) rather than exposing a framework picker.

## Decision

### Core principle

**Authors express intent; Silc chooses implementation.** Routing provenance,
codegen substrates, and supervisor roles all follow the catalogs below.

### Bun (executes TypeScript)

1. Native TypeScript execution (no separate transpile step for workers)
2. First-class async I/O / HTTP / WebSocket servers
3. Same runtime for UI ingress *and* browser bundling
4. Fast cold start for many short-lived service workers
5. Web-native JSON / fetch / Buffer ergonomics for edge/protocol code

**Typical assignment:** `service` modules; namespaces `http`, `html`, `ws`,
`ui`; static `scrape::page` / `scrape::select` (ADR-006 adapter
`bun-fetch-v1`).

### CPython

1. Unmatched scientific / ML / numeric ecosystem (numpy, pandas, tensors)
2. Best-in-class text / NLP / scoring library surface
3. Rapid domain glue for analysis pipelines
4. Mature `mmap` / buffer protocols for shared-memory workers
5. Replica-friendly CPU-bound work (Silc spawns many Python scorers)
6. First-class browser automation for JS-heavy scrape targets (Playwright)

**Typical assignment:** `processor` modules; namespaces `tensor`, `numpy`,
`pandas`, `text`, `llm`; `:prefer<CUDA>`. `llm::complete` uses a
compiler-pinned llama.cpp binding and local GGUF catalog (ADR-005).
`scrape::render`, `scrape::extract`, and `:js(true|auto)` escalation use
Playwright (ADR-006 adapter `python-playwright-v1`).

### Go

1. Low-latency systems and storage paths
2. Cheap concurrency for sinks and stream consumers
3. Rock-solid stdlib networking / UDS / file IPC
4. Simple durable storage (SQLite) with one binary artifact
5. Predictable performance under tight `latency(...)` constraints
6. High-concurrency site crawls (`scrape::site` via Colly)

**Typical assignment:** `sink` modules with `storage(SQLite)` or
`latency ≤ 10ms`; namespaces `store`, `ipc`, `sys`; `scrape::site` crawl
workers (ADR-006 adapter `go-colly-v1`).

### Single-file north star

One `.silc` file can declare a full app—Contract, service UI, processor, sink,
ports, storage. Polyglot workers under `.runtime/` are compiler *output*, not
an authoring model. Authors never scaffold per-language projects or write
`package.json`.

### Provenance

`sil-router` records Tier 1 / Tier 2 decisions with provenance strings that
cite these strengths (for example,
`tier1: sink+SQLite → Go (durable low-latency storage)`). Manifests and agent
logs teach the same story as this ADR.

## Non-goals

- User- or agent-selectable engine overrides
- Bond, Node, or other alternate TypeScript hosts as primary engines
- Changing Tier 1 / Tier 2 rules solely to match marketing language (rules
  already align with this catalog)

## Consequences

### Positive

- Routing is auditable and teachable.
- Strength catalogs justify why Silc owns three engines instead of one VM.
- Future Tier 3 (ONNX) can extend provenance without inventing a new philosophy.

### Costs and risks

- Provenance strings and docs must stay in sync when routing policy changes.
- Strength catalogs are guidance for policy, not a substitute for tests.
