//! Compiled qdrant-client identity verification.

use crate::qualified::{
    ObservedClient, QUALIFIED_CLIENT_VERSION, QualificationError, verify_client,
};

/// Verifies the compiled-in client pin.
///
/// Compares the registry/lockfile record against the pin before any live call.
/// Callers pass the checksum recorded in
/// `qualification/qdrant/artifact.toml`; a mismatch fails without touching the
/// network.
#[must_use]
pub fn verify_compiled_client(
    source_checksum: &str,
) -> Option<QualificationError> {
    verify_client(&ObservedClient {
        crate_name: "qdrant-client".to_owned(),
        version: QUALIFIED_CLIENT_VERSION.to_owned(),
        source_checksum: source_checksum.to_owned(),
    })
    .err()
}
