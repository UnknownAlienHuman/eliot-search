//! Concrete encrypted-object backend contract.

use search_contracts::OpaqueId;

use super::error::RevisionStoreError;
use super::model::{
    EncryptedRevisionPayload, RevisionKey, RevisionObjectReadback,
    RevisionRecord, RevisionWriteIntent,
};

/// Concrete encrypted-object backend contract.
pub trait RevisionObjectBackend {
    /// Concrete backend error.
    type BackendError;

    /// Attempts one atomic immutable encrypted-object write.
    fn write_immutable(
        &mut self,
        intent: &RevisionWriteIntent,
    ) -> Result<(), Self::BackendError>;

    /// Reads exact content-free object metadata after write or unknown outcome.
    fn readback(
        &mut self,
        key: &RevisionKey,
        storage_object_id: &OpaqueId,
    ) -> Result<Option<RevisionObjectReadback>, Self::BackendError>;

    /// Reads exact encrypted bytes for an active record.
    fn read_encrypted(
        &mut self,
        record: &RevisionRecord,
        max_ciphertext_bytes: u64,
    ) -> Result<EncryptedRevisionPayload, Self::BackendError>;

    /// Maps a concrete error without including source or ciphertext bytes.
    fn map_backend_error(error: &Self::BackendError) -> RevisionStoreError;
}
