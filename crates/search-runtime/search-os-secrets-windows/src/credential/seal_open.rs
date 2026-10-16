//! Finite selected-reference key use; no raw-key or callback escape.
use core::fmt;
use std::time::{Duration, Instant};
use search_control_redb::source_retention::{CommittedRevisionSealUse, RevisionSealMetadata};
use search_os_secrets::{RevisionDekCurrent, RevisionDekSelectionRecord, ValidatedSecretsConfig};
use search_revision_crypto::{SelectedPlaintextV2, SelectedRevisionBindingV2,
    SelectedRevisionEnvelopeV2, SelectedRevisionErrorV2, open_selected_revision_v2, seal_selected_revision_v2};
use super::RevisionDekContext;
use crate::{LegacyRevisionDpapiContext, LegacyRevisionDpapiWork as Work, RevisionDekError,
    observe_revision_user, read_revision_dek_key};

mod derived;
pub use derived::*;

/// Original caller refusal or expiration of the private one-use key window.
pub enum RevisionLeaseRefusal<E> {
    /// The original refusal; cleanup still uses that same allowance.
    Original(E),
    /// The private window ended; it is never renewed.
    Expired,
}
impl<E> fmt::Debug for RevisionLeaseRefusal<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self { Self::Original(_) => "REVISION_KEY_ORIGINAL_CONTEXT_REFUSED", Self::Expired => "REVISION_KEY_TTL_EXPIRED" })
    }
}

/// Closed failure; rendering and the error chain expose no bytes.
pub enum RevisionSealOpenError<E> {
    /// Original allowance or private deadline refused work.
    Context(RevisionLeaseRefusal<E>),
    /// Exact selected key/material/backend validation failed.
    Dek(RevisionDekError<RevisionLeaseRefusal<E>>),
    /// Metadata, selected settings or complete AAD do not match.
    BindingMismatch,
    /// The pure crypto owner refused the operation.
    Crypto(SelectedRevisionErrorV2),
    /// No finite remaining key window exists.
    DeadlineExceeded,
    /// Bounded preparation cannot be represented without overflow.
    SizeLimit,
}
impl<E> RevisionSealOpenError<E> {
    /// Stable content-free code; detailed original failures stay typed fields.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Context(RevisionLeaseRefusal::Expired) | Self::DeadlineExceeded => "REVISION_SEAL_OPEN_DEADLINE_EXCEEDED",
            Self::Context(_) => "REVISION_SEAL_OPEN_CONTEXT_REFUSED",
            Self::Dek(_) => "REVISION_SEAL_OPEN_DEK_FAILED",
            Self::BindingMismatch => "REVISION_SEAL_OPEN_BINDING_MISMATCH",
            Self::Crypto(_) => "REVISION_SEAL_OPEN_CRYPTO_FAILED",
            Self::SizeLimit => "REVISION_SEAL_OPEN_SIZE_LIMIT",
        }
    }
}
impl<E> fmt::Debug for RevisionSealOpenError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.code()) }
}
impl<E> fmt::Display for RevisionSealOpenError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.code()) }
}

// Only closed operations construct this window. No caller time, reusable lease,
// deadline setter or new meter. Credential waits see the private deadline.
struct KeyWindow<'a, C> { original: &'a mut C, deadline: Instant, expired: bool }
impl<'a, C: RevisionDekContext> KeyWindow<'a, C> {
    fn new(original: &'a mut C, config: &ValidatedSecretsConfig) -> Result<Self, RevisionSealOpenError<C::Error>> {
        let now = Instant::now();
        let ttl = now.checked_add(Duration::from_millis(config.lease_ttl_ms()))
            .ok_or(RevisionSealOpenError::DeadlineExceeded)?;
        let deadline = ttl.min(original.deadline());
        if config.lease_ttl_ms() == 0 || deadline <= now { return Err(RevisionSealOpenError::DeadlineExceeded); }
        Ok(Self { original, deadline, expired: false })
    }
    fn poll_expiry(&mut self) -> Result<(), RevisionLeaseRefusal<C::Error>> {
        self.expired |= Instant::now() >= self.deadline;
        if self.expired { Err(RevisionLeaseRefusal::Expired) } else { Ok(()) }
    }
}
impl<C: RevisionDekContext> LegacyRevisionDpapiContext for KeyWindow<'_, C> {
    type Error = RevisionLeaseRefusal<C::Error>;
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        self.poll_expiry()?;
        self.original.checkpoint().map_err(RevisionLeaseRefusal::Original)?;
        self.poll_expiry()
    }
    fn charge(&mut self, work: Work) -> Result<(), Self::Error> {
        self.checkpoint()?;
        self.original.charge(work).map_err(RevisionLeaseRefusal::Original)?;
        self.poll_expiry()
    }
    fn account_native_clear(&mut self, actual_bytes: usize) -> Result<(), Self::Error> {
        // TTL/cancellation never gates mandatory native clearing.
        self.original.account_native_clear(actual_bytes).map_err(RevisionLeaseRefusal::Original)
    }
}
impl<C: RevisionDekContext> RevisionDekContext for KeyWindow<'_, C> {
    fn deadline(&self) -> Instant { self.deadline }
    fn charge_open(&mut self) -> Result<(), Self::Error> {
        self.checkpoint()?;
        self.original.charge_open().map_err(RevisionLeaseRefusal::Original)?;
        self.poll_expiry()
    }
}

fn current<E>(encoded: &[u8], config: &ValidatedSecretsConfig) -> Result<RevisionDekCurrent, RevisionSealOpenError<E>> {
    let RevisionDekSelectionRecord::Current(current) = search_os_secrets::decode_revision_dek_selection_record(encoded)
        .map_err(|_| RevisionSealOpenError::BindingMismatch)? else { return Err(RevisionSealOpenError::BindingMismatch); };
    if current.intent().policy().backend() != config.backend()
        || current.intent().policy().secrets_section_digest() != *search_os_secrets::section_digest(config).as_bytes() {
        return Err(RevisionSealOpenError::BindingMismatch);
    }
    Ok(current)
}
fn binding<E>(metadata: &RevisionSealMetadata, current: &[u8], counter: u64)
    -> Result<SelectedRevisionBindingV2, RevisionSealOpenError<E>> {
    let expected = SelectedRevisionBindingV2::from_owner_records(current,
        &metadata.preseal_occurrence_canonical, &metadata.residency_canonical, &metadata.operation_id,
        counter, metadata.content_digest, metadata.plaintext_length).map_err(RevisionSealOpenError::Crypto)?;
    if expected.aad_bytes() != metadata.aad_canonical { return Err(RevisionSealOpenError::BindingMismatch); }
    Ok(expected)
}
fn reserve<C: RevisionDekContext>(window: &mut KeyWindow<'_, C>, metadata: &RevisionSealMetadata,
    current: &[u8], body: usize, envelope: usize,
) -> Result<(), RevisionSealOpenError<C::Error>> {
    let metadata_bytes = current.len().checked_add(metadata.preseal_occurrence_canonical.len())
        .and_then(|n| n.checked_add(metadata.residency_canonical.len()))
        .and_then(|n| n.checked_add(metadata.aad_canonical.len())).ok_or(RevisionSealOpenError::SizeLimit)?;
    let allocation = body.checked_mul(5).and_then(|n| envelope.checked_mul(3).and_then(|e| n.checked_add(e)))
        .and_then(|n| metadata_bytes.checked_mul(12).and_then(|m| n.checked_add(m)))
        .and_then(|n| n.checked_add(8192)).ok_or(RevisionSealOpenError::SizeLimit)?;
    let process = allocation.checked_add(body).and_then(|n| n.checked_add(envelope))
        .ok_or(RevisionSealOpenError::SizeLimit)?;
    window.charge(Work::Allocate(allocation)).map_err(RevisionSealOpenError::Context)?;
    window.charge(Work::Process(process)).map_err(RevisionSealOpenError::Context)
}

/// Consumes one fresh C02 permit in one finite exact-key seal operation.
///
/// Complete Current/settings/AAD and prepaid block count are checked before key
/// access. Errors also consume the permit: its nonce stays burned.
/// # Errors
/// Refuses context/TTL, exact material/settings/AAD mismatch or crypto failure.
pub fn seal_revision<C: RevisionDekContext>(permit: CommittedRevisionSealUse, plaintext: &[u8],
    config: &ValidatedSecretsConfig, context: &mut C,
) -> Result<Vec<u8>, RevisionSealOpenError<C::Error>> {
    let mut window = KeyWindow::new(context, config)?;
    reserve(&mut window, permit.metadata(), permit.selected_current_canonical(), plaintext.len(), 0)?;
    let current = current(permit.selected_current_canonical(), config)?;
    let expected = binding(permit.metadata(), permit.selected_current_canonical(), permit.counter())?;
    if expected.authenticated_blocks().map_err(RevisionSealOpenError::Crypto)? != permit.authenticated_blocks() {
        return Err(RevisionSealOpenError::BindingMismatch);
    }
    // Retain only the immutable reconstruction; the capability cannot be used
    // for a second seal, including after a failed native/crypto attempt.
    drop(permit);
    window.checkpoint().map_err(RevisionSealOpenError::Context)?;
    let key = read_revision_dek_key(&current, &mut window).map_err(RevisionSealOpenError::Dek)?
        .ok_or(RevisionSealOpenError::Dek(RevisionDekError::CredentialReadbackInvalid))?;
    let result = seal_selected_revision_v2(key.as_bytes(), &expected, plaintext);
    drop(key);
    window.checkpoint().map_err(RevisionSealOpenError::Context)?;
    let envelope = result.map_err(RevisionSealOpenError::Crypto)?;
    let encoded = envelope.encode().map_err(RevisionSealOpenError::Crypto)?;
    finish(&mut window, &current)?;
    Ok(encoded)
}

/// Opens exact committed-link metadata under one finite selected-key window.
/// # Errors
/// Refuses settings/AAD/envelope/content mismatch or original refusal.
#[allow(clippy::too_many_arguments)]
pub fn open_revision<C: RevisionDekContext>(envelope_bytes: &[u8], metadata: &RevisionSealMetadata,
    current_canonical: &[u8], counter: u64, config: &ValidatedSecretsConfig, context: &mut C,
) -> Result<SelectedPlaintextV2, RevisionSealOpenError<C::Error>> {
    let mut window = KeyWindow::new(context, config)?;
    let body = usize::try_from(metadata.plaintext_length).map_err(|_| RevisionSealOpenError::SizeLimit)?;
    reserve(&mut window, metadata, current_canonical, body, envelope_bytes.len())?;
    let current = current(current_canonical, config)?;
    let expected = binding(metadata, current_canonical, counter)?;
    let envelope = SelectedRevisionEnvelopeV2::decode(envelope_bytes).map_err(RevisionSealOpenError::Crypto)?;
    if &expected != envelope.binding() { return Err(RevisionSealOpenError::BindingMismatch); }
    window.checkpoint().map_err(RevisionSealOpenError::Context)?;
    let key = read_revision_dek_key(&current, &mut window).map_err(RevisionSealOpenError::Dek)?
        .ok_or(RevisionSealOpenError::Dek(RevisionDekError::CredentialReadbackInvalid))?;
    let result = open_selected_revision_v2(key.as_bytes(), &expected, &envelope);
    drop(key);
    window.checkpoint().map_err(RevisionSealOpenError::Context)?;
    let plaintext = result.map_err(RevisionSealOpenError::Crypto)?;
    finish(&mut window, &current)?;
    Ok(plaintext)
}
fn finish<C: RevisionDekContext>(window: &mut KeyWindow<'_, C>, current: &RevisionDekCurrent)
    -> Result<(), RevisionSealOpenError<C::Error>> {
    let actual = observe_revision_user(window).map_err(RevisionSealOpenError::Dek)?;
    if actual.user_scope_digest() != current.intent().candidate().intent().binding().user_scope_digest() {
        return Err(RevisionSealOpenError::BindingMismatch);
    }
    window.checkpoint().map_err(RevisionSealOpenError::Context)
}

#[cfg(test)]
#[path = "seal_open/tests.rs"]
mod tests;
