# chatApp

Standalone Silc 0.4.0 multi-session chat assistant.

## Authored files

- `main.silc` — contracts, session resource, chat UI, processor (SQLite persistence synthesized)
- `AGENTS.md` — agent guidance
- `.gitignore` — ignores compiler-owned `.runtime/` and `.silc/`

## Run

```bash
silc build main.silc
silc main.silc
```

- Web: `http://127.0.0.1:18090/`
- Terminal: `telnet 127.0.0.1 18091`

Uses the default **silclm** catalog model (`llm::complete()` with no `:model`).
First LLM run downloads the pinned GGUF into `~/.silc/models/silclm/`.
