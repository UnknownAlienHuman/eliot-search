//! Read-only Qdrant API-key selection from the current-user Windows secret store.
//!
//! The credential name and DPAPI entropy bind the stable config reference to
//! the exact Search installation, incarnation, current-user scope, and Qdrant
//! API-key purpose. This adapter never creates, rotates, or deletes a key.

use core::{convert::Infallible, fmt, marker::PhantomData, num::NonZeroU64, time::Duration};
use std::time::Instant;

use crate::{ProtectedSecret, ProtectionScope, SecretBytes, unprotect_current_user};
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
#[cfg(any(windows, test))]
const QDRANT_CREDENTIAL_FIXED_BYTES: usize = 8 + 8 + 32 + 2 + 4;
const MIN_LEASE_TTL_MS: u64 = 1_000;
const MAX_LEASE_TTL_MS: u64 = 300_000;

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

/// Redacted failure from the read-only Qdrant selected-secret adapter.
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
            Self::SecretNotFound | Self::UnsupportedPlatform => {
                PortErrorKind::DependencyUnavailable
            }
            Self::ContextRefused => PortErrorKind::DeadlineDuringOperation,
            Self::InvalidReference | Self::MutationUnsupported | Self::InvalidLeasePolicy => {
                PortErrorKind::InvalidInput
            }
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
            | Self::UnsupportedPlatform => PortRetryability::SameRequest,
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
    issuer_request_id: RequestId,
    issuer_budget_ref: OpaqueRef,
    issuer_relative_deadline_ms: u64,
    reference_digest: Blake3Digest32,
    installation_incarnation_id: InstallationIncarnationId,
}

/// Non-secret launch evidence sampled from a live Qdrant API lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QdrantApiSecretLeaseEvidence {
    reference_digest: Blake3Digest32,
    installation_incarnation_id: InstallationIncarnationId,
    purpose_digest: Blake3Digest32,
    observed_tick: NonZeroU64,
    expires_at_tick: NonZeroU64,
}

impl QdrantApiSecretLeaseEvidence {
    /// Digest of the exact selected Qdrant secret reference and binding.
    #[must_use]
    pub const fn reference_digest(self) -> Blake3Digest32 {
        self.reference_digest
    }

    /// Installation incarnation bound into the selected credential.
    #[must_use]
    pub const fn installation_incarnation_id(self) -> InstallationIncarnationId {
        self.installation_incarnation_id
    }

    /// Fixed Qdrant API-key purpose digest.
    #[must_use]
    pub const fn purpose_digest(self) -> Blake3Digest32 {
        self.purpose_digest
    }

    /// Actual Qdrant-secret monotonic clock sample for launch-plan validation.
    #[must_use]
    pub const fn observed_tick(self) -> NonZeroU64 {
        self.observed_tick
    }

    /// Exact lease expiry on that same monotonic clock.
    #[must_use]
    pub const fn expires_at_tick(self) -> NonZeroU64 {
        self.expires_at_tick
    }
}

impl<C: CancellationProbe + Clone> QdrantApiSecretLease<C> {
    /// Exact versioned reference whose bytes are privately leased.
    #[must_use]
    pub const fn reference(&self) -> &SecretReference {
        self.lease.reference()
    }

    /// Captures non-secret launch evidence only while the exact issuer context
    /// and finite lease remain live. The caller cannot substitute its clock.
    pub fn launch_evidence(
        &self,
        context: &OperationContext<impl CancellationProbe>,
    ) -> Result<QdrantApiSecretLeaseEvidence, QdrantApiSecretStoreError> {
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
        ) {
            return Err(QdrantApiSecretStoreError::ContextRefused);
        }
        #[cfg(windows)]
        let observed = monotonic_now().ticks();
        #[cfg(not(windows))]
        let observed = monotonic_now()?.ticks();
        let observed_tick =
            NonZeroU64::new(observed).ok_or(QdrantApiSecretStoreError::InvalidLeasePolicy)?;
        let expires_at_tick = NonZeroU64::new(self.expires_at_tick)
            .ok_or(QdrantApiSecretStoreError::InvalidLeasePolicy)?;
        if !lease_window_is_live(observed_tick, expires_at_tick) {
            return Err(QdrantApiSecretStoreError::ContextRefused);
        }
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
        ) {
            return Err(QdrantApiSecretStoreError::ContextRefused);
        }
        Ok(QdrantApiSecretLeaseEvidence {
            reference_digest: self.reference_digest,
            installation_incarnation_id: self.installation_incarnation_id,
            purpose_digest: Blake3Digest32::from_bytes(
                *blake3::hash(QDRANT_API_SECRET_PURPOSE_ID.as_bytes()).as_bytes(),
            ),
            observed_tick,
            expires_at_tick,
        })
    }

    /// Exposes the bytes only during a callback, after checking the same
    /// Windows monotonic clock that bounded lease creation.
    pub fn with_secret<T>(
        &self,
        context: &OperationContext<impl CancellationProbe>,
        consume: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, QdrantApiSecretStoreError> {
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
        ) {
            return Err(QdrantApiSecretStoreError::ContextRefused);
        }
        #[cfg(windows)]
        let now = monotonic_now();
        #[cfg(not(windows))]
        let now = monotonic_now()?;
        let result = self
            .lease
            .with_secret(now, consume)
            .map_err(QdrantApiSecretStoreError::LeaseRejected)?;
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
        ) {
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
        if *purpose != QdrantApiSecretPurpose::new()
            || secret_ref.binding.purpose().as_str() != QDRANT_API_SECRET_PURPOSE_ID
        {
            return Err(QdrantApiSecretStoreError::InvalidReference);
        }
        context
            .preflight()
            .map_err(|_| QdrantApiSecretStoreError::ContextRefused)?;
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
        let (protected, version) = load_selected_windows(secret_ref, &remaining)?;
        #[cfg(not(windows))]
        return Err(QdrantApiSecretStoreError::UnsupportedPlatform);
        if remaining().is_none() {
            return Err(QdrantApiSecretStoreError::ContextRefused);
        }
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
        let plaintext = unprotect_selected(&protected, secret_ref)?;
        if remaining().is_none() {
            return Err(QdrantApiSecretStoreError::ContextRefused);
        }
        let version = NonZeroRevision::new(version)
            .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
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
            plaintext.expose_secret().to_vec(),
            self.limits,
        )
        .map_err(QdrantApiSecretStoreError::LeaseRejected)?;
        Ok(QdrantApiSecretLease {
            lease,
            issuer_cancellation,
            issuer_deadline,
            expires_at_tick: expires.ticks(),
            issuer_request_id,
            issuer_budget_ref,
            issuer_relative_deadline_ms,
            reference_digest: Blake3Digest32::from_bytes(selection_digest(secret_ref)),
            installation_incarnation_id: secret_ref.binding.installation_incarnation_id(),
        })
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
) -> Result<(ProtectedSecret, u64), QdrantApiSecretStoreError> {
    use super::windows::WindowsCredentialPlatform;

    remaining().ok_or(QdrantApiSecretStoreError::ContextRefused)?;
    let locator = selection_digest(selected);
    let record = WindowsCredentialPlatform::read_provider_credential(&credential_target(&locator))
        .map_err(|_| QdrantApiSecretStoreError::CredentialReadFailed)?
        .ok_or(QdrantApiSecretStoreError::SecretNotFound)?;
    if remaining().is_none() {
        return Err(QdrantApiSecretStoreError::ContextRefused);
    }
    let (version, protected_version, protected_bytes) = decode_record(record.as_slice(), &locator)?;
    let protected = ProtectedSecret::from_bytes(protected_version, protected_bytes)
        .map_err(|_| QdrantApiSecretStoreError::CredentialRecordInvalid)?;
    Ok((protected, version))
}

#[cfg(windows)]
fn unprotect_selected(
    protected: &ProtectedSecret,
    selected: &QdrantApiSecretReference,
) -> Result<SecretBytes, QdrantApiSecretStoreError> {
    let scope = protection_scope(selected)?;
    unprotect_current_user(protected, &scope)
        .map_err(|_| QdrantApiSecretStoreError::ProtectionFailed)
}

#[cfg(not(windows))]
fn unprotect_selected(
    _protected: &ProtectedSecret,
    _selected: &QdrantApiSecretReference,
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

#[cfg(windows)]
fn protection_scope(
    selected: &QdrantApiSecretReference,
) -> Result<ProtectionScope, QdrantApiSecretStoreError> {
    let locator = selection_digest(selected);
    let user = hex(selected.binding.user_scope_digest().as_bytes());
    let installation = hex(selected.binding.installation_id().as_bytes());
    let incarnation = hex(selected.binding.installation_incarnation_id().as_bytes());
    let purpose = format!("{QDRANT_API_SECRET_PURPOSE_ID}/{}", hex(&locator));
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

#[cfg(windows)]
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
        Blake3Digest32, InstallationId, InstallationIncarnationId, OpaqueId, OpaqueRef, RequestId,
    };
    use search_ports::{CancellationProbe, OperationContext, PackageOpaque};

    use super::{
        QDRANT_API_SECRET_PURPOSE_ID, QDRANT_CREDENTIAL_FIXED_BYTES, QDRANT_CREDENTIAL_MAGIC,
        QdrantApiSecretReference, QdrantApiSecretStoreError, decode_record,
        issuer_context_allows_use, lease_window_is_live, selection_digest,
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
    fn launch_evidence_requires_the_same_live_monotonic_lease_window() {
        let issued = NonZeroU64::new(41).expect("nonzero tick");
        let expired = NonZeroU64::new(40).expect("nonzero tick");
        let live = NonZeroU64::new(42).expect("nonzero tick");
        assert!(!lease_window_is_live(issued, expired));
        assert!(lease_window_is_live(issued, live));
        assert!(!lease_window_is_live(live, live));
    }
}
