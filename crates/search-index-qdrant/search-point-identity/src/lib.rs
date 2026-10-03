//! Canonical collision-detectable Qdrant point identities.
//!
//! One point identity is derived from the exact Architecture 8.4 S11.1
//! `ProjectionPointKey`: installation incarnation, collection generation,
//! projection membership, representation, unit, projection profile set and
//! point role. The key is deterministic CBOR, the full identity is BLAKE3-256,
//! and the Qdrant UUID is only a namespace-separated 128-bit projection of that
//! full digest. A matching UUID never authorizes overwrite without full digest
//! and canonical identity-field equality.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

mod canonical;
mod collision;
mod digest;
mod error;
mod key;
mod uuid;

pub use canonical::{
    CanonicalPointKeyBytes, canonical_point_key_bytes, encode_canonical_key,
};
pub use collision::{
    CollisionDecision, ExistingPointIdentity, PointIdentityPayload,
    compare_existing_identity, validate_identity_fields,
    validate_identity_payload,
};
pub use digest::{
    PointIdentity, PointIdentityDigest, derive_point_identity, full_digest,
    point_identity_digest,
};
pub use error::PointIdentityError;
pub use key::{
    DEFAULT_POINT_IDENTITY_LIMITS, POINT_IDENTITY_SCHEMA_VERSION,
    PointIdentityKey, PointIdentityLimits, PointRole, ProjectionPointKey,
};
pub use uuid::{
    PointId128, QdrantPointUuid, derive_qdrant_uuid, project_qdrant_uuid,
};

#[cfg(test)]
mod tests;
