//! Durable intent, native credential and atomic registration coordination.
//!
//! Credential Manager and redb are not one transaction. A five-record intent is
//! published first, then the exact key+command identity, then the final atomic
//! binding/policy/header mutation. Every uncertain effect has explicit readback.

use search_contracts::{BindingId, protocol::PeerRole};
use search_control_redb::{
    ControlCallError, ControlCommitReceipt, ControlSnapshotPublisher, MutationId,
    PersistentControlJournal,
};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{BindingKey, TransportPeer};

use crate::access_composition::{
    NativeBindingExpectation, NativePairingCredentialError,
};
use super::{
    NativeGrantPolicyError, ProviderBindingRecord, StandalonePolicyRecord,
    StandaloneProvisioningIntent, StandaloneProvisioningIntentState,
    StandaloneRegistrationCommit, StandaloneRegistrationReadback, begin, check,
};
use super::super::NativeBindingError;
use super::readback::remaining_context;

/// Content-free recovery phase; none grants source or connection authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandaloneProvisioningPhase {
    /// Exact command and candidate exist only in this process.
    IntentPrepared,
    /// Intent transaction may have committed; recover before another write.
    IntentCommitUnresolved,
    /// Intent is durable but its admission snapshot must be recovered/published.
    IntentPublicationUnresolved,
    /// PREPARED intent is current and credential publication may begin.
    IntentDurable,
    /// Credential publication may have run; exact intent/key readback is required.
    CredentialUnresolved,
    /// Exact intent and candidate were observed in Credential Manager.
    CredentialReady,
    /// Final binding/policy/header transaction may have committed.
    RegistrationUnresolved,
    /// A real final commit receipt is retained; barriers/publication remain.
    Committed,
}

/// A failure never undoes an earlier successful or possible effect.
#[derive(Debug)]
pub enum StandaloneProvisioningError {
    /// Native record, journal, publication, clock or cancellation failure.
    Registration(NativeGrantPolicyError),
    /// Credential read/publication failure, retaining unknown-write class.
    Credential(NativePairingCredentialError),
    /// No production credential store on this platform.
    UnsupportedPlatform,
    /// Qualified entropy failed before credential publication.
    EntropyUnavailable,
    /// An unresolved dispatch requires explicit recovery before another write.
    RecoveryRequired,
    /// No final native registration receipt is retained.
    NotCommitted,
    /// Exact original credential is absent and cannot be healed as recovery.
    CredentialMissing,
    /// Durable intent, operation ledger and observed receipt contradict.
    InconsistentReadback,
    /// No durable intent exists for the requested binding/operation.
    IntentMissing,
}

impl std::fmt::Display for StandaloneProvisioningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registration(error) => std::fmt::Display::fmt(error, f),
            Self::Credential(error) => std::fmt::Display::fmt(error, f),
            Self::UnsupportedPlatform => {
                f.write_str("PAIRING_CREDENTIAL_UNSUPPORTED_PLATFORM")
            }
            Self::EntropyUnavailable => {
                f.write_str(crate::qualified_entropy::QUALIFIED_ENTROPY_UNAVAILABLE)
            }
            Self::RecoveryRequired => {
                f.write_str("STANDALONE_PROVISIONING_RECOVERY_REQUIRED")
            }
            Self::NotCommitted => f.write_str("STANDALONE_PROVISIONING_NOT_COMMITTED"),
            Self::CredentialMissing => f.write_str("PAIRING_CREDENTIAL_MISSING"),
            Self::InconsistentReadback => {
                f.write_str("STANDALONE_PROVISIONING_READBACK_CONFLICT")
            }
            Self::IntentMissing => f.write_str("STANDALONE_PROVISIONING_INTENT_MISSING"),
        }
    }
}

impl std::error::Error for StandaloneProvisioningError {}

impl From<NativeGrantPolicyError> for StandaloneProvisioningError {
    fn from(error: NativeGrantPolicyError) -> Self { Self::Registration(error) }
}

impl From<NativeBindingError> for StandaloneProvisioningError {
    fn from(error: NativeBindingError) -> Self {
        Self::Registration(error.into())
    }
}

impl From<NativePairingCredentialError> for StandaloneProvisioningError {
    fn from(error: NativePairingCredentialError) -> Self { Self::Credential(error) }
}

impl From<ControlCallError> for StandaloneProvisioningError {
    fn from(error: ControlCallError) -> Self {
        Self::Registration(NativeGrantPolicyError::Call(error))
    }
}

/// Sole in-process owner of one durable intent, candidate and final command.
///
/// No Clone, secret accessor, replacement key or rebase operation exists. Native
/// administration retains the actual root/policy lock across every method. The
/// phase changes before each possibly mutating call, including unwind. Drop clears
/// the key but never deletes a credential, intent or registration.
#[must_use = "retain provisioning owner until every possible effect is resolved"]
pub struct StandaloneRegistrationProvisioning {
    intent: StandaloneProvisioningIntent,
    expected: NativeBindingExpectation,
    peer: TransportPeer,
    candidate: BindingKey,
    phase: StandaloneProvisioningPhase,
    intent_receipt: Option<ControlCommitReceipt>,
    receipt: Option<ControlCommitReceipt>,
}

impl StandaloneRegistrationReadback {
    /// Prepare initial registration or active-generation rotation.
    ///
    /// A complete exact-input intent is built for `generation + 1` before one key
    /// is drawn. No durable effect occurs here. Revocation remains the key-free
    /// direct registration path.
    pub fn prepare_provisioning<C: CancellationProbe>(
        &self,
        operation_id: MutationId,
        binding: &ProviderBindingRecord,
        policy: &StandalonePolicyRecord,
        expected: NativeBindingExpectation,
        context: &OperationContext<C>,
    ) -> Result<StandaloneRegistrationProvisioning, StandaloneProvisioningError> {
        let (started, deadline) = begin(context)?;
        require_windows()?;
        let intent = StandaloneProvisioningIntent::prepare(
            self.identity(),
            self.generation(),
            operation_id,
            self.records(),
            binding,
            policy,
            expected.profile_id.clone(),
        )?;
        let peer = peer_for(binding)?;
        expected.validate_registration(binding, &peer)?;
        check(context, started, deadline)?;
        let candidate = draw_candidate()?;
        check(context, started, deadline)?;
        Ok(StandaloneRegistrationProvisioning {
            intent,
            expected,
            peer,
            candidate,
            phase: StandaloneProvisioningPhase::IntentPrepared,
            intent_receipt: None,
            receipt: None,
        })
    }
}

impl StandaloneRegistrationProvisioning {
    /// Restore one operation solely from its durable exact-input records.
    ///
    /// The caller supplies only binding and operation identities plus independently
    /// resolved native expectations. No prior/replacement binding or policy is
    /// accepted from the caller. The current durable journal head is read without
    /// publishing it. A matching credential restores the original key; verified
    /// absence while PREPARED draws one new candidate because no native key exists.
    ///
    /// PREPARED starts in `IntentPublicationUnresolved`, forcing explicit snapshot
    /// recovery before credential/final writes. COMMITTED starts in
    /// `RegistrationUnresolved`; its authority-bearing snapshot is never published
    /// here before required barriers.
    pub fn restore_durable<C: CancellationProbe>(
        journal: &PersistentControlJournal,
        binding_id: BindingId,
        operation_id: MutationId,
        expected: NativeBindingExpectation,
        context: &OperationContext<C>,
    ) -> Result<Self, StandaloneProvisioningError> {
        let (started, deadline) = begin(context)?;
        require_windows()?;
        let intent = StandaloneProvisioningIntent::read_current_for_recovery(
            journal, binding_id, operation_id, context,
        )?.ok_or(StandaloneProvisioningError::IntentMissing)?;
        let (binding, _) = intent.replacement();
        let peer = peer_for(binding)?;
        expected.validate_registration(binding, &peer)?;
        if intent.profile_id() != &expected.profile_id {
            return Err(NativeBindingError::Unavailable.into());
        }
        let credential_intent = intent.registration().credential_intent();
        let call = remaining_context(context, started, deadline)?;
        let restored = expected.restore_pairing_key(&peer, credential_intent, &call)?;
        let (candidate, phase) = match (intent.state(), restored) {
            (StandaloneProvisioningIntentState::Committed, None) => {
                return Err(StandaloneProvisioningError::CredentialMissing);
            }
            (StandaloneProvisioningIntentState::Committed, Some(candidate)) => {
                (candidate, StandaloneProvisioningPhase::RegistrationUnresolved)
            }
            (StandaloneProvisioningIntentState::Prepared, Some(candidate)) => {
                (candidate, StandaloneProvisioningPhase::IntentPublicationUnresolved)
            }
            (StandaloneProvisioningIntentState::Prepared, None) => {
                (draw_candidate()?, StandaloneProvisioningPhase::IntentPublicationUnresolved)
            }
        };
        check(context, started, deadline)?;
        Ok(Self {
            intent,
            expected,
            peer,
            candidate,
            phase,
            intent_receipt: None,
            receipt: None,
        })
    }

    /// Last observed content-free phase.
    #[must_use]
    pub const fn phase(&self) -> StandaloneProvisioningPhase { self.phase }

    /// Existing final journal evidence, still requiring barriers and publication.
    #[must_use]
    pub fn committed(&self) -> Option<StandaloneRegistrationCommit<'_>> {
        self.receipt.clone().map(|receipt| StandaloneRegistrationCommit {
            registration: self.intent.registration(),
            receipt,
        })
    }

    /// Persist/publish intent, publish credential and commit final registration.
    ///
    /// One diminishing call budget covers all stages. PREPARED intent publication
    /// is completed before the first external credential write. The final commit is
    /// deliberately NOT published here; caller-owned restriction/dependent barriers
    /// precede its guarded snapshot publication.
    pub fn commit<C: CancellationProbe + Clone>(
        &mut self,
        journal: &mut PersistentControlJournal,
        publisher: &mut ControlSnapshotPublisher,
        context: &OperationContext<C>,
    ) -> Result<StandaloneRegistrationCommit<'_>, StandaloneProvisioningError> {
        use StandaloneProvisioningPhase as Phase;
        let (started, deadline) = begin(context)?;
        if journal.requires_recovery()
            || matches!(
                self.phase,
                Phase::IntentCommitUnresolved
                    | Phase::IntentPublicationUnresolved
                    | Phase::CredentialUnresolved
                    | Phase::RegistrationUnresolved
            )
        {
            return Err(StandaloneProvisioningError::RecoveryRequired);
        }

        if self.phase == Phase::IntentPrepared {
            let call = remaining_context(context, started, deadline)?;
            self.phase = Phase::IntentCommitUnresolved;
            let receipt = self.intent.persist(journal, &call)?;
            self.intent_receipt = Some(receipt);
            self.phase = Phase::IntentPublicationUnresolved;

            let call = remaining_context(context, started, deadline)?;
            let receipt = self.intent_receipt.as_ref()
                .ok_or(StandaloneProvisioningError::InconsistentReadback)?;
            journal.publish_committed_snapshot_with_context(
                receipt, publisher, &call,
            )?;
            let call = remaining_context(context, started, deadline)?;
            self.intent.confirm_published(
                journal,
                publisher,
                StandaloneProvisioningIntentState::Prepared,
                &call,
            )?;
            self.phase = Phase::IntentDurable;
        }

        let credential_intent = self.intent.registration().credential_intent();
        if self.phase == Phase::IntentDurable {
            let call = remaining_context(context, started, deadline)?;
            self.phase = Phase::CredentialUnresolved;
            if let Err(error) = self.expected.publish_pairing_key(
                &self.peer, &self.candidate, credential_intent, &call,
            ) {
                if !error.outcome_unknown() {
                    self.phase = Phase::IntentDurable;
                }
                return Err(error.into());
            }
            self.phase = Phase::CredentialReady;
        }

        let call = remaining_context(context, started, deadline)?;
        if !self.expected.pairing_key_matches(
            &self.peer, &self.candidate, credential_intent, &call,
        )? {
            if self.receipt.is_none() {
                self.phase = Phase::IntentDurable;
            }
            return Err(StandaloneProvisioningError::CredentialMissing);
        }

        if self.phase == Phase::CredentialReady {
            let call = remaining_context(context, started, deadline)?;
            self.phase = Phase::RegistrationUnresolved;
            let receipt = {
                let committed = self.intent.registration().commit(journal, &call)?;
                committed.receipt().clone()
            };
            self.receipt = Some(receipt);
            self.intent.mark_committed();
            self.phase = Phase::Committed;
        }

        check(context, started, deadline)?;
        self.committed().ok_or(StandaloneProvisioningError::NotCommitted)
    }

    /// Resolve every uncertain phase without repeating a write.
    ///
    /// Intent transaction recovery precedes intent snapshot recovery. Final journal
    /// recovery precedes credential comparison. PREPARED publication recovery is
    /// safe because it leaves binding/policy unchanged; COMMITTED publication is
    /// intentionally outside this method and still requires live barriers.
    pub fn recover<C: CancellationProbe + Clone>(
        &mut self,
        journal: &mut PersistentControlJournal,
        publisher: &mut ControlSnapshotPublisher,
        context: &OperationContext<C>,
    ) -> Result<StandaloneProvisioningPhase, StandaloneProvisioningError> {
        use StandaloneProvisioningPhase as Phase;
        let (started, deadline) = begin(context)?;

        if self.phase == Phase::IntentCommitUnresolved {
            let call = remaining_context(context, started, deadline)?;
            match self.intent.recover(journal, &call)? {
                Some(receipt) => {
                    self.intent_receipt = Some(receipt);
                    self.phase = Phase::IntentPublicationUnresolved;
                }
                None => {
                    self.phase = Phase::IntentPrepared;
                    check(context, started, deadline)?;
                    return Ok(self.phase);
                }
            }
        }

        if self.phase == Phase::IntentPublicationUnresolved {
            if self.intent.state() != StandaloneProvisioningIntentState::Prepared {
                return Err(StandaloneProvisioningError::InconsistentReadback);
            }
            let call = remaining_context(context, started, deadline)?;
            let _ = journal.recover_snapshot_publication_with_context(publisher, &call)?;
            let call = remaining_context(context, started, deadline)?;
            self.intent.confirm_published(
                journal,
                publisher,
                StandaloneProvisioningIntentState::Prepared,
                &call,
            )?;
            self.phase = Phase::IntentDurable;
        }

        if matches!(self.phase, Phase::RegistrationUnresolved | Phase::Committed) {
            let call = remaining_context(context, started, deadline)?;
            match self.intent.registration().recover(journal, &call)? {
                Some(committed) => {
                    let receipt = committed.receipt().clone();
                    self.receipt = Some(receipt);
                    self.intent.mark_committed();
                    self.phase = Phase::Committed;
                }
                None if self.receipt.is_some()
                    || self.intent.state() == StandaloneProvisioningIntentState::Committed =>
                {
                    return Err(StandaloneProvisioningError::InconsistentReadback);
                }
                None => self.phase = Phase::CredentialReady,
            }
        }

        if self.phase == Phase::IntentPrepared {
            check(context, started, deadline)?;
            return Ok(self.phase);
        }

        let credential_intent = self.intent.registration().credential_intent();
        let call = remaining_context(context, started, deadline)?;
        let present = self.expected.pairing_key_matches(
            &self.peer, &self.candidate, credential_intent, &call,
        )?;
        if self.receipt.is_some() {
            if !present {
                return Err(StandaloneProvisioningError::CredentialMissing);
            }
            self.phase = Phase::Committed;
        } else if present {
            self.phase = Phase::CredentialReady;
        } else {
            self.phase = Phase::IntentDurable;
        }
        check(context, started, deadline)?;
        Ok(self.phase)
    }

    /// Confirm credential, COMMITTED intent header and all final published rows.
    pub fn confirm_published<C: CancellationProbe + Clone>(
        &self,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        context: &OperationContext<C>,
    ) -> Result<(), StandaloneProvisioningError> {
        let (started, deadline) = begin(context)?;
        let committed = self.committed()
            .ok_or(StandaloneProvisioningError::NotCommitted)?;
        let credential_intent = self.intent.registration().credential_intent();
        let call = remaining_context(context, started, deadline)?;
        if !self.expected.pairing_key_matches(
            &self.peer, &self.candidate, credential_intent, &call,
        )? {
            return Err(StandaloneProvisioningError::CredentialMissing);
        }
        let call = remaining_context(context, started, deadline)?;
        committed.confirm_published(journal, publisher, &call)?;
        let call = remaining_context(context, started, deadline)?;
        self.intent.confirm_published(
            journal,
            publisher,
            StandaloneProvisioningIntentState::Committed,
            &call,
        )?;
        let call = remaining_context(context, started, deadline)?;
        if !self.expected.pairing_key_matches(
            &self.peer, &self.candidate, credential_intent, &call,
        )? {
            return Err(StandaloneProvisioningError::CredentialMissing);
        }
        check(context, started, deadline)?;
        Ok(())
    }
}

fn require_windows() -> Result<(), StandaloneProvisioningError> {
    if cfg!(windows) {
        Ok(())
    } else {
        Err(StandaloneProvisioningError::UnsupportedPlatform)
    }
}

fn draw_candidate() -> Result<BindingKey, StandaloneProvisioningError> {
    let mut bytes = zeroize::Zeroizing::new([0_u8; 32]);
    crate::qualified_entropy::fill_qualified_entropy(&mut bytes[..])
        .map_err(|_| StandaloneProvisioningError::EntropyUnavailable)?;
    let key = BindingKey::from_bytes(*bytes).map_err(NativeBindingError::from)?;
    Ok(key)
}

fn peer_for(binding: &ProviderBindingRecord) -> Result<TransportPeer, StandaloneProvisioningError> {
    if binding.peer_role != PeerRole::StandaloneCli {
        return Err(NativeBindingError::Unavailable.into());
    }
    Ok(TransportPeer {
        role: binding.peer_role,
        incarnation: binding.installation_incarnation_id,
        binding: binding.binding_id,
    })
}

impl std::fmt::Debug for StandaloneRegistrationProvisioning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StandaloneRegistrationProvisioning")
            .field("phase", &self.phase)
            .field("intent_state", &self.intent.state())
            .finish_non_exhaustive()
    }
}
