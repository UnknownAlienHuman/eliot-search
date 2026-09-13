//! Baseline decoding: encoding decision and text/code decoding.
//!
//! Public decoding contracts, BOM/encoding admission and byte-to-scalar
//! execution have distinct owners while retaining the existing module API.

mod detect;
mod engine;
mod model;

pub use detect::detect_or_validate_encoding;
pub use engine::decode_text_or_code;
pub use model::{DecodedLine, DecodedRepresentation, EncodingDecision, StepCounter};

#[cfg(test)]
mod tests;
