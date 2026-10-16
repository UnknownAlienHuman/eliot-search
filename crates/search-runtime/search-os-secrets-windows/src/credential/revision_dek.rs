//! Candidate-only Windows persistence for one revision data-encryption key.

use core::fmt;
use std::time::Instant;

use search_os_secrets::RevisionDekBinding as CoreRevisionDekBinding;

#[cfg(windows)]
use super::windows::{
    MAX_CREDENTIAL_BLOB_BYTES, ProviderCredentialBlob, WindowsCredentialPlatform,
    WindowsVaultLock,
};
use super::LegacyRevisionRootSecretError;
use crate::{
    ContextualLegacyRevisionDpapiError, LegacyRevisionDpapiContext, LegacyRevisionDpapiWork,
    protect_revision_dek_with_context, unprotect_revision_dek_with_context,
};

const RECORD_MAGIC: &[u8; 8] = b"ELSDEK01";
const RECORD_HEADER_BYTES: usize = 8 + 32 + 4;
const MAX_PROTECTED_RECORD_BYTES: usize = MAX_CREDENTIAL_BLOB_BYTES - RECORD_HEADER_BYTES;
const TARGET_PREFIX: &str = "ELIOT Search/revision-dek-v1/";
const MUTEX_PREFIX: &str = "Global\\ELIOT-Search-RevisionDek-v1-";
const MAX_USER_TOKEN_BYTES: usize = 1024;

/// The original caller context required by every candidate effect.
pub trait RevisionDekContext: LegacyRevisionDpapiContext {
    /// Original absolute deadline used to bound Credential Manager waits.
    fn deadline(&self) -> Instant;

    /// Reserves one Credential Manager or handle-open/close attempt.
    fn charge_open(&mut self) -> Result<(), Self::Error>;
}

/// Closed candidate-effect failure with no secret or target text in its public form.
pub enum RevisionDekError<E> {
    /// The current target is not a Windows platform.
    UnsupportedPlatform,
    /// The caller context refused the next operation.
    Context(E),
    /// Current-user token/SID observation was malformed or failed.
    UserObservationInvalid,
    /// A thread token was present; this profile refuses impersonation.
    ThreadImpersonationDetected,
    /// The observed user differs from the Core binding.
    UserScopeMismatch,
    /// Native RNG failed or returned an all-zero key.
    RandomGenerationFailed(i32),
    /// The shared contextual DPAPI operation failed.
    Dpapi(ContextualLegacyRevisionDpapiError<E>),
    /// Credential Manager read or validation failed.
    Credential(LegacyRevisionRootSecretError),
    /// The structured candidate frame was malformed or contradicted its binding.
    CredentialReadbackInvalid,
    /// A target already exists; this operation never overwrites it.
    ExistingCredential,
    /// A write may have reached Credential Manager but exact readback failed.
    WriteOutcomeUnknown,
    /// The bounded Credential Manager mutex could not be acquired.
    LockUnavailable,
    /// The requested record exceeds the installed Credential Manager ceiling.
    RecordTooLarge,
    /// A protected DEK did not unwrap to exactly one nonzero 32-byte key.
    ProtectedKeyInvalid,
    /// The Core binding could not provide a bounded protection entropy value.
    BindingInvalid,
}

impl<E> RevisionDekError<E> {
    /// Stable redacted machine-readable reason.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "REVISION_DEK_UNSUPPORTED_PLATFORM",
            Self::Context(_) => "REVISION_DEK_CONTEXT_REFUSED",
            Self::UserObservationInvalid => "REVISION_DEK_USER_OBSERVATION_INVALID",
            Self::ThreadImpersonationDetected => "REVISION_DEK_THREAD_IMPERSONATION_REFUSED",
            Self::UserScopeMismatch => "REVISION_DEK_USER_SCOPE_MISMATCH",
            Self::RandomGenerationFailed(_) => "REVISION_DEK_RNG_FAILED",
            Self::Dpapi(_) => "REVISION_DEK_DPAPI_FAILED",
            Self::Credential(_) => "REVISION_DEK_CREDENTIAL_FAILED",
            Self::CredentialReadbackInvalid => "REVISION_DEK_CREDENTIAL_READBACK_INVALID",
            Self::ExistingCredential => "REVISION_DEK_CREDENTIAL_EXISTS",
            Self::WriteOutcomeUnknown => "REVISION_DEK_WRITE_OUTCOME_UNKNOWN",
            Self::LockUnavailable => "REVISION_DEK_LOCK_UNAVAILABLE",
            Self::RecordTooLarge => "REVISION_DEK_RECORD_TOO_LARGE",
            Self::ProtectedKeyInvalid => "REVISION_DEK_PROTECTED_KEY_INVALID",
            Self::BindingInvalid => "REVISION_DEK_BINDING_INVALID",
        }
    }
}

impl<E> fmt::Debug for RevisionDekError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl<E> fmt::Display for RevisionDekError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// Opaque observation of the Windows user that owns the current process token.
pub struct RevisionUserObservation {
    user_scope_digest: [u8; 32],
}

impl RevisionUserObservation {
    /// Digest of the observed current-user SID under the Core recipe.
    #[must_use]
    pub const fn user_scope_digest(&self) -> [u8; 32] {
        self.user_scope_digest
    }
}

/// Prepared DPAPI-wrapped DEK candidate. The raw 32-byte key is never exposed.
pub struct PreparedRevisionDek {
    binding_digest: [u8; 32],
    reference_digest: [u8; 32],
    user_scope_digest: [u8; 32],
    entropy: Vec<u8>,
    protected: Vec<u8>,
}

impl PreparedRevisionDek {
    /// Exact DPAPI ciphertext bytes retained for one immediate write.
    #[must_use]
    pub fn protected_bytes(&self) -> &[u8] {
        &self.protected
    }
}

impl Drop for PreparedRevisionDek {
    fn drop(&mut self) {
        clear_bytes(&mut self.entropy);
        clear_bytes(&mut self.protected);
    }
}

/// Exact protected bytes observed from the structured Credential Manager record.
pub struct RevisionDekReadback {
    protected: Vec<u8>,
}

impl RevisionDekReadback {
    /// Exact DPAPI ciphertext bytes observed after a strict frame readback.
    #[must_use]
    pub fn protected_bytes(&self) -> &[u8] {
        &self.protected
    }
}

impl Drop for RevisionDekReadback {
    fn drop(&mut self) {
        clear_bytes(&mut self.protected);
    }
}

/// Observes the actual current user and refuses thread impersonation.
pub fn observe_revision_user<C: RevisionDekContext>(
    context: &mut C,
) -> Result<RevisionUserObservation, RevisionDekError<C::Error>> {
    #[cfg(windows)]
    {
        platform::observe(context)
    }
    #[cfg(not(windows))]
    {
        let _ = context;
        Err(RevisionDekError::UnsupportedPlatform)
    }
}

/// Generates and DPAPI-wraps one fresh 32-byte candidate without serving it.
pub fn prepare_revision_dek<C: RevisionDekContext>(
    binding: &CoreRevisionDekBinding,
    context: &mut C,
) -> Result<PreparedRevisionDek, RevisionDekError<C::Error>> {
    let observed = observe_revision_user(context)?;
    if observed.user_scope_digest() != binding.user_scope_digest() {
        return Err(RevisionDekError::UserScopeMismatch);
    }
    context.charge(LegacyRevisionDpapiWork::Allocate(crate::MAX_SCOPE_BYTES)).map_err(RevisionDekError::Context)?;
    context.charge(LegacyRevisionDpapiWork::Process(crate::MAX_SCOPE_BYTES.saturating_mul(2))).map_err(RevisionDekError::Context)?;
    let entropy = binding.protection_entropy();
    if entropy.is_empty() || entropy.len() > crate::MAX_SCOPE_BYTES {
        return Err(RevisionDekError::BindingInvalid);
    }
    context
        .charge(LegacyRevisionDpapiWork::Process(entropy.len()))
        .map_err(RevisionDekError::Context)?;
    #[cfg(windows)]
    let key = super::windows::WindowsCredentialPlatform::generate_revision_dek_key(context)?;
    #[cfg(not(windows))]
    let key = return Err(RevisionDekError::UnsupportedPlatform);
    let protected = match protect_revision_dek_with_context(context, key.bytes(), &entropy) {
        Ok(protected) => protected,
        Err(error) => {
            return Err(RevisionDekError::Dpapi(error));
        }
    };
    drop(key);
    if protected.is_empty() || protected.len() > MAX_PROTECTED_RECORD_BYTES {
        return Err(RevisionDekError::RecordTooLarge);
    }
    Ok(PreparedRevisionDek {
        binding_digest: binding.binding_digest(),
        reference_digest: binding.reference_digest(),
        user_scope_digest: binding.user_scope_digest(),
        entropy,
        protected,
    })
}

/// Writes one candidate record once and verifies the exact stored frame.
#[cfg(windows)]
pub fn write_revision_dek_once<C: RevisionDekContext>(
    prepared: &PreparedRevisionDek,
    context: &mut C,
) -> Result<RevisionDekReadback, RevisionDekError<C::Error>> {
    let observed = observe_revision_user(context)?;
    if observed.user_scope_digest() != prepared.user_scope_digest {
        return Err(RevisionDekError::UserScopeMismatch);
    }
    charge_name_budget(context, TARGET_PREFIX.len())?;
    charge_name_budget(context, MUTEX_PREFIX.len())?;
    let frame_bytes = RECORD_HEADER_BYTES
        .checked_add(prepared.protected.len())
        .ok_or(RevisionDekError::RecordTooLarge)?;
    context
        .charge(LegacyRevisionDpapiWork::Allocate(frame_bytes))
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::Process(frame_bytes.saturating_mul(2)))
        .map_err(RevisionDekError::Context)?;
    let frame = encode_frame(prepared.binding_digest, &prepared.protected)?;
    let mut record = ProviderCredentialBlob::new(frame).map_err(RevisionDekError::Credential)?;
    let target = target_name(prepared.reference_digest);
    let mutex = mutex_name(prepared.reference_digest);
    let lock = acquire_lock(context, &mutex)?;
    let existing = match read_credential(context, &target) {
        Ok(existing) => existing,
        Err(error) => {
            let _ = release_lock(context, lock);
            return Err(error);
        }
    };
    if existing.is_some() {
        let _ = release_lock(context, lock);
        return Err(RevisionDekError::ExistingCredential);
    }
    if let Err(error) = context.charge(LegacyRevisionDpapiWork::NativeCall) {
        let _ = release_lock(context, lock);
        return Err(RevisionDekError::Context(error));
    }
    let written = WindowsCredentialPlatform::write_revision_dek_credential(&target, &mut record, context);
    if written.is_err() {
        let _ = release_lock(context, lock);
        return Err(RevisionDekError::WriteOutcomeUnknown);
    }
    if context.checkpoint().is_err() {
        let _ = release_lock(context, lock);
        return Err(RevisionDekError::WriteOutcomeUnknown);
    }
    let Ok(Some(observed)) = read_credential(context, &target) else {
        let _ = release_lock(context, lock);
        return Err(RevisionDekError::WriteOutcomeUnknown);
    };
    if observed.as_slice() != record.as_slice() {
        let _ = release_lock(context, lock);
        return Err(RevisionDekError::WriteOutcomeUnknown);
    }
    let Ok(protected) = validate_frame_and_unwrap(
        context,
        observed.as_slice(),
        prepared.binding_digest,
        &prepared.entropy,
    ) else {
        let _ = release_lock(context, lock);
        return Err(RevisionDekError::WriteOutcomeUnknown);
    };
    if release_lock(context, lock).is_err() { return Err(RevisionDekError::WriteOutcomeUnknown); }
    Ok(RevisionDekReadback {
        protected,
    })
}

/// Writes one candidate record once and verifies the exact stored frame.
#[cfg(not(windows))]
pub fn write_revision_dek_once<C: RevisionDekContext>(
    _prepared: &PreparedRevisionDek,
    _context: &mut C,
) -> Result<RevisionDekReadback, RevisionDekError<C::Error>> {
    Err(RevisionDekError::UnsupportedPlatform)
}

/// Reads one exact candidate and validates its binding and wrapped-key shape.
#[cfg(windows)]
pub fn read_revision_dek<C: RevisionDekContext>(
    binding: &CoreRevisionDekBinding,
    context: &mut C,
) -> Result<Option<RevisionDekReadback>, RevisionDekError<C::Error>> {
    let observed = observe_revision_user(context)?;
    if observed.user_scope_digest() != binding.user_scope_digest() {
        return Err(RevisionDekError::UserScopeMismatch);
    }
    charge_name_budget(context, TARGET_PREFIX.len())?;
    let target = target_name(binding.reference_digest());
    let Some(frame) = read_credential(context, &target)? else {
        return Ok(None);
    };
    context.charge(LegacyRevisionDpapiWork::Allocate(crate::MAX_SCOPE_BYTES)).map_err(RevisionDekError::Context)?;
    context.charge(LegacyRevisionDpapiWork::Process(crate::MAX_SCOPE_BYTES.saturating_mul(2))).map_err(RevisionDekError::Context)?;
    let mut entropy = binding.protection_entropy();
    if entropy.is_empty() || entropy.len() > crate::MAX_SCOPE_BYTES {
        return Err(RevisionDekError::BindingInvalid);
    }
    let protected = validate_frame_and_unwrap(
        context,
        frame.as_slice(),
        binding.binding_digest(),
        &entropy,
    );
    clear_bytes(&mut entropy);
    let protected = protected?;
    Ok(Some(RevisionDekReadback {
        protected,
    }))
}

/// Reads one exact candidate and validates its binding and wrapped-key shape.
#[cfg(not(windows))]
pub fn read_revision_dek<C: RevisionDekContext>(
    _binding: &CoreRevisionDekBinding,
    _context: &mut C,
) -> Result<Option<RevisionDekReadback>, RevisionDekError<C::Error>> {
    Err(RevisionDekError::UnsupportedPlatform)
}

/// Reads, unwraps and validates the DEK, returning a zeroizing key guard.
///
/// This is the key-returning path for the closed seal/open consumer.
/// It performs the same read and validation as `read_revision_dek` but
/// returns the validated key guard instead of the protected bytes.
/// The caller must consume the guard within the finite operation deadline.
#[cfg(windows)]
pub(crate) fn read_revision_dek_key<C: RevisionDekContext>(
    current: &search_os_secrets::RevisionDekCurrent,
    context: &mut C,
) -> Result<Option<ValidatedRevisionDekKey>, RevisionDekError<C::Error>> {
    let candidate = current.intent().candidate();
    let binding = candidate.intent().binding();
    let observed = observe_revision_user(context)?;
    if observed.user_scope_digest() != binding.user_scope_digest() {
        return Err(RevisionDekError::UserScopeMismatch);
    }
    charge_name_budget(context, TARGET_PREFIX.len())?;
    let target = target_name(binding.reference_digest());
    let Some(frame) = read_credential(context, &target)? else {
        return Ok(None);
    };
    // Validate THIS read against the exact selected protected material. The
    // credential header is never passed to CryptUnprotectData.
    let protected = decode_frame::<C::Error>(frame.as_slice(), binding.binding_digest())?;
    context.charge(LegacyRevisionDpapiWork::Allocate(2 * search_os_secrets::REVISION_DEK_MAX_RECORD_BYTES))
        .map_err(RevisionDekError::Context)?;
    context.charge(LegacyRevisionDpapiWork::Process(protected.len() + 4 * search_os_secrets::REVISION_DEK_MAX_RECORD_BYTES))
        .map_err(RevisionDekError::Context)?;
    let actual = search_os_secrets::RevisionDekCandidate::new(candidate.intent(), binding, protected)
        .map_err(|_| RevisionDekError::CredentialReadbackInvalid)?;
    if &actual != candidate { return Err(RevisionDekError::CredentialReadbackInvalid); }
    context.charge(LegacyRevisionDpapiWork::Allocate(crate::MAX_SCOPE_BYTES)).map_err(RevisionDekError::Context)?;
    context.charge(LegacyRevisionDpapiWork::Process(crate::MAX_SCOPE_BYTES.saturating_mul(2))).map_err(RevisionDekError::Context)?;
    let mut entropy = binding.protection_entropy();
    if entropy.is_empty() || entropy.len() > crate::MAX_SCOPE_BYTES {
        return Err(RevisionDekError::BindingInvalid);
    }
    let guard = unwrap_and_validate_revision_dek(context, protected, &entropy);
    clear_bytes(&mut entropy);
    let guard = guard?;
    context.checkpoint().map_err(RevisionDekError::Context)?;
    Ok(Some(guard))
}

/// Reads, unwraps and validates the DEK, returning a zeroizing key guard.
#[cfg(not(windows))]
pub(crate) fn read_revision_dek_key<C: RevisionDekContext>(
    _current: &search_os_secrets::RevisionDekCurrent,
    _context: &mut C,
) -> Result<Option<ValidatedRevisionDekKey>, RevisionDekError<C::Error>> {
    Err(RevisionDekError::UnsupportedPlatform)
}

#[cfg(feature = "test-credential-cleanup")]
/// Deletes one exact candidate target and proves its absence for native tests.
#[cfg(windows)]
pub fn delete_revision_dek_for_test<C: RevisionDekContext>(
    binding: &CoreRevisionDekBinding,
    context: &mut C,
) -> Result<(), RevisionDekError<C::Error>> {
    let observed = observe_revision_user(context)?;
    if observed.user_scope_digest() != binding.user_scope_digest() {
        return Err(RevisionDekError::UserScopeMismatch);
    }
    charge_name_budget(context, TARGET_PREFIX.len())?;
    charge_name_budget(context, MUTEX_PREFIX.len())?;
    let target = target_name(binding.reference_digest());
    let mutex = mutex_name(binding.reference_digest());
    let lock = acquire_lock(context, &mutex)?;
    context
        .charge(LegacyRevisionDpapiWork::NativeCall)
        .map_err(RevisionDekError::Context)?;
    let _ = WindowsCredentialPlatform::delete_credential(&target);
    let absent = read_credential(context, &target)?.is_none();
    release_lock(context, lock).map_err(RevisionDekError::Context)?;
    if absent { Ok(()) } else { Err(RevisionDekError::WriteOutcomeUnknown) }
}

#[cfg(all(feature = "test-credential-cleanup", not(windows)))]
/// Deletes one exact candidate target and proves its absence for native tests.
pub fn delete_revision_dek_for_test<C: RevisionDekContext>(
    _binding: &CoreRevisionDekBinding,
    _context: &mut C,
) -> Result<(), RevisionDekError<C::Error>> {
    Err(RevisionDekError::UnsupportedPlatform)
}

fn encode_frame<E>(
    binding_digest: [u8; 32],
    protected: &[u8],
) -> Result<Vec<u8>, RevisionDekError<E>> {
    if protected.is_empty() || protected.len() > MAX_PROTECTED_RECORD_BYTES {
        return Err(RevisionDekError::RecordTooLarge);
    }
    let length = u32::try_from(protected.len()).map_err(|_| RevisionDekError::RecordTooLarge)?;
    let mut frame = Vec::with_capacity(RECORD_HEADER_BYTES + protected.len());
    frame.extend_from_slice(RECORD_MAGIC);
    frame.extend_from_slice(&binding_digest);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(protected);
    Ok(frame)
}

fn decode_frame<E>(
    frame: &[u8],
    expected_binding: [u8; 32],
) -> Result<&[u8], RevisionDekError<E>> {
    if frame.len() < RECORD_HEADER_BYTES
        || frame.len() > MAX_CREDENTIAL_BLOB_BYTES
        || frame.get(..8) != Some(RECORD_MAGIC)
        || &frame[8..40] != expected_binding.as_slice()
    {
        return Err(RevisionDekError::CredentialReadbackInvalid);
    }
    let length = usize::try_from(u32::from_be_bytes(
        frame[40..44].try_into().map_err(|_| RevisionDekError::CredentialReadbackInvalid)?,
    ))
    .map_err(|_| RevisionDekError::CredentialReadbackInvalid)?;
    if length == 0 || frame.len() != RECORD_HEADER_BYTES + length {
        return Err(RevisionDekError::CredentialReadbackInvalid);
    }
    Ok(&frame[RECORD_HEADER_BYTES..])
}

/// Private non-Clone zeroizing guard for an unwrapped revision DEK.
///
/// This guard holds the raw 32-byte key returned by DPAPI unprotect and
/// guarantees it is nonzero and exactly 32 bytes. It zeroizes on drop and
/// cannot be cloned, preventing accidental key duplication. The guard is
/// used only inside the existing read-validation path; it grants no key
/// export, lease or durable authority.
pub(crate) struct ValidatedRevisionDekKey {
    bytes: [u8; 32],
    #[cfg(test)]
    cleared: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl ValidatedRevisionDekKey {
    /// Validates the unwrapped key bytes and returns a zeroizing guard.
    ///
    /// # Errors
    /// Returns [`RevisionDekError::ProtectedKeyInvalid`] if the key is not
    /// exactly 32 bytes or is the all-zero value.
    fn from_unwrapped(mut key: Vec<u8>) -> Result<Self, RevisionDekError<core::convert::Infallible>> {
        copy_and_clear(key.as_mut_slice())
    }

    /// Borrows the validated key bytes for the immediate AEAD call.
    ///
    /// This grants no ownership or clone; the guard retains exclusive access.
    /// Used by the closed seal/open consumer once the C02 permit interface lands.
    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
}

/// Private slice helper: validates, copies to [u8; 32], clears source slice.
///
/// Called from `from_unwrapped` with the raw Vec's mutable slice.
/// Tests call this directly with a mutable buffer to prove cleanup.
fn copy_and_clear(key: &mut [u8]) -> Result<ValidatedRevisionDekKey, RevisionDekError<core::convert::Infallible>> {
    if key.len() != 32 || key.iter().all(|byte| *byte == 0) {
        clear_bytes(key);
        return Err(RevisionDekError::ProtectedKeyInvalid);
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(key);
    clear_bytes(key);
    Ok(ValidatedRevisionDekKey {
        bytes,
        #[cfg(test)]
        cleared: None,
    })
}

impl Drop for ValidatedRevisionDekKey {
    fn drop(&mut self) {
        clear_bytes(&mut self.bytes);
        #[cfg(test)]
        if let Some(witness) = &self.cleared {
            witness.store(self.bytes.iter().all(|byte| *byte == 0), std::sync::atomic::Ordering::Release);
        }
    }
}

impl fmt::Debug for ValidatedRevisionDekKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ValidatedRevisionDekKey(<redacted>)")
    }
}

/// Unwraps and validates the protected DEK, returning a zeroizing guard.
///
/// This is the single factorization point for the existing read-validation
/// path. It performs the native unprotect call, validates the key shape and
/// nonzero constraint, and returns a guard that zeroizes on drop. The caller
/// retains the protected bytes separately for the readback result.
fn unwrap_and_validate_revision_dek<C: RevisionDekContext>(
    context: &mut C,
    protected: &[u8],
    entropy: &[u8],
) -> Result<ValidatedRevisionDekKey, RevisionDekError<C::Error>> {
    // Prepay clearing even an unexpected, bounded short-secret output before
    // native unprotect; only an exact nonzero 32-byte DEK can pass below.
    context.charge(LegacyRevisionDpapiWork::Process(crate::MAX_SECRET_BYTES)).map_err(RevisionDekError::Context)?;
    let key = unprotect_revision_dek_with_context(context, protected, entropy)
        .map_err(RevisionDekError::Dpapi)?;
    ValidatedRevisionDekKey::from_unwrapped(key).map_err(|_| RevisionDekError::ProtectedKeyInvalid)
}

fn validate_frame_and_unwrap<C: RevisionDekContext>(
    context: &mut C,
    frame: &[u8],
    binding_digest: [u8; 32],
    entropy: &[u8],
) -> Result<Vec<u8>, RevisionDekError<C::Error>> {
    let protected = decode_frame::<()>(frame, binding_digest)
        .map_err(|_| RevisionDekError::CredentialReadbackInvalid)?;
    let _guard = unwrap_and_validate_revision_dek(context, protected, entropy)?;
    context
        .charge(LegacyRevisionDpapiWork::Allocate(protected.len()))
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::Process(protected.len().saturating_mul(2)))
        .map_err(RevisionDekError::Context)?;
    Ok(protected.to_vec())
}

#[cfg(windows)]
fn read_credential<C: RevisionDekContext>(
    context: &mut C,
    target: &[u16],
) -> Result<Option<ProviderCredentialBlob>, RevisionDekError<C::Error>> {
    context
        .checkpoint()
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::NativeCall)
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::Allocate(MAX_CREDENTIAL_BLOB_BYTES))
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::Process(MAX_CREDENTIAL_BLOB_BYTES.saturating_mul(3)))
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::NativeCall)
        .map_err(RevisionDekError::Context)?;
    let result = WindowsCredentialPlatform::read_provider_credential(target)
        .map_err(RevisionDekError::Credential)?;
    context
        .checkpoint()
        .map_err(RevisionDekError::Context)?;
    Ok(result)
}

#[cfg(windows)]
fn acquire_lock<C: RevisionDekContext>(context: &mut C, name: &[u16]) -> Result<WindowsVaultLock, RevisionDekError<C::Error>> {
    context.charge_open().map_err(RevisionDekError::Context)?;
    // Wait plus both release calls are reserved before native acquisition.
    for _ in 0..3 { context.charge(LegacyRevisionDpapiWork::NativeCall).map_err(RevisionDekError::Context)?; }
    let lock = WindowsVaultLock::acquire_named(name, wait_millis(context))
        .map_err(|()| RevisionDekError::LockUnavailable)?.ok_or(RevisionDekError::LockUnavailable)?;
    context.checkpoint().map_err(RevisionDekError::Context)?;
    Ok(lock)
}

#[cfg(windows)]
fn release_lock<C: RevisionDekContext>(context: &mut C, lock: WindowsVaultLock) -> Result<(), C::Error> {
    drop(lock);
    context.checkpoint()
}

#[cfg(windows)]
fn wait_millis<C: RevisionDekContext>(context: &C) -> u32 {
    let millis = context
        .deadline()
        .saturating_duration_since(Instant::now())
        .as_millis();
    u32::try_from(millis.min(5_000)).unwrap_or(5_000)
}

#[cfg(windows)]
fn charge_name_budget<C: RevisionDekContext>(
    context: &mut C,
    prefix_bytes: usize,
) -> Result<(), RevisionDekError<C::Error>> {
    let bytes = prefix_bytes
        .checked_add(64)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(2))
        .ok_or(RevisionDekError::RecordTooLarge)?;
    context
        .charge(LegacyRevisionDpapiWork::Allocate(bytes))
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::Process(bytes))
        .map_err(RevisionDekError::Context)
}

fn target_name(reference_digest: [u8; 32]) -> Vec<u16> {
    wide_with_digest(TARGET_PREFIX, reference_digest)
}

fn mutex_name(reference_digest: [u8; 32]) -> Vec<u16> {
    wide_with_digest(MUTEX_PREFIX, reference_digest)
}

fn wide_with_digest(prefix: &str, digest: [u8; 32]) -> Vec<u16> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = Vec::with_capacity(prefix.len() + 65);
    value.extend(prefix.encode_utf16());
    for byte in digest {
        value.push(u16::from(HEX[usize::from(byte >> 4)]));
        value.push(u16::from(HEX[usize::from(byte & 0x0f)]));
    }
    value.push(0);
    value
}

fn clear_bytes(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: every byte belongs to this owner and is uniquely borrowed.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

#[cfg(windows)]
mod platform {
    use super::{LegacyRevisionDpapiWork, MAX_USER_TOKEN_BYTES, RevisionDekContext,
        RevisionDekError, RevisionUserObservation};
    use core::ffi::c_void;
    use core::ptr::{null_mut, read_unaligned};

    #[repr(C)]
    struct SidAndAttributes {
        sid: *mut c_void,
        attributes: u32,
    }

    #[repr(C)]
    struct TokenUser {
        user: SidAndAttributes,
    }

    #[link(name = "Advapi32")]
    unsafe extern "system" {
        fn OpenThreadToken(
            thread: *mut c_void,
            access: u32,
            open_as_self: i32,
            token: *mut *mut c_void,
        ) -> i32;
        fn OpenProcessToken(process: *mut c_void, access: u32, token: *mut *mut c_void) -> i32;
        fn GetTokenInformation(
            token: *mut c_void,
            class: u32,
            information: *mut c_void,
            length: u32,
            returned: *mut u32,
        ) -> i32;
        fn GetLengthSid(sid: *mut c_void) -> u32;
        fn IsValidSid(sid: *mut c_void) -> i32;
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn GetCurrentProcess() -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }

    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_USER_CLASS: u32 = 1;
    const ERROR_NO_TOKEN: u32 = 1_008;

    pub(super) fn observe<C: RevisionDekContext>(
        context: &mut C,
    ) -> Result<RevisionUserObservation, RevisionDekError<C::Error>> {
        context
            .checkpoint()
            .map_err(RevisionDekError::Context)?;
        context
            .charge_open()
            .map_err(RevisionDekError::Context)?;
        for _ in 0..3 {
            // Current-thread pseudo-handle, possible error capture and token
            // release are prepaid; no callback may precede GetLastError.
            context.charge(LegacyRevisionDpapiWork::NativeCall).map_err(RevisionDekError::Context)?;
        }
        let mut thread_token = null_mut();
        // SAFETY: the current thread pseudo-handle and output token pointer are valid.
        let (thread_opened, thread_error) = unsafe {
            let opened = OpenThreadToken(
                GetCurrentThread(),
                TOKEN_QUERY,
                1,
                &raw mut thread_token,
            );
            (opened, if opened == 0 { GetLastError() } else { 0 })
        };
        let mut thread_guard = TokenGuard(thread_token);
        context.checkpoint().map_err(RevisionDekError::Context)?;
        if thread_opened != 0 {
            close_token(context, &mut thread_guard)?;
            return Err(RevisionDekError::ThreadImpersonationDetected);
        }
        if thread_error != ERROR_NO_TOKEN || !thread_token.is_null() {
            return Err(RevisionDekError::UserObservationInvalid);
        }

        context
            .charge_open()
            .map_err(RevisionDekError::Context)?;
        for _ in 0..2 {
            // Current-process pseudo-handle and release of the owned token.
            context.charge(LegacyRevisionDpapiWork::NativeCall).map_err(RevisionDekError::Context)?;
        }
        let mut process_token = null_mut();
        // SAFETY: the current process pseudo-handle and output token pointer are valid.
        let process_opened = unsafe {
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut process_token)
        };
        let mut guard = TokenGuard(process_token);
        context.checkpoint().map_err(RevisionDekError::Context)?;
        if process_opened == 0 || process_token.is_null() {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        let result = observe_token_info(context, process_token);
        let closed = close_token(context, &mut guard);
        match (result, closed) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(observation), Ok(())) => Ok(observation),
        }
    }

    fn observe_token_info<C: RevisionDekContext>(
        context: &mut C,
        token: *mut c_void,
    ) -> Result<RevisionUserObservation, RevisionDekError<C::Error>> {
        let mut required = 0_u32;
        for _ in 0..2 {
            context.charge(LegacyRevisionDpapiWork::NativeCall).map_err(RevisionDekError::Context)?;
        }
        // SAFETY: the first query intentionally supplies no output buffer.
        let (sized, sizing_error) = unsafe {
            let sized = GetTokenInformation(
                token,
                TOKEN_USER_CLASS,
                null_mut(),
                0,
                &raw mut required,
            );
            (sized, if sized == 0 { GetLastError() } else { 0 })
        };
        context.checkpoint().map_err(RevisionDekError::Context)?;
        let required_bytes = usize::try_from(required).map_err(|_| RevisionDekError::UserObservationInvalid)?;
        if sized != 0 || sizing_error != 122 || required_bytes < core::mem::size_of::<TokenUser>()
            || required_bytes > MAX_USER_TOKEN_BYTES {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        context
            .charge(LegacyRevisionDpapiWork::Allocate(required_bytes))
            .map_err(RevisionDekError::Context)?;
        let mut buffer = Vec::new();
        buffer.try_reserve_exact(required_bytes).map_err(|_| RevisionDekError::UserObservationInvalid)?;
        buffer.resize(required_bytes, 0);
        context
            .charge(LegacyRevisionDpapiWork::NativeCall)
            .map_err(RevisionDekError::Context)?;
        // SAFETY: the buffer is live and has the exact requested capacity.
        let success = unsafe {
            GetTokenInformation(
                token,
                TOKEN_USER_CLASS,
                buffer.as_mut_ptr().cast(),
                u32::try_from(buffer.len()).map_err(|_| RevisionDekError::UserObservationInvalid)?,
                &raw mut required,
            )
        };
        context.checkpoint().map_err(RevisionDekError::Context)?;
        if success == 0 {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        let returned = usize::try_from(required).map_err(|_| RevisionDekError::UserObservationInvalid)?;
        if returned < core::mem::size_of::<TokenUser>() || returned > buffer.len() {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        let token_user = unsafe { read_unaligned(buffer.as_ptr().cast::<TokenUser>()) };
        let sid = checked_sid(&buffer, returned, token_user.user.sid)
            .ok_or(RevisionDekError::UserObservationInvalid)?;
        context
            .charge(LegacyRevisionDpapiWork::NativeCall)
            .map_err(RevisionDekError::Context)?;
        let valid = unsafe { IsValidSid(sid.as_ptr().cast_mut().cast()) };
        context.checkpoint().map_err(RevisionDekError::Context)?;
        if valid == 0 {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        context
            .charge(LegacyRevisionDpapiWork::NativeCall)
            .map_err(RevisionDekError::Context)?;
        let sid_length = usize::try_from(unsafe { GetLengthSid(sid.as_ptr().cast_mut().cast()) })
            .map_err(|_| RevisionDekError::UserObservationInvalid)?;
        context.checkpoint().map_err(RevisionDekError::Context)?;
        if sid_length != sid.len() {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        context
            .charge(LegacyRevisionDpapiWork::Process(sid_length.saturating_mul(2)))
            .map_err(RevisionDekError::Context)?;
        let digest = search_os_secrets::hash_native_revision_user_sid(sid);
        Ok(RevisionUserObservation { user_scope_digest: digest })
    }

    // Derive a Rust slice only from a checked offset into the owned buffer;
    // the native SID pointer is never dereferenced to validate its own bounds.
    fn checked_sid(buffer: &[u8], returned: usize, pointer: *mut c_void) -> Option<&[u8]> {
        if pointer.is_null() || returned > buffer.len() { return None; }
        let offset = (pointer as usize).checked_sub(buffer.as_ptr() as usize)?;
        let header_end = offset.checked_add(8)?;
        if header_end > returned { return None; }
        let header = buffer.get(offset..header_end)?;
        if header[0] != 1 || header[1] > 15 { return None; }
        let end = offset.checked_add(8 + 4 * usize::from(header[1]))?;
        if end > returned { return None; }
        buffer.get(offset..end)
    }

    #[cfg(test)]
    mod sid_bounds_tests {
        use super::checked_sid;
        #[test]
        fn malformed_sid_pointers_and_counts_are_rejected_without_native_reads() {
            let mut buffer = [0u8; 84];
            let start = buffer.as_mut_ptr();
            assert!(checked_sid(&buffer, buffer.len(), core::ptr::null_mut()).is_none());
            assert!(checked_sid(&buffer, buffer.len(), start.wrapping_sub(1).cast()).is_none());
            buffer[16] = 1;
            buffer[17] = 15;
            assert!(checked_sid(&buffer, 24, start.wrapping_add(16).cast()).is_none());
            buffer[17] = 1;
            assert!(checked_sid(&buffer, 27, start.wrapping_add(16).cast()).is_none());
            assert!(checked_sid(&buffer, 28, start.wrapping_add(16).cast()).is_some_and(|sid| sid.len() == 12));
            buffer[17] = 16;
            assert!(checked_sid(&buffer, buffer.len(), start.wrapping_add(16).cast()).is_none());
        }
    }

    struct TokenGuard(*mut c_void);

    impl TokenGuard {
        fn close(&mut self) {
            if !self.0.is_null() {
                // SAFETY: this guard owns the token handle returned by Open*Token.
                unsafe { CloseHandle(self.0) };
                self.0 = null_mut();
            }
        }
    }

    fn close_token<C: RevisionDekContext>(
        context: &mut C,
        guard: &mut TokenGuard,
    ) -> Result<(), RevisionDekError<C::Error>> {
        if guard.0.is_null() {
            return Ok(());
        }
        // The release was reserved before the corresponding Open*Token call.
        // SAFETY: this guard owns the token handle returned by Open*Token.
        let closed = unsafe { CloseHandle(guard.0) };
        guard.0 = null_mut();
        if closed == 0 {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        context
            .checkpoint()
            .map_err(RevisionDekError::Context)
    }

    impl Drop for TokenGuard {
        fn drop(&mut self) {
            self.close();
        }
    }
}

#[cfg(test)]
mod validated_key_cleanup_tests {
    use super::{ValidatedRevisionDekKey, clear_bytes, copy_and_clear};
    use crate::RevisionDekError;

    #[test]
    fn copy_and_clear_invalid_length_clears_source_slice() {
        let mut key = [0xABu8; 31];
        let result = copy_and_clear(&mut key);
        assert!(matches!(result, Err(RevisionDekError::ProtectedKeyInvalid)));
        assert!(key.iter().all(|&b| b == 0), "invalid length must clear source slice");
    }

    #[test]
    fn copy_and_clear_all_zero_clears_source_slice() {
        let mut key = [0u8; 32];
        let result = copy_and_clear(&mut key);
        assert!(matches!(result, Err(RevisionDekError::ProtectedKeyInvalid)));
        assert!(key.iter().all(|&b| b == 0), "all-zero must clear source slice");
    }

    #[test]
    fn copy_and_clear_valid_preserves_key_and_clears_source() {
        let mut key = [0u8; 32];
        key[0] = 0x01;
        key[31] = 0xFF;
        let expected = key;
        let guard = copy_and_clear(&mut key).unwrap();
        assert_eq!(guard.as_bytes(), &expected);
        assert!(key.iter().all(|&b| b == 0), "valid path must clear source slice");
    }

    #[test]
    fn guard_drop_zeroizes_bytes_via_witness() {
        use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
        let witness = Arc::new(AtomicBool::new(false));
        let mut bytes = [0u8; 32];
        bytes[0] = 0x42;
        bytes[31] = 0xFF;
        let guard = ValidatedRevisionDekKey {
            bytes,
            cleared: Some(Arc::clone(&witness)),
        };
        drop(guard);
        assert!(witness.load(Ordering::Acquire), "guard drop must zeroize bytes");
    }

    #[test]
    fn clear_bytes_wipes_slice() {
        let mut data = [0xCDu8; 64];
        clear_bytes(&mut data);
        assert!(data.iter().all(|&b| b == 0));
    }
}
