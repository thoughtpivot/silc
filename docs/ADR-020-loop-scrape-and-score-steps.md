# ADR-020: `scrape::*` reads and `text::score` inside loops

- **Status:** Accepted
- **Date:** 2026-11-26
- **Related:** [ADR-006](ADR-006-scrape-namespace.md),
  [ADR-014](ADR-014-loop-subject.md),
  [ADR-019](ADR-019-vran-language-into-loop.md)
- **Canonical:** `LOOP_READ_OPS`, `LOOP_LET_OPS`, and `COMPATIBILITY` in
  [`sil-core`](../crates/sil-core/src/loops.rs); the kernel in
  [`loop_kernel.go`](../crates/sil-codegen/templates/loop_kernel.go).
  Linear THO-579, part of THO-551.

## Context

Loops could read one page (`scrape::page`) or call one MCP tool. Sites in the
0.8.0 gap list also crawl a few pages, render pages that need JavaScript, take
every match of a selector, and drop low-quality text before asking silclm.
`text::score` ran only as a pipeline processor, and the compatibility matrix
banned it next to `loop::ask`, `scrape::*`, and `doc::*`.

## Decision

1. **Reads.** `loop::read` takes `scrape::site`, `scrape::render`, and
   `scrape::select` next to `scrape::page` and `mcp::call`. Each read is one
   recorded event, so a replay never refetches.
   - `scrape::site(:url, :max(N), :depth?, :same_host?, :link_css?)` binds a
     list of pages. `:max` is required (1 to 100) and caps pages fetched.
     `:depth` defaults to 2, where 1 is the start page. The crawl is a
     breadth-first walk in the Go kernel over the same fetcher as
     `scrape::page`: links in page order, fragments dropped, each URL once.
     The start page must load; later pages that fail are left out. The
     kernel does not use the Colly sidecar, so the crawl stays inside the
     recorded read and its fetch count is the cap.
   - `scrape::render(:url)` loads the page in headless Chromium through the
     compiler's Playwright worker (CPython), then shapes it like a page read.
   - `scrape::select(:url, :css, :max(N))` binds every match, up to `:max`,
     each `url`, `text`, and `href`. No match is an empty list.
   - `:select`, `:into`, and `:fields` work as on `scrape::page` (per page for
     `site`, per match for `select`). Options that do not apply to a read
     are compile errors.
2. **Score.** The form is `loop::let(:as(x), text::score(:text(expr)))`.
   It is a binding, not a read: the score is a pure function of text the loop
   already has, so the kernel computes it in Go (the same heuristic as the
   CPython processor) and recomputes it on replay instead of recording it.
   It binds a number from 0 to 1 and is not a model call. It cannot score
   `loop::ask` output before a `gate` or `approve` checks it.
3. **Placement.** A `scrape::*` read anywhere else in a loop is a compile
   error that points at `loop::read`; `text::score` anywhere but
   `loop::let` points at the `let` form.
4. **Cost.** `silc build` counts a `scrape::site` read as `:max × (1 +
   :retry)` fetches, scaled by `each`. Scores are listed on their own and do
   not add model calls.
5. **Compatibility.** Lifted: `text::score` with `loop::ask`, with
   `scrape::*`, and with `doc::extract`. Kept, with the reason in the
   message: `text::score` with `llm::complete` (both need the one processor
   slot; `loop::ask` uses its own frame), `text::score` with `tensor::infer`,
   and loops with `scrape::*` pipelines (the scrape service summarizes
   through the processor slot, which loop answers would share).

## Consequences

- A loop can crawl, render, select, score, and ask in one `each`, bounded at
  build time.
- A program with `scrape::render` in a loop builds the Playwright venv.
- Pipelines and loops still cannot both scrape in one program.
