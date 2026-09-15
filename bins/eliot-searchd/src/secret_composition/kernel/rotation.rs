//! Rotation preparation, exact readback recovery and catalog confirmation.

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};
use search_os_secrets::{
    EncryptedPayload, SecretMutationEffect, SecretOperation, SecretRecordState,
    SecretReference, SecretWriteReadback,
};

use super::binding::{check_key_bytes, verify_blob_readback};
use super::composer::PairingSecretComposer;
use super::receipts::{MutationOutcome, RotationReceipt};
use super::spec::{PAIRING_KEY_BYTES, SecretCompositionError};
use super::vault::{PairingVault, VaultWriteEvidence};

/// Verified replacement inputs for one rotation commit.
struct PreparedRotation<'a> {
    observed: Vec<u8>,
    evidence: &'a VaultWriteEvidence,
    replacement_version: NonZeroRevision,
    replacement_revision: NonZeroRevision,
    operation: &'a SecretOperation,
    recovered: bool,
}

impl PairingSecretComposer {
    /// Rotates the active secret exactly one version forward.
    ///
    /// An ambiguous platform write is resolved by exact readback and reported
    /// as [`MutationOutcome::Recovered`], never relabeled a clean commit.
    pub fn rotate(
        &mut self,
        vault: &mut impl PairingVault,
        operation: &SecretOperation,
    ) -> Result<MutationOutcome<RotationReceipt>, SecretCompositionError> {
        let id = self
            .active_id
            .clone()
            .ok_or(SecretCompositionError::NotFound)?;
        let current = self.catalog.get(&id)?.clone();
        if !matches!(current.state(), SecretRecordState::Active) {
            return Err(SecretCompositionError::NotLeaseable);
        }
        if current.reference().binding() != &self.binding {
            return Err(SecretCompositionError::BindingMismatch);
        }
        let replacement_version = current
            .reference()
            .version()
            .checked_next()
            .map_err(|_| SecretCompositionError::ContractExhausted)?;
        let replacement_revision = current
            .record_revision()
            .checked_next()
            .map_err(|_| SecretCompositionError::ContractExhausted)?;
        let key = vault.generate_key()?;
        check_key_bytes(&key[..])?;
        let replacement_digest = Blake3Digest32::from_bytes(*blake3::hash(&key[..]).as_bytes());
        let Ok(evidence) = vault.store_blob(&id, &key) else {
            return self.resolve_ambiguous_store(
                vault,
                &id,
                replacement_digest,
                replacement_version,
                replacement_revision,
                operation,
            );
        };
        if evidence.blob_digest != replacement_digest {
            let _ = self.catalog.mark_outcome_unknown(&id, operation);
            return Err(SecretCompositionError::ReadbackMismatch);
        }
        let observed = vault
            .load_blob(&id)?
            .ok_or(SecretCompositionError::EvidenceMissing)?;
        verify_blob_readback(&observed, &evidence)?;
        self.commit_prepared_rotation(
            vault,
            &id,
            PreparedRotation {
                observed,
                evidence: &evidence,
                replacement_version,
                replacement_revision,
                operation,
                recovered: false,
            },
        )
    }

    fn resolve_ambiguous_store(
        &mut self,
        vault: &mut impl PairingVault,
        id: &OpaqueId,
        replacement_digest: Blake3Digest32,
        replacement_version: NonZeroRevision,
        replacement_revision: NonZeroRevision,
        operation: &SecretOperation,
    ) -> Result<MutationOutcome<RotationReceipt>, SecretCompositionError> {
        let observed = vault
            .load_blob(id)
            .map_err(|_| SecretCompositionError::OutcomeUnknown)?;
        let Some(bytes) = observed else {
            return Err(SecretCompositionError::VaultWriteFailed);
        };
        if Blake3Digest32::from_bytes(*blake3::hash(&bytes).as_bytes()) != replacement_digest {
            return Err(SecretCompositionError::VaultWriteFailed);
        }
        let evidence = VaultWriteEvidence {
            blob_digest: replacement_digest,
            receipt: VaultWriteEvidence::receipt_for(replacement_digest)?,
        };
        self.commit_prepared_rotation(
            vault,
            id,
            PreparedRotation {
                observed: bytes,
                evidence: &evidence,
                replacement_version,
                replacement_revision,
                operation,
                recovered: true,
            },
        )
    }

    fn commit_prepared_rotation(
        &mut self,
        vault: &mut impl PairingVault,
        id: &OpaqueId,
        prepared: PreparedRotation<'_>,
    ) -> Result<MutationOutcome<RotationReceipt>, SecretCompositionError> {
        let PreparedRotation {
            observed,
            evidence,
            replacement_version,
            replacement_revision,
            operation,
            recovered,
        } = prepared;
        let payload = EncryptedPayload::new(observed, self.limits.max_ciphertext_bytes)
            .map_err(SecretCompositionError::from)?;
        let effect = self.catalog.prepare_rotation(
            id,
            &self.binding,
            replacement_version,
            payload,
            evidence.blob_digest,
            replacement_revision,
            operation.clone(),
        );
        let (replacement_reference, replacement_operation) = match effect {
            Ok(SecretMutationEffect::WriteEncrypted {
                reference,
                operation,
                ..
            }) => (reference, operation),
            Ok(SecretMutationEffect::Delete { .. }) => {
                return Err(SecretCompositionError::Quarantined);
            }
            Err(error) => return Err(error.into()),
        };
        let readback = SecretWriteReadback {
            reference: replacement_reference,
            ciphertext_digest: evidence.blob_digest,
            record_revision: replacement_revision,
            operation: replacement_operation,
            durable_receipt: Some(evidence.receipt.clone()),
        };
        if let Ok(confirmed) = self.catalog.confirm_rotation(id, &readback) {
            let receipt = RotationReceipt {
                reference: confirmed.reference,
                record_revision: confirmed.record_revision,
                receipt: confirmed.durable_receipt,
            };
            if recovered {
                Ok(MutationOutcome::Recovered(receipt))
            } else {
                Ok(MutationOutcome::Committed(receipt))
            }
        } else {
            let _ = self.catalog.mark_outcome_unknown(id, operation);
            self.recover_rotation(vault, id, operation)
        }
    }

    fn recover_rotation(
        &mut self,
        vault: &mut impl PairingVault,
        id: &OpaqueId,
        operation: &SecretOperation,
    ) -> Result<MutationOutcome<RotationReceipt>, SecretCompositionError> {
        let observed = vault
            .load_blob(id)?
            .ok_or(SecretCompositionError::EvidenceMissing)?;
        if observed.len() != PAIRING_KEY_BYTES {
            self.quarantine_active_record(id);
            return Err(SecretCompositionError::ReadbackMismatch);
        }
        let observed_digest = Blake3Digest32::from_bytes(*blake3::hash(&observed).as_bytes());
        let current = self.catalog.get(id)?.clone();
        let (expected_version, expected_revision) = match current.state() {
            SecretRecordState::RotationPending(_) | SecretRecordState::OutcomeUnknown(_) => {
                let version = current
                    .reference()
                    .version()
                    .checked_next()
                    .map_err(|_| SecretCompositionError::ContractExhausted)?;
                let revision = current
                    .record_revision()
                    .checked_next()
                    .map_err(|_| SecretCompositionError::ContractExhausted)?;
                (version, revision)
            }
            _ => return Err(SecretCompositionError::OutcomeUnknown),
        };
        let expected_reference =
            SecretReference::new(id.clone(), self.binding.clone(), expected_version);
        let receipt = VaultWriteEvidence::receipt_for(observed_digest)?;
        let readback = SecretWriteReadback {
            reference: expected_reference,
            ciphertext_digest: observed_digest,
            record_revision: expected_revision,
            operation: operation.clone(),
            durable_receipt: Some(receipt),
        };
        if let Ok(confirmed) = self.catalog.recover_rotation(id, &readback) {
            Ok(MutationOutcome::Recovered(RotationReceipt {
                reference: confirmed.reference,
                record_revision: confirmed.record_revision,
                receipt: confirmed.durable_receipt,
            }))
        } else {
            self.quarantine_active_record(id);
            Err(SecretCompositionError::OutcomeUnknown)
        }
    }
}
