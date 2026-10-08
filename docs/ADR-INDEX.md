# Silc ADR index

Canonical index of Architecture Decision Records, specifications, and
historical appendices. Decision records capture *why*; the authoring contract
for agents and humans lives in
[`crates/silc/templates/AGENTS.md`](../crates/silc/templates/AGENTS.md) and the
root [README](../README.md).

## Decision records

| ADR | Title | Status | Notes |
| --- | --- | --- | --- |
| [001](ADR-001-runtime-and-ipc.md) | Bun runtime and Silc-owned shared-memory IPC | Accepted (v1) | Spec detail in [SILC-IPC-ABI-v1.md](SILC-IPC-ABI-v1.md) |
| [002](ADR-002-silc-surface-syntax.md) | Raku-inspired Silc surface syntax | Accepted | Direct declarations; subject-first evidence in appendices |
| [003](ADR-003-declarative-ui.md) | Declarative dual-surface UI | Accepted | Dual-surface **outcome**; authoring mechanics in ADR-009 |
| [004](ADR-004-runtime-strengths.md) | Runtime engine strength catalog | Accepted | Bun / CPython / Go routing rationale |
| [005](ADR-005-local-llm-complete.md) | Local LLM completions (`llm::complete` / silclm) | Accepted (v1) | Persistence synthesized (ADR-009) |
| [006](ADR-006-scrape-namespace.md) | Scrape namespace (`scrape::*`) | Accepted | All five operations shipped |
| [007](ADR-007-pipeline-feeds.md) | Pipeline feeds (`==>`) | Accepted | Author vs synthesized steps clarified by ADR-009 |
| [008](ADR-008-recursive-silclm-assist.md) | Recursive silclm assist (`silc assist`) | Accepted (Phase 1) | Distinct from in-app `llm::complete` |
| [009](ADR-009-compiler-synthesized-runtime.md) | Compiler-synthesized runtime (0.4.0) | Accepted | Partially supersedes authoring examples in 002/003/005/007 |
| [010](ADR-010-tensor-minilm-pipeline.md) | Tensor / MiniLM embedding pipeline | Accepted | Closed CPU pipeline; `pipelineApp` |
| [011](ADR-011-document-extract.md) | Document extract (`doc::*`) | Accepted | Upload + Python extract; `dataExtractorApp` |
| [012](ADR-012-webgpu-game-subject.md) | WebGPU game subject (`game::*`) | Accepted | Web-only surface; Bun+CPython+Go spine |
| [013](ADR-013-procedural-asset-generation.md) | Procedural asset generation | Accepted | Game asset bake step |
| [014](ADR-014-loop-subject.md) | Loop subject (`loop::*`) | Accepted | Go kernel; receipts, replay, approvals inbox |
| [015](ADR-015-silc-loop-and-vran.md) | Silc `loop` and Vran | Accepted | Amended 2026-10-08: the Vran app is not paused. Language draft superseded by ADR-019 |
| [016](ADR-016-generic-kernel-domain-layers.md) | Generic kernels, domain layers | Accepted | `scene::` kernel, `game::` gameplay layer; verticals stay outside the compiler |
| [017](ADR-017-ui-embed.md) | `ui::embed` dual-surface URL viewport | Accepted | Sandboxed iframe on web; card fallback on terminal; does not relax ADR-012 |
| [018](ADR-018-files-capability.md) | Sysop-shared files (`files` + `ui::file_browser`) | Accepted | `/files/list` + `/files/download`; folders zip via pinned CPython |
| [019](ADR-019-vran-language-into-loop.md) | Vran 0.1 language folded into `loop::*` | Accepted | Maps the vran-oss draft onto the closed catalog. Proposed additions are not shipped |

### Partial supersession (0.4.0)

```text
ADR-009 ──partial──► ADR-002 (serve / sink / resource body examples)
         ──partial──► ADR-003 (author-declared ui::web / ui::terminal)
         ──partial──► ADR-005 (author ipc/store persistence chain)
         ──partial──► ADR-007 (resource::* as author feed example)
```

Historical Decision and Consequences sections remain; each amended ADR links
forward to ADR-009 for the current authoring rule.

## Specifications (not numbered ADRs)

| Spec | Role |
| --- | --- |
| [SILC-LANGUAGE.md](SILC-LANGUAGE.md) | Normative language surface: declarations, nodes, options, operations, compatibility |
| [GLOSSARY.md](GLOSSARY.md) | One meaning per term; retired vocabulary |
| [SILC-IPC-ABI-v1.md](SILC-IPC-ABI-v1.md) | Shared-buffer and UDS framing constants |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Subject model, crate map, execution layout |
| [intent-vs-subjects.md](intent-vs-subjects.md) | Author intent surface vs compiler subjects |
| [domains/vdc.md](domains/vdc.md) | VDC / AEC go-to-market; not a compiler vocabulary |

## Historical / reproducibility appendices

| Doc | Evidence class | Role |
| --- | --- | --- |
| [subject-first-decision.md](subject-first-decision.md) | Historical | July 2026 benchmark no-go + owner override |
| [subject-first-declarators.md](subject-first-declarators.md) | Harness | `subject-first-bench` CLI and go/no-go criteria |
| [vran-0.1-loop-grammar.md](vran-0.1-loop-grammar.md) | Source text | Billy's Loop grammar as implemented in vran-oss `699ec980`. Verbatim EBNF and syntax rules. Conformance to `loop::*` is unconfirmed |

## Metadata template

Every ADR uses:

```markdown
- **Status:** …
- **Date:** YYYY-MM-DD
- **Updated:** YYYY-MM-DD          # when amended
- **Related:** …
- **Supersedes:** …                 # optional
- **Superseded by (partial):** …    # optional forward pointer
- **Canonical:** …                  # optional code/doc source of truth
```
