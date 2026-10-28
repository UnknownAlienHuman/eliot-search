//! Compiled qdrant-client identity verification.

use crate::qualified::{
    ObservedClient, QUALIFIED_CLIENT_VERSION, QualificationError, verify_client,
};

/// Verifies the compiled-in client pin.
///
/// Compares a caller-observed Cargo package checksum with the compiled pin before any live call.
/// It does not read a qualification file or issue runtime authority; a mismatch simply rejects this
/// diagnostic path before touching the network.
#[must_use]
pub fn verify_compiled_client(source_checksum: &str) -> Option<QualificationError> {
    verify_client(&ObservedClient {
        crate_name: "qdrant-client".to_owned(),
        version: QUALIFIED_CLIENT_VERSION.to_owned(),
        source_checksum: source_checksum.to_owned(),
    })
    .err()
}
