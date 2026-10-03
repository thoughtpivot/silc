# whatToDoTodayApp

What Dan should do today, as a Silc `loop` you drive from the terminal.

Every day at 05:00 UTC, `WhatToDoToday` reads the last week of the
ThoughtPivot Moz company brain over MCP, briefs it with **silclm**, and
reduces it to three to five imperative items in priority order plus one
sentence of why. The list lands in the `DailyPlans` table and as a notice
in the `/loops` inbox.

This example has **no authored UI**. `main.silc` declares contracts, one
resource, and one loop. Because there is no `app`, the compiler synthesizes
`LoopsApp` and routes both `/` and `/loops` to the loop inbox, which renders
on the web and on the terminal like any other Silc app.

## Authored files

- `main.silc` — the loop and the `DailyPlans` resource
- `AGENTS.md` — agent guidance
- `.gitignore` — ignores compiler-owned `.runtime/` and `.silc/`

## Run from the terminal

```bash
export MOZ_MCP_TOKEN='<Moz bearer token>'
silc build main.silc
silc main.silc --terminal
```

`--terminal` attaches the OpenTUI inbox in the current shell. In it:

1. Type your name under **Approvals** (the name is recorded on the run).
2. Press **Run WhatToDoToday now**.
3. Watch the **Runs** table move to `succeeded`, then read the list in the
   **Notices** table.

Remote fallback when the terminal surface is attached:
`telnet 127.0.0.1 18023` (override with `SILC_TERMINAL_PORT`). The telnet
CLI speaks to the same resource tables, so a run can be started and read
there too:

```text
> /create loop_requests {"loop":"WhatToDoToday","requested_by":"Dan","status":"pending","run_id":""}
> /list loop_runs
> /list loop_notices
> /list daily_plans
```

The web inbox is also served at `http://127.0.0.1:18088/loops` (override
with `SILC_HTTP_PORT`).

## Run from a shell script

The inbox is built from ordinary resources, so everything it does is also
available over the compiler-owned `/api` routes. With `silc main.silc`
running in another terminal:

```bash
# start a run (the kernel picks up pending requests within a few seconds)
curl -s -X POST http://127.0.0.1:18088/api/loop_requests \
  -H 'content-type: application/json' \
  -d '{"loop":"WhatToDoToday","requested_by":"Dan","status":"pending","run_id":""}'

# follow the run
curl -s http://127.0.0.1:18088/api/loop_runs

# read today's list
curl -s http://127.0.0.1:18088/api/loop_notices
curl -s http://127.0.0.1:18088/api/daily_plans
```

A loop never overlaps itself: a request made while a run is active waits
until that run finishes.

## What `silc build` prints

```text
WhatToDoToday  schedule "0 5 * * *" UTC, catch up 12h
               model calls 6 · effects 2 (writes 1, notices 1) · approvals 0 · reads 12 · rows scanned 0
```

## What a run does

1. Calls `kb_jsonl_read_window` on the Moz MCP server four times: recent
   decisions, interactions, projects, and opportunities. `:select("records.parsed")`
   keeps the parsed records only.
2. Asks silclm for a `Brief` (facts in the last 7 days, live goals through
   next week, and the one pressure) using the `$calendar` window.
3. Asks silclm for a `TodoList`: `first` through `fifth` and `why`. Unused
   slots are the sentence `No further action today.` (a blank field is
   rejected).
4. Gates on non-empty `first`, `second`, `third`, `why`, and `pressure`, then
   writes one `DailyPlan` row and one notice, both keyed by the run's recorded
   time. Every run adds its own row, newest first.

Every MCP result and model answer is recorded in the loop event log, so a
resumed run replays instead of calling Moz or the model again. Without
`MOZ_MCP_TOKEN` the run fails before any network or model call.
