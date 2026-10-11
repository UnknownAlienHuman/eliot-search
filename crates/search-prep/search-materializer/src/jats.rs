//! Inert Publishing 1.4 subset observations over original retained UTF8 XML.
//! This owner supplies preparation and replay, never a `UnitSet` or admission.
mod identity;
mod meter;
mod model;
mod parse;
mod pipeline;
mod profile;
pub use model::{
    JatsAttribute, JatsAttributeKind, JatsComment, JatsElement, JatsElementKind, JatsError,
    JatsLimits, JatsObservationStatus, JatsSpan, JatsText, JatsTextKind,
};
pub use pipeline::{JatsProduct, JatsVerificationReceipt, materialize_jats, verify_jats};
pub use profile::{
    JatsOperationLimits, JatsProfile, MAX_JATS_LIMITS, MAX_JATS_PREIMAGE_BYTES,
    MAX_JATS_TOTAL_CANONICAL_BYTES,
};
#[cfg(test)]
mod fixtures;
