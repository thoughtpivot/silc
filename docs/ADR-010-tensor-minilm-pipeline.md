# ADR-010: Tensor / MiniLM Embedding Pipeline

- **Status:** Accepted; amended 2026-11-25 (embeddings in apps and loops,
  `query nearest`, served embedding models — THO-132)
- **Date:** 2026-07-27
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-004](ADR-004-runtime-strengths.md),
  [ADR-006](ADR-006-scrape-namespace.md),
  [ADR-007](ADR-007-pipeline-feeds.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ADR-014](ADR-014-loop-subject.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)
- **Canonical:** [`model_catalog.rs`](../crates/sil-core/src/model_catalog.rs)
  (`MINILM_*`, `EMBEDDING_MODEL_CATALOG`, `EmbeddingProvider`),
  [`EXECUTABLE_OPS`](../crates/sil-core/src/operation.rs) (`tensor::*`,
  `EmbeddingPlan`),
  [`registry.rs`](../crates/sil-core/src/registry.rs) (`COMPATIBILITY`),
  [`examples/core/pipelineApp/`](../examples/core/pipelineApp/),
  `crates/sil-codegen/templates/processor_worker.py`,
  `embedding_models.py`, `embed_worker.py`,
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
  *(Amended below: this now holds only for scrape pipelines.)*
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

[`examples/core/pipelineApp/main.silc`](../examples/core/pipelineApp/main.silc) is the
integration target: scrape → extract → tokenize → infer → synthesized SQLite.

## Amendment (2026-11-25): embeddings in apps and loops

Decided by Dan Stephenson on 25 Nov 2026 for Silc 0.8.0 (THO-132). The
pipeline above is unchanged; this section adds a second way to run
`tensor::infer`.

### Where embeddings run

Embedding and nearest-match run **only in CPython**, on CPU for MiniLM. A
dedicated **embed worker** (`python/embed_worker.py`, from
`python/.venv-tensor`) serves JSON over HTTP on a Unix socket
(`SILC_EMBED_SOCKET`):

| Route | Caller | Does |
| --- | --- | --- |
| `POST /embed {texts}` | Bun app worker, Go loop kernel | One vector per text |
| `POST /nearest {table, needle \| vector, limit}` | Bun app worker, Go loop kernel | Rows ranked by cosine |
| `GET /health` | Supervisor | Startup check |

The Go loop kernel keeps doing loop reads and calls the embed worker for
embeds. Bun is not required for embeddings: a loop command with no `app`
embeds through the kernel alone. Embeds never call silclm and never count
as model calls; the loop cost report's `model_calls` counts `loop::ask` only.

### Tensor feeds

In a program with an `app` or a `loop`, a processor method that embeds a
contract field is an **on-write feed**:

```silc
subset Emb384 of Vec[num32; 384];
contract FileEntry {
    has Str $.id;
    has Str $.text_excerpt;
    has Emb384 $.vector_embedding;
}
resource Files for FileEntry { query list; query nearest; mutation create; }
processor Embedder {
    method embed(FileEntry $e) {
        $e.text_excerpt ==> tensor::tokenize(:model("minilm-l6-v2"))
            ==> tensor::infer(:prefer(CPU))
    }
}
```

- The string on the left of `tensor::tokenize` is what gets embedded. Any
  `Str` field works; `raw_content` is no longer required outside pipelines.
- The vector is written to the contract's `vector_embedding` field. Its type
  must be `Vec[num32; N]` where N is the model's declared dimension
  (`Emb384` for MiniLM).
- Every write of that contract's resource table embeds the field: Bun
  `POST`/`PUT`, `loop::write`, and rows that other paths insert (doc
  extract, scrape, MCP, seeds), which a background sweep fills in.
- A feed processor does not take the single processor slot, so one program
  can hold an `app`, a `loop`, `llm::complete`, and a tensor feed.
- The contract needs a `resource … for Contract` to store vectors.

### Vector store

Vectors live in the row's data JSON in `data/app.db`. Ranking uses
**sqlite-vec** (`sqlite-vec==0.1.6`, loaded by the embed worker):
`vec_distance_cosine(vec_f32(json_extract(data, '$.vector_embedding')), vec_f32(?))`,
ascending, then capped. Rows with no vector or a vector of the wrong length
are skipped. Results never include the vector. The query scans the table;
there is no approximate index.

### Replay

Vectors are **never** written to `loop_events`. `loop::find` and mutation
triggers strip the vector field before recording a row, and the kernel's
`embed` step is not recorded, so a replay recomputes the vector from the same
text.

### `query nearest`

```silc
resource Files for FileEntry { query nearest; }
component SearchPage {
    has state Str $.needle = "";
    query $.hits = Files.nearest($.needle, 50);
}
```

- `nearest(needle: Str, limit: Int) -> Array<Contract>`, served at
  `GET /api/<table>/nearest?q=…&k=…`.
- `limit` is a required integer literal from 1 to `MAX_LOOP_LIST` (10000).
- The needle is a string literal or a `$.field` of the component.
- A blank needle returns no rows and makes no request. The query refetches
  (debounced) when the needle changes and never calls `GET /api/<table>`.
- Rows carry `_score` (1 − cosine distance), best first.
- The contract must declare `vector_embedding: Vec[num32; N]` matching the
  program's model; with no tensor feed the model is MiniLM.
- `query nearest` is not exposed as an MCP tool.

### Embedding models

Each catalog entry declares `id`, `dimension`, and an `EmbeddingProvider`:

| Id | Dimension | Provider | Notes |
| --- | --- | --- | --- |
| `minilm-l6-v2` | 384 | `onnx-cpu` | Default; no configuration |
| `nomic-embed-text` | 768 | `openai-compatible` | Local server, default `http://127.0.0.1:11434/v1` |
| `text-embedding-3-small` | 1536 | `openai-compatible` | Hosted; key from `OPENAI_API_KEY` |

`openai-compatible` providers call `POST {base_url}/embeddings`; the base URL
can be overridden with `SILC_EMBED_BASE_URL`, and keys are named by env var,
never written in source. The worker L2-normalizes served vectors and rejects
a reply whose length differs from the declared dimension. Adding a model
means adding a catalog entry (and, for a new provider kind, an adapter in
`embedding_models.py`), not opening arbitrary model shapes in `.silc`.

### Compatibility

`LocalModel × Embedding` and `Loop × Embedding` are lifted. Still banned:
`text::score × tensor::infer` and `loop × scrape::*`. `:prefer(CUDA)` still
fails; the message names the compiler's major.minor release.

### Pipeline mode

A program whose only surface is a `scrape::*` service keeps the original
pipeline: `tensor::infer` takes the processor slot, the contract needs
`raw_content` and `vector_embedding`, and it runs with
`silc run --input-json`.

### Kernel primitives (for THO-552 and THO-553)

The Go kernel has an `Embedder` interface (`Embed`, `Nearest`) and two plan
step ops: `embed` (`as`, `from`, `model`; not recorded) and `nearest` (`as`,
`table`, `from`, `max`; recorded like `loop::find`, without vectors; no
author syntax yet). The plan's `embedding` section names the model, its
dimension, and the feed and nearest tables.

`embed` is the lowering of `loop::let(:as(x), tensor::infer(:model(...),
:from(expr)))` (THO-552, [ADR-014](ADR-014-loop-subject.md) amendment). A
loop embed alone is enough to give the program an embedding plan and start
the embed worker; it must use the same model as any tensor feed.

## Consequences

- Agents cannot invent embedding models or GPU paths in `.silc`.
- Training and docs treat MiniLM as a closed product capability, parallel to
  silclm for chat.
- Expanding embeddings means growing the catalog and adapters, not opening
  arbitrary ONNX graphs.
- Apps and loops get semantic search without a separate vector database.

## Non-goals

- CUDA / GPU tensor execution
- Arbitrary model ids or tokenizer formats (served models are catalog entries)
- Author-declared sinks or IPC for embeddings
- Making general `tensor::*` / `numpy::*` / `pandas::*` executable
- Approximate nearest-neighbour indexes
- Nearest-match `loop::find` (THO-553)
