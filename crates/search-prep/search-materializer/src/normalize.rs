//! Newline and Unicode normalization with recorded offset changes.
//!
//! Public normalization names remain stable while canonical models,
//! transformation execution and tests have separate private owners.

mod engine;
mod model;

pub use engine::normalize_representation;
pub use model::{CanonicalLine, CanonicalRepresentation};

#[cfg(test)]
mod tests;
