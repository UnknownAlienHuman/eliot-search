//! Private identity join over already verified source-map revision rows.
//! No source bytes, paths, alternate inventory or search semantics.

use crate::ControlError;
use super::{ROW_BYTES, codec};

/// One canonical occurrence's original object binding.
#[expect(
    clippy::redundant_pub_crate,
    reason = "The sibling persistent owner uses this type through the crate-private migration reexport."
)]
pub(crate) struct ContentRevisionBinding {
    pub(crate) legacy_pair: [u8; 64],
    pub(crate) content_sha256: [u8; 32],
    pub(crate) source_bytes: u64,
}

#[expect(
    clippy::redundant_pub_crate,
    reason = "The sibling persistent owner uses this helper through the crate-private migration reexport."
)]
pub(crate) fn content_revision_binding(
    canonical_revision: &[u8],
    sequence: u64,
    row: &[u8],
) -> Result<ContentRevisionBinding, ControlError> {
    if row.len() != ROW_BYTES
        || canonical_revision != &row[16..32]
        || u64::from_be_bytes(row[65..73].try_into().map_err(|_| ControlError::StoreCorrupt)?) != sequence
        || row[81] & 2 == 0
    {
        return Err(ControlError::MigrationUnverified);
    }
    let mut legacy_pair = [0; 64];
    legacy_pair[..32].copy_from_slice(codec::hash_field(row, 1)?);
    legacy_pair[32..].copy_from_slice(codec::hash_field(row, 2)?);
    Ok(ContentRevisionBinding {
        legacy_pair,
        content_sha256: codec::hash_field(row, 3)?.try_into().map_err(|_| ControlError::StoreCorrupt)?,
        source_bytes: u64::from_be_bytes(row[73..81].try_into().map_err(|_| ControlError::StoreCorrupt)?),
    })
}
