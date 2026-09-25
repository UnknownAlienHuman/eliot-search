//! Coherent native registration readback for bootstrap and live policy use.

use search_contracts::BindingId;
use search_control_redb::{ControlSnapshotPublisher, JournalIdentity, MutationId, PersistentControlJournal};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{BindingContext, MonotonicMillis};

use crate::access_composition::{GrantUseError, NativeGrantPolicyError, StandalonePolicyRecord, SystemGrantClock};
use crate::access_composition::native_grant_policy::policy_key;
use super::{StandaloneRegistrationMutation, validate_pair};
use super::super::{
    NativeBindingError, NativeBindingPin, ProviderBindingRecord, ProviderBindingStatus,
    begin, binding_key, check, codec, monotonic_millis,
};

/// Both records, or verified absence of both, at one disk-published generation.
///
/// Only a native read constructs this observation. It is not a live permit or
/// evidence that an uncertain historical command committed. Recover that command
/// through its original operation identity; never reconstruct it from this head.
/// No journal, snapshot pointer, issuer ledger or credential is retained here.
#[must_use = "readback is metadata; mutation, publication and live checks remain explicit"]
pub struct StandaloneRegistrationReadback {
    identity: JournalIdentity,
    generation: u64,
    binding_id: BindingId,
    records: Option<(ProviderBindingRecord, StandalonePolicyRecord)>,
}

impl StandaloneRegistrationReadback {
    /// Read the pair without requiring a session, grant or caller-supplied policy.
    /// Native administration must retain the actual root/policy owner lock.
    /// This is suitable for restoring registration metadata before pairing; it
    /// does not open/create the journal, recover publication or acknowledge a peer.
    /// An unpublished/empty publisher is an error, never proof of absent records.
    /// Terminal rows remain inspectable but cannot be reactivated by prepare_change.
    ///
    /// # Errors
    /// Rejects half-present, inconsistent, malformed or foreign pairs. Native
    /// failures and the single original read/decode budget are preserved.
    pub fn read_published<C: CancellationProbe + Clone>(
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        binding_id: BindingId,
        context: &OperationContext<C>,
    ) -> Result<Self, NativeGrantPolicyError> {
        let (started, deadline) = begin(context)?;
        let identity = journal.identity();
        let incarnation = identity.installation_incarnation_id;
        let binding_key = binding_key(incarnation, binding_id)?;
        let policy_key = policy_key(incarnation, binding_id)?;
        let read_context = remaining_context(context, started, deadline)?;
        let (generation, values) = journal.read_published_record_pair(
            publisher, [&binding_key, &policy_key], &read_context,
        )?;
        check(context, started, deadline)?;
        let records = match values {
            [None, None] => None,
            [Some(binding), Some(policy)] => {
                let binding = codec::decode(&binding)?;
                let policy = StandalonePolicyRecord::decode_native(&policy)?;
                validate_pair(&binding, &policy)?;
                if binding.binding_id != binding_id || binding.installation_incarnation_id != incarnation {
                    return Err(NativeGrantPolicyError::InvalidRecord);
                }
                Some((binding, policy))
            }
            _ => return Err(NativeGrantPolicyError::InvalidRecord),
        };
        check(context, started, deadline)?;
        Ok(Self { identity, generation, binding_id, records })
    }

    /// Exact journal/root/owner identity at the read, not permission to rebind it.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity { self.identity }

    /// Common observed journal generation; it is never advanced during preparation.
    #[must_use]
    pub const fn generation(&self) -> u64 { self.generation }

    /// Validated pair or absence of both. Neither record alone becomes authority.
    #[must_use]
    pub fn records(&self) -> Option<(&ProviderBindingRecord, &StandalonePolicyRecord)> {
        self.records.as_ref().map(|(binding, policy)| (binding, policy))
    }

    /// Build an atomic command using the exact observed pre-state and generation.
    /// The caller supplies a NEW administrative operation identity and intended
    /// replacement, not guessed old records or a refreshed expected generation.
    /// A later journal change makes commit conflict; it is never silently rebased.
    /// Native credential, barrier and publication steps remain caller-owned.
    ///
    /// # Errors
    /// Rejects another binding, invalid replacements and terminal reactivation
    /// through the existing registration/binding/policy transition validators.
    pub fn prepare_change(
        &self,
        operation_id: MutationId,
        binding: &ProviderBindingRecord,
        policy: &StandalonePolicyRecord,
    ) -> Result<StandaloneRegistrationMutation, NativeGrantPolicyError> {
        if binding.binding_id != self.binding_id {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }
        StandaloneRegistrationMutation::new(
            self.identity, operation_id, self.generation, self.records(), binding, policy,
        )
    }

    pub(in crate::access_composition) fn into_policy(self) -> Option<StandalonePolicyRecord> {
        self.records.map(|(_, policy)| policy)
    }
}

impl core::fmt::Debug for StandaloneRegistrationReadback {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StandaloneRegistrationReadback")
            .field("generation", &self.generation)
            .field("present", &self.records.is_some())
            .finish_non_exhaustive()
    }
}

impl NativeBindingPin {
    /// Revalidate the exact pinned binding and read its policy in the SAME head.
    /// Every call performs native readback; a caller cannot supply an old pair.
    /// Keep the actual owner lock through use. The returned expiry is binding-only;
    /// the policy reader additionally checks boot, policy and request lifetime.
    ///
    /// # Errors
    /// Refuses a foreign ceremony/journal, absent/changed/inactive binding,
    /// inconsistent policy pair, clock failure or original budget exhaustion.
    pub fn read_standalone_registration<C: CancellationProbe + Clone>(
        &self,
        binding: &BindingContext,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        clock: &mut SystemGrantClock,
        context: &OperationContext<C>,
    ) -> Result<(StandaloneRegistrationReadback, Option<MonotonicMillis>), NativeGrantPolicyError> {
        let (started, deadline) = begin(context)?;
        if binding != &self.context || journal.identity() != self.journal_identity {
            return Err(NativeBindingError::Unavailable.into());
        }
        let read_context = remaining_context(context, started, deadline)?;
        let registration = StandaloneRegistrationReadback::read_published(
            journal, publisher, binding.binding_id(), &read_context,
        )?;
        let (current, _) = registration.records().ok_or(NativeBindingError::Unavailable)?;
        if current != &self.record || current.status != ProviderBindingStatus::Active {
            return Err(NativeBindingError::Unavailable.into());
        }
        let expiry = clock.check_policy_window(&current.issued_at, current.expires_at.as_ref())?;
        check(context, started, deadline)?;
        if expiry.is_some_and(|end| monotonic_millis() >= end) {
            return Err(GrantUseError::Expired.into());
        }
        Ok((registration, expiry))
    }
}

pub(super) fn remaining_context<C: CancellationProbe + Clone>(
    context: &OperationContext<C>,
    started: MonotonicMillis,
    deadline: MonotonicMillis,
) -> Result<OperationContext<C>, NativeBindingError> {
    check(context, started, deadline)?;
    let remaining = deadline.get().checked_sub(monotonic_millis().get())
        .filter(|left| *left > 0).ok_or(NativeBindingError::Interrupted)?;
    OperationContext::new(
        context.request_id(), remaining, context.cancellation().clone(), context.budget_ref().clone(),
    ).map_err(|_| NativeBindingError::Interrupted)
}
