//! Platform-vault boundary, content-free write evidence and memory-only seam.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use super::spec::{
    PAIRING_KEY_BYTES, TEST_RNG_DOMAIN, SecretCompositionError,
};

/// Content-free evidence of one durable vault write.
///
/// The receipt names the exact blob digest observed on readback, so it is
/// derived from executed evidence rather than fabricated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultWriteEvidence {
    /// Digest of the exact bytes the vault returned on readback.
    pub blob_digest: Blake3Digest32,
    /// Content-free receipt naming the observed digest.
    pub receipt: ReceiptRef,
}

impl VaultWriteEvidence {
    /// Builds the content-free receipt naming one observed blob digest.
    ///
    /// Platform adapters call this after verifying their own write by exact
    /// readback; the receipt derives from executed evidence, never invented.
    pub fn receipt_for(blob_digest: Blake3Digest32) -> Result<ReceiptRef, SecretCompositionError> {
        ReceiptRef::new(format!("receipt:loopback-pairing:vault:{blob_digest}"))
            .map_err(|_| SecretCompositionError::ContractExhausted)
    }
}

/// Durable key-vault boundary owned by the platform adapter.
///
/// The composer drives provisioning, rotation, revocation and absence
/// readback through this trait; the implementation owns all OS I/O. Blobs are
/// opaque to the composer: exactly what `load_blob` returns after `store_blob`
/// is what the catalog lifecycle binds.
pub trait PairingVault {
    /// Draws one fresh 32-byte key. Non-zero by construction.
    fn generate_key(&mut self) -> Result<zeroize::Zeroizing<[u8; 32]>, SecretCompositionError>;
    /// Durably stores the key for `id`, overwriting any previous blob.
    fn store_blob(
        &mut self,
        id: &OpaqueId,
        key: &[u8; 32],
    ) -> Result<VaultWriteEvidence, SecretCompositionError>;
    /// Reads the exact blob for `id`; `None` means verifiably absent.
    fn load_blob(&mut self, id: &OpaqueId) -> Result<Option<Vec<u8>>, SecretCompositionError>;
    /// Deletes the blob for `id`; absence afterwards is verified by the
    /// composer through [`PairingVault::load_blob`], never trusted here.
    fn remove_blob(&mut self, id: &OpaqueId) -> Result<(), SecretCompositionError>;
    /// Whether this vault is OS-backed. The memory seam reports `false`.
    fn is_os_backed(&self) -> bool;
}

/// Explicit development/test vault holding blobs in process memory.
///
/// This is never an OS secret store: blobs sit in heap memory, and
/// [`PairingVault::is_os_backed`] reports `false`. All blobs are zeroized on
/// drop and on overwrite/remove. Deterministic key draws come from a
/// counter-keyed hash (test-only distribution, never advertised as OS
/// randomness).
#[derive(Debug, Default)]
pub struct MemoryPairingVault {
    blobs: BTreeMap<String, Vec<u8>>,
    draws: u64,
    /// When set, the next `remove_blob` reports success but keeps the blob,
    /// simulating a timeout after a possible external write.
    pub fail_next_remove_ambiguously: bool,
    /// When set, the next `store_blob` persists the blob but reports a write
    /// failure, simulating a timeout after a possible external write.
    pub fail_next_store_ambiguously: bool,
}

impl MemoryPairingVault {
    /// Creates an empty memory vault.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Directly drops the blob without touching the catalog, simulating
    /// external loss. Test-only fault injection.
    pub fn drop_blob_for_test(&mut self, id: &OpaqueId) {
        if let Some(mut blob) = self.blobs.remove(id.as_str()) {
            blob.fill(0);
        }
    }
}

impl Drop for MemoryPairingVault {
    fn drop(&mut self) {
        for blob in self.blobs.values_mut() {
            blob.fill(0);
        }
    }
}

impl PairingVault for MemoryPairingVault {
    fn generate_key(&mut self) -> Result<zeroize::Zeroizing<[u8; 32]>, SecretCompositionError> {
        self.draws = self
            .draws
            .checked_add(1)
            .ok_or(SecretCompositionError::ContractExhausted)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(TEST_RNG_DOMAIN);
        hasher.update(&self.draws.to_be_bytes());
        let mut key = *hasher.finalize().as_bytes();
        if key.iter().all(|byte| *byte == 0) {
            key[0] = 1;
        }
        Ok(zeroize::Zeroizing::new(key))
    }

    fn store_blob(
        &mut self,
        id: &OpaqueId,
        key: &[u8; 32],
    ) -> Result<VaultWriteEvidence, SecretCompositionError> {
        if key.len() != PAIRING_KEY_BYTES || key.iter().all(|byte| *byte == 0) {
            return Err(SecretCompositionError::InvalidKeyMaterial);
        }
        if let Some(previous) = self.blobs.get_mut(id.as_str()) {
            previous.fill(0);
        }
        self.blobs.insert(id.as_str().to_owned(), key.to_vec());
        let blob_digest = Blake3Digest32::from_bytes(*blake3::hash(key).as_bytes());
        let receipt = VaultWriteEvidence::receipt_for(blob_digest)?;
        if self.fail_next_store_ambiguously {
            self.fail_next_store_ambiguously = false;
            return Err(SecretCompositionError::VaultWriteFailed);
        }
        Ok(VaultWriteEvidence {
            blob_digest,
            receipt,
        })
    }

    fn load_blob(&mut self, id: &OpaqueId) -> Result<Option<Vec<u8>>, SecretCompositionError> {
        Ok(self.blobs.get(id.as_str()).cloned())
    }

    fn remove_blob(&mut self, id: &OpaqueId) -> Result<(), SecretCompositionError> {
        if self.fail_next_remove_ambiguously {
            self.fail_next_remove_ambiguously = false;
            return Ok(());
        }
        if let Some(mut blob) = self.blobs.remove(id.as_str()) {
            blob.fill(0);
        }
        Ok(())
    }

    fn is_os_backed(&self) -> bool {
        false
    }
}
