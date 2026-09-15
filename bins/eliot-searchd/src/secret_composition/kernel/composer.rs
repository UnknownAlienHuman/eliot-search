//! Base composer state, provisioning, lease exposure and quarantine boundary.

use search_contracts::{NonZeroRevision, OpaqueId};
use search_os_secrets::{
    EncryptedPayload, SecretBinding, SecretCatalog, SecretError, SecretLease,
    SecretLimits, SecretOperation, SecretRecord, SecretRecordState,
    SecretReference,
};
use search_ports::MonotonicInstant;

use super::binding::{
    check_key_bytes, lease_window, pairing_reference_id,
    verify_blob_readback,
};
use super::receipts::ProvisionReceipt;
use super::spec::{
    DEFAULT_LIMITS, DEFAULT_PAIRING_LEASE_TTL_TICKS, PAIRING_KEY_BYTES,
    SecretCompositionError,
};
use super::vault::PairingVault;

/// Purpose-bound pairing-secret composer: catalog lifecycle plus vault
/// effects plus lease-bound keyed proofs.
///
/// Exactly one active reference is managed. The composer holds no key
/// material itself; every proof goes through a short-lived [`SecretLease`]
/// inside one callback.
#[derive(Debug)]
pub struct PairingSecretComposer {
    pub(super) catalog: SecretCatalog,
    pub(super) binding: SecretBinding,
    pub(super) active_id: Option<OpaqueId>,
    pub(super) limits: SecretLimits,
}

impl PairingSecretComposer {
    /// Creates a composer for one exact authority binding.
    pub fn new(binding: SecretBinding) -> Result<Self, SecretCompositionError> {
        Self::with_limits(binding, DEFAULT_LIMITS)
    }

    /// Creates a composer with explicit finite limits.
    pub fn with_limits(
        binding: SecretBinding,
        limits: SecretLimits,
    ) -> Result<Self, SecretCompositionError> {
        if limits.validate().is_err() {
            return Err(SecretCompositionError::CapacityExceeded);
        }
        Ok(Self {
            catalog: SecretCatalog::with_limits(limits)?,
            binding,
            active_id: None,
            limits,
        })
    }

    /// Exact authority binding this composer serves.
    #[must_use]
    pub const fn binding(&self) -> &SecretBinding {
        &self.binding
    }

    /// Currently active reference identity, if any.
    #[must_use]
    pub const fn active_id(&self) -> Option<&OpaqueId> {
        self.active_id.as_ref()
    }

    /// Whether the active reference (if any) is currently leaseable.
    #[must_use]
    pub fn has_active_leaseable(&self) -> bool {
        self.active_id.as_ref().is_some_and(|id| {
            self.catalog
                .get(id)
                .is_ok_and(|record| matches!(record.state(), SecretRecordState::Active))
        })
    }

    /// Provisions the first active pairing secret through the vault.
    pub fn provision(
        &mut self,
        vault: &mut impl PairingVault,
        operation: SecretOperation,
    ) -> Result<ProvisionReceipt, SecretCompositionError> {
        if self.has_active_leaseable() {
            return Err(SecretCompositionError::InvalidTransition);
        }
        let id = pairing_reference_id(&operation)?;
        if self.catalog.get(&id).is_ok() {
            return Err(SecretCompositionError::InvalidTransition);
        }
        let key = vault.generate_key()?;
        check_key_bytes(&key[..])?;
        let evidence = vault.store_blob(&id, &key)?;
        let observed = vault
            .load_blob(&id)?
            .ok_or(SecretCompositionError::EvidenceMissing)?;
        verify_blob_readback(&observed, &evidence)?;
        let payload = EncryptedPayload::new(observed, self.limits.max_ciphertext_bytes)
            .map_err(SecretCompositionError::from)?;
        let one = NonZeroRevision::new(1).map_err(|_| SecretCompositionError::Quarantined)?;
        let reference = SecretReference::new(id.clone(), self.binding.clone(), one);
        let record = SecretRecord::new_active(
            reference.clone(),
            payload,
            evidence.blob_digest,
            one,
            operation,
        );
        self.catalog.create(record)?;
        self.active_id = Some(id);
        Ok(ProvisionReceipt {
            reference,
            record_revision: one,
            receipt: evidence.receipt,
        })
    }

    /// Verifies absence: no leaseable record and no vault blob.
    pub fn absence_verified(
        &self,
        vault: &mut impl PairingVault,
    ) -> Result<bool, SecretCompositionError> {
        let leaseable = self.has_active_leaseable();
        let blob_present = match self.active_id.clone() {
            Some(id) => vault.load_blob(&id)?.is_some(),
            None => false,
        };
        Ok(!leaseable && !blob_present)
    }

    /// Issues one finite plaintext lease for the active reference.
    pub fn issue_lease(
        &self,
        vault: &mut impl PairingVault,
        now: MonotonicInstant,
        ttl_ticks: u64,
    ) -> Result<SecretLease, SecretCompositionError> {
        let id = self
            .active_id
            .clone()
            .ok_or(SecretCompositionError::NotFound)?;
        let record = self.catalog.get(&id)?.clone();
        let (issued_at, expires_at) = lease_window(now, ttl_ticks)?;
        let plaintext = vault
            .load_blob(&id)?
            .ok_or(SecretCompositionError::EvidenceMissing)?;
        if plaintext.len() != PAIRING_KEY_BYTES {
            return Err(SecretCompositionError::InvalidKeyMaterial);
        }
        Ok(SecretLease::issue(
            &record,
            &self.binding,
            issued_at,
            expires_at,
            plaintext,
            self.limits,
        )?)
    }

    /// Exposes the active 32-byte key only for the callback duration.
    pub fn with_pairing_key<T>(
        &self,
        vault: &mut impl PairingVault,
        now: MonotonicInstant,
        use_key: impl FnOnce(&[u8; 32]) -> T,
    ) -> Result<T, SecretCompositionError> {
        let lease = self.issue_lease(vault, now, DEFAULT_PAIRING_LEASE_TTL_TICKS)?;
        lease
            .with_secret(now, |bytes| {
                let key: &[u8; 32] = bytes
                    .try_into()
                    .map_err(|_| SecretCompositionError::InvalidKeyMaterial)?;
                if key.iter().all(|byte| *byte == 0) {
                    return Err(SecretCompositionError::InvalidKeyMaterial);
                }
                Ok(use_key(key))
            })
            .map_err(SecretCompositionError::from)?
    }

    /// Explicitly quarantines the active reference; leases stop immediately.
    pub fn quarantine_active(&mut self, reason: SecretError) -> Result<(), SecretCompositionError> {
        let id = self
            .active_id
            .clone()
            .ok_or(SecretCompositionError::NotFound)?;
        self.catalog.quarantine(&id, reason)?;
        Ok(())
    }

    pub(super) fn quarantine_active_record(&mut self, id: &OpaqueId) {
        let _ = self.catalog.quarantine(id, SecretError::Quarantined);
    }
}
