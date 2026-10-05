# hotelSignupApp

Standalone Silc 0.7.0 example: the smallest useful two-route app. It shows the
direct-mutation handler style (`Guests.create(Guest.new(...))` inside
`on_submit`) as the alternative to the starter's `submit()` + processor
pattern.

- **Sign Up** (`/`) — form bound to component state; a successful submit clears
  the fields
- **Ledger** (`/ledger`) — sortable, searchable `ui::table` over the live
  `Guests.list()` query

## Authored files

- `main.silc`
- `AGENTS.md`
- `.gitignore`

`.runtime/` and `.silc/` are compiler-owned — do not commit or hand-edit them.

## Data model

`Guest`: name, phone, room, comment. Persisted in SQLite table `guests` through
the synthesized `GET/POST /api/guests` routes.

## Run

```bash
silc build main.silc
silc main.silc              # web on http://127.0.0.1:18088
silc main.silc --terminal   # also attach OpenTUI (telnet fallback 18023)
```
