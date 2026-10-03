# oneThingCliApp

The one thing Dan should really do today, as a Silc **loop command**.

There is no web page and no terminal UI. `silc main.silc` reads the last
week of the ThoughtPivot Moz company brain over MCP, briefs it with
**silclm**, reduces it to one imperative sentence, prints that sentence to
stdout, and exits. Each run is still recorded in the `DailyActions` table
under `.runtime/`, so the history survives between runs.

This is the command-line shape of [`oneThingApp`](../oneThingApp/): the same
reads, asks, gate, write, and notice, with `loop::manual()` in place of the
schedule and no `component` or `app`. Because every loop is manual and there
is no `app`, the compiler builds a command instead of the `/loops` inbox.

## Authored files

- `main.silc` — the loop and the `DailyActions` resource
- `AGENTS.md` — agent guidance
- `.gitignore` — ignores compiler-owned `.runtime/` and `.silc/`

## Run

```bash
export MOZ_MCP_TOKEN='<Moz bearer token>'
silc main.silc
```

The result is the only thing on stdout, so it composes:

```bash
silc main.silc | pbcopy
silc main.silc 2>/dev/null
```

Everything else (compiler status, worker startup, step-by-step progress) goes
to stderr:

```text
silc: running OneThingToday once
silc loop: OneThingToday: run run-67ab71ddbb5e3566 started (command:2026-10-03T22:18:14Z)
silc loop: OneThingToday: reading decisions (mcp::call kb_jsonl_read_window)
silc loop: OneThingToday: read decisions ok
...
silc loop: OneThingToday: asking the model for Brief (brief)
silc loop: OneThingToday: ask brief ok
silc loop: OneThingToday: asking the model for OneThing (action)
silc loop: OneThingToday: ask action ok
silc loop: OneThingToday: gate passed
silc loop: OneThingToday: wrote one daily_actions row
silc loop: OneThingToday: notice posted to Dan Stephenson
silc loop: run run-67ab71ddbb5e3566: finished succeeded
Send the corrected subcontract SOW to Emil via Sign.com
```

The exit code is `0` when every loop succeeded and `1` otherwise. A failed
run prints nothing on stdout; the reason is on stderr.

`silc build main.silc` compiles without running and prints the worst-case
cost of one run:

```text
OneThingToday  manual
               model calls 6 · effects 2 (writes 1, notices 1) · approvals 0 · reads 12 · rows scanned 0
```

## What a run does

1. Calls `kb_jsonl_read_window` on the Moz MCP server four times: recent
   decisions, interactions, projects, and opportunities. `:select("records.parsed")`
   keeps the parsed records only.
2. Asks silclm for a `Brief` (facts in the last 7 days, live goals through
   next week, and the one pressure) using the `$calendar` window.
3. Asks silclm for `OneThing`: exactly one imperative sentence.
4. Gates on a non-empty sentence and pressure, then writes one `DailyAction`
   and one notice, both keyed by the run's recorded time. The notice text is
   what the command prints.

Every MCP result and model answer is recorded in the loop event log. Without
`MOZ_MCP_TOKEN` the run fails before any network or model call.

## Why no approvals

A command has nobody waiting to answer, so `loop::approve` is a compile error
here. Use `loop::gate` for machine-checkable conditions, or add an `app` to get
the `/loops` inbox as `oneThingApp` does.
