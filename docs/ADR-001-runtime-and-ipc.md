# ADR-001: Bun Runtime and SIL-Owned Shared-Memory IPC

- **Status:** Accepted
- **Date:** 2026-07-25

## Context

SIL compiles semantic programs into a local polyglot runtime. The runtime needs
three complementary execution engines:

- Go for systems, streams, and storage;
- Python for machine learning and scientific computing;
- TypeScript for asynchronous I/O and web protocols.

The original vision named Node as the TypeScript engine and Apache Arrow as the
required shared-memory format. Neither is required by Rust or by SIL's
language model. Because SIL generates every worker and owns its Contracts, the
compiler can generate direct accessors for a smaller, purpose-built memory
layout.

## Decision

### Runtime engines

SIL targets **Go, Python, and Bun**. TypeScript remains the generated language
and the directory name remains `typescript`; Bun is the process that executes
that source. The supervisor will pin and invoke Bun rather than Node.

Bun is the default because its native TypeScript execution and mmap support fit
SIL's generated, local-worker model. Node compatibility is not a requirement
for the primary runtime.

### Data plane

ThoughtPivot owns a versioned **SIL Shared Buffer ABI**. The Rust supervisor
allocates a POSIX shared-memory segment or an mmap-backed file under
`.runtime/`. Contracts lower deterministically into that layout, and codegen
emits typed views for Go, Python, and Bun.

The ABI will define, at minimum:

- magic bytes, ABI version, byte order, and alignment;
- contract/schema identifier;
- segment length and payload bounds;
- field offsets and typed data regions;
- ownership, lifetime, and producer/consumer state.

Large payloads stay in shared memory. Consumers access mapped bytes through
native slices, memory views, or typed arrays without serializing the payload
through JSON, Protobuf, or an HTTP stack.

### Control plane

Processes exchange small wakeups over Unix Domain Sockets. A signal identifies
shared data with fields equivalent to:

```text
{ segment_id, offset, len, schema_id }
```

The exact binary control frame is a later implementation decision. It does not
change the data-plane ABI.

### Apache Arrow

Apache Arrow is not part of SIL's required hot path. A future adapter may
project SIL buffers into Arrow for interoperability with external analytical
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
- The data layout can match SIL Contracts exactly.
- Bun can consume mapped bytes as typed views while Python and Go use their
  native buffer facilities.
- Generated workers remain small and deterministic.

### Costs and risks

- ThoughtPivot owns ABI versioning, alignment, bounds checking, lifecycle, and
  language bindings.
- Memory safety and crash recovery require explicit design and testing.
- Bun APIs used by the runtime must be pinned and validated before production.
- External Arrow interoperability requires a separate adapter.

## Non-goals for the scaffold

This ADR does not implement mmap allocation, UDS signaling, generated accessors,
runtime installation, process supervision, or benchmarks. The current
repository remains scaffold and documentation only.
