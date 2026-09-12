//! Immutable artifact and optional-signature reference validation.

mod model;
mod validation;

pub use model::{ArtifactRef, OptionalSignature, SignatureValue};
pub use validation::{validate_artifact_ref, validate_optional_signature};
