//! Revocation preparation, absence proof and bounded ambiguous-delete recovery.

use search_contracts::{Blake3Digest32, OpaqueId};
use search_os_secrets::{
    SecretDeleteReadback, SecretError, SecretMutationEffect, SecretOperation,
    SecretRecordState,
};

use super::composer::PairingSecretComposer;
use super::receipts::{MutationOutcome, RevocationReceipt};
use super::spec::{MAX_RECOVERY_ATTEMPTS, SecretCompositionError};
use super::vault::{PairingVault, VaultWriteEvidence};

impl PairingSecretComposer {
    /// Revokes the active secret, proving absence by exact readback.
    ///
    /// An ambiguous delete is recovered by bounded re-read/retry and reported
    /// as [`MutationOutcome::Recovered`]; a surviving blob quarantines instead
    /// of being relabeled deleted.
    pub fn revoke(
        &mut self,
        vault: &mut impl PairingVault,
        operation: &SecretOperation,
    ) -> Result<MutationOutcome<RevocationReceipt>, SecretCompositionError> {
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
        let effect = self
            .catalog
            .prepare_delete(&id, &self.binding, operation.clone());
        let pending_operation = match effect {
            Ok(SecretMutationEffect::Delete { operation, .. }) => operation,
            Ok(SecretMutationEffect::WriteEncrypted { .. }) | Err(_) => {
                return Err(SecretCompositionError::InvalidTransition);
            }
        };
        if vault.remove_blob(&id).is_err() {
            let _ = self.catalog.mark_outcome_unknown(&id, &pending_operation);
            return self.recover_delete(vault, &id, &pending_operation);
        }
        self.confirm_absent_delete(vault, &id, &pending_operation, false)
    }

    fn recover_delete(
        &mut self,
        vault: &mut impl PairingVault,
        id: &OpaqueId,
        operation: &SecretOperation,
    ) -> Result<MutationOutcome<RevocationReceipt>, SecretCompositionError> {
        for _ in 0..MAX_RECOVERY_ATTEMPTS {
            match vault.load_blob(id) {
                Ok(None) => return self.confirm_absent_delete(vault, id, operation, true),
                Ok(Some(_)) => {
                    let _ = vault.remove_blob(id);
                }
                Err(_) => {}
            }
        }
        if vault.load_blob(id).is_ok_and(|blob| blob.is_none()) {
            self.confirm_absent_delete(vault, id, operation, true)
        } else {
            self.quarantine_active_record(id);
            Err(SecretCompositionError::OutcomeUnknown)
        }
    }

    fn confirm_absent_delete(
        &mut self,
        vault: &mut impl PairingVault,
        id: &OpaqueId,
        operation: &SecretOperation,
        recovered: bool,
    ) -> Result<MutationOutcome<RevocationReceipt>, SecretCompositionError> {
        if vault.load_blob(id)?.is_some() {
            let _ = self.catalog.mark_outcome_unknown(id, operation);
            return self.recover_delete(vault, id, operation);
        }
        let receipt = VaultWriteEvidence::receipt_for(Blake3Digest32::from_bytes([0xA5; 32]))?;
        let readback = SecretDeleteReadback {
            reference_absent: true,
            operation: operation.clone(),
            durable_receipt: Some(receipt),
        };
        let confirmed = match self.catalog.confirm_delete(id, &readback) {
            Ok(confirmed) => confirmed,
            Err(SecretError::InvalidTransition) => self
                .catalog
                .recover_delete(id, &readback)
                .map_err(SecretCompositionError::from)?,
            Err(error) => return Err(error.into()),
        };
        self.active_id = None;
        let receipt_out = RevocationReceipt {
            reference: confirmed.reference,
            record_revision: confirmed.record_revision,
            receipt: confirmed.durable_receipt,
        };
        if recovered {
            Ok(MutationOutcome::Recovered(receipt_out))
        } else {
            Ok(MutationOutcome::Committed(receipt_out))
        }
    }
}
