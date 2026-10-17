//! Exact membership-wide publication abandonment fences in the existing C02 journal.
//!
//! The fence is stored in the existing abort-finalization slot until the final
//! abort transaction atomically replaces it with the final record. It contains
//! finite operation/context metadata and membership IDs only; source-backed
//! manifests, diffs, points and vectors remain in their existing owners.

use super::{
    AbortResolutionMetadataV1, Check, ControlError, ControlKey, ControlRecordClass,
    ControlRecordCondition, ControlValue, ControlWrite, JournalLimits, MutationId, Point,
    PublicationAbortFinalizationMetadataV1, PublicationAbortFinalizationRecordV1,
    PublicationIntentBindingRecordV1, PublicationSourceShadow, PublicationVisibilityState,
};
use crate::{
    CommitRecoveryDecision, ControlCommitReceipt, JournalIdentity, JournalReadSnapshot,
    MigrationBootstrapWork as Work, PersistentControlJournal,
};
use search_contracts::{
    Blake3Digest32, NonZeroRevision, OpaqueId, OpaqueRef, ProjectionMembershipId,
    PublicationIntent, PublicationIntentState, RequestId, SourceRevisionId,
};
use std::collections::BTreeMap;

const MAGIC: &[u8; 8] = b"ELIPAF01";
const MAX_RECORD_BYTES: usize = 56 * 1024;
const MAX_ABANDONMENT_MEMBERSHIPS: usize = 2_045;

/// One exact member of the C13-derived affected-membership set.
///
/// This is request data, not proof that the set is complete. Root derives the
/// full sorted set from its retained C07/C13 inputs; C02 persists and compares
/// that exact list while applying the fence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationAbandonFenceMembershipV1 {
    /// Exact projection membership to exclude from retrieval and IDF.
    pub membership_id: ProjectionMembershipId,
    /// Current immutable source revision supplied by the source owner.
    pub source_revision_id: SourceRevisionId,
    /// Exact pre-existing shadow value, or required absence.
    pub previous_shadow: Option<PublicationSourceShadow>,
}

/// Complete bounded C02 request to fence one unresolved publication.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationAbandonFenceRequestV1 {
    resolution_operation_id: OpaqueId,
    mutation_id: MutationId,
    command_digest: Blake3Digest32,
    expected_control_generation: u64,
    request_id: RequestId,
    relative_deadline_ms: u64,
    budget_ref: OpaqueRef,
    intent: PublicationIntent,
    previous_visibility: PublicationVisibilityState,
    affected_memberships: Vec<PublicationAbandonFenceMembershipV1>,
}

impl core::fmt::Debug for PublicationAbandonFenceRequestV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PublicationAbandonFenceRequestV1")
            .field(
                "expected_control_generation",
                &self.expected_control_generation,
            )
            .field("target_epoch", &self.intent.target_epoch)
            .field("membership_count", &self.affected_memberships.len())
            .finish_non_exhaustive()
    }
}

impl PublicationAbandonFenceRequestV1 {
    /// Constructs exact request data. The executing C02 owner repeats all
    /// structural checks against its active limits before any write dispatch.
    ///
    /// # Errors
    /// Rejects an empty or unsorted membership set, an invalid lifecycle state,
    /// regressed guards, non-finite context, or exhausted generations.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        resolution_operation_id: OpaqueId,
        mutation_id: MutationId,
        command_digest: Blake3Digest32,
        expected_control_generation: u64,
        request_id: RequestId,
        relative_deadline_ms: u64,
        budget_ref: OpaqueRef,
        intent: PublicationIntent,
        previous_visibility: PublicationVisibilityState,
        affected_memberships: Vec<PublicationAbandonFenceMembershipV1>,
    ) -> Result<Self, ControlError> {
        let request = Self {
            resolution_operation_id,
            mutation_id,
            command_digest,
            expected_control_generation,
            request_id,
            relative_deadline_ms,
            budget_ref,
            intent,
            previous_visibility,
            affected_memberships,
        };
        request.validate(JournalLimits::BASELINE)?;
        Ok(request)
    }

    /// Stable Root-issued identity for this exact exclusion operation.
    #[must_use]
    pub const fn resolution_operation_id(&self) -> &OpaqueId {
        &self.resolution_operation_id
    }

    /// Exact C02 operation identity used for transaction/recovery.
    #[must_use]
    pub const fn mutation_id(&self) -> MutationId {
        self.mutation_id
    }

    /// C02 command digest over the complete original request.
    #[must_use]
    pub const fn command_digest(&self) -> Blake3Digest32 {
        self.command_digest
    }

    /// Complete affected membership set in strict canonical ID order.
    #[must_use]
    pub fn affected_memberships(&self) -> &[PublicationAbandonFenceMembershipV1] {
        &self.affected_memberships
    }

    fn validate(&self, limits: JournalLimits) -> Result<(), ControlError> {
        let next_generation = self
            .expected_control_generation
            .checked_add(1)
            .ok_or(ControlError::GenerationExhausted)?;
        self.previous_visibility
            .guards
            .shadow_generation
            .checked_add(1)
            .ok_or(ControlError::GenerationExhausted)?;
        if self.expected_control_generation == 0
            || next_generation == 0
            || self.relative_deadline_ms == 0
            || self.resolution_operation_id.as_str().is_empty()
            || !matches!(
                self.intent.state,
                PublicationIntentState::IntentDurable
                    | PublicationIntentState::NewPointsAcknowledged
                    | PublicationIntentState::OldPointsClosedAcknowledged
                    | PublicationIntentState::ReadbackVerified
                    | PublicationIntentState::Compensating
                    | PublicationIntentState::PublicationBlocked
            )
            || self.intent.target_epoch <= self.previous_visibility.visible_epoch
            || self.previous_visibility.guards.owner_epoch
                < self
                    .intent
                    .owner_source_membership_access_guards
                    .owner_epoch
            || self.previous_visibility.guards.source_catalog_generation
                < self
                    .intent
                    .owner_source_membership_access_guards
                    .source_catalog_generation
            || self.previous_visibility.guards.membership_generation
                < self
                    .intent
                    .owner_source_membership_access_guards
                    .membership_generation
            || self.previous_visibility.guards.access_generation
                < self
                    .intent
                    .owner_source_membership_access_guards
                    .access_generation
            || self.previous_visibility.guards.shadow_generation
                < self
                    .intent
                    .owner_source_membership_access_guards
                    .shadow_generation
            || self.previous_visibility.guards.purge_generation
                < self
                    .intent
                    .owner_source_membership_access_guards
                    .purge_generation
            || self.previous_visibility.guards.profile_digest
                != self
                    .intent
                    .owner_source_membership_access_guards
                    .profile_digest
            || self.affected_memberships.is_empty()
            || self.affected_memberships.len() > MAX_ABANDONMENT_MEMBERSHIPS
        {
            return Err(ControlError::InvalidValue);
        }
        if self
            .affected_memberships
            .windows(2)
            .any(|pair| pair[0].membership_id >= pair[1].membership_id)
        {
            return Err(ControlError::DuplicateMutationKey);
        }
        // Visibility + fence record + N shadows are written; visibility, intent,
        // original binding, fence slot and every previous shadow are guarded.
        let work = self
            .affected_memberships
            .len()
            .checked_mul(2)
            .and_then(|value| value.checked_add(6))
            .ok_or(ControlError::BudgetExceeded)?;
        if work > limits.max_mutation_items {
            return Err(ControlError::BudgetExceeded);
        }
        for member in &self.affected_memberships {
            if member.previous_shadow.is_some_and(|shadow| {
                shadow.fence_revision.get() > self.previous_visibility.guards.shadow_generation
            }) {
                return Err(ControlError::GenerationMismatch);
            }
        }
        Ok(())
    }
}

/// Durable compact fence record stored at the existing abort-finalization slot.
/// Construction is private to the actual C02 journal path.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationAbandonFenceRecordV1 {
    identity: JournalIdentity,
    resolution_operation_id: OpaqueId,
    c02_mutation_id: MutationId,
    command_digest: Blake3Digest32,
    expected_control_generation: u64,
    resulting_control_generation: u64,
    abort_request_id: RequestId,
    abort_relative_deadline_ms: u64,
    abort_budget_ref: OpaqueRef,
    intent: PublicationIntent,
    previous_visibility: PublicationVisibilityState,
    fenced_visibility: PublicationVisibilityState,
    original_binding: PublicationIntentBindingRecordV1,
    memberships: Vec<FencedMembership>,
    membership_digest: Blake3Digest32,
    canonical: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FencedMembership {
    membership_id: ProjectionMembershipId,
    source_revision_id: SourceRevisionId,
    previous_shadow: Option<PublicationSourceShadow>,
    fenced_shadow: PublicationSourceShadow,
}

impl core::fmt::Debug for PublicationAbandonFenceRecordV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PublicationAbandonFenceRecordV1")
            .field(
                "expected_control_generation",
                &self.expected_control_generation,
            )
            .field(
                "resulting_control_generation",
                &self.resulting_control_generation,
            )
            .field("membership_count", &self.memberships.len())
            .finish_non_exhaustive()
    }
}

impl PublicationAbandonFenceRecordV1 {
    /// Stable Root-issued operation identity retained with the full membership set.
    #[must_use]
    pub const fn resolution_operation_id(&self) -> &OpaqueId {
        &self.resolution_operation_id
    }

    /// Actual C02 mutation identity that atomically installed the fence.
    #[must_use]
    pub const fn mutation_id(&self) -> MutationId {
        self.c02_mutation_id
    }

    /// Original C02 command digest, independent of the mutation identity.
    #[must_use]
    pub const fn command_digest(&self) -> Blake3Digest32 {
        self.command_digest
    }

    /// C02 generation before the fence transaction.
    #[must_use]
    pub const fn expected_control_generation(&self) -> u64 {
        self.expected_control_generation
    }

    /// C02 generation committed by the exact fence transaction.
    #[must_use]
    pub const fn resulting_control_generation(&self) -> u64 {
        self.resulting_control_generation
    }

    /// Durable publication intent whose external effects are being fenced.
    #[must_use]
    pub const fn intent(&self) -> &PublicationIntent {
        &self.intent
    }

    /// Actual visibility and full guard state immediately before the fence.
    #[must_use]
    pub const fn previous_visibility(&self) -> &PublicationVisibilityState {
        &self.previous_visibility
    }

    /// Actual visibility after the shadow-generation-only CAS.
    #[must_use]
    pub const fn fenced_visibility(&self) -> &PublicationVisibilityState {
        &self.fenced_visibility
    }

    /// Complete original operation/context binding retained by C02.
    #[must_use]
    pub const fn original_binding(&self) -> &PublicationIntentBindingRecordV1 {
        &self.original_binding
    }

    /// Complete sorted membership IDs and their exact old/new shadow values.
    #[must_use]
    pub fn memberships(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            ProjectionMembershipId,
            SourceRevisionId,
            Option<PublicationSourceShadow>,
            PublicationSourceShadow,
        ),
    > + '_ {
        self.memberships.iter().map(|member| {
            (
                member.membership_id,
                member.source_revision_id,
                member.previous_shadow,
                member.fenced_shadow,
            )
        })
    }

    /// Canonical digest over the complete sorted membership ID set.
    #[must_use]
    pub const fn membership_digest(&self) -> Blake3Digest32 {
        self.membership_digest
    }

    /// Number of exact memberships in the retained set.
    #[must_use]
    pub const fn membership_count(&self) -> u64 {
        self.memberships.len() as u64
    }

    pub(super) fn matches_abort(&self, metadata: &PublicationAbortFinalizationMetadataV1) -> bool {
        let AbortResolutionMetadataV1::Excluded {
            membership_count,
            membership_digest,
        } = metadata.resolution
        else {
            return false;
        };
        metadata.resolution_operation_id == self.resolution_operation_id
            && metadata.abort_request_id == self.abort_request_id
            && metadata.abort_relative_deadline_ms == self.abort_relative_deadline_ms
            && metadata.abort_budget_ref == self.abort_budget_ref
            && metadata.publication_intent_id == self.intent.publication_intent_id
            && metadata.target_epoch == self.intent.target_epoch
            && metadata.original_guards == self.intent.owner_source_membership_access_guards
            && metadata.new_manifest_reference == self.intent.prepared_manifest_ref
            && metadata.collection_generation_id == self.fenced_visibility.collection_generation_id
            && metadata.schema_identity_digest == self.fenced_visibility.schema_identity_digest
            && metadata.previous_visible_epoch == self.fenced_visibility.visible_epoch
            && metadata.current_guards == self.fenced_visibility.guards
            && metadata.original_control_generation
                == self.original_binding.metadata().expected_control_generation + 1
            && metadata.expected_control_generation >= self.resulting_control_generation
            && membership_count == self.membership_count()
            && membership_digest == self.membership_digest
    }

    pub(in crate::persistent) fn matches_intent(&self, intent: &PublicationIntent) -> bool {
        same_intent_except_aborted(intent, &self.intent)
    }

    fn from_request(
        identity: JournalIdentity,
        request: &PublicationAbandonFenceRequestV1,
        original_binding: PublicationIntentBindingRecordV1,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        request.validate(limits)?;
        if !original_binding.matches_intent(&request.intent)
            || original_binding.metadata().original_operation_id == request.resolution_operation_id
            || original_binding.metadata().c02_mutation_id == request.mutation_id
            || original_binding.metadata().expected_control_generation
                >= request.expected_control_generation
        {
            return Err(ControlError::OperationConflict);
        }
        check.check(Point::Validated)?;
        let next_control_generation = request
            .expected_control_generation
            .checked_add(1)
            .ok_or(ControlError::GenerationExhausted)?;
        let next_shadow_generation = request
            .previous_visibility
            .guards
            .shadow_generation
            .checked_add(1)
            .ok_or(ControlError::GenerationExhausted)?;
        let next_fence = NonZeroRevision::new(next_shadow_generation)
            .map_err(|_| ControlError::GenerationExhausted)?;
        let mut fenced_visibility = request.previous_visibility;
        fenced_visibility.guards.shadow_generation = next_shadow_generation;
        let memberships = request
            .affected_memberships
            .iter()
            .map(|member| FencedMembership {
                membership_id: member.membership_id,
                source_revision_id: member.source_revision_id,
                previous_shadow: member.previous_shadow,
                fenced_shadow: PublicationSourceShadow {
                    source_revision_id: member.source_revision_id,
                    fence_revision: next_fence,
                },
            })
            .collect::<Vec<_>>();
        let membership_digest = membership_set_digest(&memberships)?;
        let mut record = Self {
            identity,
            resolution_operation_id: request.resolution_operation_id.clone(),
            c02_mutation_id: request.mutation_id,
            command_digest: request.command_digest,
            expected_control_generation: request.expected_control_generation,
            resulting_control_generation: next_control_generation,
            abort_request_id: request.request_id,
            abort_relative_deadline_ms: request.relative_deadline_ms,
            abort_budget_ref: request.budget_ref.clone(),
            intent: request.intent.clone(),
            previous_visibility: request.previous_visibility,
            fenced_visibility,
            original_binding,
            memberships,
            membership_digest,
            canonical: Vec::new(),
        };
        record.canonical = record.encode(limits, check)?;
        Ok(record)
    }

    pub(super) fn value(
        &self,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<ControlValue, ControlError> {
        check.reserve(Work::Allocate(self.canonical.len()))?;
        check.reserve(Work::Process(self.canonical.len()))?;
        ControlValue::new(
            ControlRecordClass::Operation,
            self.canonical.clone(),
            limits,
        )
    }

    pub(in crate::persistent) fn decode(
        value: &ControlValue,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        check.check(Point::ReadRecord)?;
        let bytes = value.as_bytes();
        if value.class() != ControlRecordClass::Operation
            || bytes.len() < MAGIC.len()
            || bytes.len() > limits.max_value_bytes.min(MAX_RECORD_BYTES)
            || bytes.get(..MAGIC.len()) != Some(MAGIC.as_slice())
        {
            return Err(ControlError::StoreCorrupt);
        }
        check.reserve(Work::Allocate(
            bytes
                .len()
                .checked_mul(3)
                .ok_or(ControlError::BudgetExceeded)?,
        ))?;
        check.reserve(Work::Process(bytes.len()))?;
        let mut input = Input(bytes);
        input.take::<8>()?;
        let identity = decode_identity(&mut input)?;
        let resolution_operation_id =
            OpaqueId::new(input.text(search_contracts::MAX_OPAQUE_ID_BYTES)?)
                .map_err(|_| ControlError::StoreCorrupt)?;
        let c02_mutation_id = MutationId(input.take()?);
        let command_digest = Blake3Digest32::from_bytes(input.take()?);
        let expected_control_generation = input.u64()?;
        let resulting_control_generation = input.u64()?;
        let abort_request_id = RequestId::from_bytes(input.take()?);
        let abort_relative_deadline_ms = input.u64()?;
        let abort_budget_ref = OpaqueRef::new(input.text(search_contracts::MAX_OPAQUE_REF_BYTES)?)
            .map_err(|_| ControlError::StoreCorrupt)?;
        let intent = decode_intent(&mut input, limits)?;
        let previous_visibility = decode_visibility(&mut input, limits)?;
        let fenced_visibility = decode_visibility(&mut input, limits)?;
        let original_binding = decode_binding(&mut input, limits, check)?;
        let count = usize::try_from(input.u32()?).map_err(|_| ControlError::StoreCorrupt)?;
        if count == 0 || count > MAX_ABANDONMENT_MEMBERSHIPS {
            return Err(ControlError::StoreCorrupt);
        }
        let mut memberships = Vec::with_capacity(count);
        for _ in 0..count {
            check.check(Point::ReadRecord)?;
            let membership_id = ProjectionMembershipId::from_bytes(input.take()?);
            let source_revision_id = SourceRevisionId::from_bytes(input.take()?);
            let previous_shadow = input.optional_shadow()?;
            let fenced_shadow = input.shadow()?;
            memberships.push(FencedMembership {
                membership_id,
                source_revision_id,
                previous_shadow,
                fenced_shadow,
            });
        }
        let membership_digest = Blake3Digest32::from_bytes(input.take()?);
        input.finish()?;
        let record = Self {
            identity,
            resolution_operation_id,
            c02_mutation_id,
            command_digest,
            expected_control_generation,
            resulting_control_generation,
            abort_request_id,
            abort_relative_deadline_ms,
            abort_budget_ref,
            intent,
            previous_visibility,
            fenced_visibility,
            original_binding,
            memberships,
            membership_digest,
            canonical: bytes.to_vec(),
        };
        record.validate()?;
        check.check(Point::ReadComplete)?;
        Ok(record)
    }

    fn encode(&self, limits: JournalLimits, check: &dyn Check) -> Result<Vec<u8>, ControlError> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        encode_identity(self.identity, &mut out);
        put_text(
            &mut out,
            self.resolution_operation_id.as_str(),
            search_contracts::MAX_OPAQUE_ID_BYTES,
        )?;
        out.extend_from_slice(&self.c02_mutation_id.0);
        out.extend_from_slice(self.command_digest.as_bytes());
        out.extend_from_slice(&self.expected_control_generation.to_be_bytes());
        out.extend_from_slice(&self.resulting_control_generation.to_be_bytes());
        out.extend_from_slice(self.abort_request_id.as_bytes());
        out.extend_from_slice(&self.abort_relative_deadline_ms.to_be_bytes());
        put_text(
            &mut out,
            self.abort_budget_ref.as_str(),
            search_contracts::MAX_OPAQUE_REF_BYTES,
        )?;
        put_bytes(
            &mut out,
            super::codec::encode(&self.intent, limits)?.as_bytes(),
        )?;
        put_bytes(
            &mut out,
            super::visibility::codec::state(&self.previous_visibility, limits)?.as_bytes(),
        )?;
        put_bytes(
            &mut out,
            super::visibility::codec::state(&self.fenced_visibility, limits)?.as_bytes(),
        )?;
        put_bytes(
            &mut out,
            self.original_binding.value(limits, check)?.as_bytes(),
        )?;
        out.extend_from_slice(
            &u32::try_from(self.memberships.len())
                .map_err(|_| ControlError::BudgetExceeded)?
                .to_be_bytes(),
        );
        for member in &self.memberships {
            check.check(Point::PlanRecord)?;
            out.extend_from_slice(member.membership_id.as_bytes());
            out.extend_from_slice(member.source_revision_id.as_bytes());
            encode_optional_shadow(member.previous_shadow, &mut out);
            encode_shadow(member.fenced_shadow, &mut out);
        }
        out.extend_from_slice(self.membership_digest.as_bytes());
        if out.len() > limits.max_value_bytes.min(MAX_RECORD_BYTES) {
            return Err(ControlError::BudgetExceeded);
        }
        check.reserve(Work::Allocate(out.len()))?;
        check.reserve(Work::Process(out.len()))?;
        Ok(out)
    }

    fn validate(&self) -> Result<(), ControlError> {
        if self.expected_control_generation == 0
            || self.expected_control_generation.checked_add(1)
                != Some(self.resulting_control_generation)
            || self.abort_relative_deadline_ms == 0
            || self.intent.state == PublicationIntentState::Prepared
            || self.intent.state == PublicationIntentState::Aborted
            || !super::unresolved(self.intent.state)
            || self.intent.target_epoch <= self.previous_visibility.visible_epoch
            || self.fenced_visibility.collection_generation_id
                != self.previous_visibility.collection_generation_id
            || self.fenced_visibility.schema_identity_digest
                != self.previous_visibility.schema_identity_digest
            || self.fenced_visibility.visible_epoch != self.previous_visibility.visible_epoch
            || self.fenced_visibility.last_receipt != self.previous_visibility.last_receipt
            || self.fenced_visibility.guards.owner_epoch
                != self.previous_visibility.guards.owner_epoch
            || self.fenced_visibility.guards.source_catalog_generation
                != self.previous_visibility.guards.source_catalog_generation
            || self.fenced_visibility.guards.membership_generation
                != self.previous_visibility.guards.membership_generation
            || self.fenced_visibility.guards.access_generation
                != self.previous_visibility.guards.access_generation
            || self.fenced_visibility.guards.purge_generation
                != self.previous_visibility.guards.purge_generation
            || self.fenced_visibility.guards.profile_digest
                != self.previous_visibility.guards.profile_digest
            || self.fenced_visibility.guards.shadow_generation
                != self
                    .previous_visibility
                    .guards
                    .shadow_generation
                    .checked_add(1)
                    .ok_or(ControlError::StoreCorrupt)?
            || !self.original_binding.matches_intent(&self.intent)
            || self.original_binding.metadata().original_operation_id
                == self.resolution_operation_id
            || self.original_binding.metadata().c02_mutation_id == self.c02_mutation_id
            || self.original_binding.metadata().expected_control_generation
                >= self.expected_control_generation
            || self.memberships.is_empty()
            || self.memberships.len() > MAX_ABANDONMENT_MEMBERSHIPS
            || self
                .memberships
                .windows(2)
                .any(|pair| pair[0].membership_id >= pair[1].membership_id)
        {
            return Err(ControlError::StoreCorrupt);
        }
        for member in &self.memberships {
            if member.fenced_shadow.source_revision_id != member.source_revision_id
                || member.fenced_shadow.fence_revision.get()
                    != self.fenced_visibility.guards.shadow_generation
                || member.previous_shadow.is_some_and(|shadow| {
                    shadow.fence_revision.get() > self.previous_visibility.guards.shadow_generation
                })
            {
                return Err(ControlError::StoreCorrupt);
            }
        }
        if self.membership_digest != membership_set_digest(&self.memberships)? {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(())
    }
}

/// Owner-read C02 proof of a still-effective membership-wide fence.
/// Its private fields are created only from a verified redb read transaction.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationAbandonFenceReadbackV1 {
    record: PublicationAbandonFenceRecordV1,
    generation: u64,
    receipt: ControlCommitReceipt,
    visibility: PublicationVisibilityState,
    intent: PublicationIntent,
    shadows: BTreeMap<ProjectionMembershipId, PublicationSourceShadow>,
}

impl core::fmt::Debug for PublicationAbandonFenceReadbackV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PublicationAbandonFenceReadbackV1")
            .field("generation", &self.generation)
            .field("membership_count", &self.record.membership_count())
            .field("intent_state", &self.intent.state)
            .finish_non_exhaustive()
    }
}

impl PublicationAbandonFenceReadbackV1 {
    /// Exact retained operation and full membership-set record.
    #[must_use]
    pub const fn record(&self) -> &PublicationAbandonFenceRecordV1 {
        &self.record
    }

    /// C02 generation shared by the actual visibility, intent and shadow readback.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Actual C02 receipt for the original fence operation.
    #[must_use]
    pub const fn receipt(&self) -> &ControlCommitReceipt {
        &self.receipt
    }

    /// Actual current visibility, with only the shadow generation advanced by the fence.
    #[must_use]
    pub const fn visibility(&self) -> &PublicationVisibilityState {
        &self.visibility
    }

    /// Actual current intent; remains unresolved until exact abort finalization.
    #[must_use]
    pub const fn intent(&self) -> &PublicationIntent {
        &self.intent
    }

    /// Actual C02 shadows from the same recovery-plane read transaction.
    #[must_use]
    pub const fn shadows(&self) -> &BTreeMap<ProjectionMembershipId, PublicationSourceShadow> {
        &self.shadows
    }
}

impl PersistentControlJournal {
    /// Atomically writes the complete membership shadow set and advances only
    /// `shadow_generation`, then verifies the operation, visibility and shadows
    /// through one recovery-plane read. No serving readiness is granted.
    pub(in crate::persistent) fn abandon_publication_checked(
        &mut self,
        request: &PublicationAbandonFenceRequestV1,
        boundary: super::super::Boundary,
        check: &dyn Check,
    ) -> Result<PublicationAbandonFenceReadbackV1, ControlError> {
        let result = (|| {
            self.ensure_available()?;
            check.check(Point::Start)?;
            if !crate::has_visibility_schema(self.identity.schema_version) {
                return Err(ControlError::SchemaUnsupported);
            }
            super::super::operating_profile::require_indexed_checked(self, check)?;
            request.validate(self.limits)?;
            let (record, replay) = self.prepare_abandonment_record(request, check)?;
            if replay {
                match self.read_abandonment_fence_checked(&record, check) {
                    Ok(mut readback) => {
                        readback.receipt.replayed = true;
                        return Ok(readback);
                    }
                    Err(ControlError::OperationConflict) => {}
                    Err(error) => return Err(error),
                }
            }
            let command = record.command(self.limits, check)?;
            let receipt =
                super::super::transaction::execute_publication(self, &command, boundary, check)?;
            self.read_abandonment_fence_checked(&record, check)
                .map_err(|_| ControlError::CommitOutcomeUnknown)
                .map(|mut readback| {
                    readback.receipt = receipt;
                    readback
                })
        })();
        if result
            .as_ref()
            .err()
            .is_some_and(|error| super::super::is_corruption(*error))
        {
            self.quarantined = true;
        }
        result
    }

    pub(in crate::persistent) fn recover_abandon_publication_checked(
        &mut self,
        request: &PublicationAbandonFenceRequestV1,
        check: &dyn Check,
    ) -> Result<CommitRecoveryDecision, ControlError> {
        let result = (|| {
            self.ensure_available()?;
            check.check(Point::Start)?;
            if !crate::has_visibility_schema(self.identity.schema_version) {
                return Err(ControlError::SchemaUnsupported);
            }
            super::super::operating_profile::require_indexed_checked(self, check)?;
            request.validate(self.limits)?;
            let (record, _) = self.prepare_abandonment_record(request, check)?;
            let command = record.command(self.limits, check)?;
            self.recover_transaction_with_conditions_checked(
                command.mutation(),
                command.conditions(),
                check,
            )
        })();
        if result
            .as_ref()
            .err()
            .is_some_and(|error| super::super::is_corruption(*error))
        {
            self.quarantined = true;
        }
        result
    }

    fn prepare_abandonment_record(
        &self,
        request: &PublicationAbandonFenceRequestV1,
        check: &dyn Check,
    ) -> Result<(PublicationAbandonFenceRecordV1, bool), ControlError> {
        let (binding, replay) = self.load_original_binding_for_abandon(request, check)?;
        let record = PublicationAbandonFenceRecordV1::from_request(
            self.identity,
            request,
            binding,
            self.limits,
            check,
        )?;
        Ok((record, replay))
    }

    fn load_original_binding_for_abandon(
        &self,
        request: &PublicationAbandonFenceRequestV1,
        check: &dyn Check,
    ) -> Result<(PublicationIntentBindingRecordV1, bool), ControlError> {
        let read = self
            .database
            .begin_read()
            .map_err(|_| ControlError::StoreUnavailable)?;
        let header = self.header_from_checked(&read, check)?;
        let snapshot = self.snapshot_from_checked(&read, check)?;
        let replay = super::super::operation_from_checked(
            &read,
            request.mutation_id,
            &header,
            self.limits,
            check,
        )?
        .is_some();
        if !replay && header.generation != request.expected_control_generation {
            return Err(ControlError::TransactionConflict);
        }
        let intent = super::read_intent_from_snapshot(&snapshot, self.limits)?
            .ok_or(ControlError::OperationConflict)?;
        if intent.publication_intent_id != request.intent.publication_intent_id
            || intent.target_epoch != request.intent.target_epoch
            || intent.prepared_manifest_ref != request.intent.prepared_manifest_ref
            || intent.owner_source_membership_access_guards
                != request.intent.owner_source_membership_access_guards
            || (!replay && intent != request.intent)
        {
            return Err(ControlError::OperationConflict);
        }
        let state_key = ControlKey::new(super::visibility::STATE.to_vec(), self.limits)?;
        let visibility = snapshot
            .get(&state_key)
            .map(super::visibility::codec::read_state)
            .transpose()?
            .ok_or(ControlError::StoreCorrupt)?;
        if !replay && visibility != request.previous_visibility {
            return Err(ControlError::GenerationMismatch);
        }
        let binding = super::operation_binding::read_snapshot(&snapshot, self.limits, check)?
            .ok_or(ControlError::OperationConflict)?;
        if !binding.matches_intent(&intent) {
            return Err(ControlError::StoreCorrupt);
        }
        Ok((binding, replay))
    }

    pub(in crate::persistent) fn read_abandonment_fence_checked(
        &self,
        expected: &PublicationAbandonFenceRecordV1,
        check: &dyn Check,
    ) -> Result<PublicationAbandonFenceReadbackV1, ControlError> {
        self.ensure_available()?;
        check.check(Point::Start)?;
        let read = self
            .database
            .begin_read()
            .map_err(|_| ControlError::StoreUnavailable)?;
        let header = self.header_from_checked(&read, check)?;
        let snapshot = self.snapshot_from_checked(&read, check)?;
        let fence_value =
            abort_slot(&snapshot, self.limits)?.ok_or(ControlError::OperationConflict)?;
        let actual_record = decode_slot(&fence_value, self.limits, check)?;
        let record = match &actual_record {
            AbortSlotRecord::Fence(record) => record.as_ref().clone(),
            AbortSlotRecord::Final(finalized) => finalized
                .abandonment_fence()
                .cloned()
                .ok_or(ControlError::OperationConflict)?,
        };
        if &record != expected || record.identity != snapshot.identity {
            return Err(ControlError::OperationConflict);
        }
        let operation = super::super::operation_from_checked(
            &read,
            record.c02_mutation_id,
            &header,
            self.limits,
            check,
        )?
        .ok_or(ControlError::StoreCorrupt)?;
        let receipt = operation.receipt;
        let expected_changed = record.changed_keys(self.limits)?;
        if receipt.operation_id != record.c02_mutation_id {
            return Err(ControlError::StoreCorrupt);
        }
        if receipt.command_digest != record.command_digest {
            return Err(ControlError::StoreCorrupt);
        }
        if receipt.before_generation != record.expected_control_generation {
            return Err(ControlError::StoreCorrupt);
        }
        if receipt.after_generation != record.resulting_control_generation {
            return Err(ControlError::StoreCorrupt);
        }
        if receipt.changed_keys != expected_changed {
            return Err(ControlError::StoreCorrupt);
        }
        let visibility_key = ControlKey::new(super::visibility::STATE.to_vec(), self.limits)?;
        let visibility = snapshot
            .get(&visibility_key)
            .map(super::visibility::codec::read_state)
            .transpose()?
            .ok_or(ControlError::StoreCorrupt)?;
        let intent = super::read_intent_from_snapshot(&snapshot, self.limits)?
            .ok_or(ControlError::StoreCorrupt)?;
        if visibility != record.fenced_visibility
            || !same_intent_except_aborted(&intent, &record.intent)
        {
            return Err(ControlError::GenerationMismatch);
        }
        let binding = super::operation_binding::read_snapshot(&snapshot, self.limits, check)?
            .ok_or(ControlError::StoreCorrupt)?;
        if binding != record.original_binding {
            return Err(ControlError::StoreCorrupt);
        }
        if let AbortSlotRecord::Final(finalized) = &actual_record {
            if intent.state != PublicationIntentState::Aborted
                || finalized.abandonment_fence() != Some(&record)
                || !finalized.matches_intent(&intent)
            {
                return Err(ControlError::StoreCorrupt);
            }
        } else if intent.state == PublicationIntentState::Aborted {
            return Err(ControlError::StoreCorrupt);
        }
        let (_, _, shadows) =
            super::visibility::read_recovery_references(&snapshot, self.limits, check)?;
        for member in &record.memberships {
            if shadows.get(&member.membership_id) != Some(&member.fenced_shadow) {
                return Err(ControlError::GenerationMismatch);
            }
        }
        check.check(Point::ReadComplete)?;
        Ok(PublicationAbandonFenceReadbackV1 {
            record,
            generation: snapshot.generation,
            receipt,
            visibility,
            intent,
            shadows,
        })
    }
}

impl PublicationAbandonFenceRecordV1 {
    fn changed_keys(&self, limits: JournalLimits) -> Result<Vec<ControlKey>, ControlError> {
        let mut keys = Vec::with_capacity(self.memberships.len() + 2);
        keys.push(ControlKey::new(super::visibility::STATE.to_vec(), limits)?);
        keys.push(super::abort_finalization::key(limits)?);
        for member in &self.memberships {
            keys.push(super::visibility::id_key(
                super::visibility::SHADOWS,
                member.membership_id.as_bytes(),
                limits,
            )?);
        }
        keys.sort();
        if keys.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(keys)
    }

    fn command(
        &self,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<crate::ConditionalControlMutation, ControlError> {
        let state_key = ControlKey::new(super::visibility::STATE.to_vec(), limits)?;
        let intent_key = ControlKey::new(super::KEY.to_vec(), limits)?;
        let binding_key = super::operation_binding::key(limits)?;
        let abort_key = super::abort_finalization::key(limits)?;
        let next_state = super::visibility::codec::state(&self.fenced_visibility, limits)?;
        let previous_state = super::visibility::codec::state(&self.previous_visibility, limits)?;
        let next_intent = super::codec::encode(&self.intent, limits)?;
        let current_binding = self.original_binding.value(limits, check)?;
        let fence_value = self.value(limits, check)?;
        let mut writes = vec![
            ControlWrite {
                key: state_key.clone(),
                value: next_state,
            },
            ControlWrite {
                key: abort_key.clone(),
                value: fence_value,
            },
        ];
        let mut conditions = vec![
            ControlRecordCondition::exact(state_key, previous_state),
            ControlRecordCondition::exact(intent_key, next_intent),
            ControlRecordCondition::exact(binding_key, current_binding),
            ControlRecordCondition::absent(abort_key),
        ];
        for member in &self.memberships {
            check.check(Point::PlanRecord)?;
            let key = super::visibility::id_key(
                super::visibility::SHADOWS,
                member.membership_id.as_bytes(),
                limits,
            )?;
            conditions.push(match member.previous_shadow {
                Some(shadow) => ControlRecordCondition::exact(
                    key,
                    super::visibility::codec::shadow(&shadow, limits)?,
                ),
                None => ControlRecordCondition::absent(key),
            });
            let shadow_key = super::visibility::id_key(
                super::visibility::SHADOWS,
                member.membership_id.as_bytes(),
                limits,
            )?;
            writes.push(ControlWrite {
                key: shadow_key,
                value: super::visibility::codec::shadow(&member.fenced_shadow, limits)?,
            });
        }
        Ok(crate::ConditionalControlMutation::new(
            crate::ControlMutation::new(
                self.c02_mutation_id,
                self.command_digest,
                self.expected_control_generation,
                writes,
                Vec::new(),
            ),
            conditions,
        ))
    }
}

/// Readback of the fixed C02 abort slot, either before or after abort finalization.
pub(in crate::persistent) enum AbortSlotRecord {
    Fence(Box<PublicationAbandonFenceRecordV1>),
    Final(Box<PublicationAbortFinalizationRecordV1>),
}

pub(in crate::persistent) fn is_fence_value(value: &ControlValue) -> bool {
    value.as_bytes().get(..MAGIC.len()) == Some(MAGIC.as_slice())
}

pub(in crate::persistent) fn decode_slot(
    value: &ControlValue,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<AbortSlotRecord, ControlError> {
    if is_fence_value(value) {
        PublicationAbandonFenceRecordV1::decode(value, limits, check)
            .map(Box::new)
            .map(AbortSlotRecord::Fence)
    } else {
        PublicationAbortFinalizationRecordV1::decode(value, limits, check)
            .map(Box::new)
            .map(AbortSlotRecord::Final)
    }
}

pub(in crate::persistent) fn read_snapshot(
    snapshot: &JournalReadSnapshot,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<Option<PublicationAbandonFenceRecordV1>, ControlError> {
    let Some(value) = abort_slot(snapshot, limits)? else {
        return Ok(None);
    };
    match decode_slot(&value, limits, check)? {
        AbortSlotRecord::Fence(record) => Ok(Some(*record)),
        AbortSlotRecord::Final(record) => Ok(record.abandonment_fence().cloned()),
    }
}

fn abort_slot(
    snapshot: &JournalReadSnapshot,
    limits: JournalLimits,
) -> Result<Option<ControlValue>, ControlError> {
    let key = super::abort_finalization::key(limits)?;
    Ok(snapshot.get(&key).cloned())
}

pub(in crate::persistent) fn validate_finalized_snapshot(
    snapshot: &JournalReadSnapshot,
    visibility: &PublicationVisibilityState,
    intent: &PublicationIntent,
    record: &PublicationAbortFinalizationRecordV1,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(), ControlError> {
    let Some(fence) = record.abandonment_fence() else {
        if matches!(
            record.metadata().resolution,
            AbortResolutionMetadataV1::Excluded { .. }
        ) {
            return Err(ControlError::StoreCorrupt);
        }
        return Ok(());
    };
    if !fence.matches_abort(record.metadata())
        || visibility.visible_epoch != fence.fenced_visibility.visible_epoch
        || visibility.collection_generation_id != fence.fenced_visibility.collection_generation_id
        || visibility.schema_identity_digest != fence.fenced_visibility.schema_identity_digest
        || visibility.last_receipt != fence.fenced_visibility.last_receipt
        || visibility.guards != fence.fenced_visibility.guards
        || intent.state != PublicationIntentState::Aborted
    {
        return Err(ControlError::StoreCorrupt);
    }
    let (_, _, shadows) = super::visibility::read_recovery_references(snapshot, limits, check)?;
    for member in &fence.memberships {
        if shadows.get(&member.membership_id) != Some(&member.fenced_shadow) {
            return Err(ControlError::StoreCorrupt);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // The full read transaction and typed control state are checked together.
pub(in crate::persistent) fn validate_retained_fence(
    read: &crate::persistent::ReadTransaction,
    snapshot: &JournalReadSnapshot,
    visibility: &PublicationVisibilityState,
    intent: &PublicationIntent,
    finalization: Option<&PublicationAbortFinalizationRecordV1>,
    fence: &PublicationAbandonFenceRecordV1,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(), ControlError> {
    if snapshot.identity != fence.identity
        || visibility != &fence.fenced_visibility
        || !fence.matches_intent(intent)
        || (finalization.is_some() != (intent.state == PublicationIntentState::Aborted))
        || finalization.is_some_and(|record| {
            record.abandonment_fence() != Some(fence)
                || !record.matches_intent(intent)
                || !fence.matches_abort(record.metadata())
        })
    {
        return Err(ControlError::StoreCorrupt);
    }
    if let Some(record) = finalization {
        validate_finalized_snapshot(snapshot, visibility, intent, record, limits, check)?;
    }
    let binding = super::operation_binding::read_snapshot(snapshot, limits, check)?
        .ok_or(ControlError::StoreCorrupt)?;
    if binding != fence.original_binding {
        return Err(ControlError::StoreCorrupt);
    }
    let metadata = read
        .open_table(super::super::META)
        .map_err(super::super::map_table_error)?;
    let header_bytes = metadata
        .get("header")
        .map_err(|error| super::super::map_storage_error(&error))?
        .ok_or(ControlError::StoreCorrupt)?;
    check.reserve(Work::Read(header_bytes.value().len()))?;
    check.reserve(Work::Process(header_bytes.value().len()))?;
    let header = super::super::Header::decode(header_bytes.value(), snapshot.identity, limits)?;
    let operation =
        super::super::operation_from_checked(read, fence.c02_mutation_id, &header, limits, check)?
            .ok_or(ControlError::StoreCorrupt)?;
    if operation.receipt.operation_id != fence.c02_mutation_id {
        return Err(ControlError::StoreCorrupt);
    }
    if operation.receipt.command_digest != fence.command_digest {
        return Err(ControlError::StoreCorrupt);
    }
    if operation.receipt.before_generation != fence.expected_control_generation {
        return Err(ControlError::StoreCorrupt);
    }
    if operation.receipt.after_generation != fence.resulting_control_generation {
        return Err(ControlError::StoreCorrupt);
    }
    if operation.receipt.changed_keys != fence.changed_keys(limits)? {
        return Err(ControlError::StoreCorrupt);
    }
    let (_, _, shadows) = super::visibility::read_recovery_references(snapshot, limits, check)?;
    for member in &fence.memberships {
        if shadows.get(&member.membership_id) != Some(&member.fenced_shadow) {
            return Err(ControlError::StoreCorrupt);
        }
    }
    Ok(())
}

pub(in crate::persistent) fn validate_record(
    value: &ControlValue,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(), ControlError> {
    match decode_slot(value, limits, check)? {
        AbortSlotRecord::Fence(_) | AbortSlotRecord::Final(_) => Ok(()),
    }
}

fn same_intent_except_aborted(actual: &PublicationIntent, expected: &PublicationIntent) -> bool {
    if actual == expected {
        return true;
    }
    if actual.state != PublicationIntentState::Aborted {
        return false;
    }
    let mut expected = expected.clone();
    expected.state = PublicationIntentState::Aborted;
    *actual == expected
}

fn membership_set_digest(members: &[FencedMembership]) -> Result<Blake3Digest32, ControlError> {
    let count = u64::try_from(members.len()).map_err(|_| ControlError::BudgetExceeded)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot/search-control-redb/publication-abandon-memberships/v1\0");
    hasher.update(&count.to_be_bytes());
    for member in members {
        hasher.update(member.membership_id.as_bytes());
    }
    Ok(Blake3Digest32::from_bytes(*hasher.finalize().as_bytes()))
}

fn encode_identity(identity: JournalIdentity, out: &mut Vec<u8>) {
    out.extend_from_slice(identity.installation_incarnation_id.as_bytes());
    out.extend_from_slice(identity.data_root_id.as_bytes());
    out.extend_from_slice(&identity.owner_epoch.get().to_be_bytes());
    out.extend_from_slice(identity.path_identity_digest.as_bytes());
    out.extend_from_slice(identity.schema_family_digest.as_bytes());
    out.extend_from_slice(&identity.schema_version.to_be_bytes());
}

fn decode_identity(input: &mut Input<'_>) -> Result<JournalIdentity, ControlError> {
    Ok(JournalIdentity {
        installation_incarnation_id: search_contracts::InstallationIncarnationId::from_bytes(
            input.take()?,
        ),
        data_root_id: search_contracts::DataRootId::from_bytes(input.take()?),
        owner_epoch: search_contracts::OwnerEpoch::new(input.u64()?)
            .map_err(|_| ControlError::StoreCorrupt)?,
        path_identity_digest: Blake3Digest32::from_bytes(input.take()?),
        schema_family_digest: Blake3Digest32::from_bytes(input.take()?),
        schema_version: input.u32()?,
    })
}

fn decode_intent(
    input: &mut Input<'_>,
    limits: JournalLimits,
) -> Result<PublicationIntent, ControlError> {
    let bytes = input.bytes(limits.max_value_bytes)?;
    let value = ControlValue::new(ControlRecordClass::Operation, bytes, limits)?;
    super::codec::decode(&value)
}

fn decode_visibility(
    input: &mut Input<'_>,
    limits: JournalLimits,
) -> Result<PublicationVisibilityState, ControlError> {
    let bytes = input.bytes(limits.max_value_bytes)?;
    let value = ControlValue::new(ControlRecordClass::Revision, bytes, limits)?;
    super::visibility::codec::read_state(&value)
}

fn decode_binding(
    input: &mut Input<'_>,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<PublicationIntentBindingRecordV1, ControlError> {
    let bytes = input.bytes(limits.max_value_bytes)?;
    let value = ControlValue::new(ControlRecordClass::Operation, bytes, limits)?;
    PublicationIntentBindingRecordV1::decode(&value, limits, check)
}

fn put_text(out: &mut Vec<u8>, value: &str, max: usize) -> Result<(), ControlError> {
    if value.is_empty() || value.len() > max {
        return Err(ControlError::InvalidValue);
    }
    let length = u16::try_from(value.len()).map_err(|_| ControlError::InvalidValue)?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn put_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), ControlError> {
    let length = u32::try_from(value.len()).map_err(|_| ControlError::BudgetExceeded)?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn encode_optional_shadow(shadow: Option<PublicationSourceShadow>, out: &mut Vec<u8>) {
    match shadow {
        Some(shadow) => {
            out.push(1);
            encode_shadow(shadow, out);
        }
        None => out.push(0),
    }
}

fn encode_shadow(shadow: PublicationSourceShadow, out: &mut Vec<u8>) {
    out.extend_from_slice(shadow.source_revision_id.as_bytes());
    out.extend_from_slice(&shadow.fence_revision.get().to_be_bytes());
}

struct Input<'a>(&'a [u8]);

impl Input<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], ControlError> {
        let (head, tail) = self
            .0
            .split_at_checked(N)
            .ok_or(ControlError::StoreCorrupt)?;
        self.0 = tail;
        head.try_into().map_err(|_| ControlError::StoreCorrupt)
    }

    fn u16(&mut self) -> Result<u16, ControlError> {
        Ok(u16::from_be_bytes(self.take()?))
    }

    fn u32(&mut self) -> Result<u32, ControlError> {
        Ok(u32::from_be_bytes(self.take()?))
    }

    fn u64(&mut self) -> Result<u64, ControlError> {
        Ok(u64::from_be_bytes(self.take()?))
    }

    fn bytes(&mut self, max: usize) -> Result<Vec<u8>, ControlError> {
        let length = usize::try_from(self.u32()?).map_err(|_| ControlError::StoreCorrupt)?;
        if length == 0 || length > max {
            return Err(ControlError::StoreCorrupt);
        }
        let (head, tail) = self
            .0
            .split_at_checked(length)
            .ok_or(ControlError::StoreCorrupt)?;
        self.0 = tail;
        Ok(head.to_vec())
    }

    fn text(&mut self, max: usize) -> Result<String, ControlError> {
        let length = usize::from(self.u16()?);
        if length == 0 || length > max {
            return Err(ControlError::StoreCorrupt);
        }
        let (head, tail) = self
            .0
            .split_at_checked(length)
            .ok_or(ControlError::StoreCorrupt)?;
        self.0 = tail;
        String::from_utf8(head.to_vec()).map_err(|_| ControlError::StoreCorrupt)
    }

    fn optional_shadow(&mut self) -> Result<Option<PublicationSourceShadow>, ControlError> {
        match self.take::<1>()?[0] {
            0 => Ok(None),
            1 => self.shadow().map(Some),
            _ => Err(ControlError::StoreCorrupt),
        }
    }

    fn shadow(&mut self) -> Result<PublicationSourceShadow, ControlError> {
        Ok(PublicationSourceShadow {
            source_revision_id: SourceRevisionId::from_bytes(self.take()?),
            fence_revision: NonZeroRevision::new(self.u64()?)
                .map_err(|_| ControlError::StoreCorrupt)?,
        })
    }

    const fn finish(self) -> Result<(), ControlError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(ControlError::StoreCorrupt)
        }
    }
}
