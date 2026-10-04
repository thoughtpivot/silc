# ADR-018: Sysop-shared files (`files` + `ui::file_browser`)

- **Status:** Accepted
- **Date:** 2026-10-04
- **Related:** [ADR-003](ADR-003-declarative-ui.md),
  [ADR-009](ADR-009-compiler-synthesized-runtime.md),
  [ADR-011](ADR-011-document-extract.md)
- **Canonical:** `files` on [`App`](../crates/sil-core/src/app.rs);
  `ui::file_browser` in
  [`UI_COMPONENT_CATALOG`](../crates/sil-core/src/ui.rs);
  routes in
  [`app_worker.ts`](../crates/sil-codegen/templates/app_worker.ts);
  zip in
  [`files_zip_worker.py`](../crates/sil-codegen/templates/files_zip_worker.py)

## Context

An `app` can persist rows and extract uploaded documents, but it cannot point
at a directory the operator already has on disk and let callers browse and
download it. Bulletin-board “file areas” and similar catalogs need exactly
that: one sysop-chosen folder, listed in the UI, with files downloaded as-is
and folders downloaded as a zip. Authors must not write Bun, filesystem, or
zip code.

## Decision

One optional directive inside `app`:

```silc
app Board {
    route "/files" => FilesPage;
    files "./files";
}
```

- At most one `files` per app. The path is relative to the entry file and must
  not contain `..`. A missing directory warns at startup; it does not fail
  `silc build`.
- `ui::file_browser` (options `title?`, `empty_text?`; no events, no children)
  is a compile error unless the app declares `files`. Folder state lives in
  the compiler component, not author state.

The synthesized Bun worker adds two routes when `files` is present:

| Route | Behavior |
| --- | --- |
| `GET /files/list?path=` | `{ path, parent, entries: [{ name, kind, size, modified, path }] }`. Directories first. Dot-files hidden. |
| `GET /files/download?path=` | A file streams with `Content-Disposition: attachment`. A directory (or `zip=1`) streams a zip built by the pinned CPython `zipfile` module. |

Every target is `realpath`'d and must stay under the `realpath`'d share, so
`..` and symlinks that leave the share are 404.

| Surface | Lowering |
| --- | --- |
| Web | Breadcrumb, enter-folder rows, per-entry Download, “Download as .zip” for the current folder |
| Terminal | Same listing over `/files/list`. Folders are buttons. Files show size and the full download URL plus “Open in a browser”. No binary transfer over telnet or OpenTUI |

`SILC_FILES_DIR` overrides the directory. `SILC_FILES_PYTHON_BIN` overrides the
interpreter used to zip (default: the pinned CPython).

## Non-goals

- Uploads into the share (ADR-011 upload is extract-and-discard, not storage).
- Per-user permissions, quotas, or caching of zips.
- More than one shared directory per app.
- Serving the share as a static tree under arbitrary URLs.

## Consequences

- Zip is produced per request. Fine for a hobby library; large trees should
  stay small or move behind a cache later.
- The terminal surface can browse but not receive the bytes.
