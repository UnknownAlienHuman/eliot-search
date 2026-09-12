//! Exact qualified Rust client verification.

use super::{
    QUALIFIED_CLIENT_CHECKSUM, QUALIFIED_CLIENT_CRATE,
    QUALIFIED_CLIENT_VERSION, QualificationError,
};

/// Observed Rust client identity resolved from registry and lockfile data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedClient {
    /// Crate name.
    pub crate_name: String,
    /// Exact semantic version.
    pub version: String,
    /// crates.io source checksum.
    pub source_checksum: String,
}

/// Accepted Rust client receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientReceipt {
    /// Exact accepted client version.
    pub client_version: String,
}

/// Verifies one observed Rust client against the exact qualified pin.
///
/// # Errors
///
/// Returns the first closed identity mismatch.
pub fn verify_client(
    observed: &ObservedClient,
) -> Result<ClientReceipt, QualificationError> {
    if observed.crate_name != QUALIFIED_CLIENT_CRATE {
        return Err(QualificationError::ClientCrateMismatch);
    }
    if observed.version != QUALIFIED_CLIENT_VERSION {
        return Err(QualificationError::ClientVersionMismatch);
    }
    if observed.source_checksum != QUALIFIED_CLIENT_CHECKSUM {
        return Err(QualificationError::ClientChecksumMismatch);
    }
    Ok(ClientReceipt {
        client_version: observed.version.clone(),
    })
}
