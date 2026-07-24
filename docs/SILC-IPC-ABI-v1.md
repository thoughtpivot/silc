# Silc IPC ABI v1

Status: Implemented for the feedback-portal vertical slice.

## Goals

- Portable on macOS and Linux without `/dev/shm`
- Supervisor-owned file-backed mmap slot pool under `{workdir}/.runtime/<program>/ipc/`
- Small Unix-domain-socket control frames for wakeups and orchestration
- Honest v1 boundary: browser/Bun request bytes are copied once into a supervisor-owned slot; Python and Go then share that mapped slot

## Data plane (shared buffer)

Each slot is a file `slot_NNNN.sbuf` with:

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | magic `SILC` |
| 4 | 2 | `abi_version` (u16 LE) = 1 |
| 6 | 2 | `flags` (u16 LE) |
| 8 | 4 | `schema_id` (u32 LE) |
| 12 | 8 | `segment_id` (u64 LE) |
| 20 | 4 | `payload_offset` (u32 LE) = 128 |
| 24 | 4 | `payload_capacity` (u32 LE) |
| 28 | 4 | `payload_len` (u32 LE) |
| 32 | 8 | `seq` (u64 LE) |
| 40 | 4 | `state` (u32 LE) |
| 44 | 4 | `producer_id` |
| 48 | 4 | `consumer_id` |
| 52 | 76 | reserved / padding to 128 |
| 128 | … | JSON payload bytes |

States: `EMPTY=0`, `WRITING=1`, `READY=2`, `READING=3`, `RETIRED=4`.

Default pool: 64 slots × 64 KiB payload.

## Control plane (UDS)

Stream socket framing:

```
u32le payload_len | u16le protocol_version(=1) | JSON payload
```

Frame `type` values (serde SCREAMING_SNAKE_CASE):

- `HELLO` / `READY` — worker registration
- `INGEST` — Bun → supervisor (author, text, request_id)
- `NOTIFY` — supervisor → worker (segment_id, offset, len, schema_id, seq, stage)
- `ACK` / `ERROR` — worker → supervisor
- `RESPONSE` — supervisor → Bun
- `SHUTDOWN` — supervisor → workers

Socket paths may be shortened under the OS temp directory when the macOS path length limit would be exceeded; `.runtime/<program>/run.json` records the resolved path.

## Ownership

| Resource | Owner |
| --- | --- |
| Slot files | Rust supervisor (`silc`) via `sil-ipc` |
| Layout / schema id | Silc compiler (manifest) |
| Engines (Bun / CPython / Go) | Global `~/.silc/runtimes/` — never copied into `.runtime/` |
| SQLite DB | `.runtime/<program>/data/feedback.db` |

## Non-goals (v1)

- Typed zero-copy field views per language
- Linux `shm_open` fast path
- Crash recovery beyond whole-runtime restart
- `silc bundle` self-contained deployment artifacts
