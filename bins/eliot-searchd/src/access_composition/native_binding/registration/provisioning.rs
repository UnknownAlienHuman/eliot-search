//! One retained owner for credential publication followed by atomic registration.
//!
//! Credential Manager and redb are not one transaction. This owner orders their
//! existing effects and retains the exact candidate/command across interruption;
//! it does not manufacture rollback or a durable cross-store transaction.

use search_contracts::protocol::PeerRole;
use search_control_redb::{
    ControlCommitReceipt, ControlSnapshotPublisher, JournalIdentity, MutationId,
    PersistentControlJournal,
};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{BindingKey, TransportPeer};

use crate::access_composition::{NativeBindingExpectation, NativePairingCredentialError};
use super::{
    NativeGrantPolicyError, ProviderBindingRecord, StandalonePolicyRecord,
    StandaloneRegistrationCommit, StandaloneRegistrationMutation, StandaloneRegistrationReadback,
    begin, check,
};
use super::super::NativeBindingError;
use super::readback::remaining_context;

/// Content-free progress, not a source permit or proof of current publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandaloneProvisioningPhase {
    /// The single candidate and exact registration command exist only in memory.
    Prepared,
    /// Credential publication may have run; exact readback is required first.
    CredentialUnresolved,
    /// The candidate and command intent were observed in Credential Manager.
    CredentialReady,
    /// The atomic registration may have committed; recover the original command.
    RegistrationUnresolved,
    /// A real commit receipt is retained; barriers/publication are still required.
    Committed,
}

/// A failure never undoes earlier successful or possibly completed effects.
#[derive(Debug)]
pub enum StandaloneProvisioningError {
    /// Native record, journal, clock or cancellation failure.
    Registration(NativeGrantPolicyError),
    /// Credential read/publication failure, retaining its unknown-write class.
    Credential(NativePairingCredentialError),
    /// No production credential store on this platform.
    UnsupportedPlatform,
    /// The one qualified entropy draw failed; no durable operation was started.
    EntropyUnavailable,
    /// An unresolved earlier dispatch must be read back before another write.
    RecoveryRequired,
    /// This owner has not obtained a native registration receipt.
    NotCommitted,
    /// The exact original credential is absent and cannot be recreated as recovery.
    CredentialMissing,
    /// Native recovery contradicted a previously observed committed receipt.
    InconsistentReadback,
}

impl std::fmt::Display for StandaloneProvisioningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registration(error) => std::fmt::Display::fmt(error, f),
            Self::Credential(error) => std::fmt::Display::fmt(error, f),
            Self::UnsupportedPlatform => f.write_str("PAIRING_CREDENTIAL_UNSUPPORTED_PLATFORM"),
            Self::EntropyUnavailable => f.write_str(crate::qualified_entropy::QUALIFIED_ENTROPY_UNAVAILABLE),
            Self::RecoveryRequired => f.write_str("STANDALONE_PROVISIONING_RECOVERY_REQUIRED"),
            Self::NotCommitted => f.write_str("STANDALONE_PROVISIONING_NOT_COMMITTED"),
            Self::CredentialMissing => f.write_str("PAIRING_CREDENTIAL_MISSING"),
            Self::InconsistentReadback => f.write_str("STANDALONE_PROVISIONING_READBACK_CONFLICT"),
        }
    }
}
impl std::error::Error for StandaloneProvisioningError {}
impl From<NativeGrantPolicyError> for StandaloneProvisioningError {
    fn from(error: NativeGrantPolicyError) -> Self { Self::Registration(error) }
}
impl From<NativeBindingError> for StandaloneProvisioningError {
    fn from(error: NativeBindingError) -> Self { Self::Registration(error.into()) }
}
impl From<NativePairingCredentialError> for StandaloneProvisioningError {
    fn from(error: NativePairingCredentialError) -> Self { Self::Credential(error) }
}

/// Retains exactly one generated/restored key, native coordinates and immutable
/// two-row command. No Clone, secret accessor, replacement key or rebase operation.
///
/// Native administration must retain the actual root/policy lock across calls.
/// The phase is set BEFORE each possibly mutating call, including panic unwinding.
/// A new bounded recovery call reads state; it never renews an interrupted call's
/// budget or repeats a write implicitly. Drop clears the candidate through
/// `BindingKey` but never deletes a credential or registration.
///
/// Across process restart, reconstruct this value only through [`Self::restore_exact`]
/// with the exact original command input. Credential Manager binds the recovered
/// key to that operation ID, command digest and expected generation.
#[must_use = "retain the provisioning owner until its possible effects are resolved"]
pub struct StandaloneRegistrationProvisioning {
    registration: StandaloneRegistrationMutation,
    expected: NativeBindingExpectation,
    peer: TransportPeer,
    candidate: BindingKey,
    phase: StandaloneProvisioningPhase,
    receipt: Option<ControlCommitReceipt>,
}

impl StandaloneRegistrationReadback {
    /// Prepare initial registration or active-generation rotation from this exact
    /// native readback. Existing row validators check both transitions; native
    /// credential/profile expectations must match the intended registration.
    /// Only then draw one key through the daemon's existing qualified OS CSPRNG.
    ///
    /// No credential or journal write occurs here. Revocation uses the existing
    /// key-free atomic registration operation, never a newly generated key.
    pub fn prepare_provisioning<C: CancellationProbe>(
        &self,
        operation_id: MutationId,
        binding: &ProviderBindingRecord,
        policy: &StandalonePolicyRecord,
        expected: NativeBindingExpectation,
        context: &OperationContext<C>,
    ) -> Result<StandaloneRegistrationProvisioning, StandaloneProvisioningError> {
        let (started, deadline) = begin(context)?;
        if !cfg!(windows) { return Err(StandaloneProvisioningError::UnsupportedPlatform); }
        let (registration, peer) = prepare_exact(
            self.identity(), operation_id, self.generation(), self.records(),
            binding, policy, &expected,
        )?;
        check(context, started, deadline)?;
        let mut bytes = zeroize::Zeroizing::new([0_u8; 32]);
        crate::qualified_entropy::fill_qualified_entropy(&mut bytes[..])
            .map_err(|_| StandaloneProvisioningError::EntropyUnavailable)?;
        check(context, started, deadline)?;
        let candidate = BindingKey::from_bytes(*bytes).map_err(NativeBindingError::from)?;
        Ok(StandaloneRegistrationProvisioning {
            registration, expected, peer, candidate,
            phase: StandaloneProvisioningPhase::Prepared, receipt: None,
        })
    }

    /// Restore a possibly effected provisioning operation from this exact readback.
    ///
    /// The same operation ID and intended replacement are required. No new entropy
    /// is drawn. This convenience works when the readback still represents the
    /// original pre-state; use [`StandaloneRegistrationProvisioning::restore_exact`]
    /// with retained original inputs when journal publication has moved forward.
    pub fn restore_provisioning<C: CancellationProbe>(
        &self,
        operation_id: MutationId,
        binding: &ProviderBindingRecord,
        policy: &StandalonePolicyRecord,
        expected: NativeBindingExpectation,
        context: &OperationContext<C>,
    ) -> Result<StandaloneRegistrationProvisioning, StandaloneProvisioningError> {
        StandaloneRegistrationProvisioning::restore_exact(
            self.identity(), operation_id, self.generation(), self.records(),
            binding, policy, expected, context,
        )
    }
}

impl StandaloneRegistrationProvisioning {
    /// Rehydrate one exact operation after process restart without generating a key.
    ///
    /// The caller must resupply the immutable original journal identity, expected
    /// generation, prior records, replacement records and operation ID. Those bytes
    /// reconstruct the command digest stored with the credential. A missing or
    /// differently tagged credential is not repaired or adopted. The returned
    /// owner starts in `RegistrationUnresolved`, so journal readback precedes any
    /// later write attempt.
    #[allow(clippy::too_many_arguments)]
    pub fn restore_exact<C: CancellationProbe>(
        identity: JournalIdentity,
        operation_id: MutationId,
        expected_generation: u64,
        prior: Option<(&ProviderBindingRecord, &StandalonePolicyRecord)>,
        binding: &ProviderBindingRecord,
        policy: &StandalonePolicyRecord,
        expected: NativeBindingExpectation,
        context: &OperationContext<C>,
    ) -> Result<Self, StandaloneProvisioningError> {
        let (started, deadline) = begin(context)?;
        if !cfg!(windows) { return Err(StandaloneProvisioningError::UnsupportedPlatform); }
        let (registration, peer) = prepare_exact(
            identity, operation_id, expected_generation, prior, binding, policy, &expected,
        )?;
        let intent = registration.credential_intent();
        let call = remaining_context(context, started, deadline)?;
        let candidate = expected.restore_pairing_key(&peer, intent, &call)?
            .ok_or(StandaloneProvisioningError::CredentialMissing)?;
        check(context, started, deadline)?;
        Ok(Self {
            registration, expected, peer, candidate,
            phase: StandaloneProvisioningPhase::RegistrationUnresolved,
            receipt: None,
        })
    }

    /// Last observed phase; a receipt remains historical until confirm_published.
    #[must_use]
    pub const fn phase(&self) -> StandaloneProvisioningPhase { self.phase }

    /// Existing native evidence, tied to this owner's original command. It proves
    /// neither current credential presence nor publication/security-barrier completion.
    #[must_use]
    pub fn committed(&self) -> Option<StandaloneRegistrationCommit<'_>> {
        self.receipt.clone().map(|receipt| StandaloneRegistrationCommit {
            registration: &self.registration, receipt,
        })
    }

    /// Publish the retained candidate and intent, then commit both metadata rows.
    ///
    /// An unresolved dispatch refuses another commit until recovery. Cancellation
    /// after either effect preserves phase/receipt. Repeated calls after a known
    /// commit only recheck the exact credential; missing state is never recreated.
    pub fn commit<C: CancellationProbe + Clone>(
        &mut self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<StandaloneRegistrationCommit<'_>, StandaloneProvisioningError> {
        use StandaloneProvisioningPhase as Phase;
        let (started, deadline) = begin(context)?;
        self.registration.check_identity(journal)?;
        if journal.requires_recovery()
            || matches!(self.phase, Phase::CredentialUnresolved | Phase::RegistrationUnresolved)
        {
            return Err(StandaloneProvisioningError::RecoveryRequired);
        }
        let intent = self.registration.credential_intent();
        if self.phase == Phase::Prepared {
            let call = remaining_context(context, started, deadline)?;
            self.phase = Phase::CredentialUnresolved;
            if let Err(error) = self.expected.publish_pairing_key(
                &self.peer, &self.candidate, intent, &call,
            ) {
                if !error.outcome_unknown() { self.phase = Phase::Prepared; }
                return Err(error.into());
            }
            self.phase = Phase::CredentialReady;
        }
        let call = remaining_context(context, started, deadline)?;
        if !self.expected.pairing_key_matches(&self.peer, &self.candidate, intent, &call)? {
            if self.receipt.is_none() { self.phase = Phase::Prepared; }
            return Err(StandaloneProvisioningError::CredentialMissing);
        }
        if self.phase == Phase::CredentialReady {
            let call = remaining_context(context, started, deadline)?;
            self.phase = Phase::RegistrationUnresolved;
            let committed = self.registration.commit(journal, &call)?;
            self.receipt = Some(committed.receipt);
            self.phase = Phase::Committed;
        }
        check(context, started, deadline)?;
        self.committed().ok_or(StandaloneProvisioningError::NotCommitted)
    }

    /// Resolve an interrupted invocation without writing either store.
    ///
    /// Journal uncertainty is resolved first through the exact operation ledger.
    /// Credential readback then requires both original candidate and stored command
    /// intent. A conflicting record is never adopted or overwritten.
    pub fn recover<C: CancellationProbe + Clone>(
        &mut self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<StandaloneProvisioningPhase, StandaloneProvisioningError> {
        use StandaloneProvisioningPhase as Phase;
        let (started, deadline) = begin(context)?;
        self.registration.check_identity(journal)?;
        if matches!(self.phase, Phase::RegistrationUnresolved | Phase::Committed) {
            let call = remaining_context(context, started, deadline)?;
            match self.registration.recover(journal, &call)? {
                Some(committed) => {
                    self.receipt = Some(committed.receipt);
                    self.phase = Phase::Committed;
                }
                None if self.receipt.is_some() => {
                    return Err(StandaloneProvisioningError::InconsistentReadback);
                }
                None => self.phase = Phase::CredentialReady,
            }
        }
        let intent = self.registration.credential_intent();
        let call = remaining_context(context, started, deadline)?;
        let present = self.expected.pairing_key_matches(
            &self.peer, &self.candidate, intent, &call,
        )?;
        if self.receipt.is_some() {
            if !present { return Err(StandaloneProvisioningError::CredentialMissing); }
        } else {
            self.phase = if present { Phase::CredentialReady } else { Phase::Prepared };
        }
        check(context, started, deadline)?;
        Ok(self.phase)
    }

    /// Confirm the exact credential intent/key and both published metadata rows.
    ///
    /// Credential checks bracket coherent metadata confirmation. The root lock,
    /// live barriers and guarded publication remain mandatory caller-owned work.
    pub fn confirm_published<C: CancellationProbe + Clone>(
        &self,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        context: &OperationContext<C>,
    ) -> Result<(), StandaloneProvisioningError> {
        let (started, deadline) = begin(context)?;
        self.registration.check_identity(journal)?;
        let committed = self.committed().ok_or(StandaloneProvisioningError::NotCommitted)?;
        let intent = self.registration.credential_intent();
        let call = remaining_context(context, started, deadline)?;
        if !self.expected.pairing_key_matches(&self.peer, &self.candidate, intent, &call)? {
            return Err(StandaloneProvisioningError::CredentialMissing);
        }
        let call = remaining_context(context, started, deadline)?;
        committed.confirm_published(journal, publisher, &call)?;
        let call = remaining_context(context, started, deadline)?;
        if !self.expected.pairing_key_matches(&self.peer, &self.candidate, intent, &call)? {
            return Err(StandaloneProvisioningError::CredentialMissing);
        }
        check(context, started, deadline)?;
        Ok(())
    }
}

fn prepare_exact(
    identity: JournalIdentity,
    operation_id: MutationId,
    expected_generation: u64,
    prior: Option<(&ProviderBindingRecord, &StandalonePolicyRecord)>,
    binding: &ProviderBindingRecord,
    policy: &StandalonePolicyRecord,
    expected: &NativeBindingExpectation,
) -> Result<(StandaloneRegistrationMutation, TransportPeer), StandaloneProvisioningError> {
    if binding.peer_role != PeerRole::StandaloneCli {
        return Err(NativeBindingError::Unavailable.into());
    }
    let peer = TransportPeer {
        role: binding.peer_role,
        incarnation: binding.installation_incarnation_id,
        binding: binding.binding_id,
    };
    expected.validate_registration(binding, &peer)?;
    let registration = StandaloneRegistrationMutation::new(
        identity, operation_id, expected_generation, prior, binding, policy,
    )?;
    Ok((registration, peer))
}

impl std::fmt::Debug for StandaloneRegistrationProvisioning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StandaloneRegistrationProvisioning")
            .field("phase", &self.phase).finish_non_exhaustive()
    }
}
