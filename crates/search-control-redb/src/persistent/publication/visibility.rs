//! Atomic durable visibility finalization. This is not an index adapter or grant issuer.

use core::fmt;
use search_contracts::{Blake3Digest32, CollectionGenerationId, Epoch, NonZeroRevision,
    ProjectionMembershipId, PublicationGuards, PublicationIntent,
    PublicationIntentState, PublicationReceipt, PublicationReceiptId, ReceiptRef, SourceRevisionId};
use search_ports::{CancellationProbe, OperationContext};
use crate::{ConditionalControlMutation, ControlRecordCondition, ControlRecordClass, ControlValue, ControlWrite};
use super::super::operation::{Budget, Check, Point};
use super::super::{Boundary, CommitRecoveryDecision, ControlCallError, ControlCommitReceipt,
    ControlError, ControlKey, ControlMutation, JournalLimits, JournalReadSnapshot, MutationId,
    PersistentControlJournal, ReadTransaction, operation_from, is_corruption};
use super::codec as intent_codec;

mod codec;
mod validation;
mod succession;
pub use succession::PublicationSuccessor;
#[cfg(test)]
mod tests;

/// Explicit schema for durable visibility, receipt and manifest/shadow bindings.
/// Schema 1/2 is not upgraded. Older implementations reject this version.
pub const PUBLICATION_VISIBILITY_SCHEMA_VERSION: u32 = 3;
const STATE: &[u8] = b"collection_route/visibility/v1";
const RECEIPTS: &[u8] = b"publication_receipts/visible/v1/";
const MANIFESTS: &[u8] = b"projection_memberships/manifest/v1/";
const SHADOWS: &[u8] = b"shadow_fences/publication/v1/";
const RETIRED: &[u8] = b"publication_receipts/retired/v1/";

// Ordinary shared-port writes and deletes cannot manufacture visibility or
// bypass manifest/shadow/receipt guards. Typed commands use these same keys.
pub(super) fn port_reserved_key(key: &[u8]) -> bool {
    key == STATE || [RECEIPTS, MANIFESTS, SHADOWS, RETIRED].iter()
        .any(|prefix| key.starts_with(prefix))
}

/// Committed route and authoritative publication-generation guards.
/// The source/access owners must update their counters in the same control
/// transaction as the changes they protect. This record does not observe sources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationVisibilityState {
    /// Qualified physical collection generation, not a vendor collection name.
    pub collection_generation_id: CollectionGenerationId,
    /// Exact qualified collection schema identity.
    pub schema_identity_digest: Blake3Digest32,
    /// Committed visible epoch; never advanced by intent-only operations.
    pub visible_epoch: Epoch,
    /// Actual control-state guard values, not values echoed from a request.
    pub guards: PublicationGuards,
    /// Latest exact publication receipt, absent only at initial epoch zero.
    pub last_receipt: Option<PublicationReceiptId>,
}

/// Exact source-specific shadow eligible for removal by this publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationSourceShadow {
    /// Immutable source revision observed by the shadow's owning producer.
    pub source_revision_id: SourceRevisionId,
    /// Nonzero fence revision; a newer fence must not be removed by old work.
    pub fence_revision: NonZeroRevision,
}

/// One exact membership manifest replacement/deletion and its matching shadow.
/// The coordinator derives the complete bounded set from verified CAS manifests.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationManifestChange {
    /// Exact projection membership; no broad source or collection filter exists.
    pub membership_id: ProjectionMembershipId,
    /// Current manifest, or required absence for a newly published membership.
    pub previous_manifest: Option<ReceiptRef>,
    /// New manifest, or explicit retirement without replacement.
    pub next_manifest: Option<ReceiptRef>,
    /// Exact matching shadow to remove; None requires actual shadow absence.
    pub matching_shadow: Option<PublicationSourceShadow>,
}
impl fmt::Debug for PublicationManifestChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicationManifestChange").field("membership_id", &self.membership_id)
            .field("retires_manifest", &self.previous_manifest.is_some())
            .field("publishes_manifest", &self.next_manifest.is_some())
            .field("removes_shadow", &self.matching_shadow.is_some()).finish()
    }
}

/// Input evidence already verified by the publication coordinator/index adapter.
/// These references/digest do not themselves prove that an external read occurred.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationReadbackEvidence {
    /// Newly staged exact point-manifest reference.
    pub exact_new_manifest_ref: ReceiptRef,
    /// Exactly closed/retired point-manifest reference, including a real empty manifest.
    pub exact_retired_manifest_ref: ReceiptRef,
    /// Producer-computed exact readback digest; no digest algorithm is substituted.
    pub readback_digest: Blake3Digest32,
}
impl fmt::Debug for PublicationReadbackEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublicationReadbackEvidence { references: <redacted> }")
    }
}

/// Immutable, complete request for the durable half of publication finalization.
#[derive(Clone, Eq, PartialEq)]
pub struct VisibleEpochCommit {
    operation_id: MutationId,
    command_digest: Blake3Digest32,
    prior_commit: ControlCommitReceipt,
    intent: PublicationIntent,
    previous: PublicationVisibilityState,
    receipt: PublicationReceipt,
    changes: Vec<PublicationManifestChange>,
}
impl fmt::Debug for VisibleEpochCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VisibleEpochCommit").field("expected_generation", &self.prior_commit.after_generation)
            .field("target_epoch", &self.intent.target_epoch).field("changes", &self.changes.len()).finish()
    }
}

impl VisibleEpochCommit {
    /// Builds a closed command; runtime validation applies the actual journal limits.
    ///
    /// # Errors
    /// Requires READBACK_VERIFIED, matching prepared/live guards, exactly the next
    /// visible epoch and a nonempty distinct membership delta. Epoch skipping,
    /// invalidation-only publication and route replacement have separate protocols.
    #[allow(clippy::too_many_arguments)] // One closed request; each identity remains distinct.
    pub fn new(operation_id: MutationId, command_digest: Blake3Digest32,
        prior_commit: ControlCommitReceipt, intent: PublicationIntent,
        previous: PublicationVisibilityState, receipt_id: PublicationReceiptId,
        readback: PublicationReadbackEvidence, changes: Vec<PublicationManifestChange>,
    ) -> Result<Self, ControlError> {
        let after = prior_commit.after_generation.checked_add(1).ok_or(ControlError::GenerationExhausted)?;
        let receipt = PublicationReceipt { publication_receipt_id: receipt_id,
            target_epoch: intent.target_epoch,
            exact_new_manifest_ref: readback.exact_new_manifest_ref,
            exact_retired_manifest_ref: readback.exact_retired_manifest_ref,
            readback_digest: readback.readback_digest,
            control_commit_revision: NonZeroRevision::new(after).map_err(|_| ControlError::InvalidValue)? };
        let result = Self { operation_id, command_digest, prior_commit, intent, previous, receipt, changes };
        result.validate(JournalLimits::BASELINE)?;
        Ok(result)
    }

    pub(in crate::persistent) const fn port_operation_id(&self) -> MutationId { self.operation_id }

    pub(in crate::persistent) fn port_prior_matches(&self, receipt: &ControlCommitReceipt) -> bool {
        same_commit(&self.prior_commit, receipt)
    }

    fn validate(&self, limits: JournalLimits) -> Result<(), ControlError> {
        validation::validate_request(self, limits)
    }

    pub(in crate::persistent) fn command(&self, limits: JournalLimits, check: &dyn Check) -> Result<ConditionalControlMutation, ControlError> {
        self.validate(limits)?;
        check.check(Point::Validated)?;
        let mut next = self.previous;
        next.visible_epoch = self.intent.target_epoch;
        next.last_receipt = Some(self.receipt.publication_receipt_id);
        if self.changes.iter().any(|change| change.matching_shadow.is_some()) {
            next.guards.shadow_generation = next.guards.shadow_generation.checked_add(1)
                .ok_or(ControlError::GenerationExhausted)?;
        }
        let committed = search_domain::transition_publication(&self.intent, PublicationIntentState::ControlCommitted)
            .map_err(|_| ControlError::InvalidValue)?;
        let intent_key = key(super::KEY, limits)?;
        let state_key = key(STATE, limits)?;
        let receipt_key = id_key(RECEIPTS, self.receipt.publication_receipt_id.as_bytes(), limits)?;
        let mut writes = vec![
            ControlWrite { key: intent_key.clone(), value: intent_codec::encode(&committed, limits)? },
            ControlWrite { key: state_key.clone(), value: codec::state(&next, limits)? },
            ControlWrite { key: receipt_key.clone(), value: codec::receipt(self, limits)? },
        ];
        let mut conditions = vec![
            ControlRecordCondition::exact(intent_key, intent_codec::encode(&self.intent, limits)?),
            ControlRecordCondition::exact(state_key, codec::state(&self.previous, limits)?),
            ControlRecordCondition::absent(receipt_key),
        ];
        let mut deletes = Vec::new();
        for change in &self.changes {
            check.check(Point::PlanRecord)?;
            let manifest_key = id_key(MANIFESTS, change.membership_id.as_bytes(), limits)?;
            conditions.push(match &change.previous_manifest {
                Some(reference) => ControlRecordCondition::exact(manifest_key.clone(), codec::manifest(reference, limits)?),
                None => ControlRecordCondition::absent(manifest_key.clone()),
            });
            if let Some(reference) = &change.next_manifest {
                writes.push(ControlWrite { key: manifest_key, value: codec::manifest(reference, limits)? });
            } else { deletes.push(manifest_key); }
            if let Some(reference) = &change.previous_manifest {
                let retired_key = retired_key(self.receipt.publication_receipt_id, change.membership_id, limits)?;
                conditions.push(ControlRecordCondition::absent(retired_key.clone()));
                writes.push(ControlWrite { key: retired_key, value: codec::manifest(reference, limits)? });
            }
            let shadow_key = id_key(SHADOWS, change.membership_id.as_bytes(), limits)?;
            match &change.matching_shadow {
                Some(shadow) => {
                    conditions.push(ControlRecordCondition::exact(shadow_key.clone(), codec::shadow(shadow, limits)?));
                    deletes.push(shadow_key);
                }
                None => conditions.push(ControlRecordCondition::absent(shadow_key)),
            }
        }
        let mutation = ControlMutation::new(self.operation_id, self.command_digest,
            self.prior_commit.after_generation, writes, deletes);
        Ok(ConditionalControlMutation::new(mutation, conditions))
    }
}

impl PersistentControlJournal {
    /// Atomically records a new route at epoch zero with explicitly supplied guards.
    /// This is for a newly initialized schema-3 journal, not restore or migration.
    ///
    /// # Errors
    /// Existing state/data/intent, wrong owner or nonzero initial visibility is refused.
    pub fn initialize_publication_visibility<C: CancellationProbe>(&mut self,
        state: PublicationVisibilityState, operation_id: MutationId, command_digest: Blake3Digest32,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, ControlCallError> {
        let budget = Budget::new(context);
        let result = (|| {
            self.ensure_available()?; budget.check(Point::Start)?; require_schema(self)?;
            if state.visible_epoch.get() != 0 || state.last_receipt.is_some()
                || state.guards.owner_epoch != self.identity.owner_epoch { return Err(ControlError::InvalidValue); }
            let state_key = key(STATE, self.limits)?;
            let mutation = ControlMutation::new(operation_id, command_digest, 0,
                vec![ControlWrite { key: state_key.clone(), value: codec::state(&state, self.limits)? }], vec![]);
            self.transact_conditionally_checked(ConditionalControlMutation::new(mutation,
                vec![ControlRecordCondition::absent(state_key), ControlRecordCondition::absent(key(super::KEY, self.limits)?)]),
                Boundary::Normal, &budget)
        })();
        result.map_err(|error| budget.failure(error, Some(operation_id)))
    }

    /// Commits epoch, manifest transitions, matching shadows, intent and receipt
    /// in the existing single redb transaction. Success is durable control commit,
    /// NOT acknowledgement of usable indexed results: publish the snapshot next.
    ///
    /// # Errors
    /// False live guards or prior receipt fail without partial publication. Any
    /// possible-write interruption requires recovery of the exact original command.
    pub fn commit_visible_epoch<C: CancellationProbe>(&mut self, request: &VisibleEpochCommit,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, ControlCallError> {
        let budget = Budget::new(context);
        self.commit_visibility_checked(request, Boundary::Normal, &budget)
            .map_err(|error| budget.failure(error, Some(request.operation_id)))
    }

    /// Resolves only the original visibility mutation, never a newly built command.
    ///
    /// # Errors
    /// Failed inspection keeps recovery pending; success does not publish a snapshot.
    pub fn recover_visible_epoch_commit<C: CancellationProbe>(&mut self, request: &VisibleEpochCommit,
        context: &OperationContext<C>,
    ) -> Result<CommitRecoveryDecision, ControlCallError> {
        let budget = Budget::new(context);
        let result = (|| {
            budget.check(Point::Start)?; require_schema(self)?;
            let command = request.command(self.limits, &budget)?;
            self.recover_transaction_with_conditions_checked(command.mutation(), command.conditions(), &budget)
        })();
        result.map_err(|error| budget.failure(if budget.interrupted() {
            ControlError::CommitOutcomeUnknown
        } else { error }, Some(request.operation_id)).for_recovery())
    }

    /// Reads the exact validated committed visibility state without performing writes.
    ///
    /// # Errors
    /// Missing-after-write state, receipt contradictions, interruption or quarantine fails closed.
    pub fn read_publication_visibility<C: CancellationProbe>(&self, context: &OperationContext<C>)
        -> Result<Option<PublicationVisibilityState>, ControlCallError> {
        let budget = Budget::new(context);
        let result = (|| {
            require_schema(self)?;
            let snapshot = self.read_snapshot_checked(&budget)?;
            let state = snapshot.get(&key(STATE, self.limits)?)
                .map(codec::read_state).transpose()?;
            budget.check(Point::ReadComplete)?; Ok(state)
        })();
        result.map_err(|error| budget.failure(error, None))
    }

    pub(in crate::persistent) fn commit_visibility_checked(&mut self, request: &VisibleEpochCommit, boundary: Boundary, check: &dyn Check)
        -> Result<ControlCommitReceipt, ControlError> {
        let result = (|| {
            self.ensure_available()?; check.check(Point::Start)?; require_schema(self)?;
            let command = request.command(self.limits, check)?;
            check.check(Point::Validated)?;
            {
                let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
                let header = self.header_from(&read)?;
                let prior = operation_from(&read, request.prior_commit.operation_id, &header, self.limits)?
                    .ok_or(ControlError::OperationConflict)?;
                if !same_commit(&prior.receipt, &request.prior_commit) { return Err(ControlError::OperationConflict); }
                let replay = operation_from(&read, request.operation_id, &header, self.limits)?.is_some();
                if !replay && request.previous.guards.owner_epoch != self.identity.owner_epoch {
                    return Err(ControlError::GenerationMismatch);
                }
                // Validate old/current typed relationships before allowing mutation.
                // The common engine rechecks exact keys AND generation in its write transaction.
                self.snapshot_from_checked(&read, check)?;
            }
            self.transact_conditionally_checked(command, boundary, check)
        })();
        if self.identity.schema_version == PUBLICATION_VISIBILITY_SCHEMA_VERSION
            && result.as_ref().err().is_some_and(|error| is_corruption(*error)) { self.quarantined = true; }
        result
    }
}

pub(super) fn validate_initial_intent(snapshot: &JournalReadSnapshot, intent: &PublicationIntent,
    limits: JournalLimits) -> Result<(), ControlError> {
    let stored = snapshot.get(&key(STATE, limits)?).ok_or(ControlError::StoreCorrupt)?;
    let state = codec::read_state(stored)?;
    if state.visible_epoch.checked_next().ok() != Some(intent.target_epoch)
        || state.guards != intent.owner_source_membership_access_guards {
        return Err(ControlError::GenerationMismatch);
    }
    Ok(())
}

fn require_schema(journal: &PersistentControlJournal) -> Result<(), ControlError> {
    if journal.identity.schema_version == PUBLICATION_VISIBILITY_SCHEMA_VERSION { Ok(()) }
    else { Err(ControlError::SchemaUnsupported) }
}
fn key(bytes: &[u8], limits: JournalLimits) -> Result<ControlKey, ControlError> { ControlKey::new(bytes.to_vec(), limits) }
fn id_key(prefix: &[u8], id: &[u8; 16], limits: JournalLimits) -> Result<ControlKey, ControlError> {
    let mut bytes = prefix.to_vec(); bytes.extend_from_slice(id); key(&bytes, limits)
}
fn retired_key(receipt: PublicationReceiptId, member: ProjectionMembershipId, limits: JournalLimits)
    -> Result<ControlKey, ControlError> {
    let mut bytes = RETIRED.to_vec(); bytes.extend_from_slice(receipt.as_bytes());
    bytes.extend_from_slice(member.as_bytes()); key(&bytes, limits)
}
fn same_commit(left: &ControlCommitReceipt, right: &ControlCommitReceipt) -> bool {
    left.operation_id == right.operation_id && left.command_digest == right.command_digest
        && left.before_generation == right.before_generation && left.after_generation == right.after_generation
        && left.changed_keys == right.changed_keys
}

pub(in crate::persistent) use validation::validate_snapshot;
