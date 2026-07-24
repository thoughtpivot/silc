//! The subject-oriented semantic core of ThoughtPivot SIL.
//!
//! Each module represents a durable language concept and will own that
//! concept's types, invariants, and semantic behavior. Compiler phase crates
//! are adapters around this core rather than owners of the domain model.

pub mod constraint;
pub mod contract;
pub mod module;
pub mod pipeline;
pub mod target;
