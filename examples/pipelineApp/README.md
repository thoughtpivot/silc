# pipelineApp

A Silc 0.4.0 pipeline-only example
([ADR-010](../../docs/ADR-010-tensor-minilm-pipeline.md)) that fetches a static
HTTP page with Bun, extracts bounded text, creates a normalized 384-float MiniLM
embedding with CPython/ONNX on CPU, and persists the complete article record
with a compiler-synthesized Go/SQLite sink ([ADR-009](../../docs/ADR-009-compiler-synthesized-runtime.md)).

Build it:

```bash
silc build main.silc
```

Run it with inline JSON or a JSON file:

```bash
silc run main.silc --input-json '{"url":"https://example.com/"}'
silc run main.silc --input input.json
```

The generated database is `.runtime/main/data/app.db`; the `articles` table
stores each record as generic JSON, including `raw_content` and the
`vector_embedding` array. The example is deliberately static-fetch only:
JavaScript rendering and CUDA are outside Silc 0.4.0's tensor pipeline.
