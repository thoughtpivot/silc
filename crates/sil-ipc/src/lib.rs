//! SIL-owned shared-memory ABI and UDS signaling.
//!
//! The future runtime will map versioned SIL Shared Buffers into Go, Python,
//! and Bun workers. Apache Arrow is not required by this boundary; external
//! interoperability adapters may be added separately.
//!
//! Implementation is deferred to a later milestone.

/// Placeholder so the workspace crate graph compiles.
pub fn stub() {}
