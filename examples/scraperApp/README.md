# scraperApp

Standalone Silc 0.4.0 scraping application:

- Enter a **website URL** and **crawl depth** (1–10)
- Silc runs `scrape::site` (Go Colly) with optional Playwright escalation (`:js(auto)`)
- SilcLM produces a grounded summary for every successfully scraped page
- Every scrape is retained in the `scraped_pages` resource
- Website chips and fuzzy search filter the complete scrape catalog

## Authored files

- `main.silc`
- `AGENTS.md`
- `.gitignore`

`.runtime/` and `.silc/` are compiler-owned — do not commit or hand-edit them.

## Data model

`ScrapedPage`: id, scrape_id, scraped_at, site, url, title, summary,
summary_model, depth, status.
Persisted as append-only history in SQLite table `scraped_pages` through the `Pages`
resource. Re-scraping a site creates a new cataloged run instead of replacing earlier results.
The summary shown in the table is generated locally through `llm::complete()` using
SilcLM and the scraped page content as grounded context.

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
