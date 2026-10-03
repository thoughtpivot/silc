# oneThingApp

The one thing Dan should really do today, as a Silc `loop`.

Every day at 05:00 UTC, `OneThingToday` reads the last week of the
ThoughtPivot Moz company brain over MCP, briefs it with **silclm**, and
reduces it to one imperative sentence. The sentence lands in the
`DailyActions` table on `/` and as a notice in `/loops`.

For the same workflow with no UI at all, run once from the shell, see
[`oneThingCliApp`](../oneThingCliApp/).

## Authored files

- `main.silc` — the loop, the `DailyActions` resource, and a small board
- `AGENTS.md` — agent guidance
- `.gitignore` — ignores compiler-owned `.runtime/` and `.silc/`

## Run

```bash
export MOZ_MCP_TOKEN='<Moz bearer token>'
silc build main.silc
silc main.silc
```

- Web: `http://127.0.0.1:18088/` (override with `SILC_HTTP_PORT`)
- Loops inbox: `http://127.0.0.1:18088/loops` — press **Run OneThingToday now** to run it off schedule

`silc build` prints the worst-case cost of one run:

```text
OneThingToday  schedule "0 5 * * *" UTC, catch up 12h
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
   and one notice, both keyed by the run's recorded time. Every run adds its own
   row to the board, newest first.

Every MCP result and model answer is recorded in the loop event log, so a
resumed run replays instead of calling Moz or the model again. Without
`MOZ_MCP_TOKEN` the run fails before any network or model call.
