# ADR-005: Local LLM Completions (`llm::complete`)

- **Status:** Accepted (v1)
- **Date:** 2026-07-25
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-003](ADR-003-declarative-ui.md),
  [ADR-004](ADR-004-runtime-strengths.md)

## Context

Silc applications need to run small language models locally without exposing
Python packages, model hosting tools, GGUF paths, or inference engines in the
authoring surface. A model is a processor resource, not a new module kind or UI
surface.

## Decision

The first runnable capability is:

```silc
class LocalLlm is processor {
    has Str $.model_ref = "llama3.2-1b";

    method complete(ChatTurn $turn) {
        $turn.prompt ==> llm::complete(:model($.model_ref))
    }
}
```

`llm::complete` routes the processor to CPython. Silc generates and owns a
`llama-cpp-python` worker, provisions one pinned GGUF artifact, and passes its
absolute path through supervisor-owned environment variables. Authors select a
catalog id, never an adapter or file path.

### v1 catalog

| ID | Artifact | Size |
| --- | --- | --- |
| `llama3.2-1b` | Llama 3.2 1B Instruct Q4_K_M GGUF | 807,694,464 bytes |

The artifact is downloaded from the compiler-pinned Hugging Face URL into
`~/.silc/models/llama3.2-1b/` and verified against its pinned SHA-256 digest.
Unknown catalog ids fail before worker startup. Omitting `:model` defaults to
`llama3.2-1b`.

### Runtime profile

- Adapter: `python-llamacpp-v1`
- Dependency: compiler-pinned `llama-cpp-python`
- Worker count: one Python process per app, so weights load once
- Context: 2,048 tokens; response cap: 256 tokens
- UI: compiler-owned React/Bun prompt form
- Persistence: compiler-owned Go/SQLite `chat_turns` sink

`silc build` creates an isolated Python environment under the generated
runtime, installs the pinned binding, downloads/verifies the model, and builds
the UI and Go sink. Unit and code-generation tests remain offline. The real
inference e2e is ignored by default because its first run downloads roughly
808 MB:

```bash
cargo test -p silc --test llm_e2e -- --ignored --nocapture
```

## Consequences

- Local completions are real and reproducible without Ollama or a user-managed
  Python environment.
- Catalog expansion is additive; future Qwen or Kimi-family entries reuse the
  same authoring surface.
- CPU-only machines work but may complete slowly. Available llama.cpp GPU
  acceleration is used by the generated worker.
- Verifying an existing model protects the cache from corrupt or substituted
  artifacts.

## Non-goals

- Streaming tokens, multi-turn conversations, or shared cross-program model
  daemons
- `is model` modules or `ui::model`
- Headless `service::http` chat in this slice
- Additional catalog models in v1
