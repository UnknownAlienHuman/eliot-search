//! Durable provider bindings on the existing technical journal (W8 section 3).
//! Binding metadata is separate from grant policy, credentials and source scope.

mod codec;
mod mutation;
mod opening;
mod registration;

pub use mutation::ProviderBindingMutation;
pub use opening::{
    BindingConnectionRegistry, BindingConnectionRegistryError, BindingDrainReceipt,
    MAX_REGISTERED_BINDING_CONNECTIONS, NativeBindingExpectation,
    NativePairingCredentialError, NativePairingCredentialIntent, NativeTcpOpenError,
};
pub use registration::{
    StandaloneProvisioningError, StandaloneProvisioningPhase,
    StandalonePublicationError, StandalonePublicationReceipt, StandaloneRegistrationCommit,
    StandaloneRegistrationMutation, StandaloneRegistrationProvisioning,
    StandaloneRegistrationReadback, publish_committed_standalone_registration,
};

use search_contracts::{BindingId, Blake3Digest32, BoundedSet, InstallationId,
    InstallationIncarnationId, NonZeroRevision, OpaqueRef, ProfileId, UtcTimestamp,
    MAX_SET_ITEMS, protocol::PeerRole};
use search_control_redb::{ControlCallError, ControlError, ControlKey,
    ControlSnapshotPublisher, JournalIdentity, JournalLimits, PersistentControlJournal};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{BindingContext, MonotonicMillis, ProtocolError};

use crate::provider_composition::monotonic_millis;
use super::{GrantUseError, SystemGrantClock};

/// Persisted terminal states are retained; they never become an absent binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderBindingStatus {
    /// Registered peer; current time, profile and pairing still require checks.
    Active,
    /// Explicit revocation; this binding identity cannot be reactivated.
    Revoked,
    /// Explicitly recorded expiry; this binding identity cannot be reactivated.
    Expired,
}

/// Exact W8 BindingRecord metadata. Constructing a record authenticates nothing.
/// Native administration alone supplies these fields; no key or raw credential
/// is stored here. The selected profile and disclosure reference are locators,
/// not a substitute for the grant/source owners' actual authorization checks.
#[derive(Clone, Eq, PartialEq)]
pub struct ProviderBindingRecord {
    /// Durable provider-local binding identity.
    pub binding_id: BindingId,
    /// Owning installation, independent from the process boot identity.
    pub installation_id: InstallationId,
    /// Owning installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Only standalone_cli and client_adapter are admitted peers.
    pub peer_role: PeerRole,
    /// Native identity digest resolved with the peer's credential registration.
    pub peer_identity_digest: Blake3Digest32,
    /// Generation of the registered pairing, never inferred from an old session.
    pub pairing_generation: NonZeroRevision,
    /// Explicitly permitted client profiles; not recipe or budget aliases.
    pub permitted_profile_ids: BoundedSet<ProfileId, MAX_SET_ITEMS>,
    /// Reference resolved by the native disclosure-policy owner.
    pub disclosure_ceiling_ref: OpaqueRef,
    /// Trusted issue time in the shared canonical UTC format.
    pub issued_at: UtcTimestamp,
    /// Optional binding expiry; request and grant deadlines remain finite.
    pub expires_at: Option<UtcTimestamp>,
    /// Monotone binding revocation revision, distinct from grant/domain revisions.
    pub revocation_generation: NonZeroRevision,
    /// Durable lifecycle state.
    pub status: ProviderBindingStatus,
}

impl ProviderBindingRecord {
    fn validate(&self) -> Result<(), NativeBindingError> {
        if !matches!(self.peer_role, PeerRole::StandaloneCli | PeerRole::ClientAdapter)
            || self.expires_at.as_ref().is_some_and(|end| end <= &self.issued_at)
        {
            return Err(NativeBindingError::InvalidRecord);
        }
        Ok(())
    }

    /// Inspect one exact current disk-published record, including terminal rows.
    /// This is metadata readback, not a pairing or grant permit. Missing remains
    /// None; stale/unpublished/corrupt state remains an error. No full table scan,
    /// initialization, publication, fallback or recovery occurs here.
    pub fn read_published<C: CancellationProbe>(
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        binding: BindingId,
        context: &OperationContext<C>,
    ) -> Result<Option<Self>, NativeBindingError> {
        let (started, deadline) = begin(context)?;
        let incarnation = journal.identity().installation_incarnation_id;
        let key = binding_key(incarnation, binding)?;
        let value = journal.read_published_record(publisher, &key, context)?;
        let record = value.as_ref().map(codec::decode).transpose()?;
        if record.as_ref().is_some_and(|record| record.binding_id != binding
            || record.installation_incarnation_id != incarnation)
        {
            return Err(NativeBindingError::InvalidRecord);
        }
        check(context, started, deadline)?;
        Ok(record)
    }
}

impl std::fmt::Debug for ProviderBindingRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderBindingRecord").field("status", &self.status)
            .field("pairing_generation", &self.pairing_generation).finish_non_exhaustive()
    }
}

/// Record pinned by a successful native session open, not a reusable permit.
/// No Clone or public constructor: an arbitrary historical record cannot be
/// attached to a session. Each use must reread the published row under the
/// original journal/policy lock; any changed field requires new pairing.
pub struct NativeBindingPin {
    journal_identity: JournalIdentity,
    context: BindingContext,
    record: ProviderBindingRecord,
}

impl NativeBindingPin {
    /// Original registration metadata. Retaining this reference does not keep
    /// its lifetime or authority alive and cannot resolve disclosure by itself.
    #[must_use]
    pub const fn record(&self) -> &ProviderBindingRecord { &self.record }

    /// Revalidate the exact record and originating pairing against current disk
    /// publication, then check UTC using the retained rollback-fenced clock.
    /// The optional deadline is binding expiry only; callers must intersect it
    /// with the original request/grant limits before backend work or output.
    /// Keep the actual native lock held through those operations.
    pub fn revalidate<C: CancellationProbe>(
        &self,
        binding: &BindingContext,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        clock: &mut SystemGrantClock,
        context: &OperationContext<C>,
    ) -> Result<Option<MonotonicMillis>, NativeBindingError> {
        let (started, deadline) = begin(context)?;
        if binding != &self.context || journal.identity() != self.journal_identity {
            return Err(NativeBindingError::Unavailable);
        }
        let current = ProviderBindingRecord::read_published(
            journal, publisher, binding.binding_id(), context,
        )?.ok_or(NativeBindingError::Unavailable)?;
        if current != self.record || current.status != ProviderBindingStatus::Active {
            return Err(NativeBindingError::Unavailable);
        }
        let expiry = clock.check_policy_window(&current.issued_at, current.expires_at.as_ref())?;
        check(context, started, deadline)?;
        Ok(expiry)
    }
}

/// Closed native errors; no peer names, profiles, credentials or record bytes.
#[derive(Debug)]
pub enum NativeBindingError {
    /// Malformed, unsupported, oversized or contradictory record/transition.
    InvalidRecord,
    /// Missing, changed, inactive, foreign or incorrectly registered peer.
    Unavailable,
    /// Cancellation, deadline overflow/expiry or a regressed native clock.
    Interrupted,
    /// Original journal identity/condition failure.
    Control(ControlError),
    /// Original native call failure, including possible commit information.
    Call(ControlCallError),
    /// Actual pairing/key/session verification failed.
    Protocol(ProtocolError),
    /// Canonical lifetime or rollback-fenced UTC validation failed.
    Clock(GrantUseError),
}

impl std::fmt::Display for NativeBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRecord => f.write_str("DAEMON_BINDING_RECORD_INVALID"),
            Self::Unavailable => f.write_str("DAEMON_BINDING_UNAVAILABLE"),
            Self::Interrupted => f.write_str("DAEMON_BINDING_INTERRUPTED"),
            Self::Control(error) => std::fmt::Display::fmt(error, f),
            Self::Call(error) => std::fmt::Display::fmt(error, f),
            Self::Protocol(error) => f.write_str(error.code()),
            Self::Clock(error) => std::fmt::Display::fmt(error, f),
        }
    }
}
impl std::error::Error for NativeBindingError {}
impl From<ControlError> for NativeBindingError {
    fn from(error: ControlError) -> Self { Self::Control(error) }
}
impl From<ControlCallError> for NativeBindingError {
    fn from(error: ControlCallError) -> Self { Self::Call(error) }
}
impl From<ProtocolError> for NativeBindingError {
    fn from(error: ProtocolError) -> Self { Self::Protocol(error) }
}
impl From<GrantUseError> for NativeBindingError {
    fn from(error: GrantUseError) -> Self { Self::Clock(error) }
}

fn binding_key(incarnation: InstallationIncarnationId, binding: BindingId) -> Result<ControlKey, ControlError> {
    let mut key = b"eliot.control.provider-binding.v1\0".to_vec();
    key.extend_from_slice(incarnation.as_bytes());
    key.extend_from_slice(binding.as_bytes());
    ControlKey::new(key, JournalLimits::BASELINE)
}

fn begin<C: CancellationProbe>(context: &OperationContext<C>) -> Result<(MonotonicMillis, MonotonicMillis), NativeBindingError> {
    let started = monotonic_millis();
    let deadline = started.get().checked_add(context.relative_deadline_ms().get())
        .map(MonotonicMillis::new).ok_or(NativeBindingError::Interrupted)?;
    check(context, started, deadline)?;
    Ok((started, deadline))
}

fn check<C: CancellationProbe>(context: &OperationContext<C>, started: MonotonicMillis, deadline: MonotonicMillis) -> Result<(), NativeBindingError> {
    let now = monotonic_millis();
    if context.cancellation().is_cancelled() || now < started || now >= deadline {
        return Err(NativeBindingError::Interrupted);
    }
    Ok(())
}
