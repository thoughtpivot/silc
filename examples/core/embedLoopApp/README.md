# embedLoopApp

Standalone Silc 0.7.0 example that proves three backlog fixes in one program:

1. **`ui::embed`** (THO-119) — a dialog and page host `https://example.com/` with required `:src` and a `:title`. Web lowers to a sandboxed iframe; terminal lowers to a title / full URL / “Open in a browser” card. No `game` subject (ADR-012).
2. **Blank `Str` in `loop::ask`** (THO-120) — `NextSlot.item` may be `""`; that means “no more items” and is accepted by contract checking instead of failing as a type error.
3. **Live loop kernel + `app.db` lock** (THO-121) — `Heartbeat` is a scheduled loop beside the UI `app`, so `silc main.silc` starts a kernel against `.runtime/app.db`. A second kernel must refuse that live database.

## Authored files

- `main.silc`
- `AGENTS.md`
- `.gitignore`

`.runtime/` and `.silc/` are compiler-owned — do not commit or hand-edit them.

## Run

```bash
silc build main.silc
SILC_HTTP_PORT=18150 SILC_TERMINAL_PORT=18151 silc main.silc
SILC_HTTP_PORT=18150 SILC_TERMINAL_PORT=18151 silc main.silc --terminal
```

Open `http://127.0.0.1:18150/` for the board (embed + slots table). `/loops` is synthesized for `CollectSlots` and `Heartbeat`.
