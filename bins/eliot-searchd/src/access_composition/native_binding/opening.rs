//! Native session opening against an already installed, published registration.

mod connections;
mod credential;
mod tcp;

pub use connections::{
    BindingConnectionRegistry, BindingConnectionRegistryError, BindingDrainReceipt,
    MAX_REGISTERED_BINDING_CONNECTIONS,
};
pub use credential::{NativePairingCredentialError, NativePairingCredentialIntent};
pub use tcp::NativeTcpOpenError;

use search_contracts::{Blake3Digest32, InstallationId, NonZeroRevision, OpaqueRef, ProfileId};
use search_control_redb::{ControlSnapshotPublisher, PersistentControlJournal};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{
    BindingContext, BindingKey, PairingMachine, ProtocolLimits, ServerNonce, TransportPeer,
};

use crate::provider_composition::{CanonicalProviderConnection, monotonic_millis};
use super::{NativeBindingError, NativeBindingPin, ProviderBindingRecord, ProviderBindingStatus,
    SystemGrantClock, begin, check};

/// Trusted lookup inputs from the installation, credential and profile owners.
/// This is data, not an authenticated capability. In particular, the actual
/// pairing key must have been resolved for THIS peer and generation under the
/// same native lock; client assertions or a bare token file cannot supply them.
pub struct NativeBindingExpectation {
    /// Exact native installation identity.
    pub installation_id: InstallationId,
    /// Independently resolved peer identity, not echoed from the wire record.
    pub peer_identity_digest: Blake3Digest32,
    /// Generation for which the supplied pairing key was resolved.
    pub pairing_generation: NonZeroRevision,
    /// Actual client profile being activated; not a recipe or budget-class name.
    pub profile_id: ProfileId,
    /// Reference independently resolved by the disclosure-policy owner.
    pub disclosure_ceiling_ref: OpaqueRef,
}

impl NativeBindingExpectation {
    // Data/coordinate consistency only. Publication, time and keyed ceremony
    // validation remain with the caller; a matching record is not authority.
    pub(in crate::access_composition) fn validate_registration(
        &self,
        record: &ProviderBindingRecord,
        peer: &TransportPeer,
    ) -> Result<(), NativeBindingError> {
        record.validate()?;
        if record.status != ProviderBindingStatus::Active
            || record.binding_id != peer.binding
            || record.installation_id != self.installation_id
            || record.installation_incarnation_id != peer.incarnation
            || record.peer_role != peer.role
            || record.peer_identity_digest != self.peer_identity_digest
            || record.pairing_generation != self.pairing_generation
            || !record.permitted_profile_ids.contains(&self.profile_id)
            || record.disclosure_ceiling_ref != self.disclosure_ceiling_ref
        {
            return Err(NativeBindingError::Unavailable);
        }
        Ok(())
    }
}

impl CanonicalProviderConnection {
    /// Open the canonical session only after a current durable binding read.
    ///
    /// Retain the real root/binding lock across credential resolution, this
    /// call and the original-socket handoff. The exact key must reproduce the
    /// completed pairing proof through the existing `open` implementation.
    /// Return a non-clonable registration pin for subsequent native policy reads;
    /// neither the pin nor pairing grants source access.
    ///
    /// This reads existing state only. Binding commit and live revocation/
    /// dependent publication must already be complete before bootstrap calls it.
    /// The expected disclosure reference is checked, never interpreted as a
    /// numeric ceiling or a grant. No credential resolver or listener is invented.
    ///
    /// # Errors
    /// Missing/unpublished/inactive/foreign/expired records, profile or generation
    /// mismatch, key/proof failure and original setup interruption all refuse open.
    /// Failure drops owned pairing-key material; no unsigned fallback is exposed.
    #[allow(clippy::too_many_arguments)]
    pub fn open_published<C: CancellationProbe>(
        binding: BindingContext,
        ceremony: PairingMachine,
        key: BindingKey,
        server_nonce: ServerNonce,
        limits: ProtocolLimits,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        expected: &NativeBindingExpectation,
        clock: &mut SystemGrantClock,
        context: &OperationContext<C>,
    ) -> Result<(Self, NativeBindingPin), NativeBindingError> {
        let (started, deadline) = begin(context)?;
        let record = ProviderBindingRecord::read_published(
            journal, publisher, binding.binding_id(), context,
        )?.ok_or(NativeBindingError::Unavailable)?;
        expected.validate_registration(&record, &TransportPeer {
            role: binding.role(), incarnation: binding.incarnation(), binding: binding.binding_id(),
        })?;
        let expiry = clock.check_policy_window(&record.issued_at, record.expires_at.as_ref())?;
        check(context, started, deadline)?;
        let connection = Self::open(binding, ceremony, key, server_nonce, limits)?;
        // The original native borrows exclude journal/publisher mutation here.
        // A historical receipt is never substituted for the published row.
        let current_expiry = clock.check_policy_window(&record.issued_at, record.expires_at.as_ref())?;
        check(context, started, deadline)?;
        let now = monotonic_millis();
        if expiry.is_some_and(|end| now >= end) || current_expiry.is_some_and(|end| now >= end) {
            return Err(NativeBindingError::Unavailable);
        }
        Ok((connection, NativeBindingPin { journal_identity: journal.identity(), context: binding, record }))
    }
}
