# blogApp

Standalone Silc 0.4.0 blog application:

- **Home** — chronological article cards, **silclm** natural-language feed filter, and grounded Q&A
- **Admin** — create form plus searchable table; row click opens an edit/delete modal
- **Seeds** — thirty short SilcLM-authored articles declared as idempotent `seed` rows

## Authored files

- `main.silc`
- `AGENTS.md`
- `.gitignore`

`.runtime/` and `.silc/` are compiler-owned — do not commit or hand-edit them.

## Data model

`Article`: id, title, body, author, published_at, year, month.
Persisted in SQLite table `articles` through the `Articles` resource.
Seeds insert with `INSERT OR IGNORE` using stable ids so admin edits survive restarts.

## Run

```bash
silc build main.silc
SILC_HTTP_PORT=18120 silc main.silc
SILC_HTTP_PORT=18120 SILC_TERMINAL_PORT=18121 silc main.silc --terminal
```

- Web: `http://127.0.0.1:18120/`
- Terminal (with `--terminal`): OpenTUI locally, or `telnet 127.0.0.1 18121`
- API: `http://127.0.0.1:18120/api/articles`
