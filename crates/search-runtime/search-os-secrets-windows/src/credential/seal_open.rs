//! Closed seal/open facade for selected-reference revision encryption.
//!
//! This module owns the finite Windows operation that:
//! 1. Reads the DEK from the Credential Manager (DPAPI unprotect)
//! 2. Reconstructs the expected binding from authoritative committed metadata
//! 3. Compares the reconstructed binding to the envelope's self-reported binding
//! 4. Calls the pure AES-256-GCM seal/open primitive
//!
//! The facade never trusts envelope self-authentication as source identity.
//! The expected binding is always reconstructed from caller-committed metadata
//! and compared before any cryptographic use.

use core::fmt;
use std::time::{Duration, Instant};

use search_control_redb::source_retention::CommittedRevisionSealUse;
use search_control_redb::source_retention::RevisionSealMetadata;
use search_os_secrets::ValidatedSecretsConfig;
use search_revision_crypto::{
    SelectedRevisionBindingV2, SelectedRevisionEnvelopeV2, SelectedPlaintextV2,
    SelectedRevisionErrorV2, open_selected_revision_v2, seal_selected_revision_v2,
};

use super::RevisionDekContext;
use crate::{RevisionDekError, read_revision_dek_key};

/// Closed failure from the seal/open facade.
pub enum RevisionSealOpenError<E> {
    /// The caller context refused the next operation.
    Context(E),
    /// The DEK read or validation failed.
    Dek(RevisionDekError<E>),
    /// The reconstructed binding does not match the envelope's binding.
    BindingMismatch,
    /// The crypto primitive refused the operation.
    Crypto(SelectedRevisionErrorV2),
    /// The operation deadline was exceeded before starting.
    DeadlineExceeded,
}

impl<E> RevisionSealOpenError<E> {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Context(_) => "REVISION_SEAL_OPEN_CONTEXT_REFUSED",
            Self::Dek(_) => "REVISION_SEAL_OPEN_DEK_FAILED",
            Self::BindingMismatch => "REVISION_SEAL_OPEN_BINDING_MISMATCH",
            Self::Crypto(_) => "REVISION_SEAL_OPEN_CRYPTO_FAILED",
            Self::DeadlineExceeded => "REVISION_SEAL_OPEN_DEADLINE_EXCEEDED",
        }
    }
}

impl<E> fmt::Debug for RevisionSealOpenError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl<E> fmt::Display for RevisionSealOpenError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// Computes the private deadline from config TTL, clamped to original.
///
/// Returns None if the deadline has already passed.
fn compute_seal_deadline(
    config: &ValidatedSecretsConfig,
    original: Instant,
) -> Option<Instant> {
    let ttl = Duration::from_millis(config.lease_ttl_ms());
    let now = Instant::now();
    let ttl_deadline = now.checked_add(ttl)?;
    let deadline = ttl_deadline.min(original);
    if deadline > now {
        Some(deadline)
    } else {
        None
    }
}

/// Seals plaintext under the committed permit and DEK.
///
/// Reconstructs the expected binding from permit metadata, compares AAD,
/// then calls the pure seal primitive. The DEK is read from the Credential
/// Manager and zeroized after use.
///
/// # Errors
/// Returns [`RevisionSealOpenError::DeadlineExceeded`] if the TTL deadline
/// has passed. Returns [`RevisionSealOpenError::BindingMismatch`] if the
/// reconstructed AAD does not match the permit's committed AAD.
pub fn seal_revision<C: RevisionDekContext>(
    permit: &CommittedRevisionSealUse,
    plaintext: &[u8],
    dek_binding: &search_os_secrets::RevisionDekBinding,
    config: &ValidatedSecretsConfig,
    context: &mut C,
) -> Result<Vec<u8>, RevisionSealOpenError<C::Error>> {
    let original = context.deadline();
    let _deadline = compute_seal_deadline(config, original)
        .ok_or(RevisionSealOpenError::DeadlineExceeded)?;

    let key_guard = read_revision_dek_key(dek_binding, context)
        .map_err(RevisionSealOpenError::Dek)?
        .ok_or(RevisionDekError::CredentialReadbackInvalid)
        .map_err(RevisionSealOpenError::Dek)?;

    let metadata = permit.metadata();
    let expected = SelectedRevisionBindingV2::from_owner_records(
        permit.selected_current_canonical(),
        &metadata.preseal_occurrence_canonical,
        &metadata.residency_canonical,
        &metadata.operation_id,
        permit.counter(),
        metadata.content_digest,
        metadata.plaintext_length,
    )
    .map_err(RevisionSealOpenError::Crypto)?;

    if expected.aad_bytes() != metadata.aad_canonical.as_slice() {
        return Err(RevisionSealOpenError::BindingMismatch);
    }

    let envelope = seal_selected_revision_v2(
        key_guard.as_bytes(),
        &expected,
        plaintext,
    )
    .map_err(RevisionSealOpenError::Crypto)?;

    envelope.encode().map_err(RevisionSealOpenError::Crypto)
}

/// Opens an envelope under the expected binding and DEK.
///
/// Reconstructs the expected binding from caller-committed metadata,
/// decodes the envelope, compares the full binding, then calls the pure
/// open primitive. The DEK is read from the Credential Manager and zeroized
/// after use.
///
/// # Errors
/// Returns [`RevisionSealOpenError::BindingMismatch`] if the reconstructed
/// binding does not match the envelope's binding.
pub fn open_revision<C: RevisionDekContext>(
    envelope_bytes: &[u8],
    expected: &RevisionSealMetadata,
    expected_current: &[u8],
    expected_counter: u64,
    dek_binding: &search_os_secrets::RevisionDekBinding,
    config: &ValidatedSecretsConfig,
    context: &mut C,
) -> Result<SelectedPlaintextV2, RevisionSealOpenError<C::Error>> {
    let original = context.deadline();
    let _deadline = compute_seal_deadline(config, original)
        .ok_or(RevisionSealOpenError::DeadlineExceeded)?;

    let key_guard = read_revision_dek_key(dek_binding, context)
        .map_err(RevisionSealOpenError::Dek)?
        .ok_or(RevisionDekError::CredentialReadbackInvalid)
        .map_err(RevisionSealOpenError::Dek)?;

    let envelope = SelectedRevisionEnvelopeV2::decode(envelope_bytes)
        .map_err(RevisionSealOpenError::Crypto)?;

    let expected = SelectedRevisionBindingV2::from_owner_records(
        expected_current,
        &expected.preseal_occurrence_canonical,
        &expected.residency_canonical,
        &expected.operation_id,
        expected_counter,
        expected.content_digest,
        expected.plaintext_length,
    )
    .map_err(RevisionSealOpenError::Crypto)?;

    if expected != *envelope.binding() {
        return Err(RevisionSealOpenError::BindingMismatch);
    }

    open_selected_revision_v2(
        key_guard.as_bytes(),
        &expected,
        &envelope,
    )
    .map_err(RevisionSealOpenError::Crypto)
}
