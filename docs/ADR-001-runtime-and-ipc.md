# ADR-001: Bun Runtime and Silc-Owned Shared-Memory IPC

- **Status:** Accepted (v1 partial implementation)
- **Date:** 2026-07-25
- **Implemented:** Silc-owned Bun + CPython + Go cache (`~/.silc/runtimes/`),
  file-backed mmap slots, framed UDS control plane, feedback-portal supervisor.
  See [SILC-IPC-ABI-v1.md](SILC-IPC-ABI-v1.md).

## Context

Silc compiles semantic programs into a local polyglot runtime. The runtime needs
three complementary execution engines:

- Go for systems, streams, and storage;
- Python for machine learning and scientific computing;
- TypeScript for asynchronous I/O and web protocols.

The original vision named Node as the TypeScript engine and Apache Arrow as the
required shared-memory format. Neither is required by Rust or by Silc's
language model. Because Silc generates every worker and owns its Contracts, the
compiler can generate direct accessors for a smaller, purpose-built memory
layout.

## Decision

### Runtime engines

Silc targets **Go, Python, and Bun**. TypeScript remains the generated language
and the directory name remains `typescript`; Bun is the process that executes
that source. Silc provisions pinned, checksum-verified engines into its own
cache and invokes them by absolute path rather than consulting user PATH.

Bun is the default because its native TypeScript execution and mmap support fit
Silc's generated, local-worker model. Node compatibility is not a requirement
for the primary runtime.

Bun also executes compiler-owned UI substrates: React + Tailwind +
ShadCN-style primitives for `ui::web`, a telnet-compatible TCP adapter for
remote `ui::terminal` sessions, and in the future OpenTUI for rich local
terminals. Those substrates are implementation details under `.runtime/`;
Silc source never names them. See
[ADR-003-declarative-ui.md](ADR-003-declarative-ui.md).

Engine assignment rationale (why Bun vs CPython vs Go) lives in
[ADR-004-runtime-strengths.md](ADR-004-runtime-strengths.md).

### Data plane

ThoughtPivot owns a versioned **Silc Shared Buffer ABI**. ABI v1 uses a bounded
pool of portable mmap-backed files under `.runtime/`. The Rust supervisor owns
allocation and lifecycle; Python mutates and Go consumes the same mapped slot.
Bun ingress is copied once into the supervisor-owned slot. ABI v1 carries a
schema-tagged JSON payload; deterministic typed contract views are the next ABI
layer rather than a claim of the current implementation.

The ABI will define, at minimum:

- magic bytes, ABI version, byte order, and alignment;
- contract/schema identifier;
- segment length and payload bounds;
- field offsets and typed data regions;
- ownership, lifetime, and producer/consumer state.

Payloads stay in shared memory between processor and sink. JSON parsing still
occurs within the mapped buffer in v1; the transport does not send payload
bytes through an HTTP or UDS message between those workers.

### Control plane

Processes exchange small wakeups over Unix Domain Sockets. A signal identifies
shared data with fields equivalent to:

```text
{ segment_id, offset, len, schema_id }
```

ABI v1 uses `u32le payload_len | u16le protocol_version | JSON frame`, with
versioned `HELLO`, `READY`, `INGEST`, `NOTIFY`, `ACK`, `ERROR`, `RESPONSE`, and
`SHUTDOWN` messages. See [SILC-IPC-ABI-v1.md](SILC-IPC-ABI-v1.md).

### Apache Arrow

Apache Arrow is not part of Silc's required hot path. A future adapter may
project Silc buffers into Arrow for interoperability with external analytical
tools. Such an adapter must not become a mandatory dependency for generated
workers.

## Architectural ownership

- `sil-core::contract` owns logical schemas and layout invariants.
- `sil-codegen` lowers validated Contracts and emits per-engine accessors.
- `sil-ipc` owns shared-memory allocation, ABI framing, process-safe handles,
  lifecycle rules, and UDS signaling.
- `silc` composes these boundaries and supervises generated workers.

## Consequences

### Positive

- No PyArrow, arrow-go, or arrow-js dependency on every generated program.
- The data layout can match Silc Contracts exactly.
- Bun can consume mapped bytes as typed views while Python and Go use their
  native buffer facilities.
- Generated workers remain small and deterministic.

### Costs and risks

- ThoughtPivot owns ABI versioning, alignment, bounds checking, lifecycle, and
  language bindings.
- Memory safety and crash recovery require explicit design and testing.
- Bun APIs used by the runtime must be pinned and validated before production.
- External Arrow interoperability requires a separate adapter.

## Non-goals for ABI v1

- Typed zero-copy contract field views
- General execution lowering beyond the feedback operation set
- Per-worker crash recovery (v1 restarts the runtime)
- Linux-specific `shm_open` optimization
- Self-contained deployment bundles
