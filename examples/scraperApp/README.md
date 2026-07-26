# scraperApp

Standalone Silc 0.2.0 scraping application:

- Enter a **website URL** and **crawl depth** (1–5)
- Silc runs `scrape::site` (Go Colly) with optional Playwright escalation (`:js(auto)`)
- Results appear in a searchable `ui::table` backed by the `scraped_pages` resource

## Authored files

- `main.silc`
- `AGENTS.md`
- `.gitignore`

`.runtime/` and `.silc/` are compiler-owned — do not commit or hand-edit them.

## Data model

`ScrapedPage`: id, url, title, snippet, depth, status  
Persisted in SQLite table `scraped_pages` through the `Pages` resource.

## Run

```bash
silc build main.silc
silc main.silc
```

- Web: `http://127.0.0.1:18110/`
- Terminal: OpenTUI locally, or `telnet 127.0.0.1 18111`
- API: `http://127.0.0.1:18110/api/scraped_pages`

Authors never name Bun, Colly, or Playwright — see
[ADR-006](../../docs/ADR-006-scrape-namespace.md).
