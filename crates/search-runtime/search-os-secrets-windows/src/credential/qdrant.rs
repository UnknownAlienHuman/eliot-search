//! Read-only Qdrant API-key selection from the current-user Windows secret store.
//!
//! The credential name binds the stable config reference to the exact Search
//! installation, incarnation, current-user scope, and Qdrant API-key purpose.
//! DPAPI entropy additionally authenticates the exact credential version. This
//! adapter never creates, rotates, or deletes a key.

use core::{convert::Infallible, fmt, marker::PhantomData, num::NonZeroU64, time::Duration};
use std::time::Instant;

use crate::{
    ContextualLegacyRevisionDpapiError, LegacyRevisionDpapiContext, LegacyRevisionDpapiWork,
    ProtectedSecret, ProtectionScope, SecretBytes,
};
use search_contracts::{
    Blake3Digest32, InstallationId, InstallationIncarnationId, NonZeroRevision, OpaqueId,
    OpaqueRef, RequestId,
};
use search_os_secrets::{
    DEFAULT_SECRET_LIMITS, SecretBinding, SecretError, SecretLease, SecretLimits, SecretReference,
};
use search_ports::{
    CancellationProbe, DisclosureClass, MonotonicInstant, MutationIdentity, OperationContext,
    PackageOpaque, Port, PortErrorKind, PortFailure, PortRetryability, SecretStorePort,
};

const QDRANT_API_SECRET_PURPOSE_ID: &str = "secret-purpose:qdrant-api-key/v1";
#[cfg(any(windows, test))]
const QDRANT_CREDENTIAL_MAGIC: &[u8; 8] = b"ELQDRT01";
#[cfg(windows)]
const QDRANT_CREDENTIAL_TARGET_PREFIX: &str = "ELIOT Search/qdrant-api-secret/v1/";
#[cfg(windows)]
const QDRANT_CREDENTIAL_MUTEX_PREFIX: &str = "Global\\ELIOT-Search-QdrantApiSecret-v1-";
#[cfg(any(windows, test))]
const QDRANT_CREDENTIAL_FIXED_BYTES: usize = 8 + 8 + 32 + 2 + 4;
const MIN_LEASE_TTL_MS: u64 = 1_000;
const MAX_LEASE_TTL_MS: u64 = 300_000;

struct UnmeteredQdrantContext;

impl LegacyRevisionDpapiContext for UnmeteredQdrantContext {
    type Error = Infallible;

    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn charge(&mut self, _: LegacyRevisionDpapiWork) -> Result<(), Self::Error> {
        Ok(())
    }

    fn account_native_clear(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn contextual_checkpoint<C: LegacyRevisionDpapiContext>(
    context: &mut C,
) -> Result<(), QdrantApiSecretStoreError> {
    context
        .checkpoint()
        .map_err(|_| QdrantApiSecretStoreError::ContextRefused)
}

fn charge_contextual<C: LegacyRevisionDpapiContext>(
    context: &mut C,
    work: LegacyRevisionDpapiWork,
) -> Result<(), QdrantApiSecretStoreError> {
    contextual_checkpoint(context)?;
    context
        .charge(work)
        .map_err(|_| QdrantApiSecretStoreError::ContextRefused)?;
    contextual_checkpoint(context)
}

fn reserve_native_calls<C: LegacyRevisionDpapiContext>(
    context: &mut C,
    count: usize,
) -> Result<(), QdrantApiSecretStoreError> {
    for _ in 0..count {
        charge_contextual(context, LegacyRevisionDpapiWork::NativeCall)?;
    }
    Ok(())
}

fn contextual_dpapi_error<E>(
    error: &ContextualLegacyRevisionDpapiError<E>,
) -> QdrantApiSecretStoreError {
    match error {
        ContextualLegacyRevisionDpapiError::Context(_)
        | ContextualLegacyRevisionDpapiError::NativeAndContext { .. }
        | ContextualLegacyRevisionDpapiError::AllocationAndContext { .. } => {
            QdrantApiSecretStoreError::ContextRefused
        }
        ContextualLegacyRevisionDpapiError::Native(_)
        | ContextualLegacyRevisionDpapiError::Allocation(_) => {
            QdrantApiSecretStoreError::ProtectionFailed
        }
    }
}

#[cfg(windows)]
fn reserve_qdrant_lock<C: LegacyRevisionDpapiContext>(
    context: &mut C,
) -> Result<(), QdrantApiSecretStoreError> {
    // CreateMutex/possible GetLastError/Wait and the eventual release/close (or
    // failure close) are charged before the lock owner's native boundary.
    reserve_native_calls(context, 4)
}

#[cfg(windows)]
fn contextual_credential_error<E>(
    error: &super::windows::CredentialWorkError<E>,
    credential_failure: QdrantApiSecretStoreError,
) -> QdrantApiSecretStoreError {
    match error {
        super::windows::CredentialWorkError::Context(_) => {
            QdrantApiSecretStoreError::ContextRefused
        }
        super::windows::CredentialWorkError::Credential => credential_failure,
        super::windows::CredentialWorkError::KeyGeneration => {
            QdrantApiSecretStoreError::CredentialGenerationFailed
        }
    }
}

/// Closed Qdrant API-key purpose. Callers cannot substitute a provider or
/// revision-key purpose through the selected-secret port.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct QdrantApiSecretPurpose;

impl QdrantApiSecretPurpose {
    /// Creates the one supported Qdrant API-key purpose value.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn id() -> Result<OpaqueId, QdrantApiSecretStoreError> {
        OpaqueId::new(QDRANT_API_SECRET_PURPOSE_ID)
            .map_err(|_| QdrantApiSecretStoreError::InvalidReference)
    }
}

impl Default for QdrantApiSecretPurpose {
    fn default() -> Self {
        Self::new()
    }
}

/// Stable selected reference combined with the actual current Root and user
/// facts. It contains no credential bytes and grants no lease by itself.
#[derive(Clone, Eq, PartialEq)]
pub struct QdrantApiSecretReference {
    stable_reference: OpaqueId,
    binding: SecretBinding,
}

impl QdrantApiSecretReference {
    /// Binds the selected stable reference to one installation incarnation and
    /// the current user's observed scope under the fixed Qdrant purpose.
    pub fn for_owner(
        stable_reference: OpaqueId,
        installation_id: InstallationId,
        installation_incarnation_id: InstallationIncarnationId,
        user_scope_digest: Blake3Digest32,
    ) -> Result<Self, QdrantApiSecretStoreError> {
        let binding = SecretBinding::new(
            installation_id,
            installation_incarnation_id,
            user_scope_digest,
            QdrantApiSecretPurpose::id()?,
        );
        Ok(Self {
            stable_reference,
            binding,
        })
    }

    /// Stable opaque config reference.
    #[must_use]
    pub const fn stable_reference(&self) -> &OpaqueId {
        &self.stable_reference
    }

    /// Exact Root/user/purpose binding selected by composition.
    #[must_use]
    pub const fn binding(&self) -> &SecretBinding {
        &self.binding
    }

    /// Stable digest of the exact selected reference and all Root/user/purpose
    /// binding facts. Root may use it as the bounded C02 technical-record key.
    #[must_use]
    pub fn selection_digest(&self) -> Blake3Digest32 {
        Blake3Digest32::from_bytes(selection_digest(self))
    }
}

impl fmt::Debug for QdrantApiSecretReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QdrantApiSecretReference")
            .field("stable_reference", &"<redacted>")
            .field("binding", &self.binding)
            .finish()
    }
}

/// Redacted failure from Qdrant selected-secret or initial-provision operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QdrantApiSecretStoreError {
    /// No qualified current-user Credential Manager implementation exists here.
    UnsupportedPlatform,
    /// The original request context expired or was cancelled.
    ContextRefused,
    /// The selected stable reference has no existing Qdrant credential.
    SecretNotFound,
    /// The selected reference or requested purpose is malformed or mismatched.
    InvalidReference,
    /// Credential Manager refused the exact selected record.
    CredentialReadFailed,
    /// The sealed credential record was malformed or bound elsewhere.
    CredentialRecordInvalid,
    /// Current-user DPAPI did not open the exact selected secret.
    ProtectionFailed,
    /// The lifecycle owner rejected the finite private lease.
    LeaseRejected(SecretError),
    /// This port instance is intentionally read-only.
    MutationUnsupported,
    /// The configured lease duration is outside the accepted finite bounds.
    InvalidLeasePolicy,
    /// The exact selected target already contains a credential.
    CredentialAlreadyExists,
    /// The observed encoded credential frame differs from the Root inventory.
    CredentialFrameMismatch,
    /// The bounded per-reference Credential Manager mutex was unavailable.
    CredentialLockUnavailable,
    /// Qdrant API-key generation failed before credential write dispatch.
    CredentialGenerationFailed,
    /// Credential Manager write was dispatched but exact readback did not resolve it.
    CredentialWriteOutcomeUnknown,
}

impl QdrantApiSecretStoreError {
    /// Stable content-free failure code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "QDRANT_SECRET_UNSUPPORTED_PLATFORM",
            Self::ContextRefused => "QDRANT_SECRET_CONTEXT_REFUSED",
            Self::SecretNotFound => "QDRANT_SECRET_NOT_FOUND",
            Self::InvalidReference => "QDRANT_SECRET_REFERENCE_INVALID",
            Self::CredentialReadFailed => "QDRANT_SECRET_CREDENTIAL_READ_FAILED",
            Self::CredentialRecordInvalid => "QDRANT_SECRET_CREDENTIAL_RECORD_INVALID",
            Self::ProtectionFailed => "QDRANT_SECRET_PROTECTION_FAILED",
            Self::LeaseRejected(_) => "QDRANT_SECRET_LEASE_REJECTED",
            Self::MutationUnsupported => "QDRANT_SECRET_MUTATION_UNSUPPORTED",
            Self::InvalidLeasePolicy => "QDRANT_SECRET_LEASE_POLICY_INVALID",
            Self::CredentialAlreadyExists => "QDRANT_SECRET_CREDENTIAL_ALREADY_EXISTS",
            Self::CredentialFrameMismatch => "QDRANT_SECRET_FRAME_MISMATCH",
            Self::CredentialLockUnavailable => "QDRANT_SECRET_LOCK_UNAVAILABLE",
            Self::CredentialGenerationFailed => "QDRANT_SECRET_GENERATION_FAILED",
            Self::CredentialWriteOutcomeUnknown => "QDRANT_SECRET_WRITE_OUTCOME_UNKNOWN",
        }
    }
}

impl fmt::Display for QdrantApiSecretStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for QdrantApiSecretStoreError {}

impl PortFailure for QdrantApiSecretStoreError {
    fn kind(&self) -> PortErrorKind {
        match self {
            Self::SecretNotFound
            | Self::UnsupportedPlatform
            | Self::CredentialLockUnavailable
            | Self::CredentialGenerationFailed => PortErrorKind::DependencyUnavailable,
            Self::ContextRefused => PortErrorKind::DeadlineDuringOperation,
            Self::InvalidReference | Self::MutationUnsupported | Self::InvalidLeasePolicy => {
                PortErrorKind::InvalidInput
            }
            Self::CredentialAlreadyExists | Self::CredentialFrameMismatch => {
                PortErrorKind::Conflict
            }
            Self::CredentialWriteOutcomeUnknown => PortErrorKind::OutcomeUnknown,
            Self::CredentialReadFailed
            | Self::CredentialRecordInvalid
            | Self::ProtectionFailed
            | Self::LeaseRejected(SecretError::BindingMismatch | SecretError::NotLeaseable) => {
                PortErrorKind::Unauthorized
            }
            Self::LeaseRejected(_) => PortErrorKind::InvalidInput,
        }
    }

    fn retryability(&self) -> PortRetryability {
        match self {
            Self::SecretNotFound
            | Self::CredentialReadFailed
            | Self::ContextRefused
            | Self::UnsupportedPlatform
            | Self::CredentialLockUnavailable => PortRetryability::SameRequest,
            _ => PortRetryability::Never,
        }
    }

    fn disclosure(&self) -> DisclosureClass {
        DisclosureClass::Redacted
    }

    fn operation_id(&self) -> Option<&OpaqueId> {
        None
    }
}

/// Private purpose-bound lease returned by the selected OS credential read.
pub struct QdrantApiSecretLease<C: CancellationProbe + Clone> {
    lease: SecretLease,
    issuer_cancellation: C,
    issuer_deadline: Instant,
    expires_at_tick: u64,
    encoded_frame_digest: Blake3Digest32,
    issuer_request_id: RequestId,
    issuer_budget_ref: OpaqueRef,
    issuer_relative_deadline_ms: u64,
}

impl<C: CancellationProbe + Clone> QdrantApiSecretLease<C> {
    /// Exact versioned reference whose bytes are privately leased.
    #[must_use]
    pub const fn reference(&self) -> &SecretReference {
        self.lease.reference()
    }

    /// Exposes the bytes only within the live lease callback. Call this at the
    /// external-effect boundary; do not detach launch evidence or retain a
    /// secret copy for a later dispatch. The same native monotonic clock is
    /// checked again after the callback returns.
    pub fn with_secret<T>(
        &self,
        context: &OperationContext<impl CancellationProbe>,
        consume: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, QdrantApiSecretStoreError> {
        self.with_secret_live(context, |bytes, _, _| consume(bytes))
    }

    /// Exposes the selected bytes, exact versioned reference and encoded-frame
    /// digest only inside one live callback. Use this at the external-effect
    /// boundary when the Root inventory must match the backend frame. Neither
    /// the lease expiry tick nor detached launch evidence is returned.
    pub fn with_secret_live<T>(
        &self,
        context: &OperationContext<impl CancellationProbe>,
        consume: impl FnOnce(&[u8], &SecretReference, Blake3Digest32) -> T,
    ) -> Result<T, QdrantApiSecretStoreError> {
        self.with_secret_live_window(context, |bytes, reference, digest, _, _| {
            consume(bytes, reference, digest)
        })
    }

    /// Exposes the exact selected secret and its live monotonic lease window
    /// only during one guarded callback. The observed and expiry ticks are
    /// descriptive values, never detached launch authority; any effect must
    /// still run under a fresh callback on this same lease and issuer context.
    /// The issuer request coordinates, cancellation, absolute deadline and
    /// native TTL are checked both before and after user work.
    pub fn with_secret_live_window<T>(
        &self,
        context: &OperationContext<impl CancellationProbe>,
        consume: impl FnOnce(&[u8], &SecretReference, Blake3Digest32, NonZeroU64, NonZeroU64) -> T,
    ) -> Result<T, QdrantApiSecretStoreError> {
        context
            .preflight()
            .map_err(|_| QdrantApiSecretStoreError::ContextRefused)?;
        let expires_at_tick = NonZeroU64::new(self.expires_at_tick)
            .ok_or(QdrantApiSecretStoreError::InvalidLeasePolicy)?;
        #[cfg(windows)]
        let observed_before = monotonic_now();
        #[cfg(not(windows))]
        let observed_before = monotonic_now()?;
        let observed_before = NonZeroU64::new(observed_before.ticks())
            .ok_or(QdrantApiSecretStoreError::InvalidLeasePolicy)?;
        if !issuer_context_allows_use(
            &self.issuer_cancellation,
            self.issuer_deadline,
            self.issuer_request_id,
            &self.issuer_budget_ref,
            self.issuer_relative_deadline_ms,
            context,
            Instant::now(),
        ) || !lease_window_is_live(observed_before, expires_at_tick)
        {
            return Err(QdrantApiSecretStoreError::ContextRefused);
        }
        let now = MonotonicInstant::from_ticks(observed_before.get());
        let result = self
            .lease
            .with_secret(now, |bytes| {
                consume(
                    bytes,
                    self.lease.reference(),
                    self.encoded_frame_digest,
                    observed_before,
                    expires_at_tick,
                )
            })
            .map_err(QdrantApiSecretStoreError::LeaseRejected)?;

        // The issuer can remain live after the shorter Qdrant lease TTL. Check
        // the same native clock after user work so a slow callback cannot turn
        // an expired lease into a successful dispatch authorization.
        #[cfg(windows)]
        let observed_after = monotonic_now();
        #[cfg(not(windows))]
        let observed_after = monotonic_now()?;
        let observed_after = NonZeroU64::new(observed_after.ticks())
            .ok_or(QdrantApiSecretStoreError::InvalidLeasePolicy)?;
        context
            .preflight()
            .map_err(|_| QdrantApiSecretStoreError::ContextRefused)?;
        if !issuer_context_allows_use(
            &self.issuer_cancellation,
            self.issuer_deadline,
            self.issuer_request_id,
            &self.issuer_budget_ref,
            self.issuer_relative_deadline_ms,
            context,
            Instant::now(),
        ) || !lease_window_is_live(observed_after, expires_at_tick)
        {
            return Err(QdrantApiSecretStoreError::ContextRefused);
        }
        Ok(result)
    }
}

fn issuer_context_allows_use<C: CancellationProbe, D: CancellationProbe>(
    issuer_cancellation: &C,
    issuer_deadline: Instant,
    issuer_request_id: RequestId,
    issuer_budget_ref: &OpaqueRef,
    issuer_relative_deadline_ms: u64,
    context: &OperationContext<D>,
    now: Instant,
) -> bool {
    !issuer_cancellation.is_cancelled()
        && !context.cancellation().is_cancelled()
        && now < issuer_deadline
        && context.request_id() == issuer_request_id
        && context.budget_ref() == issuer_budget_ref
        && context.relative_deadline_ms().get() == issuer_relative_deadline_ms
}

const fn lease_window_is_live(observed: NonZeroU64, expires: NonZeroU64) -> bool {
    observed.get() < expires.get()
}

impl<C: CancellationProbe + Clone> fmt::Debug for QdrantApiSecretLease<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QdrantApiSecretLease(<private>)")
    }
}

impl<C: CancellationProbe + Clone> PackageOpaque for QdrantApiSecretLease<C> {
    fn owner_package(&self) -> &'static str {
        "search-os-secrets-windows"
    }
}

/// Metadata from an exact selected OS credential read. It has no secret or
/// protected frame bytes and is not C02 inventory or permission by itself.
#[derive(Clone, Eq, PartialEq)]
pub struct QdrantApiSecretObservation {
    selected: QdrantApiSecretReference,
    version: NonZeroRevision,
    frame_digest: Blake3Digest32,
}

impl QdrantApiSecretObservation {
    /// Exact Root/user-scoped reference observed in Credential Manager.
    #[must_use]
    pub const fn selected(&self) -> &QdrantApiSecretReference {
        &self.selected
    }

    /// Immutable record version authenticated by the Qdrant DPAPI entropy.
    #[must_use]
    pub const fn version(&self) -> NonZeroRevision {
        self.version
    }

    /// Digest of the complete encoded Credential Manager frame.
    #[must_use]
    pub const fn frame_digest(&self) -> Blake3Digest32 {
        self.frame_digest
    }
}

impl fmt::Debug for QdrantApiSecretObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QdrantApiSecretObservation")
            .field("selected", &"<redacted>")
            .field("version", &self.version)
            .field("frame_digest", &self.frame_digest)
            .finish()
    }
}

#[cfg(windows)]
struct PreparedWindowsQdrantCredential {
    target: Vec<u16>,
    frame: super::windows::ProviderCredentialBlob,
    // Excludes concurrent first-create writers while Root commits its exact
    // frame digest and dispatched marker to the retained C02 journal.
    _lock: super::windows::WindowsVaultLock,
}

/// Private prepared version-one frame. This value cannot write on its own;
/// Root must first persist its exact digest and dispatched state in C02.
pub struct PreparedQdrantApiSecret {
    selected: QdrantApiSecretReference,
    version: NonZeroRevision,
    frame_digest: Blake3Digest32,
    #[cfg(windows)]
    native: PreparedWindowsQdrantCredential,
}

impl PreparedQdrantApiSecret {
    /// Exact Root/user-scoped reference selected for initial setup.
    #[must_use]
    pub const fn selected(&self) -> &QdrantApiSecretReference {
        &self.selected
    }

    /// Initial immutable record version, one.
    #[must_use]
    pub const fn version(&self) -> NonZeroRevision {
        self.version
    }

    /// Exact encoded-frame digest to durably record before dispatch.
    #[must_use]
    pub const fn frame_digest(&self) -> Blake3Digest32 {
        self.frame_digest
    }
}

impl fmt::Debug for PreparedQdrantApiSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedQdrantApiSecret")
            .field("selected", &"<redacted>")
            .field("version", &self.version)
            .field("frame_digest", &self.frame_digest)
            .field("frame", &"<private>")
            .finish_non_exhaustive()
    }
}

/// Narrow first-credential effect owner, separate from the read-only
/// `QdrantApiSecretStore` `SecretStorePort` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct QdrantApiSecretProvisioner;

impl QdrantApiSecretProvisioner {
    /// Reads and validates one selected target without mutation. If present,
    /// the exact record is DPAPI-opened and its temporary plaintext is dropped
    /// before only metadata is returned.
    pub fn observe_selected(
        &self,
        selected: &QdrantApiSecretReference,
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<Option<QdrantApiSecretObservation>, QdrantApiSecretStoreError> {
        self.observe_selected_with_context(selected, &mut UnmeteredQdrantContext, remaining)
    }

    /// Reads and validates one selected target using the caller's original
    /// deadline, cancellation and cumulative native-work accounting context.
    pub fn observe_selected_with_context<C: LegacyRevisionDpapiContext>(
        &self,
        selected: &QdrantApiSecretReference,
        context: &mut C,
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<Option<QdrantApiSecretObservation>, QdrantApiSecretStoreError> {
        #[cfg(windows)]
        {
            observe_selected_windows(selected, context, remaining)
        }
        #[cfg(not(windows))]
        {
            let _ = (selected, context, remaining);
            Err(QdrantApiSecretStoreError::UnsupportedPlatform)
        }
    }

    /// Requires an exact selected backend frame matching Root's retained C02
    /// version and digest. It never creates or repairs a missing target.
    pub fn observe_exact(
        &self,
        selected: &QdrantApiSecretReference,
        version: NonZeroRevision,
        frame_digest: Blake3Digest32,
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<QdrantApiSecretObservation, QdrantApiSecretStoreError> {
        self.observe_exact_with_context(
            selected,
            version,
            frame_digest,
            &mut UnmeteredQdrantContext,
            remaining,
        )
    }

    /// Requires exact selected backend bytes while charging work to the
    /// caller's original native operation context.
    pub fn observe_exact_with_context<C: LegacyRevisionDpapiContext>(
        &self,
        selected: &QdrantApiSecretReference,
        version: NonZeroRevision,
        frame_digest: Blake3Digest32,
        context: &mut C,
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<QdrantApiSecretObservation, QdrantApiSecretStoreError> {
        let observed = self
            .observe_selected_with_context(selected, context, remaining)?
            .ok_or(QdrantApiSecretStoreError::SecretNotFound)?;
        if observed.version != version || observed.frame_digest != frame_digest {
            return Err(QdrantApiSecretStoreError::CredentialFrameMismatch);
        }
        Ok(observed)
    }

    /// Prepares version one only after Root has persisted the exact initial C02
    /// create intent. The backend target must be absent; no write is performed.
    pub fn prepare_initial(
        &self,
        selected: &QdrantApiSecretReference,
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<PreparedQdrantApiSecret, QdrantApiSecretStoreError> {
        self.prepare_initial_with_context(selected, &mut UnmeteredQdrantContext, remaining)
    }

    /// Prepares the exact initial frame while charging RNG, DPAPI and
    /// allocation work to the original native operation context.
    pub fn prepare_initial_with_context<C: LegacyRevisionDpapiContext>(
        &self,
        selected: &QdrantApiSecretReference,
        context: &mut C,
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<PreparedQdrantApiSecret, QdrantApiSecretStoreError> {
        #[cfg(windows)]
        {
            prepare_initial_windows(selected, context, remaining)
        }
        #[cfg(not(windows))]
        {
            let _ = (selected, context, remaining);
            Err(QdrantApiSecretStoreError::UnsupportedPlatform)
        }
    }

    /// Writes a prepared frame at most once, only after Root has durably stored
    /// the same digest and marked the original operation dispatched in C02.
    /// Possible write outcomes are resolved by exact full-frame readback.
    pub fn dispatch_prepared(
        &self,
        prepared: PreparedQdrantApiSecret,
        expected_frame_digest: Blake3Digest32,
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<QdrantApiSecretObservation, QdrantApiSecretStoreError> {
        self.dispatch_prepared_with_context(
            prepared,
            expected_frame_digest,
            &mut UnmeteredQdrantContext,
            remaining,
        )
    }

    /// Dispatches and resolves one exact credential frame using the original
    /// native operation context for every native effect and copy.
    pub fn dispatch_prepared_with_context<C: LegacyRevisionDpapiContext>(
        &self,
        prepared: PreparedQdrantApiSecret,
        expected_frame_digest: Blake3Digest32,
        context: &mut C,
        remaining: &mut dyn FnMut() -> Option<Duration>,
    ) -> Result<QdrantApiSecretObservation, QdrantApiSecretStoreError> {
        if prepared.frame_digest != expected_frame_digest {
            return Err(QdrantApiSecretStoreError::CredentialFrameMismatch);
        }
        #[cfg(windows)]
        {
            dispatch_prepared_windows(prepared, context, remaining)
        }
        #[cfg(not(windows))]
        {
            let _ = (prepared, context, remaining);
            Err(QdrantApiSecretStoreError::UnsupportedPlatform)
        }
    }
}

/// Read-only Windows Qdrant secret adapter over Credential Manager and the
/// existing `search-os-secrets` lease lifecycle.
pub struct QdrantApiSecretStore<C> {
    lease_ttl_ms: u64,
    limits: SecretLimits,
    cancellation: PhantomData<fn() -> C>,
}

impl<C> QdrantApiSecretStore<C> {
    /// Creates the read-only adapter with the owner's finite lease policy.
    pub fn new(lease_ttl_ms: u64) -> Result<Self, QdrantApiSecretStoreError> {
        if !(MIN_LEASE_TTL_MS..=MAX_LEASE_TTL_MS).contains(&lease_ttl_ms) {
            return Err(QdrantApiSecretStoreError::InvalidLeasePolicy);
        }
        Ok(Self {
            lease_ttl_ms,
            limits: DEFAULT_SECRET_LIMITS,
            cancellation: PhantomData,
        })
    }

    /// Reads and opens the selected credential using both the secret-store
    /// operation context and the caller's original cumulative native-work
    /// context. Production Root composition must use this entrypoint.
    pub fn lease_secret_with_context<N: LegacyRevisionDpapiContext>(
        &self,
        secret_ref: &QdrantApiSecretReference,
        purpose: &QdrantApiSecretPurpose,
        context: &OperationContext<C>,
        native: &mut N,
    ) -> Result<QdrantApiSecretLease<C>, QdrantApiSecretStoreError>
    where
        C: CancellationProbe + Clone,
    {
        if *purpose != QdrantApiSecretPurpose::new()
            || secret_ref.binding.purpose().as_str() != QDRANT_API_SECRET_PURPOSE_ID
        {
            return Err(QdrantApiSecretStoreError::InvalidReference);
        }
        context
            .preflight()
            .map_err(|_| QdrantApiSecretStoreError::ContextRefused)?;
        contextual_checkpoint(native)?;
        let started = Instant::now();
        let deadline = Duration::from_millis(context.relative_deadline_ms().get());
        let issuer_deadline = started
            .checked_add(deadline)
            .ok_or(QdrantApiSecretStoreError::ContextRefused)?;
        let issuer_cancellation = context.cancellation().clone();
        let issuer_request_id = context.request_id();
        let issuer_budget_ref = context.budget_ref().clone();
        let issuer_relative_deadline_ms = context.relative_deadline_ms().get();
        let remaining = || {
            if issuer_cancellation.is_cancelled() {
                return None;
            }
            issuer_deadline
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
        };
        #[cfg(windows)]
        let (protected, version, encoded_frame_digest) =
            load_selected_windows(secret_ref, &remaining, native)?;
        #[cfg(not(windows))]
        return Err(QdrantApiSecretStoreError::UnsupportedPlatform);
        if remaining().is_none() {
            return Err(QdrantApiSecretStoreError::ContextRefused);
        }
        contextual_checkpoint(native)?;
        reserve_native_calls(native, 1)?;
        #[cfg(windows)]
        let issued = monotonic_now();
        #[cfg(not(windows))]
        let issued = monotonic_now()?;
        let issuer_remaining = remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
        let issuer_remaining_ms = u64::try_from(issuer_remaining.as_millis()).unwrap_or(u64::MAX);
        let lease_ttl_ms = self.lease_ttl_ms.min(issuer_remaining_ms);
        if lease_ttl_ms == 0 {
            return Err(QdrantApiSecretStoreError::ContextRefused);
        }
        let expires = issued
            .ticks()
            .checked_add(lease_ttl_ms)
            .map(MonotonicInstant::from_ticks)
            .ok_or(QdrantApiSecretStoreError::InvalidLeasePolicy)?;
        let version = NonZeroRevision::new(version)
            .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
        let plaintext = unprotect_selected_with_context(&protected, secret_ref, version, native)?;
        let plaintext_len = plaintext.len();
        charge_contextual(native, LegacyRevisionDpapiWork::Allocate(plaintext_len))?;
        charge_contextual(native, LegacyRevisionDpapiWork::Process(plaintext_len))?;
        let mut lease_bytes = Vec::new();
        lease_bytes
            .try_reserve_exact(plaintext_len)
            .map_err(|_| QdrantApiSecretStoreError::ContextRefused)?;
        lease_bytes.extend_from_slice(plaintext.expose_secret());
        drop(plaintext);
        contextual_checkpoint(native)?;
        let selected = SecretReference::new(
            secret_ref.stable_reference.clone(),
            secret_ref.binding.clone(),
            version,
        );
        let lease = SecretLease::issue_selected(
            selected,
            &secret_ref.binding,
            issued,
            expires,
            lease_bytes,
            self.limits,
        )
        .map_err(QdrantApiSecretStoreError::LeaseRejected)?;
        contextual_checkpoint(native)?;
        Ok(QdrantApiSecretLease {
            lease,
            issuer_cancellation,
            issuer_deadline,
            expires_at_tick: expires.ticks(),
            encoded_frame_digest,
            issuer_request_id,
            issuer_budget_ref,
            issuer_relative_deadline_ms,
        })
    }
}

impl<C: CancellationProbe + Clone> Port for QdrantApiSecretStore<C> {
    type Error = QdrantApiSecretStoreError;
    type Cancellation = C;
}

impl<C: CancellationProbe + Clone> SecretStorePort for QdrantApiSecretStore<C> {
    type CreateRequest = Infallible;
    type Purpose = QdrantApiSecretPurpose;
    type SecretRef = QdrantApiSecretReference;
    type SecretLease = QdrantApiSecretLease<C>;
    type RotationReceipt = Infallible;
    type DeletionReceipt = Infallible;

    fn create_secret(
        &mut self,
        request: &Self::CreateRequest,
        _context: &OperationContext<Self::Cancellation>,
        _mutation: &MutationIdentity,
    ) -> Result<Self::SecretRef, Self::Error> {
        match *request {}
    }

    fn lease_secret(
        &self,
        secret_ref: &Self::SecretRef,
        purpose: &Self::Purpose,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::SecretLease, Self::Error> {
        self.lease_secret_with_context(secret_ref, purpose, context, &mut UnmeteredQdrantContext)
    }

    fn rotate_secret(
        &mut self,
        _secret_ref: &Self::SecretRef,
        _context: &OperationContext<Self::Cancellation>,
        _mutation: &MutationIdentity,
    ) -> Result<Self::RotationReceipt, Self::Error> {
        Err(QdrantApiSecretStoreError::MutationUnsupported)
    }

    fn delete_secret(
        &mut self,
        _secret_ref: &Self::SecretRef,
        _context: &OperationContext<Self::Cancellation>,
        _mutation: &MutationIdentity,
    ) -> Result<Self::DeletionReceipt, Self::Error> {
        Err(QdrantApiSecretStoreError::MutationUnsupported)
    }
}

#[cfg(windows)]
fn load_selected_windows(
    selected: &QdrantApiSecretReference,
    remaining: &impl Fn() -> Option<Duration>,
    context: &mut impl LegacyRevisionDpapiContext,
) -> Result<(ProtectedSecret, u64, Blake3Digest32), QdrantApiSecretStoreError> {
    use super::windows::WindowsCredentialPlatform;

    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    reserve_qdrant_target(context, false, selected)?;
    let locator = selection_digest(selected);
    let target = credential_target(&locator);
    let record = WindowsCredentialPlatform::read_provider_credential_with_context(&target, context)
        .map_err(|error| {
            contextual_credential_error(&error, QdrantApiSecretStoreError::CredentialReadFailed)
        })?;
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    contextual_checkpoint(context)?;
    let record = record.ok_or(QdrantApiSecretStoreError::SecretNotFound)?;
    reserve_frame_decode(context, record.as_slice().len())?;
    let encoded_frame_digest = encoded_credential_frame_digest(record.as_slice());
    let (version, protected_version, protected_bytes) = decode_record(record.as_slice(), &locator)?;
    let protected = ProtectedSecret::from_bytes(protected_version, protected_bytes)
        .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    Ok((protected, version, encoded_frame_digest))
}

#[cfg(windows)]
fn observe_selected_windows(
    selected: &QdrantApiSecretReference,
    context: &mut impl LegacyRevisionDpapiContext,
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<Option<QdrantApiSecretObservation>, QdrantApiSecretStoreError> {
    use super::windows::{WindowsCredentialPlatform, WindowsVaultLock};

    validate_selected_reference(selected)?;
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    reserve_qdrant_target(context, true, selected)?;
    let locator = selection_digest(selected);
    let target = credential_target(&locator);
    let lock_name = credential_mutex_target(&locator);
    let wait_ms = remaining_lock_wait(remaining)?;
    reserve_qdrant_lock(context)?;
    let lock = WindowsVaultLock::acquire_named(&lock_name, wait_ms)
        .map_err(|()| QdrantApiSecretStoreError::CredentialLockUnavailable)?
        .ok_or(QdrantApiSecretStoreError::CredentialLockUnavailable)?;
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    contextual_checkpoint(context)?;
    let observed =
        WindowsCredentialPlatform::read_provider_credential_with_context(&target, context)
            .map_err(|error| {
                contextual_credential_error(&error, QdrantApiSecretStoreError::CredentialReadFailed)
            })?;
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    contextual_checkpoint(context)?;
    let observation = observed
        .as_ref()
        .map(|frame| {
            reserve_frame_decode(context, frame.as_slice().len())?;
            observation_from_frame(selected, frame, context, remaining)
        })
        .transpose()?;
    drop(lock);
    Ok(observation)
}

#[cfg(windows)]
fn prepare_initial_windows(
    selected: &QdrantApiSecretReference,
    context: &mut impl LegacyRevisionDpapiContext,
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<PreparedQdrantApiSecret, QdrantApiSecretStoreError> {
    use super::windows::{WindowsCredentialPlatform, WindowsVaultLock};

    validate_selected_reference(selected)?;
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    reserve_qdrant_target(context, true, selected)?;
    let locator = selection_digest(selected);
    let target = credential_target(&locator);
    let lock_name = credential_mutex_target(&locator);
    let wait_ms = remaining_lock_wait(remaining)?;
    reserve_qdrant_lock(context)?;
    let lock = WindowsVaultLock::acquire_named(&lock_name, wait_ms)
        .map_err(|()| QdrantApiSecretStoreError::CredentialLockUnavailable)?
        .ok_or(QdrantApiSecretStoreError::CredentialLockUnavailable)?;
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    contextual_checkpoint(context)?;
    let existing =
        WindowsCredentialPlatform::read_provider_credential_with_context(&target, context)
            .map_err(|error| {
                contextual_credential_error(&error, QdrantApiSecretStoreError::CredentialReadFailed)
            })?;
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    contextual_checkpoint(context)?;
    if existing.is_some() {
        return Err(QdrantApiSecretStoreError::CredentialAlreadyExists);
    }

    let version =
        NonZeroRevision::new(1).map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    let plaintext = WindowsCredentialPlatform::generate_qdrant_api_key_with_context(context)
        .map_err(|error| {
            contextual_credential_error(
                &error,
                QdrantApiSecretStoreError::CredentialGenerationFailed,
            )
        })?;
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    contextual_checkpoint(context)?;
    reserve_protection_scope(context, version)?;
    let scope = protection_scope(selected, version)?;
    let protected_bytes =
        crate::protect_short_secret_with_context(context, plaintext.expose_secret(), &scope)
            .map_err(|error| contextual_dpapi_error(&error))?;
    let protected = ProtectedSecret::from_bytes(crate::PROTECTED_SECRET_VERSION, protected_bytes)
        .map_err(|_| QdrantApiSecretStoreError::ProtectionFailed)?;
    drop(plaintext);
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    let total_len = QDRANT_CREDENTIAL_FIXED_BYTES
        .checked_add(protected.len())
        .ok_or(QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    charge_contextual(context, LegacyRevisionDpapiWork::Allocate(total_len))?;
    charge_contextual(
        context,
        LegacyRevisionDpapiWork::Process(total_len.saturating_mul(2)),
    )?;
    let frame = encode_initial_frame(selected, version, &protected)?;
    let frame_digest = encoded_credential_frame_digest(frame.as_slice());
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    contextual_checkpoint(context)?;

    Ok(PreparedQdrantApiSecret {
        selected: selected.clone(),
        version,
        frame_digest,
        native: PreparedWindowsQdrantCredential {
            target,
            frame,
            _lock: lock,
        },
    })
}

#[cfg(windows)]
fn dispatch_prepared_windows(
    prepared: PreparedQdrantApiSecret,
    context: &mut impl LegacyRevisionDpapiContext,
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<QdrantApiSecretObservation, QdrantApiSecretStoreError> {
    use super::windows::WindowsCredentialPlatform;

    let PreparedQdrantApiSecret {
        selected,
        version,
        frame_digest,
        native,
    } = prepared;
    let PreparedWindowsQdrantCredential {
        target,
        mut frame,
        _lock: lock,
    } = native;
    let expected_len = frame.as_slice().len();

    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    let observed =
        WindowsCredentialPlatform::read_provider_credential_with_context(&target, context)
            .map_err(|error| {
                contextual_credential_error(&error, QdrantApiSecretStoreError::CredentialReadFailed)
            })?;
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    contextual_checkpoint(context)?;
    if let Some(observed) = observed {
        reserve_frame_decode(context, observed.as_slice().len())?;
        if observed.as_slice().len() != expected_len
            || encoded_credential_frame_digest(observed.as_slice()) != frame_digest
        {
            return Err(QdrantApiSecretStoreError::CredentialAlreadyExists);
        }
        let observation = observation_from_frame(&selected, &observed, context, remaining)?;
        if observation.version != version {
            return Err(QdrantApiSecretStoreError::CredentialAlreadyExists);
        }
        return Ok(observation);
    }

    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    // Even an error is treated as a possible write; readback below resolves it.
    match WindowsCredentialPlatform::write_qdrant_api_credential_with_context(
        &target, &mut frame, context,
    ) {
        Err(super::windows::CredentialWorkError::Credential) | Ok(()) => {}
        Err(
            super::windows::CredentialWorkError::Context(_)
            | super::windows::CredentialWorkError::KeyGeneration,
        ) => {
            return Err(QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown);
        }
    }
    remaining().ok_or(QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown)?;
    contextual_checkpoint(context)
        .map_err(|_| QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown)?;
    let observed =
        WindowsCredentialPlatform::read_provider_credential_with_context(&target, context)
            .map_err(|error| {
                contextual_credential_error(
                    &error,
                    QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown,
                )
            })?;
    remaining().ok_or(QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown)?;
    contextual_checkpoint(context)
        .map_err(|_| QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown)?;
    let observed = observed.ok_or(QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown)?;
    reserve_frame_decode(context, observed.as_slice().len())
        .map_err(|_| QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown)?;
    if observed.as_slice().len() != expected_len
        || encoded_credential_frame_digest(observed.as_slice()) != frame_digest
    {
        return Err(QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown);
    }
    let readback = observation_from_frame(&selected, &observed, context, remaining)
        .map_err(|_| QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown)?;
    if readback.version != version || readback.frame_digest != frame_digest {
        return Err(QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown);
    }
    // Keep the import use explicit: the retained guard is consumed by drop only.
    drop(lock);
    Ok(readback)
}

#[cfg(windows)]
fn observation_from_frame(
    selected: &QdrantApiSecretReference,
    frame: &super::windows::ProviderCredentialBlob,
    context: &mut impl LegacyRevisionDpapiContext,
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<QdrantApiSecretObservation, QdrantApiSecretStoreError> {
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    contextual_checkpoint(context)?;
    let locator = selection_digest(selected);
    let frame_digest = encoded_credential_frame_digest(frame.as_slice());
    let (raw_version, protected_version, protected_bytes) =
        decode_record(frame.as_slice(), &locator)?;
    let version = NonZeroRevision::new(raw_version)
        .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    let protected = ProtectedSecret::from_bytes(protected_version, protected_bytes)
        .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    let plaintext = unprotect_selected_with_context(&protected, selected, version, context)?;
    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    drop(plaintext);
    Ok(QdrantApiSecretObservation {
        selected: selected.clone(),
        version,
        frame_digest,
    })
}

#[cfg(windows)]
fn reserve_qdrant_target<C: LegacyRevisionDpapiContext>(
    context: &mut C,
    with_lock: bool,
    selected: &QdrantApiSecretReference,
) -> Result<(), QdrantApiSecretStoreError> {
    let target_units = QDRANT_CREDENTIAL_TARGET_PREFIX
        .len()
        .saturating_add(64)
        .saturating_add(1);
    let lock_units = QDRANT_CREDENTIAL_MUTEX_PREFIX
        .len()
        .saturating_add(64)
        .saturating_add(1);
    let target_bytes = target_units.saturating_mul(2);
    let lock_bytes = if with_lock {
        lock_units.saturating_mul(2)
    } else {
        0
    };
    let string_capacity = target_units.saturating_add(if with_lock { lock_units } else { 0 });
    charge_contextual(
        context,
        LegacyRevisionDpapiWork::Allocate(
            target_bytes
                .saturating_add(lock_bytes)
                .saturating_add(string_capacity),
        ),
    )?;
    let selection_bytes = selected
        .stable_reference
        .as_str()
        .len()
        .saturating_add(selected.binding.purpose().as_str().len())
        .saturating_add(96);
    charge_contextual(
        context,
        LegacyRevisionDpapiWork::Process(
            selection_bytes
                .saturating_add(target_bytes)
                .saturating_add(lock_bytes),
        ),
    )
}

#[cfg(windows)]
fn reserve_frame_decode<C: LegacyRevisionDpapiContext>(
    context: &mut C,
    frame_bytes: usize,
) -> Result<(), QdrantApiSecretStoreError> {
    charge_contextual(context, LegacyRevisionDpapiWork::Allocate(frame_bytes))?;
    charge_contextual(
        context,
        LegacyRevisionDpapiWork::Process(frame_bytes.saturating_mul(2)),
    )
}

#[cfg(windows)]
fn reserve_protection_scope<C: LegacyRevisionDpapiContext>(
    context: &mut C,
    version: NonZeroRevision,
) -> Result<(), QdrantApiSecretStoreError> {
    let mut digits = 1_usize;
    let mut value = version.get();
    while value >= 10 {
        value /= 10;
        digits = digits.saturating_add(1);
    }
    let purpose_bytes = QDRANT_API_SECRET_PURPOSE_ID
        .len()
        .saturating_add(1 + 64 + "/version/".len() + digits);
    let component_bytes = 64_usize
        .saturating_add(32)
        .saturating_add(32)
        .saturating_add(purpose_bytes);
    let scope_bytes = "eliot-search.dpapi.scope.v1"
        .len()
        .saturating_add(component_bytes)
        .saturating_add(5 * core::mem::size_of::<u32>());
    charge_contextual(
        context,
        LegacyRevisionDpapiWork::Allocate(
            64_usize
                .saturating_add(32)
                .saturating_add(32)
                .saturating_add(purpose_bytes)
                .saturating_add(scope_bytes.saturating_mul(2)),
        ),
    )?;
    charge_contextual(
        context,
        LegacyRevisionDpapiWork::Process(component_bytes.saturating_add(scope_bytes)),
    )
}

#[cfg(windows)]
fn encode_initial_frame(
    selected: &QdrantApiSecretReference,
    version: NonZeroRevision,
    protected: &ProtectedSecret,
) -> Result<super::windows::ProviderCredentialBlob, QdrantApiSecretStoreError> {
    let payload_len = u32::try_from(protected.len())
        .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    let total_len = QDRANT_CREDENTIAL_FIXED_BYTES
        .checked_add(protected.len())
        .ok_or(QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total_len)
        .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    bytes.extend_from_slice(QDRANT_CREDENTIAL_MAGIC);
    bytes.extend_from_slice(&version.get().to_be_bytes());
    bytes.extend_from_slice(&selection_digest(selected));
    bytes.extend_from_slice(&protected.version().to_be_bytes());
    bytes.extend_from_slice(&payload_len.to_be_bytes());
    bytes.extend_from_slice(protected.as_bytes());
    super::windows::ProviderCredentialBlob::new(bytes)
        .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)
}

#[cfg(windows)]
fn validate_selected_reference(
    selected: &QdrantApiSecretReference,
) -> Result<(), QdrantApiSecretStoreError> {
    if selected.stable_reference.as_str().is_empty()
        || selected.binding.purpose().as_str() != QDRANT_API_SECRET_PURPOSE_ID
    {
        return Err(QdrantApiSecretStoreError::InvalidReference);
    }
    Ok(())
}

#[cfg(windows)]
fn remaining_lock_wait(
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<u32, QdrantApiSecretStoreError> {
    let left = remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    Ok(u32::try_from(left.as_millis().min(25)).unwrap_or(25))
}

#[cfg(windows)]
fn credential_mutex_target(locator: &[u8; 32]) -> Vec<u16> {
    let mut target = String::with_capacity(QDRANT_CREDENTIAL_MUTEX_PREFIX.len() + 64);
    target.push_str(QDRANT_CREDENTIAL_MUTEX_PREFIX);
    push_hex(&mut target, locator);
    target.encode_utf16().chain(core::iter::once(0)).collect()
}

#[cfg(windows)]
fn unprotect_selected_with_context(
    protected: &ProtectedSecret,
    selected: &QdrantApiSecretReference,
    version: NonZeroRevision,
    context: &mut impl LegacyRevisionDpapiContext,
) -> Result<SecretBytes, QdrantApiSecretStoreError> {
    reserve_protection_scope(context, version)?;
    let scope = protection_scope(selected, version)?;
    let mut plaintext =
        crate::unprotect_short_secret_with_context(context, protected.as_bytes(), &scope)
            .map_err(|error| contextual_dpapi_error(&error))?;
    if let Err(error) = charge_contextual(
        context,
        LegacyRevisionDpapiWork::Process(plaintext.len().saturating_mul(2)),
    ) {
        crate::model::clear_bytes(&mut plaintext);
        return Err(error);
    }
    SecretBytes::new(plaintext).map_err(|_| QdrantApiSecretStoreError::ProtectionFailed)
}

#[cfg(not(windows))]
fn unprotect_selected_with_context(
    _protected: &ProtectedSecret,
    _selected: &QdrantApiSecretReference,
    _version: NonZeroRevision,
    _context: &mut impl LegacyRevisionDpapiContext,
) -> Result<SecretBytes, QdrantApiSecretStoreError> {
    Err(QdrantApiSecretStoreError::UnsupportedPlatform)
}

fn selection_digest(selected: &QdrantApiSecretReference) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-search/qdrant-api-secret-selection/v1\0");
    update_field(&mut hasher, selected.stable_reference.as_str().as_bytes());
    update_field(&mut hasher, selected.binding.installation_id().as_bytes());
    update_field(
        &mut hasher,
        selected.binding.installation_incarnation_id().as_bytes(),
    );
    update_field(&mut hasher, selected.binding.user_scope_digest().as_bytes());
    update_field(&mut hasher, selected.binding.purpose().as_str().as_bytes());
    *hasher.finalize().as_bytes()
}

fn encoded_credential_frame_digest(frame: &[u8]) -> Blake3Digest32 {
    Blake3Digest32::from_bytes(*blake3::hash(frame).as_bytes())
}

fn update_field(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

#[cfg(windows)]
fn credential_target(locator: &[u8; 32]) -> Vec<u16> {
    let mut target = String::with_capacity(QDRANT_CREDENTIAL_TARGET_PREFIX.len() + 64);
    target.push_str(QDRANT_CREDENTIAL_TARGET_PREFIX);
    push_hex(&mut target, locator);
    target.encode_utf16().chain(core::iter::once(0)).collect()
}

fn decode_record(
    bytes: &[u8],
    expected_selection_digest: &[u8; 32],
) -> Result<(u64, u16, Vec<u8>), QdrantApiSecretStoreError> {
    if bytes.len() < QDRANT_CREDENTIAL_FIXED_BYTES
        || bytes.get(..8) != Some(QDRANT_CREDENTIAL_MAGIC)
    {
        return Err(QdrantApiSecretStoreError::CredentialRecordInvalid);
    }
    let version = u64::from_be_bytes(
        bytes[8..16]
            .try_into()
            .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?,
    );
    if version == 0 {
        return Err(QdrantApiSecretStoreError::CredentialRecordInvalid);
    }
    let mut difference = 0_u8;
    for (observed, expected) in bytes[16..48].iter().zip(expected_selection_digest) {
        difference |= observed ^ expected;
    }
    if difference != 0 {
        return Err(QdrantApiSecretStoreError::CredentialRecordInvalid);
    }
    let protected_version = u16::from_be_bytes(
        bytes[48..50]
            .try_into()
            .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?,
    );
    let protected_len = usize::try_from(u32::from_be_bytes(
        bytes[50..54]
            .try_into()
            .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?,
    ))
    .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    let payload = bytes
        .get(QDRANT_CREDENTIAL_FIXED_BYTES..)
        .filter(|payload| payload.len() == protected_len)
        .ok_or(QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    let mut protected_bytes = Vec::new();
    protected_bytes
        .try_reserve_exact(protected_len)
        .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    protected_bytes.extend_from_slice(payload);
    Ok((version, protected_version, protected_bytes))
}

#[cfg(any(windows, test))]
fn versioned_protection_purpose(locator: &[u8; 32], version: NonZeroRevision) -> String {
    format!(
        "{QDRANT_API_SECRET_PURPOSE_ID}/{}/version/{}",
        hex(locator),
        version.get(),
    )
}

#[cfg(windows)]
fn protection_scope(
    selected: &QdrantApiSecretReference,
    version: NonZeroRevision,
) -> Result<ProtectionScope, QdrantApiSecretStoreError> {
    let locator = selection_digest(selected);
    let user = hex(selected.binding.user_scope_digest().as_bytes());
    let installation = hex(selected.binding.installation_id().as_bytes());
    let incarnation = hex(selected.binding.installation_incarnation_id().as_bytes());
    let purpose = versioned_protection_purpose(&locator, version);
    ProtectionScope::new(&user, &installation, &incarnation, &purpose)
        .map_err(|_| QdrantApiSecretStoreError::InvalidReference)
}

#[cfg(windows)]
fn monotonic_now() -> MonotonicInstant {
    MonotonicInstant::from_ticks(super::windows::monotonic_millis())
}

#[cfg(not(windows))]
fn monotonic_now() -> Result<MonotonicInstant, QdrantApiSecretStoreError> {
    Err(QdrantApiSecretStoreError::UnsupportedPlatform)
}

#[cfg(windows)]
fn push_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 15)]));
    }
}

#[cfg(any(windows, test))]
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 15)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use search_contracts::{
        Blake3Digest32, InstallationId, InstallationIncarnationId, NonZeroRevision, OpaqueId,
        OpaqueRef, RequestId,
    };
    use search_ports::{CancellationProbe, OperationContext, PackageOpaque};

    use super::{
        QDRANT_API_SECRET_PURPOSE_ID, QDRANT_CREDENTIAL_FIXED_BYTES, QDRANT_CREDENTIAL_MAGIC,
        QdrantApiSecretReference, QdrantApiSecretStoreError, decode_record,
        encoded_credential_frame_digest, issuer_context_allows_use, lease_window_is_live,
        selection_digest, versioned_protection_purpose,
    };

    #[derive(Clone, Debug)]
    struct TestCancellation(Arc<AtomicBool>);

    impl PackageOpaque for TestCancellation {
        fn owner_package(&self) -> &'static str {
            "search-os-secrets-windows-tests"
        }
    }

    impl CancellationProbe for TestCancellation {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::Acquire)
        }
    }

    fn context(
        request: u8,
        budget: &str,
        cancellation: TestCancellation,
    ) -> OperationContext<TestCancellation> {
        OperationContext::new(
            RequestId::from_bytes([request; 16]),
            5_000,
            cancellation,
            OpaqueRef::new(budget).expect("budget reference"),
        )
        .expect("finite operation context")
    }

    fn selected(reference: &str, incarnation: u8) -> QdrantApiSecretReference {
        QdrantApiSecretReference::for_owner(
            OpaqueId::new(reference).expect("stable reference"),
            InstallationId::from_bytes([0x31; 16]),
            InstallationIncarnationId::from_bytes([incarnation; 16]),
            Blake3Digest32::from_bytes([0x52; 32]),
        )
        .expect("Qdrant-purpose binding")
    }

    fn record(selection: [u8; 32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(QDRANT_CREDENTIAL_FIXED_BYTES + 3);
        bytes.extend_from_slice(QDRANT_CREDENTIAL_MAGIC);
        bytes.extend_from_slice(&7_u64.to_be_bytes());
        bytes.extend_from_slice(&selection);
        bytes.extend_from_slice(&1_u16.to_be_bytes());
        bytes.extend_from_slice(&3_u32.to_be_bytes());
        bytes.extend_from_slice(b"box");
        bytes
    }

    #[test]
    fn locator_binds_reference_root_incarnation_user_and_qdrant_purpose() {
        let reference = selected("secret://credential/qdrant", 0x41);
        let same = selected("secret://credential/qdrant", 0x41);
        let foreign_reference = selected("secret://credential/other", 0x41);
        let foreign_incarnation = selected("secret://credential/qdrant", 0x42);

        assert_eq!(selection_digest(&reference), selection_digest(&same));
        assert_ne!(
            selection_digest(&reference),
            selection_digest(&foreign_reference)
        );
        assert_ne!(
            selection_digest(&reference),
            selection_digest(&foreign_incarnation)
        );
        assert_eq!(
            reference.binding().purpose().as_str(),
            QDRANT_API_SECRET_PURPOSE_ID
        );
    }

    #[test]
    fn selected_credential_frame_requires_exact_reference_digest_and_bounds() {
        let selected = selected("secret://credential/qdrant", 0x41);
        let digest = selection_digest(&selected);
        let exact = record(digest);
        let (version, protected_version, payload) =
            decode_record(&exact, &digest).expect("exact record");
        assert_eq!(version, 7);
        assert_eq!(protected_version, 1);
        assert_eq!(payload, b"box");

        assert_eq!(
            decode_record(&exact, &[0x99; 32]),
            Err(QdrantApiSecretStoreError::CredentialRecordInvalid)
        );
        assert_eq!(
            decode_record(&exact[..QDRANT_CREDENTIAL_FIXED_BYTES - 1], &digest),
            Err(QdrantApiSecretStoreError::CredentialRecordInvalid)
        );
    }

    #[test]
    fn record_version_is_bound_to_dpapi_purpose_while_locator_stays_stable() {
        let selected = selected("secret://credential/qdrant", 0x41);
        let locator = selection_digest(&selected);
        let version_seven = NonZeroRevision::new(7).expect("nonzero version");
        let version_eight = NonZeroRevision::new(8).expect("nonzero version");
        let frame = record(locator);
        let mut changed_header = frame.clone();
        changed_header[8..16].copy_from_slice(&8_u64.to_be_bytes());
        let (observed_version, _, _) =
            decode_record(&changed_header, &locator).expect("bounded version header");

        let exact_purpose = versioned_protection_purpose(&locator, version_seven);
        let changed_header_purpose = versioned_protection_purpose(
            &locator,
            NonZeroRevision::new(observed_version).expect("nonzero observed version"),
        );

        assert_eq!(selection_digest(&selected), locator);
        assert_eq!(observed_version, version_eight.get());
        assert_ne!(
            encoded_credential_frame_digest(&frame),
            encoded_credential_frame_digest(&changed_header),
        );
        assert_ne!(exact_purpose, changed_header_purpose);
        assert!(exact_purpose.ends_with("/version/7"));
        assert!(changed_header_purpose.ends_with("/version/8"));
    }

    #[test]
    fn lease_use_remains_bound_to_issuer_request_cancellation_and_deadline() {
        let issuer_cancellation = TestCancellation(Arc::new(AtomicBool::new(false)));
        let issuer = context(1, "budget:qdrant-startup", issuer_cancellation.clone());
        let issuer_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let exact_context = context(1, "budget:qdrant-startup", issuer_cancellation.clone());
        let now = std::time::Instant::now();
        assert!(issuer_context_allows_use(
            &issuer_cancellation,
            issuer_deadline,
            issuer.request_id(),
            issuer.budget_ref(),
            issuer.relative_deadline_ms().get(),
            &exact_context,
            now,
        ));

        let substituted_context = context(
            2,
            "budget:fresh-request",
            TestCancellation(Arc::new(AtomicBool::new(false))),
        );
        assert!(!issuer_context_allows_use(
            &issuer_cancellation,
            issuer_deadline,
            issuer.request_id(),
            issuer.budget_ref(),
            issuer.relative_deadline_ms().get(),
            &substituted_context,
            now,
        ));

        assert!(!issuer_context_allows_use(
            &issuer_cancellation,
            issuer_deadline,
            issuer.request_id(),
            issuer.budget_ref(),
            issuer.relative_deadline_ms().get(),
            &exact_context,
            issuer_deadline,
        ));

        issuer_cancellation.0.store(true, Ordering::Release);
        assert!(!issuer_context_allows_use(
            &issuer_cancellation,
            issuer_deadline,
            issuer.request_id(),
            issuer.budget_ref(),
            issuer.relative_deadline_ms().get(),
            &exact_context,
            now,
        ));
    }

    #[test]
    fn secret_use_rechecks_ttl_after_callback_even_when_issuer_remains_live() {
        let cancellation = TestCancellation(Arc::new(AtomicBool::new(false)));
        let issuer = context(7, "budget:qdrant-startup", cancellation.clone());
        let issuer_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let expires = NonZeroU64::new(1_000).expect("nonzero expiry");
        let live_after_callback = NonZeroU64::new(999).expect("nonzero tick");
        let expired_after_callback = NonZeroU64::new(1_000).expect("expiry tick");
        let now = std::time::Instant::now();

        assert!(issuer_context_allows_use(
            &cancellation,
            issuer_deadline,
            issuer.request_id(),
            issuer.budget_ref(),
            issuer.relative_deadline_ms().get(),
            &issuer,
            now,
        ));
        assert!(lease_window_is_live(live_after_callback, expires));
        assert!(!lease_window_is_live(expired_after_callback, expires));
    }
}
