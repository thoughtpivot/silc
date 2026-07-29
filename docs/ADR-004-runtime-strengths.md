# ADR-004: Runtime Engine Strength Catalog

- **Status:** Accepted
- **Date:** 2026-07-25
- **Updated:** 2026-07-27
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-003](ADR-003-declarative-ui.md),
  [ADR-005](ADR-005-local-llm-complete.md),
  [ADR-006](ADR-006-scrape-namespace.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ADR-010](ADR-010-tensor-minilm-pipeline.md),
  [ADR-012](ADR-012-webgpu-game-subject.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)

## Context

Silc routes modules to **Bun** (TypeScript), **CPython**, or **Go**. Authors and
agents never choose engines. Routing must stay explainable: every `Target`
decision cites why that engine is the right tool for the job.

Personal preference is irrelevant. Silc picks engines from complementary
strengths—the same principle that blesses React for the web surface (ecosystem
default) rather than exposing a framework picker.

## Decision

### Core principle

**Authors express intent; Silc chooses implementation.** Routing provenance,
codegen substrates, and supervisor roles all follow the catalogs below.
Persistence sinks and dual-surface UI workers are compiler-synthesized
([ADR-009](ADR-009-compiler-synthesized-runtime.md)).

### Bun (executes TypeScript)

1. Native TypeScript execution (no separate transpile step for workers)
2. First-class async I/O / HTTP / WebSocket servers
3. Same runtime for UI ingress *and* browser bundling
4. Fast cold start for many short-lived service workers
5. Web-native JSON / fetch / Buffer ergonomics for edge/protocol code

**Typical assignment:** `service` modules; namespaces `http`, `html`, `ws`,
`ui`; static `scrape::page` / `scrape::select` (ADR-006 adapter
`bun-fetch-v1`); pipeline ingress helper for tensor programs (ADR-010);
`game` WebGPU host, Vite bundle, and HTTP for settings/saves/telemetry
([ADR-012](ADR-012-webgpu-game-subject.md)).

### CPython

1. Unmatched scientific / ML / numeric ecosystem (numpy, pandas, tensors)
2. Best-in-class text / NLP / scoring library surface
3. Rapid domain glue for analysis pipelines
4. Mature `mmap` / buffer protocols for shared-memory workers
5. Replica-friendly CPU-bound work (Silc spawns many Python scorers)
6. First-class browser automation for JS-heavy scrape targets (Playwright)

**Typical assignment:** `processor` modules; namespaces `tensor`, `numpy`,
`pandas`, `text`, `llm`. `llm::complete` uses a compiler-pinned llama.cpp
binding and local GGUF catalog (ADR-005). `tensor::tokenize` / `tensor::infer`
use CPU-only ONNX MiniLM ([ADR-010](ADR-010-tensor-minilm-pipeline.md));
`:prefer(CUDA)` is rejected for the 0.4.0 tensor path.
`scrape::render`, `scrape::extract`, and `:js(true|auto)` escalation use
Playwright (ADR-006 adapter `python-playwright-v1`). Compile-time `game`
asset bake (height/noise/mask buffers) for WebGPU scenes (ADR-012).

### Go

1. Low-latency systems and storage paths
2. Cheap concurrency for synthesized sinks and stream consumers
3. Rock-solid stdlib networking / UDS / file IPC
4. Simple durable storage (SQLite) with one binary artifact
5. Predictable performance under tight `latency(...)` constraints
6. High-concurrency site crawls (`scrape::site` via Colly)

**Typical assignment:** compiler-synthesized SQLite persistence
(`storage(SQLite)`); namespaces `store`, `ipc`, `sys` as runtime-owned stages;
`scrape::site` crawl workers (ADR-006 adapter `go-colly-v1`); synthesized
`game_saves` / telemetry store for `game` programs (ADR-012). Authors do not
declare `sink` modules in 0.4.0.

### Single-file north star

One `.silc` file can declare a full app—Contract, UI routes, processor,
ports, storage intent. Polyglot workers under `.runtime/` are compiler *output*,
not an authoring model. Authors never scaffold per-language projects or write
`package.json`.

### Provenance

`sil-router` records Tier 1 / Tier 2 decisions with provenance strings that
cite these strengths (for example,
`tier1: sink+SQLite → Go (durable low-latency storage)` for synthesized
persistence). Manifests and agent logs teach the same story as this ADR.

## Non-goals

- User- or agent-selectable engine overrides
- Bond, Node, or other alternate TypeScript hosts as primary engines
- Changing Tier 1 / Tier 2 rules solely to match marketing language (rules
  already align with this catalog)
- CUDA tensor execution in 0.4.0 (ADR-010)

## Consequences

### Positive

- Routing is auditable and teachable.
- Strength catalogs justify why Silc owns three engines instead of one VM.
- Future Tier 3 (ONNX beyond MiniLM) can extend provenance without inventing a
  new philosophy.

### Costs and risks

- Provenance strings and docs must stay in sync when routing policy changes.
- Strength catalogs are guidance for policy, not a substitute for tests.
