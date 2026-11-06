//! One owned unchanged UTF-8 product and its exact line inventory.

mod materialize;
mod model;
mod scan;

pub use materialize::materialize_utf8;
pub use model::{LineSpan, MaterializationLimits, MaterializedText};

#[cfg(test)]
mod tests;
