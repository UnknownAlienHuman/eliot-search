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
    let entropy = binding.protection_entropy();
    if entropy.is_empty() || entropy.len() > crate::MAX_SCOPE_BYTES {
        return Err(RevisionDekError::BindingInvalid);
    }
    context
        .charge(LegacyRevisionDpapiWork::Process(entropy.len()))
        .map_err(RevisionDekError::Context)?;
    #[cfg(windows)]
    let mut key = super::windows::WindowsCredentialPlatform::generate_revision_dek_key(context)?;
    #[cfg(not(windows))]
    let key = return Err(RevisionDekError::UnsupportedPlatform);
    let protected = match protect_revision_dek_with_context(context, &key, &entropy) {
        Ok(protected) => protected,
        Err(error) => {
            clear_bytes(&mut key);
            return Err(RevisionDekError::Dpapi(error));
        }
    };
    clear_bytes(&mut key);
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
        .charge(LegacyRevisionDpapiWork::Process(frame_bytes))
        .map_err(RevisionDekError::Context)?;
    let frame = encode_frame(prepared.binding_digest, &prepared.protected)?;
    let target = target_name(prepared.reference_digest);
    let mutex = mutex_name(prepared.reference_digest);
    let wait = wait_millis(context);
    context
        .charge_open()
        .map_err(RevisionDekError::Context)?;
    let lock = acquire_lock(&mutex, wait)?;
    let existing = match read_credential(context, &target) {
        Ok(existing) => existing,
        Err(error) => {
            release_lock(context, lock);
            return Err(error);
        }
    };
    if existing.is_some() {
        release_lock(context, lock);
        return Err(RevisionDekError::ExistingCredential);
    }
    let mut record = match ProviderCredentialBlob::new(frame.clone()) {
        Ok(record) => record,
        Err(error) => {
            release_lock(context, lock);
            return Err(RevisionDekError::Credential(error));
        }
    };
    if let Err(error) = context.charge_open() {
        release_lock(context, lock);
        return Err(RevisionDekError::Context(error));
    }
    let written = WindowsCredentialPlatform::write_revision_dek_credential(&target, &mut record);
    if written.is_err() {
        release_lock(context, lock);
        return Err(RevisionDekError::WriteOutcomeUnknown);
    }
    if context.checkpoint().is_err() {
        release_lock(context, lock);
        return Err(RevisionDekError::WriteOutcomeUnknown);
    }
    let observed = match read_credential(context, &target) {
        Ok(Some(bytes)) => bytes,
        Ok(None) | Err(_) => {
            release_lock(context, lock);
            return Err(RevisionDekError::WriteOutcomeUnknown);
        }
    };
    if observed != frame {
        release_lock(context, lock);
        return Err(RevisionDekError::WriteOutcomeUnknown);
    }
    let protected = match validate_frame_and_unwrap(
        context,
        &observed,
        prepared.binding_digest,
        &prepared.entropy,
    ) {
        Ok(protected) => protected,
        Err(_) => {
            release_lock(context, lock);
            return Err(RevisionDekError::WriteOutcomeUnknown);
        }
    };
    release_lock(context, lock);
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
    let entropy = binding.protection_entropy();
    if entropy.is_empty() || entropy.len() > crate::MAX_SCOPE_BYTES {
        return Err(RevisionDekError::BindingInvalid);
    }
    let protected = validate_frame_and_unwrap(
        context,
        &frame,
        binding.binding_digest(),
        &entropy,
    )?;
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
    let wait = wait_millis(context);
    context
        .charge_open()
        .map_err(RevisionDekError::Context)?;
    context
        .charge_open()
        .map_err(RevisionDekError::Context)?;
    let lock = acquire_lock(&mutex, wait)?;
    context
        .charge_open()
        .map_err(RevisionDekError::Context)?;
    let _ = WindowsCredentialPlatform::delete_credential(&target);
    let absent = read_credential(context, &target)?.is_none();
    release_lock(context, lock);
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

fn decode_frame<'a, E>(
    frame: &'a [u8],
    expected_binding: [u8; 32],
) -> Result<&'a [u8], RevisionDekError<E>> {
    if frame.len() < RECORD_HEADER_BYTES
        || frame.len() > MAX_CREDENTIAL_BLOB_BYTES
        || frame.get(..8) != Some(RECORD_MAGIC)
        || frame[8..40] != expected_binding.as_slice()
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

fn validate_frame_and_unwrap<C: RevisionDekContext>(
    context: &mut C,
    frame: &[u8],
    binding_digest: [u8; 32],
    entropy: &[u8],
) -> Result<Vec<u8>, RevisionDekError<C::Error>> {
    let protected = decode_frame::<()>(frame, binding_digest)
        .map_err(|_| RevisionDekError::CredentialReadbackInvalid)?;
    let mut key = unprotect_revision_dek_with_context(context, protected, entropy)
        .map_err(RevisionDekError::Dpapi)?;
    if key.len() != 32 || key.iter().all(|byte| *byte == 0) {
        clear_bytes(&mut key);
        return Err(RevisionDekError::ProtectedKeyInvalid);
    }
    clear_bytes(&mut key);
    context
        .charge(LegacyRevisionDpapiWork::Allocate(protected.len()))
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::Process(protected.len()))
        .map_err(RevisionDekError::Context)?;
    Ok(protected.to_vec())
}

#[cfg(windows)]
fn read_credential<C: RevisionDekContext>(
    context: &mut C,
    target: &[u16],
) -> Result<Option<Vec<u8>>, RevisionDekError<C::Error>> {
    context
        .checkpoint()
        .map_err(RevisionDekError::Context)?;
    context
        .charge_open()
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::Allocate(MAX_CREDENTIAL_BLOB_BYTES))
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::Process(MAX_CREDENTIAL_BLOB_BYTES))
        .map_err(RevisionDekError::Context)?;
    context
        .charge_open()
        .map_err(RevisionDekError::Context)?;
    let result = WindowsCredentialPlatform::read_provider_credential(target)
        .map_err(RevisionDekError::Credential)?;
    context
        .checkpoint()
        .map_err(RevisionDekError::Context)?;
    let Some(record) = result else { return Ok(None); };
    let length = record.as_slice().len();
    context
        .charge(LegacyRevisionDpapiWork::Allocate(length))
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::Process(length))
        .map_err(RevisionDekError::Context)?;
    Ok(Some(record.as_slice().to_vec()))
}

#[cfg(windows)]
fn acquire_lock<E>(name: &[u16], wait: u32) -> Result<WindowsVaultLock, RevisionDekError<E>> {
    WindowsVaultLock::acquire_named(name, wait).map_err(|_| RevisionDekError::LockUnavailable)?.ok_or(RevisionDekError::LockUnavailable)
}

#[cfg(windows)]
fn release_lock<C: RevisionDekContext>(context: &mut C, lock: WindowsVaultLock) {
    let _ = context.charge_open();
    let _ = context.charge_open();
    drop(lock);
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
    let mut value = String::with_capacity(prefix.len() + 64);
    value.push_str(prefix);
    for byte in digest {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value.encode_utf16().chain(core::iter::once(0)).collect()
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
    use super::*;
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
        let mut thread_token = null_mut();
        // SAFETY: the current thread pseudo-handle and output token pointer are valid.
        let thread_opened = unsafe {
            OpenThreadToken(
                GetCurrentThread(),
                TOKEN_QUERY,
                1,
                &mut thread_token,
            )
        };
        if thread_opened != 0 {
            let mut guard = TokenGuard(thread_token);
            close_token(context, &mut guard)?;
            return Err(RevisionDekError::ThreadImpersonationDetected);
        }
        context
            .charge_open()
            .map_err(RevisionDekError::Context)?;
        let thread_error = unsafe { GetLastError() };
        if thread_error != ERROR_NO_TOKEN {
            if !thread_token.is_null() {
                let mut guard = TokenGuard(thread_token);
                let _ = close_token(context, &mut guard);
            }
            return Err(RevisionDekError::UserObservationInvalid);
        }

        context
            .charge_open()
            .map_err(RevisionDekError::Context)?;
        let mut process_token = null_mut();
        // SAFETY: the current process pseudo-handle and output token pointer are valid.
        let process_opened = unsafe {
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut process_token)
        };
        if process_opened == 0 || process_token.is_null() {
            if !process_token.is_null() {
                let mut guard = TokenGuard(process_token);
                let _ = close_token(context, &mut guard);
            }
            return Err(RevisionDekError::UserObservationInvalid);
        }
        let mut guard = TokenGuard(process_token);
        let result = observe_token_info(context, process_token);
        let closed = close_token(context, &mut guard);
        match (result, closed) {
            (Err(error), _) => Err(error),
            (Ok(observation), Err(error)) => Err(error),
            (Ok(observation), Ok(())) => Ok(observation),
        }
    }

    fn observe_token_info<C: RevisionDekContext>(
        context: &mut C,
        token: *mut c_void,
    ) -> Result<RevisionUserObservation, RevisionDekError<C::Error>> {
        let mut required = 0_u32;
        context
            .charge_open()
            .map_err(RevisionDekError::Context)?;
        // SAFETY: the first query intentionally supplies no output buffer.
        let _ = unsafe {
            GetTokenInformation(
                token,
                TOKEN_USER_CLASS,
                null_mut(),
                0,
                &mut required,
            )
        };
        let required = usize::try_from(required).map_err(|_| RevisionDekError::UserObservationInvalid)?;
        if required == 0 || required > MAX_USER_TOKEN_BYTES {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        context
            .charge(LegacyRevisionDpapiWork::Allocate(required))
            .map_err(RevisionDekError::Context)?;
        let mut buffer = Vec::new();
        buffer.try_reserve_exact(required).map_err(|_| RevisionDekError::UserObservationInvalid)?;
        buffer.resize(required, 0);
        context
            .charge_open()
            .map_err(RevisionDekError::Context)?;
        // SAFETY: the buffer is live and has the exact requested capacity.
        let success = unsafe {
            GetTokenInformation(
                token,
                TOKEN_USER_CLASS,
                buffer.as_mut_ptr().cast(),
                u32::try_from(buffer.len()).map_err(|_| RevisionDekError::UserObservationInvalid)?,
                &mut required,
            )
        };
        if success == 0 {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        let token_user = unsafe { read_unaligned(buffer.as_ptr().cast::<TokenUser>()) };
        context
            .charge_open()
            .map_err(RevisionDekError::Context)?;
        if token_user.user.sid.is_null()
            || unsafe { IsValidSid(token_user.user.sid) } == 0
        {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        context
            .charge_open()
            .map_err(RevisionDekError::Context)?;
        let sid_length = usize::try_from(unsafe { GetLengthSid(token_user.user.sid) })
            .map_err(|_| RevisionDekError::UserObservationInvalid)?;
        let start = buffer.as_ptr() as usize;
        let end = start.checked_add(buffer.len()).ok_or(RevisionDekError::UserObservationInvalid)?;
        let sid_start = token_user.user.sid as usize;
        let sid_end = sid_start.checked_add(sid_length).ok_or(RevisionDekError::UserObservationInvalid)?;
        if sid_length == 0 || sid_start < start || sid_end > end {
            return Err(RevisionDekError::UserObservationInvalid);
        }
        context
            .charge(LegacyRevisionDpapiWork::Process(sid_length.saturating_mul(2)))
            .map_err(RevisionDekError::Context)?;
        let sid = unsafe { core::slice::from_raw_parts(token_user.user.sid.cast::<u8>(), sid_length) };
        let digest = search_os_secrets::derive_windows_user_scope_digest(sid)
            .map_err(|_| RevisionDekError::UserObservationInvalid)?;
        Ok(RevisionUserObservation { user_scope_digest: digest })
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
        let reservation = context.charge_open();
        // SAFETY: this guard owns the token handle returned by Open*Token.
        let closed = unsafe { CloseHandle(guard.0) };
        guard.0 = null_mut();
        reservation.map_err(RevisionDekError::Context)?;
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
