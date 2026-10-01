//! Standalone policy metadata on the existing native control journal.
//!
//! No binding or policy is inferred from pairing, roots, client claims or a
//! token file. Administrative writes and publication stay explicit. The read
//! adapter is operation-scoped and borrows the actual journal/publisher under
//! their owner's lock; it does not open a database or create a policy cache.

mod codec;
mod mutation;

pub use mutation::StandalonePolicyMutation;
pub use search_contracts::{StandalonePolicyRecord, StandalonePolicyState};

use search_contracts::{
    AuthoritativeGrantPolicy, BindingId, InstallationIncarnationId, OpaqueId, OpaqueRef,
    protocol::PeerRole,
};
use search_control_redb::{
    ControlCallError, ControlError, ControlKey, ControlSnapshotPublisher, JournalLimits,
    PersistentControlJournal, provider_authority::ProviderAuthorityJournalError,
};
use search_ports::OperationContext;
use search_provider_protocol::{BindingContext, MonotonicMillis, RequestGuard};

use crate::provider_composition::monotonic_millis;
use super::{
    GrantIssuerError, GrantUseError, NativeBindingError, NativeBindingPin,
    StandaloneGrantIssuer, StandaloneGrantMaterial, StandaloneGrantPolicySource,
    StandaloneGrantTemplate, SystemGrantClock,
};

/// Retains native read/mutation failures without printing policy contents.
#[derive(Debug)]
pub enum NativeGrantPolicyError {
    /// Missing, malformed, contradictory or unsupported policy metadata.
    InvalidRecord,
    /// Original schema/identity/generation error before a native write.
    Control(ControlError),
    /// Original call error, including possible commit and interruption metadata.
    Call(ControlCallError),
    /// Binding, lifecycle, clock or original-request refusal.
    Grant(GrantUseError),
    /// Native registration changed, expired, was revoked or failed readback.
    Binding(NativeBindingError),
}

impl std::fmt::Display for NativeGrantPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRecord => formatter.write_str("DAEMON_GRANT_POLICY_RECORD_INVALID"),
            Self::Control(error) => std::fmt::Display::fmt(error, formatter),
            Self::Call(error) => std::fmt::Display::fmt(error, formatter),
            Self::Grant(error) => std::fmt::Display::fmt(error, formatter),
            Self::Binding(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}
impl std::error::Error for NativeGrantPolicyError {}
impl From<ControlError> for NativeGrantPolicyError {
    fn from(error: ControlError) -> Self { Self::Control(error) }
}
impl From<ControlCallError> for NativeGrantPolicyError {
    fn from(error: ControlCallError) -> Self { Self::Call(error) }
}
impl From<ProviderAuthorityJournalError> for NativeGrantPolicyError {
    fn from(error: ProviderAuthorityJournalError) -> Self {
        match error {
            ProviderAuthorityJournalError::Record(error) => Self::Control(error),
            ProviderAuthorityJournalError::Call(error) => Self::Call(error),
        }
    }
}
impl From<GrantUseError> for NativeGrantPolicyError {
    fn from(error: GrantUseError) -> Self { Self::Grant(error) }
}
impl From<NativeBindingError> for NativeGrantPolicyError {
    fn from(error: NativeBindingError) -> Self { Self::Binding(error) }
}

pub(super) fn validate_policy_record(
    record: &StandalonePolicyRecord,
) -> Result<(), NativeGrantPolicyError> {
    record
        .validate_shape()
        .map_err(|_| NativeGrantPolicyError::InvalidRecord)
}

/// A current read observation, not a cacheable authorization permit. Retain the
/// native owner lock and revalidate the request/domain across actual work/output.
pub struct CurrentStandalonePolicy {
    record: StandalonePolicyRecord,
    valid_until: MonotonicMillis,
}

impl CurrentStandalonePolicy {
    /// Exact stored policy after identity, state and lifetime checks.
    #[must_use]
    pub const fn policy(&self) -> &AuthoritativeGrantPolicy { &self.record.policy }

    /// Earliest binding/policy expiry and original request deadline, never a lease.
    #[must_use]
    pub const fn valid_until(&self) -> MonotonicMillis { self.valid_until }
}

/// Native policy source for one already admitted operation. Its borrows must
/// come from the real held root/policy lock, and its guard from that session.
/// Recreate only the adapter for another operation, not the issuer ledger or
/// rollback-fenced clock. No source content or crypto key enters this type.
/// Use `SessionBoundGrantAuthority::new(source, &mut issuer)` to run the existing
/// grant-issuance path without moving, resetting or duplicating its ledger.
pub struct JournalStandaloneGrantPolicySource<'a> {
    journal: &'a PersistentControlJournal,
    publisher: &'a ControlSnapshotPublisher,
    binding_pin: &'a NativeBindingPin,
    boot_id: &'a OpaqueId,
    request: &'a RequestGuard,
    clock: &'a mut SystemGrantClock,
}

impl<'a> JournalStandaloneGrantPolicySource<'a> {
    /// Borrow the existing owners; this performs no initialization/publication.
    /// Supply the pin returned by open_published for this exact paired session.
    /// The caller still validates the exact guard against its BoundSession.
    ///
    /// # Errors
    /// Rejects cancelled, terminal, expired or deadline-free request guards.
    pub fn new(
        journal: &'a PersistentControlJournal,
        publisher: &'a ControlSnapshotPublisher,
        binding_pin: &'a NativeBindingPin,
        boot_id: &'a OpaqueId,
        request: &'a RequestGuard,
        clock: &'a mut SystemGrantClock,
    ) -> Result<Self, NativeGrantPolicyError> {
        remaining(request)?;
        Ok(Self {
            journal,
            publisher,
            binding_pin,
            boot_id,
            request,
            clock,
        })
    }

    /// Read the pinned binding and native policy from the current disk-published
    /// head in one coherent two-record read. Any changed binding field requires
    /// a new authenticated connection; partial or inconsistent pairs fail closed.
    /// The unchanged request deadline includes lookup, decode and UTC checks.
    /// Inactive/expired/foreign/missing rows never produce permissive defaults.
    ///
    /// # Errors
    /// Returns current-head, record, binding, lifetime or interruption failure.
    pub fn current(
        &mut self,
        binding: &BindingContext,
    ) -> Result<CurrentStandalonePolicy, NativeGrantPolicyError> {
        if binding.role() != PeerRole::StandaloneCli
            || self.journal.identity().installation_incarnation_id != binding.incarnation()
        {
            return Err(GrantUseError::BindingMismatch.into());
        }
        let context = OperationContext::new(
            *self.request.request_id(),
            remaining(self.request)?,
            self.request.cancellation(),
            OpaqueRef::new("standalone-policy-read-v1")
                .map_err(|_| NativeGrantPolicyError::InvalidRecord)?,
        )
        .map_err(|_| NativeGrantPolicyError::Grant(GrantUseError::RequestInactive))?;
        let (registration, binding_expiry) = self.binding_pin.read_standalone_registration(
            binding,
            self.journal,
            self.publisher,
            self.clock,
            &context,
        )?;
        remaining(self.request)?;
        let record = registration
            .into_policy()
            .ok_or(GrantUseError::PolicyUnavailable)?;
        if record.state != StandalonePolicyState::Active {
            return Err(GrantUseError::PolicyUnavailable.into());
        }
        let policy = &record.policy;
        if policy.binding_id != binding.binding_id()
            || policy.installation_incarnation_id != binding.incarnation()
            || policy.installation_id != self.binding_pin.record().installation_id
            || policy.binding_generation != self.binding_pin.record().pairing_generation.get()
            || &policy.issued_boot_id != self.boot_id
        {
            return Err(GrantUseError::BindingMismatch.into());
        }
        let expiry = self
            .clock
            .check_policy_window(&record.issued_at, record.expires_at.as_ref())?;
        let deadline = self
            .request
            .deadline()
            .ok_or(GrantUseError::RequestInactive)?;
        let deadline = binding_expiry.map_or(deadline, |end| deadline.min(end));
        let valid_until = expiry.map_or(deadline, |end| deadline.min(end));
        remaining(self.request)?;
        if monotonic_millis() >= valid_until {
            return Err(GrantUseError::Expired.into());
        }
        Ok(CurrentStandalonePolicy { record, valid_until })
    }
}

impl StandaloneGrantPolicySource for JournalStandaloneGrantPolicySource<'_> {
    type Error = NativeGrantPolicyError;

    fn snapshot(
        &mut self,
        binding: &BindingContext,
    ) -> Result<AuthoritativeGrantPolicy, Self::Error> {
        self.current(binding).map(|current| current.record.policy)
    }
}

fn remaining(request: &RequestGuard) -> Result<u64, GrantUseError> {
    let now = monotonic_millis();
    if request.is_cancelled()
        || now < request.admitted_at()
        || request
            .progress()
            .is_some_and(|progress| progress.terminal().is_some())
    {
        return Err(GrantUseError::RequestInactive);
    }
    request
        .deadline()
        .and_then(|end| end.get().checked_sub(now.get()))
        .filter(|left| *left > 0)
        .ok_or(GrantUseError::RequestInactive)
}

pub(super) fn policy_key(
    incarnation: InstallationIncarnationId,
    binding: BindingId,
) -> Result<ControlKey, ControlError> {
    let mut bytes = b"eliot.control.standalone-policy.v1\0".to_vec();
    bytes.extend_from_slice(incarnation.as_bytes());
    bytes.extend_from_slice(binding.as_bytes());
    ControlKey::new(bytes, JournalLimits::BASELINE)
}

// An operation-scoped native source can use the original long-lived issuer.
// Forward the mutation to that same owner; borrowing does not create a ledger.
impl<I: StandaloneGrantIssuer + ?Sized> StandaloneGrantIssuer for &mut I {
    fn issue(
        &mut self,
        template: &StandaloneGrantTemplate,
    ) -> Result<StandaloneGrantMaterial, GrantIssuerError> {
        (**self).issue(template)
    }
}
