# mcpLoopApp

A generic loop that reads one MCP tool, asks the local model for a typed note, and writes that note once.

The server URL is a placeholder (`https://mcp.example.test/mcp`). Set `EXAMPLE_MCP_TOKEN` to a bearer token for whatever server you point `:server` at. The token stays in the environment.

## Command (this file)

There is no `app`, and the only trigger is `loop::manual`.

```bash
export EXAMPLE_MCP_TOKEN=...
silc build main.silc
silc main.silc
```

Silc runs `DailyBrief` once, prints the notice on stdout, and exits.

## Inbox-only service

Replace `loop::manual()` with a schedule:

```silc
loop::schedule(:cron("0 8 * * *"), :tz("UTC"), :catch_up("12h"))
```

`silc main.silc` then stays up. The synthesized `/loops` inbox (web, terminal, or `/api`) is the interface. There is still no authored UI.

## App

Add a `component` and an `app` with routes for the notes you care about. Keep the `loop` declaration. The compiler adds `/loops` beside those routes, so a person can run the brief from the same program that shows `DailyNotes`.
