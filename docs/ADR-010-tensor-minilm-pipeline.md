# ADR-010: Tensor / MiniLM Embedding Pipeline

- **Status:** Accepted
- **Date:** 2026-07-27
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-004](ADR-004-runtime-strengths.md),
  [ADR-006](ADR-006-scrape-namespace.md),
  [ADR-007](ADR-007-pipeline-feeds.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)
- **Canonical:** [`model_catalog.rs`](../crates/sil-core/src/model_catalog.rs)
  (`MINILM_*`, `EMBEDDING_MODEL_CATALOG`),
  [`EXECUTABLE_OPS`](../crates/sil-core/src/operation.rs) (`tensor::*`),
  [`examples/pipelineApp/`](../examples/pipelineApp/),
  `crates/sil-codegen/templates/processor_worker.py`,
  `tensor_requirements.txt`, `pipeline_worker.ts`

## Context

Silc needs a closed, local embedding path for one-shot ingestion pipelines
without exposing ONNX runtimes, tokenizer paths, CUDA pickers, or arbitrary
model shapes on the authoring surface. Broader `tensor::*` / `numpy::*`
namespaces remain stubs; 0.4.0 ships only the MiniLM embedding slice.

## Decision

### Author surface

```silc
$article.raw_content
    ==> tensor::tokenize(:model("minilm-l6-v2"))
    ==> tensor::infer(:prefer(CPU))
```

Rules:

- Catalog id is **`minilm-l6-v2`** (default when `:model` is omitted).
- Inference is **CPU-only**. `:prefer(CUDA)` and other accelerators are rejected.
- Output dimension is fixed at **384** normalized `num32` values.
- Contracts use field names `raw_content` → `vector_embedding`, typically via
  `subset Emb384 of Vec[num32; 384]`.
- Tensor programs are **pipeline-only**: no UI `app`. Run with
  `silc run main.silc --input-json '{"url":"https://…"}'`.
- Persistence is **synthesized** (ADR-009); authors do not declare `sink`.

### Catalog and artifacts

Pinned ONNX model + tokenizer live under `~/.silc/models/minilm-l6-v2/`.
Checksums and URLs are owned by `EMBEDDING_MODEL_CATALOG` /
`MINILM_ARTIFACTS` in `sil-core`. Unknown model ids fail before worker startup.

### Worker topology

| Stage | Engine | Role |
| --- | --- | --- |
| Static fetch / extract | Bun | `scrape::page` + `scrape::extract` ingress |
| Tokenize + ONNX infer | CPython | Mean-pool + L2-normalize MiniLM |
| Persist | Go / SQLite | Synthesized sink |

Pipeline-only graphs use **64 KiB** mmap payload slots while retaining ABI
protocol v1; the general default remains 512 × 16 KiB (ADR-001 /
[SILC-IPC-ABI-v1.md](SILC-IPC-ABI-v1.md)).

### Reference program

[`examples/pipelineApp/main.silc`](../examples/pipelineApp/main.silc) is the
integration target: scrape → extract → tokenize → infer → synthesized SQLite.

## Consequences

- Agents cannot invent embedding models or GPU paths in `.silc`.
- Training and docs treat MiniLM as a closed product capability, parallel to
  silclm for chat.
- Expanding embeddings means growing the catalog and adapters, not opening
  arbitrary ONNX graphs.

## Non-goals

- CUDA / GPU tensor execution in 0.4.0
- Arbitrary model ids, dimensions, or tokenizer formats
- Author-declared sinks or IPC for embeddings
- Making general `tensor::*` / `numpy::*` / `pandas::*` executable
- UI apps that embed tensors in-process (use pipeline-only programs)
