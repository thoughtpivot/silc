# ADR-011: Document Extract (`doc::*`) and dataCollectorApp

- **Status:** Accepted
- **Date:** 2026-07-28
- **Related:** [ADR-001](ADR-001-runtime-and-ipc.md),
  [ADR-004](ADR-004-runtime-strengths.md),
  [ADR-006](ADR-006-scrape-namespace.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ARCHITECTURE.md](ARCHITECTURE.md)
- **Canonical:** [`EXECUTABLE_OPS`](../crates/sil-core/src/operation.rs)
  (`doc::extract`), [`ui::file_input`](../crates/sil-core/src/ui.rs),
  codegen templates `doc_extract_worker.py` / `app_worker.ts`

## Context

Silc apps need local document ingestion: upload a file, extract structured
fields, and store rows for a ledger UI — without a fourth runtime binary
(Pandoc) and without keeping original bytes in SQLite.

Python already owns heavy extract/score/LLM work ([ADR-004](ADR-004-runtime-strengths.md)).
Format-specific pip libraries are enough for v1 quality: DOCX/HTML strong,
PDF “good enough” plain structure.

## Decision

### Author catalog

| Construct | Intent |
| --- | --- |
| `ui::file_input` | Dual-surface file picker (`:field?`, `:label?`, `:accept?`, `:multiple?`) |
| `doc::extract(:into(Contract))` | Fill a contract from a compiler-owned upload handle |

Canonical contract fields the extractor populates (missing → empty string):

`title`, `headings`, `body`, `tables`, `filename`, `mime`, `format`, `char_count`

### Formats (v1)

`.pdf`, `.docx`, `.odt`, `.md` / `.markdown`, `.txt`, `.html` / `.htm`

### Substrate policy (no Pandoc)

| Stage | Engine | Adapter |
| --- | --- | --- |
| Multipart ingress | Bun | `bun-multipart-v1` (`POST /upload`) |
| Extract | CPython | `python-doc-extract-v1` (`pypdf`, `python-docx`, `odfpy`, BeautifulSoup) |
| Persist | Go / Bun SQLite resource path | synthesized `resource` CRUD |

Authors never write filesystem paths, pip package names, or MIME plumbing by
hand. Terminal surface accepts a path string that posts the same ingest shape.

### Discard originals

Staged uploads under `.runtime/.../uploads` are deleted after a successful
extract. Only structured fields + metadata land in the resource table.

### Mutual exclusions

- Do not mix `doc::*` with `text::score` in one program.
- `doc::extract` requires `:into(Contract)` and a `resource … for Contract`.
- Scrape (`scrape::*`) and document extract may coexist; they are separate
  ingress paths (`/scrape` vs `/upload`).
- Future `llm::complete` / `text::score` over extracted body is out of scope
  for v1 (fields are left suitable for that pass). Those two ops remain
  mutually exclusive with each other.

### dataCollectorApp

North-star example: upload form (`ui::file_input` + submit), documents ledger
table, and a detail view of one extracted document — authored via
`silc init` / `silc assist` only.

Pipeline intent:

```silc
$upload ==> doc::extract(:into(Document))
```

## Consequences

- Supervisor installs a doc-specific venv (`.venv-doc`) from
  `python/doc_requirements.txt` when the graph has `doc::extract`.
- AGENTS / IDE docs list `doc::extract` and `ui::file_input`; README catalog
  count is 39 dual-surface builtins.
- PDF heading/table quality is weaker than DOCX/HTML; title falls back to
  first line or filename.
