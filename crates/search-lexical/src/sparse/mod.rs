//! Deterministic qualified sparse lexical encoding.

#![allow(
    missing_docs,
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

mod encoding;
mod error;
mod fingerprint;
mod mapping;
mod numerical;
mod profile;
mod vector;

pub use encoding::{
    SparseEncoding, SparseEncodingKind, SparseEncodingReceipt, encode_document, encode_query,
};
pub use error::SparseError;
pub use fingerprint::SparseFingerprint;
// Crate-internal sharing with frozen-profile/scoring modules (see allow above).
#[allow(clippy::redundant_pub_crate)]
pub(crate) use fingerprint::fingerprint_bytes;
#[allow(clippy::redundant_pub_crate)]
pub(crate) use fingerprint::fingerprint_bytes_checked;
pub use mapping::{
    CollisionReport, SparseFeature, SparseFeatureSet, map_terms, measure_collision_terms,
    term_index,
};
pub use profile::{
    AcceptedSparseProfile, CollisionPolicy, DocumentTfWeighting, FrozenCorpusStatistics, IdfMode,
    QueryTfWeighting, SparseLimits, SparseProfile, SparseQualification, validate_sparse_profile,
};
pub use vector::SparseVector;

#[allow(clippy::redundant_pub_crate)]
pub(crate) use mapping::map_terms_parameters_checked;
#[allow(clippy::redundant_pub_crate)]
pub(crate) use numerical::NumericalParameters;
#[allow(clippy::redundant_pub_crate)]
pub(crate) use vector::{weight_document_parameters_checked, weight_query_parameters_checked};
