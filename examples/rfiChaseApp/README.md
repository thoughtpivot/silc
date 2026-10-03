# rfiChaseApp

Chase overdue RFIs with a Silc `loop`, with a person approving every reminder.

## Authored files

- `main.silc` — RFIs and reminders, two loops, and the RFI board
- `AGENTS.md` — agent guidance
- `.gitignore` — ignores compiler-owned `.runtime/` and `.silc/`

## Run

```bash
silc build main.silc
silc main.silc
```

- Web: `http://127.0.0.1:18088/` (override with `SILC_HTTP_PORT`)
- Loops inbox: `http://127.0.0.1:18088/loops`

## Loops

- `RfiChase` runs weekdays at 08:00 New York time (catch up 4h). For each
  overdue open RFI with an assignee, silclm drafts a reminder, the RFI's PM
  approves it in `/loops` within a day, and the loop records the reminder,
  stamps the RFI, and posts a notice. Each effect runs once per RFI per day.
- `OpenRfiDigest` is manual: **Run OpenRfiDigest now** posts a count of open RFIs.

`silc build` prints the worst-case cost of one run for each loop.
