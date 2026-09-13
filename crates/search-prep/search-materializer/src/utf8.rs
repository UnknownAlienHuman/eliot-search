//! Exact bounded UTF-8 preparation and receipt binding.
//!
//! Public UTF-8 names remain stable while models, byte/line scanning,
//! receipt-bound materialization and tests have separate private owners.

mod materialize;
mod model;
mod scan;

pub use materialize::{materialize, materialize_utf8};
pub use model::{
    DEFAULT_MATERIALIZATION_LIMITS, LineEnding, LineEndingEvidence, LineSpan,
    MaterializationLimits, MaterializationReceipt, MaterializedRevision, MaterializedText,
    RetainedRevision,
};

#[cfg(test)]
mod tests;
