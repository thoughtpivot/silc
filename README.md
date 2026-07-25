# Silc — Intent-Native Polyglot Application Compiler

ThoughtPivot’s **Silc** (pronounced *silk*) is a compact, contract-bound
language and local Rust compiler. You write a short intent program. The
compiler validates it, routes each module to the right engine, provisions
pinned Bun / CPython / Go runtimes, emits workers, and runs the app — without
asking an LLM to invent React, Python, Go, manifests, or glue.

```text
.silc intent  →  Rust compiler  →  Bun · CPython · Go workers  →  mmap IPC + UDS
```

---

## Status (v0.1)

Silc is an **early vertical slice**, not a general-purpose language yet.

**Shipped today**

- Parse → validate → deterministic Tier 1/2 route → codegen
- Runnable portal and API profiles: feedback scoring, custom declarative UI,
  local LLM chat, grocery inventory, and `service::http` APIs
- Compiler-owned Bun 1.2.18, CPython 3.12.12, and Go 1.23.6 under `~/.silc/runtimes/`
- Supervisor-owned mmap shared buffers + UDS control plane (JSON payloads in
  shared slots — not typed zero-copy contract views yet)
- CI builds and smoke-runs the feedback portal (HTTP + SQLite) on Ubuntu and macOS

**Stub-only today**

- Broader pipeline examples (`http::get`, `tensor::`, `pandas::`, …) parse, route,
  and emit inspectable stubs; they do not execute

**Not shipped**

- Tier 3 ONNX routing, Apache Arrow hot path, typed IPC field views, OpenTUI
  rich terminals, streaming LLM, and general program orchestration

Token savings, warm compile latency, and fleet cost claims are **design
hypotheses**. They are not published as measured results in this README.

---

## Why Silc exists

Coding agents are expensive for the wrong reason.

Frontier models do not mainly bill you for the ten lines of domain logic you
wanted. They bill you for:

1. **Boilerplate generation** — manifests, handlers, serializers, framework glue
2. **Context rereads** — every later turn re-sends files, diffs, tool output, and
   history (research on agentic coding shows input context dominates spend)
3. **Retry and review loops** — failed builds and automated review consume more
   tokens than the first generation
4. **Maintenance surface** — every future change reopens the same polyglot mess
5. **Cloud inference** — chat and scoring features keep paying remote model APIs

Many “AI-native languages” optimize only (1): denser syntax so the model emits
fewer output tokens. That helps, but it is not enough. Silc attacks the full
lifecycle:

| Cost driver | Silc response |
| --- | --- |
| Output boilerplate | Dense contracts + pipelines; compiler expands the rest |
| Context rereads | One short `.silc` source stays the durable artifact |
| Retry loops | Local validation and deterministic diagnostics before/without LLM repair |
| Polyglot maintenance | Agents edit Silc; generated Bun/Python/Go stay under `.runtime/` |
| Framework sprawl | Declarative `ui::*` / `service::http` — no React/Go/Python authoring |
| Cloud inference | Optional local `llm::complete` with a pinned GGUF catalog |
| Build-time API spend | Compilation is local Rust — zero model tokens in the build loop |

**The metric that matters** is not “tokens in the first draft.” It is **billed
cost per successfully built and maintained feature**, including reads, retries,
diagnostics, later edits, machine time, and runtime inference. Silc is built so
that number can fall — and so we can measure it honestly later.

Savings hold only when you treat Silc as the source of truth. If agents keep
patching generated workers under `.runtime/`, the economics collapse back into
ordinary polyglot maintenance.

---

## What makes Silc different

The market already has overlapping bets:

| Category | Typical bet | Gap Silc fills |
| --- | --- | --- |
| Token-minimal languages | Compact syntax, fewer completion tokens | Often still one runtime; agents still own stack glue |
| Formally verified agent languages | Contracts / SMT proofs for generated code | Correctness without owned UI + polyglot routing + IPC |
| Full-stack DSLs | One file → web app on a fixed stack | Usually one substrate, not compiler-selected engines |
| Agent workflow compilers | Compile once, execute without LLM | Workflow graphs, not application languages with UI/API/store |

Silc’s distinctive intersection is:

1. **Intent source** — Raku-inspired contracts, modules, pipelines, and views
2. **Deterministic local lowering** — no LLM in `silc build` / `silc run`
3. **Substrate routing** — compiler assigns Bun, CPython, or Go from module kind,
   constraints, and operation namespaces ([ADR-004](docs/ADR-004-runtime-strengths.md))
4. **Owned runtimes** — pinned engines provisioned by the compiler, not PATH
5. **Declarative UI and APIs** — semantic ops lower to React/Bun or Go/Gin
6. **Cross-worker execution** — Rust supervisor + mmap slots + UDS for runnable
   portal profiles

That combination — not a “first AI-native language” superlative — is the product.

---

## Quick start

```bash
# From this repository
cargo install --path crates/silc

# Scaffold a project (writes AGENTS.md + main.silc, provisions engines)
silc init myapp
cd myapp

# Compile only
silc build main.silc

# Run a shipped vertical slice (first run downloads pinned engines)
silc /path/to/silc/examples/feedback_portal.silc
# open http://127.0.0.1:18080
```

CLI:

```text
silc                         # version + usage
silc init [path]             # scaffold + provision engines
silc build <file.silc|.raku> # compile only
silc <file.silc|.raku>       # compile; run when execution_mode is runnable
```

First runnable build downloads checksum-verified engines into `~/.silc/runtimes/`
and writes `.silc/runtimes.lock.json`. LLM examples additionally download a
pinned ~808 MB GGUF into `~/.silc/models/`. Local is not “free” — it avoids
cloud model charges for compilation and (optionally) inference, while still
using disk, CPU, and network for provisioning.

---

## A runnable example

[`examples/feedback_portal.silc`](examples/feedback_portal.silc) is 41 lines of
intent. The compiler emits Bun UI workers, a Python scorer, a Go/SQLite sink,
IPC slots, and a React bundle under `.runtime/`.

```raku
#!/usr/bin/env silc
@version("1.0")

subset NonEmpty of Str where { .chars > 0 }

class FeedbackRecord {
    has UUID $.id;
    has NonEmpty $.author;
    has NonEmpty $.text;
    has Str $.summary;
    has num64 $.score;
}

class WebPortal is service {
    method listen(:$port = 18080) {
        FeedbackRecord
            ==> ui::web(:port(18080), :route("/"))
    }

    method terminal(:$port = 18023) {
        FeedbackRecord
            ==> ui::terminal(:port(18023))
    }
}

class TextAnalyzer is processor {
    method analyze(FeedbackRecord $record) {
        $record.text
            ==> text::score()
    }
}

class FeedbackDb is sink is latency(5ms) is storage(SQLite) {
    method persist(FeedbackRecord $record) {
        $record
            ==> ipc::publish()
            ==> store::sqlite(:table(feedback))
            ==> store::commit()
    }
}
```

Authors never write HTML, React, `package.json`, Python packaging, or Go modules.
Those are compiler substrates. See [ADR-003](docs/ADR-003-declarative-ui.md).

---

## Examples

### Runnable v1

| Program | Profile | Notes |
| --- | --- | --- |
| [`examples/feedback_portal.silc`](examples/feedback_portal.silc) | Web + telnet + `text::score` + SQLite | CI-proven path |
| [`examples/custom_feedback_ui.silc`](examples/custom_feedback_ui.silc) | Custom `is view` layout | App bar, side panel, radio |
| [`examples/llm_portal.silc`](examples/llm_portal.silc) | Local `llm::complete` chat | Downloads pinned GGUF on first run |
| [`examples/ai_chatbot_2.silc`](examples/ai_chatbot_2.silc) | Chat + history UI | Local LLM (same model catalog) |
| [`examples/grocery_inventory.silc`](examples/grocery_inventory.silc) | Product grid + AI search | Inventory profile; first run downloads GGUF |
| [`examples/feedback_api.silc`](examples/feedback_api.silc) | `service::http` → Go/Gin | API-only (no processor/sink) |

### Parse / route / stub emit

| Program | Purpose |
| --- | --- |
| [`examples/article_pipeline.silc`](examples/article_pipeline.silc) | Routing demo across Bun / Python / Go |
| [`examples/sensor_alert.silc`](examples/sensor_alert.silc) | Sensor + tensor stub pipeline |
| [`examples/url_health.silc`](examples/url_health.silc) | HTTP fetch + infer stub |
| [`examples/csv_summary.raku`](examples/csv_summary.raku) | Pandas / numpy stub |
| [`examples/log_anomaly.raku`](examples/log_anomaly.raku) | Log anomaly stub |

Runnable operations are gated by an explicit registry in
[`crates/sil-core/src/operation.rs`](crates/sil-core/src/operation.rs):

`ui::web`, `ui::terminal`, `html::form`, `http::serve`, `service::http`,
`text::score`, `llm::complete`, `ipc::publish`, `store::sqlite`, `store::commit`.

Mixing stub-only ops into a runnable graph is a compile error — by design.

---

## Architecture

```text
Silc source (.silc / conforming .raku)
        │
        ▼
   sil-lexer → sil-parser → sil-core subjects
        │                      (Contract · Module · Constraint · Pipeline · Target · UiView)
        ▼
   sil-router   Tier 1 (kind + traits) + Tier 2 (namespaces)
        │       Tier 3 ONNX classifier: planned
        ▼
   sil-codegen  stub emit  or  runnable workers + React UI lowering
        │
        ▼
   silc supervisor
        ├── Bun  (TypeScript UI / ingress)
        ├── CPython (scoring / local LLM)
        ├── Go (SQLite / HTTP API)
        └── sil-ipc (mmap slots + UDS frames)
```

Workspace crates:

| Crate | Role |
| --- | --- |
| `sil-core` | Semantic subjects, UI catalog, executable op registry |
| `sil-lexer` / `sil-parser` | Raku-inspired first-pass front end |
| `sil-router` | Deterministic engine assignment + provenance |
| `sil-codegen` | Stub and runnable templates, UI lowering |
| `sil-ipc` | Shared-buffer ABI + UDS control plane |
| `silc` | CLI, init, runtime/model provisioning, supervisor |

Per-app output lands in `{workdir}/.runtime/{program}/` (gitignored, inspectable).
Engines stay in `~/.silc/runtimes/`. Details:
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md),
[ADR-001](docs/ADR-001-runtime-and-ipc.md),
[SILC-IPC-ABI-v1.md](docs/SILC-IPC-ABI-v1.md).

**IPC honesty:** Bun request bytes are copied once into a supervisor-owned mmap
slot. Python and Go share that mapped slot. Payloads are schema-tagged JSON in
v1. Typed zero-copy contract field views are the next ABI layer.

---

## Lifecycle economics (design rationale)

Silc does **not** claim a measured 95% token reduction today. The architectural
bet is broader than completion density:

```text
Traditional agent coding
  intent → emit TS/Py/Go boilerplate → reread repo → fail build → retry → review
  (cloud tokens on every step; maintenance stays in generated languages)

Silc
  intent → emit short .silc → local compile/validate → run owned workers
  (LLM exits after intent; later edits target the same dense source)
```

Structurally true now:

- Build and run do not call a model API
- Generated framework code is compiler-owned, not agent-authored
- Declarative UI catalog rejects inventing unsupported widgets
- Local LLM path can remove recurring cloud inference for chat/scoring profiles

Still to measure:

- Tokens and dollars per successful feature vs agent-written TypeScript/Python/Go
- Warm vs cold compile times (first run includes engine downloads)
- Throughput under sustained load (a feedback-portal benchmark script exists;
  CI currently smoke-tests concurrent POSTs, not a published RPS number)

Until those studies land, treat economics as a **thesis with a clear measurement
plan**, not a scoreboard.

---

## For AI agents

`silc init` copies [`crates/silc/templates/AGENTS.md`](crates/silc/templates/AGENTS.md)
into the project. That file is the operational contract:

- Edit Silc source only
- Do not hand-edit `.runtime/` or invent HTML/React/Go/Python packaging
- Stay inside the UI component catalog and runnable op set
- Stop and report compiler limits instead of improvising substrates

Surface syntax: [ADR-002](docs/ADR-002-silc-surface-syntax.md).
`.silc` is primary; conforming `.raku` is accepted; `.sil` is not.

---

## Development

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace -- --test-threads=1

# Optional heavy e2e (downloads GGUF; ignored by default)
cargo test -p silc --test llm_e2e -- --ignored

# Manual feedback throughput gate (not CI-published)
python3 examples/feedback_portal/benchmark.py
```

CI (`.github/workflows/ci.yml`) runs fmt, check, workspace tests, an offline LLM
codegen smoke test, feedback portal build, and 25 concurrent `/submit` POSTs
with SQLite persistence checks on Ubuntu and macOS.

---

## Roadmap

Pulled from architecture and ADR non-goals:

- Expand the executable op set beyond portal/API shapes
- Program-level orchestration and richer validation
- Tier 3 ONNX-assisted routing
- Typed field views on the shared-memory ABI; Arrow remains optional later
- Rich local `ui::terminal` via OpenTUI
- Streaming LLM and a broader model catalog
- Crash recovery and deployment bundles

---

## Documentation

| Doc | Topic |
| --- | --- |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Subject model and crate layout |
| [docs/ADR-001-runtime-and-ipc.md](docs/ADR-001-runtime-and-ipc.md) | Engines and IPC |
| [docs/ADR-002-silc-surface-syntax.md](docs/ADR-002-silc-surface-syntax.md) | Language surface |
| [docs/ADR-003-declarative-ui.md](docs/ADR-003-declarative-ui.md) | UI catalog and lowering |
| [docs/ADR-004-runtime-strengths.md](docs/ADR-004-runtime-strengths.md) | Why Bun / CPython / Go |
| [docs/ADR-005-local-llm-complete.md](docs/ADR-005-local-llm-complete.md) | Local LLM completions |
| [docs/SILC-IPC-ABI-v1.md](docs/SILC-IPC-ABI-v1.md) | Shared buffer ABI |

---

## License

Apache-2.0. See the workspace `Cargo.toml` and repository license files.
