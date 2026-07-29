# dataCollectorApp

Standalone Silc 0.4.0 document collector:

- Upload PDF, DOCX, ODT, Markdown, HTML, or plain text via `ui::file_input`
- Compiler synthesizes multipart `POST /upload` and `doc::extract` (Python-native — no Pandoc)
- Extracted fields land in the `documents` resource; originals are discarded
- Documents ledger at `/documents` shows title, filename, format, char count, and body

## Authored files

- `main.silc` — authored via `silc assist` (do not hand-patch for product demos)
- `AGENTS.md`
- `.gitignore`

`.runtime/` and `.silc/` are compiler-owned — do not commit or hand-edit them.

## Data model

`Document`: title, headings, body, tables, filename, mime, format, char_count.

Pipeline intent:

```silc
$upload ==> doc::extract(:into(Document))
```

## Run

```bash
silc build main.silc
SILC_HTTP_PORT=18130 silc main.silc
silc main.silc --terminal
```

- Web: `http://127.0.0.1:18130/`
- Documents: `http://127.0.0.1:18130/documents`
- API: `http://127.0.0.1:18130/api/documents`

See [ADR-011](../../docs/ADR-011-document-extract.md).
