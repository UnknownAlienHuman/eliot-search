//! Conservative source reconciliation for the W2 direct-source spine.
//!
//! This package performs no filesystem, database, process, or network I/O. It
//! compares one registered source against an exact current observation, keeps
//! path binding and content observation revisions independent, and never turns
//! path similarity or timeout into stable identity. Missing, replaced,
//! ambiguous, stale, and quarantined states remain explicit.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

use core::fmt;
use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, OwnerEpoch, ReceiptRef};
use search_source_identity::{CanonicalRelativePath, RootBindingId, SourceIdentity};
use search_source_registry::{RegisteredSource, SourceLifecycle};

/// Conservative finite reconciliation limits.
pub const DEFAULT_RECONCILE_LIMITS: ReconcileLimits = ReconcileLimits {
    max_batch_items: 4_096,
    max_reasons: 32,
    max_operations: 2_000_000,
    max_source_bytes: 8 * 1024 * 1024 * 1024,
};

/// Closed content-free reconciliation failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReconcileError {
    /// Limits are zero or internally inconsistent.
    InvalidLimits,
    /// Batch is empty or exceeds its finite item ceiling.
    BatchSizeInvalid,
    /// Batch contains the same stable source identity more than once.
    DuplicateSource,
    /// Batch contains the same operation identity more than once.
    DuplicateOperation,
    /// Operation identity was reused with another request digest.
    OperationConflict,
    /// Finite replay ledger is full.
    OperationCapacityExceeded,
    /// Reason set is empty or exceeds its finite ceiling.
    ReasonSetInvalid,
    /// Shared revision cannot advance exactly once.
    ContractExhausted,
}

impl ReconcileError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "RECONCILE_INVALID_LIMITS",
            Self::BatchSizeInvalid => "RECONCILE_BATCH_SIZE_INVALID",
            Self::DuplicateSource => "RECONCILE_DUPLICATE_SOURCE",
            Self::DuplicateOperation => "RECONCILE_DUPLICATE_OPERATION",
            Self::OperationConflict => "RECONCILE_OPERATION_CONFLICT",
            Self::OperationCapacityExceeded => "RECONCILE_OPERATION_CAPACITY_EXCEEDED",
            Self::ReasonSetInvalid => "RECONCILE_REASON_SET_INVALID",
            Self::ContractExhausted => "RECONCILE_CONTRACT_EXHAUSTED",
        }
    }
}

impl fmt::Display for ReconcileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ReconcileError {}

/// Finite reconciliation limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconcileLimits {
    /// Maximum observations in one deterministic batch.
    pub max_batch_items: usize,
    /// Maximum distinct denial or quarantine reasons.
    pub max_reasons: usize,
    /// Maximum retained operation outcomes.
    pub max_operations: usize,
    /// Maximum accepted exact observed source bytes.
    pub max_source_bytes: u64,
}

impl ReconcileLimits {
    /// Validates every finite dimension as non-zero.
    pub const fn validate(self) -> Result<Self, ReconcileError> {
        if self.max_batch_items == 0
            || self.max_reasons == 0
            || self.max_operations == 0
            || self.max_source_bytes == 0
        {
            Err(ReconcileError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Full-payload immutable reconciliation operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileOperation {
    operation_id: OpaqueId,
    request_digest: Blake3Digest32,
}

impl ReconcileOperation {
    /// Creates a replay-fenced operation.
    #[must_use]
    pub const fn new(operation_id: OpaqueId, request_digest: Blake3Digest32) -> Self {
        Self {
            operation_id,
            request_digest,
        }
    }

    /// Immutable operation identity.
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }

    /// Digest of exact canonical reconciliation request bytes.
    pub const fn request_digest(&self) -> Blake3Digest32 {
        self.request_digest
    }
}

/// Whether the authoritative observation found the source.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ObservationPresence {
    /// Exact source was observed.
    Present,
    /// Frozen-authority inventory verified the source absent.
    Missing,
    /// Observation continuity or denominator was not authoritative.
    Unknown,
}

/// Live restrictive-security state at reconciliation time.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReconcileSecurityDisposition {
    /// Current barrier permits reconciliation.
    Permitted,
    /// Current barrier restricts the source.
    Restricted,
    /// Security evidence is contradictory or unresolved.
    Quarantined,
}

/// Exact current source observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileObservation {
    /// Stable source identity being reconciled.
    pub source_identity: SourceIdentity,
    /// Whether exact authoritative observation found the source.
    pub presence: ObservationPresence,
    /// Stable logical-root binding observed.
    pub root_binding_id: RootBindingId,
    /// Current canonical relative path when present.
    pub relative_path: Option<CanonicalRelativePath>,
    /// Stable final-file identity when present and supported.
    pub stable_file_identity_digest: Option<Blake3Digest32>,
    /// Exact current content digest when present.
    pub content_digest: Option<Blake3Digest32>,
    /// Exact current byte length when present.
    pub content_bytes: Option<u64>,
    /// Current restrictive-security disposition.
    pub security_disposition: ReconcileSecurityDisposition,
    /// Exact restrictive-security barrier revision.
    pub security_barrier_revision: NonZeroRevision,
    /// Exact current data-root owner epoch.
    pub owner_epoch: OwnerEpoch,
    /// Content-free authoritative observation receipt.
    pub observation_receipt: Option<ReceiptRef>,
}

/// Exact reconciliation context supplied by the owning daemon.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconcileContext {
    /// Exact current registry revision.
    pub registry_revision: u64,
    /// Exact current data-root owner epoch.
    pub owner_epoch: OwnerEpoch,
    /// Exact current restrictive-security barrier revision.
    pub security_barrier_revision: NonZeroRevision,
}

/// Closed non-quarantine reason.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReconcileDenialReason {
    /// Request was evaluated against a stale registry revision.
    RegistryRevisionMismatch,
    /// Observation owner epoch is stale.
    OwnerEpochMismatch,
    /// Observation security-barrier revision is stale.
    SecurityBarrierRevisionMismatch,
    /// Source is already retired.
    SourceRetired,
    /// Live security state restricts the source.
    SecurityRestricted,
    /// Exact authoritative observation receipt is absent.
    ObservationReceiptMissing,
    /// Exact present observation is missing path, digest, or size fields.
    PresentObservationIncomplete,
    /// Exact observed source size exceeds the finite ceiling.
    SourceTooLarge,
    /// Stable file identity required for a rename was not observed.
    RenameIdentityEvidenceMissing,
    /// Stable identity continuity is insufficient for content replacement.
    ReplacementIdentityEvidenceMissing,
}

/// Closed quarantine reason.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReconcileQuarantineReason {
    /// Observation denominator or continuity is unknown.
    ObservationUnknown,
    /// Security evidence is contradictory or unresolved.
    SecurityEvidenceQuarantined,
    /// Observation names another stable source identity.
    SourceIdentityMismatch,
    /// Observation unexpectedly names another logical root.
    RootBindingMismatch,
    /// Stable final-file identity changed at the same active path.
    ReplacementDetected,
    /// Path and stable file evidence both changed and cannot be associated safely.
    AmbiguousMoveOrReplacement,
}

/// Deterministic reconciliation action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReconcileAction {
    /// No source/path/content state changed.
    NoChange,
    /// Record a new exact content observation under the existing path binding.
    UpdateContent {
        /// Expected current content-observation revision.
        expected_observation_revision: NonZeroRevision,
        /// Required next content-observation revision.
        next_observation_revision: NonZeroRevision,
        /// Exact new content digest.
        content_digest: Blake3Digest32,
        /// Exact new byte length.
        content_bytes: u64,
    },
    /// Rebind the same stable file identity to a new path.
    RebindPath {
        /// Expected current path-binding revision.
        expected_binding_revision: NonZeroRevision,
        /// Required next path-binding revision.
        next_binding_revision: NonZeroRevision,
        /// Exact prior path.
        old_path: CanonicalRelativePath,
        /// Exact new path.
        new_path: CanonicalRelativePath,
        /// Stable file identity binding the rename.
        stable_file_identity_digest: Blake3Digest32,
    },
    /// Rebind path and record changed content as one explicit composite plan.
    RebindPathAndUpdateContent {
        /// Expected current path-binding revision.
        expected_binding_revision: NonZeroRevision,
        /// Required next path-binding revision.
        next_binding_revision: NonZeroRevision,
        /// Expected current content-observation revision.
        expected_observation_revision: NonZeroRevision,
        /// Required next content-observation revision.
        next_observation_revision: NonZeroRevision,
        /// Exact prior path.
        old_path: CanonicalRelativePath,
        /// Exact new path.
        new_path: CanonicalRelativePath,
        /// Stable file identity binding the rename.
        stable_file_identity_digest: Blake3Digest32,
        /// Exact new content digest.
        content_digest: Blake3Digest32,
        /// Exact new byte length.
        content_bytes: u64,
    },
    /// Bind newly available stable file identity at the unchanged path.
    BindStableFileIdentity {
        /// Expected current path-binding revision.
        expected_binding_revision: NonZeroRevision,
        /// Required next path-binding revision.
        next_binding_revision: NonZeroRevision,
        /// Newly observed stable final-file identity.
        stable_file_identity_digest: Blake3Digest32,
    },
    /// Retire a source proven absent from a frozen authoritative inventory.
    RetireMissing {
        /// Expected current source-record revision.
        expected_source_revision: NonZeroRevision,
    },
    /// Retire the current stable source and nominate an unassigned replacement.
    RetireAndNominateReplacement {
        /// Expected current source-record revision.
        expected_source_revision: NonZeroRevision,
        /// Current active path occupied by the replacement candidate.
        path: CanonicalRelativePath,
        /// Newly observed stable final-file identity.
        replacement_file_identity_digest: Blake3Digest32,
        /// Exact candidate content digest.
        replacement_content_digest: Blake3Digest32,
        /// Exact candidate byte length.
        replacement_content_bytes: u64,
    },
}

/// Exact executable reconciliation plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcilePlan {
    /// Stable source identity.
    pub source_identity: SourceIdentity,
    /// Registry revision from which the plan was produced.
    pub expected_registry_revision: u64,
    /// Exact authoritative observation receipt.
    pub observation_receipt: ReceiptRef,
    /// Deterministic action.
    pub action: ReconcileAction,
    /// Full-payload operation.
    pub operation: ReconcileOperation,
}

/// Denied reconciliation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileDenial {
    /// Stable source identity.
    pub source_identity: SourceIdentity,
    /// Distinct reasons in canonical order.
    pub reasons: BTreeSet<ReconcileDenialReason>,
    /// Full-payload operation.
    pub operation: ReconcileOperation,
}

/// Quarantined reconciliation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileQuarantine {
    /// Stable source identity.
    pub source_identity: SourceIdentity,
    /// Distinct reasons in canonical order.
    pub reasons: BTreeSet<ReconcileQuarantineReason>,
    /// Full-payload operation.
    pub operation: ReconcileOperation,
}

/// Terminal reconciliation outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReconcileOutcome {
    /// Exact deterministic plan.
    Planned(ReconcilePlan),
    /// Preconditions or policy deny reconciliation.
    Denied(ReconcileDenial),
    /// Evidence is unsafe or ambiguous.
    Quarantined(ReconcileQuarantine),
}

impl ReconcileOutcome {
    /// Full-payload operation that produced the terminal result.
    pub const fn operation(&self) -> &ReconcileOperation {
        match self {
            Self::Planned(plan) => &plan.operation,
            Self::Denied(denial) => &denial.operation,
            Self::Quarantined(quarantine) => &quarantine.operation,
        }
    }

    /// Stable source identity.
    pub const fn source_identity(&self) -> &SourceIdentity {
        match self {
            Self::Planned(plan) => &plan.source_identity,
            Self::Denied(denial) => &denial.source_identity,
            Self::Quarantined(quarantine) => &quarantine.source_identity,
        }
    }
}

/// Produces one fail-closed reconciliation outcome.
pub fn reconcile_source(
    registered: &RegisteredSource,
    observation: ReconcileObservation,
    context: ReconcileContext,
    operation: ReconcileOperation,
    limits: ReconcileLimits,
) -> Result<ReconcileOutcome, ReconcileError> {
    let limits = limits.validate()?;
    let mut denial = BTreeSet::new();
    let mut quarantine = BTreeSet::new();

    if registered.identity() != &observation.source_identity {
        quarantine.insert(ReconcileQuarantineReason::SourceIdentityMismatch);
    }
    if registered.registry_revision() != context.registry_revision {
        denial.insert(ReconcileDenialReason::RegistryRevisionMismatch);
    }
    if observation.owner_epoch != context.owner_epoch {
        denial.insert(ReconcileDenialReason::OwnerEpochMismatch);
    }
    if observation.security_barrier_revision != context.security_barrier_revision {
        denial.insert(ReconcileDenialReason::SecurityBarrierRevisionMismatch);
    }
    if registered.lifecycle() != SourceLifecycle::Active {
        denial.insert(ReconcileDenialReason::SourceRetired);
    }
    match observation.security_disposition {
        ReconcileSecurityDisposition::Permitted => {}
        ReconcileSecurityDisposition::Restricted => {
            denial.insert(ReconcileDenialReason::SecurityRestricted);
        }
        ReconcileSecurityDisposition::Quarantined => {
            quarantine.insert(ReconcileQuarantineReason::SecurityEvidenceQuarantined);
        }
    }
    if observation.presence == ObservationPresence::Unknown {
        quarantine.insert(ReconcileQuarantineReason::ObservationUnknown);
    }
    if observation.observation_receipt.is_none() {
        denial.insert(ReconcileDenialReason::ObservationReceiptMissing);
    }
    if denial.len() > limits.max_reasons || quarantine.len() > limits.max_reasons {
        return Err(ReconcileError::ReasonSetInvalid);
    }

    if !quarantine.is_empty() {
        return Ok(ReconcileOutcome::Quarantined(ReconcileQuarantine {
            source_identity: observation.source_identity,
            reasons: quarantine,
            operation,
        }));
    }
    if !denial.is_empty() {
        return Ok(ReconcileOutcome::Denied(ReconcileDenial {
            source_identity: observation.source_identity,
            reasons: denial,
            operation,
        }));
    }

    let receipt = observation
        .observation_receipt
        .clone()
        .expect("absence was converted into a denial above");
    let action = match observation.presence {
        ObservationPresence::Unknown => unreachable!("unknown presence quarantined above"),
        ObservationPresence::Missing => ReconcileAction::RetireMissing {
            expected_source_revision: registered.source_revision(),
        },
        ObservationPresence::Present => {
            reconcile_present(registered, &observation, limits)?
        }
    };
    Ok(ReconcileOutcome::Planned(ReconcilePlan {
        source_identity: observation.source_identity,
        expected_registry_revision: context.registry_revision,
        observation_receipt: receipt,
        action,
        operation,
    }))
}

fn reconcile_present(
    registered: &RegisteredSource,
    observation: &ReconcileObservation,
    limits: ReconcileLimits,
) -> Result<ReconcileAction, ReconcileError> {
    let Some(observed_path) = observation.relative_path.as_ref() else {
        return denial_as_error(ReconcileDenialReason::PresentObservationIncomplete);
    };
    let Some(observed_content_digest) = observation.content_digest else {
        return denial_as_error(ReconcileDenialReason::PresentObservationIncomplete);
    };
    let Some(observed_content_bytes) = observation.content_bytes else {
        return denial_as_error(ReconcileDenialReason::PresentObservationIncomplete);
    };
    if observed_content_bytes > limits.max_source_bytes {
        return denial_as_error(ReconcileDenialReason::SourceTooLarge);
    }

    let binding = registered.binding();
    if binding.root_binding_id() != observation.root_binding_id {
        return quarantine_as_error(ReconcileQuarantineReason::RootBindingMismatch);
    }
    let path_changed = binding.relative_path() != observed_path;
    let content_changed = binding.content_digest() != observed_content_digest
        || binding.content_bytes() != observed_content_bytes;
    let current_file = binding.stable_file_identity_digest();
    let observed_file = observation.stable_file_identity_digest;

    match (current_file, observed_file, path_changed, content_changed) {
        (Some(current), Some(observed), _, _) if current != observed => {
            if !path_changed {
                Ok(ReconcileAction::RetireAndNominateReplacement {
                    expected_source_revision: registered.source_revision(),
                    path: observed_path.clone(),
                    replacement_file_identity_digest: observed,
                    replacement_content_digest: observed_content_digest,
                    replacement_content_bytes: observed_content_bytes,
                })
            } else {
                quarantine_as_error(ReconcileQuarantineReason::AmbiguousMoveOrReplacement)
            }
        }
        (Some(file), Some(_), false, false) => Ok(ReconcileAction::NoChange),
        (Some(file), Some(_), false, true) => Ok(ReconcileAction::UpdateContent {
            expected_observation_revision: binding.observation_revision(),
            next_observation_revision: next_revision(binding.observation_revision())?,
            content_digest: observed_content_digest,
            content_bytes: observed_content_bytes,
        }),
        (Some(file), Some(_), true, false) => Ok(ReconcileAction::RebindPath {
            expected_binding_revision: binding.binding_revision(),
            next_binding_revision: next_revision(binding.binding_revision())?,
            old_path: binding.relative_path().clone(),
            new_path: observed_path.clone(),
            stable_file_identity_digest: file,
        }),
        (Some(file), Some(_), true, true) => {
            Ok(ReconcileAction::RebindPathAndUpdateContent {
                expected_binding_revision: binding.binding_revision(),
                next_binding_revision: next_revision(binding.binding_revision())?,
                expected_observation_revision: binding.observation_revision(),
                next_observation_revision: next_revision(binding.observation_revision())?,
                old_path: binding.relative_path().clone(),
                new_path: observed_path.clone(),
                stable_file_identity_digest: file,
                content_digest: observed_content_digest,
                content_bytes: observed_content_bytes,
            })
        }
        (None, Some(file), false, false) | (None, Some(file), false, true) => {
            Ok(ReconcileAction::BindStableFileIdentity {
                expected_binding_revision: binding.binding_revision(),
                next_binding_revision: next_revision(binding.binding_revision())?,
                stable_file_identity_digest: file,
            })
        }
        (None, Some(_), true, _) | (Some(_), None, true, _) | (None, None, true, _) => {
            denial_as_error(ReconcileDenialReason::RenameIdentityEvidenceMissing)
        }
        (Some(_), None, false, true) | (None, None, false, true) => {
            denial_as_error(ReconcileDenialReason::ReplacementIdentityEvidenceMissing)
        }
        (Some(_), None, false, false) | (None, None, false, false) => {
            Ok(ReconcileAction::NoChange)
        }
    }
}

fn next_revision(current: NonZeroRevision) -> Result<NonZeroRevision, ReconcileError> {
    current
        .checked_next()
        .map_err(|_| ReconcileError::ContractExhausted)
}

fn denial_as_error<T>(_reason: ReconcileDenialReason) -> Result<T, ReconcileError> {
    Err(ReconcileError::ReasonSetInvalid)
}

fn quarantine_as_error<T>(_reason: ReconcileQuarantineReason) -> Result<T, ReconcileError> {
    Err(ReconcileError::ReasonSetInvalid)
}

/// Finite duplicate-free reconciliation batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileBatch {
    /// Exact registry revision shared by the batch.
    pub expected_registry_revision: u64,
    /// Finite source observations and operations.
    pub items: Vec<(SourceIdentity, ReconcileOperation)>,
}

impl ReconcileBatch {
    /// Validates finite size and duplicate source/operation targets.
    pub fn validate(&self, limits: ReconcileLimits) -> Result<(), ReconcileError> {
        let limits = limits.validate()?;
        if self.items.is_empty() || self.items.len() > limits.max_batch_items {
            return Err(ReconcileError::BatchSizeInvalid);
        }
        let mut sources = BTreeSet::new();
        let mut operations = BTreeSet::new();
        for (source, operation) in &self.items {
            if !sources.insert(source.clone()) {
                return Err(ReconcileError::DuplicateSource);
            }
            if !operations.insert(operation.operation_id().clone()) {
                return Err(ReconcileError::DuplicateOperation);
            }
        }
        Ok(())
    }
}

/// Replay-aware terminal reconciliation outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileLedgerOutcome {
    /// Exact terminal outcome.
    pub outcome: ReconcileOutcome,
    /// Whether an exact prior result was replayed.
    pub replayed: bool,
}

/// Finite exact-operation reconciliation ledger.
#[derive(Clone, Debug)]
pub struct ReconcileLedger {
    max_operations: usize,
    entries: Vec<(OpaqueId, Blake3Digest32, ReconcileOutcome)>,
}

impl ReconcileLedger {
    /// Creates an empty finite replay ledger.
    pub fn new(limits: ReconcileLimits) -> Result<Self, ReconcileError> {
        let limits = limits.validate()?;
        Ok(Self {
            max_operations: limits.max_operations,
            entries: Vec::new(),
        })
    }

    /// Records or exactly replays one terminal outcome.
    pub fn record(
        &mut self,
        outcome: ReconcileOutcome,
    ) -> Result<ReconcileLedgerOutcome, ReconcileError> {
        let operation = outcome.operation();
        if let Some((_, digest, existing)) = self
            .entries
            .iter()
            .find(|(operation_id, _, _)| operation_id == operation.operation_id())
        {
            if *digest != operation.request_digest() {
                return Err(ReconcileError::OperationConflict);
            }
            return Ok(ReconcileLedgerOutcome {
                outcome: existing.clone(),
                replayed: true,
            });
        }
        if self.entries.len() >= self.max_operations {
            return Err(ReconcileError::OperationCapacityExceeded);
        }
        self.entries.push((
            operation.operation_id().clone(),
            operation.request_digest(),
            outcome.clone(),
        ));
        Ok(ReconcileLedgerOutcome {
            outcome,
            replayed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::{OwnerEpoch, ReceiptRef};
    use search_source_admission::{
        AdmissionGrant, AdmissionOperation, AdmissionProfile, ResidencyClass, SourceModality,
    };
    use search_source_identity::{SourceBinding, SourceObservation};
    use search_source_registry::{AdmissionBindingProof, RegistryBatch, RegistryChange, RegistryOperation, SourceRegistry, DEFAULT_REGISTRY_LIMITS};
    use search_ports::{IdempotencyClass, MutationIdentity};

    fn source_identity() -> SourceIdentity {
        SourceIdentity::new(
            OpaqueId::new("namespace:test").expect("namespace"),
            OpaqueId::new("source:test").expect("source"),
        )
    }

    fn path(value: &str) -> CanonicalRelativePath {
        CanonicalRelativePath::new(
            value,
            search_source_identity::DEFAULT_IDENTITY_LIMITS,
        )
        .expect("path")
    }

    fn registered() -> RegisteredSource {
        let identity = source_identity();
        let binding = SourceBinding::new(
            identity.clone(),
            SourceObservation {
                root_binding_id: RootBindingId::from_bytes([1; 32]),
                relative_path: path("old.rs"),
                stable_file_identity_digest: Some(Blake3Digest32::from_bytes([2; 32])),
                content_digest: Blake3Digest32::from_bytes([3; 32]),
                content_bytes: 10,
                observation_receipt: ReceiptRef::new("receipt:source")
                    .expect("receipt"),
            },
            NonZeroRevision::new(1).expect("revision"),
            NonZeroRevision::new(1).expect("revision"),
        );
        let admission = AdmissionGrant {
            candidate_id: OpaqueId::new("candidate:test").expect("candidate"),
            profile: AdmissionProfile::Direct,
            modality: SourceModality::RegularFile,
            residency: ResidencyClass::LocalFixed,
            policy_revision: NonZeroRevision::new(1).expect("revision"),
            owner_epoch: OwnerEpoch::new(1).expect("epoch"),
            security_barrier_revision: NonZeroRevision::new(1).expect("revision"),
            evidence_digest: Blake3Digest32::from_bytes([4; 32]),
            operation: AdmissionOperation::new(
                MutationIdentity::new(
                    OpaqueId::new("admission-operation:test").expect("operation"),
                    IdempotencyClass::RetrySameIdentity,
                ),
                Blake3Digest32::from_bytes([5; 32]),
            ),
        };
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS)
            .expect("registry");
        registry
            .apply(RegistryBatch {
                expected_registry_revision: 0,
                operation: RegistryOperation::new(
                    OpaqueId::new("registry-operation:test").expect("operation"),
                    Blake3Digest32::from_bytes([6; 32]),
                ),
                changes: vec![RegistryChange::RegisterSource {
                    admission,
                    binding,
                    assignment: AdmissionBindingProof {
                        candidate_id: OpaqueId::new("candidate:test").expect("candidate"),
                        source_identity: identity.clone(),
                        assignment_digest: Blake3Digest32::from_bytes([7; 32]),
                        assignment_receipt: ReceiptRef::new("receipt:assignment")
                            .expect("receipt"),
                        readback_verified: true,
                    },
                    receipt: ReceiptRef::new("receipt:register").expect("receipt"),
                }],
            })
            .expect("register");
        registry.source(&identity).expect("source").clone()
    }

    fn operation(name: &str, digest: u8) -> ReconcileOperation {
        ReconcileOperation::new(
            OpaqueId::new(format!("reconcile-operation:{name}"))
                .expect("operation"),
            Blake3Digest32::from_bytes([digest; 32]),
        )
    }

    fn observation() -> ReconcileObservation {
        ReconcileObservation {
            source_identity: source_identity(),
            presence: ObservationPresence::Present,
            root_binding_id: RootBindingId::from_bytes([1; 32]),
            relative_path: Some(path("old.rs")),
            stable_file_identity_digest: Some(Blake3Digest32::from_bytes([2; 32])),
            content_digest: Some(Blake3Digest32::from_bytes([3; 32])),
            content_bytes: Some(10),
            security_disposition: ReconcileSecurityDisposition::Permitted,
            security_barrier_revision: NonZeroRevision::new(1).expect("revision"),
            owner_epoch: OwnerEpoch::new(1).expect("epoch"),
            observation_receipt: Some(
                ReceiptRef::new("receipt:reconcile").expect("receipt"),
            ),
        }
    }

    fn context() -> ReconcileContext {
        ReconcileContext {
            registry_revision: 1,
            owner_epoch: OwnerEpoch::new(1).expect("epoch"),
            security_barrier_revision: NonZeroRevision::new(1).expect("revision"),
        }
    }

    #[test]
    fn exact_unchanged_source_produces_no_change() {
        let ReconcileOutcome::Planned(plan) = reconcile_source(
            &registered(),
            observation(),
            context(),
            operation("unchanged", 1),
            DEFAULT_RECONCILE_LIMITS,
        )
        .expect("reconcile") else {
            panic!("exact source should plan")
        };
        assert_eq!(plan.action, ReconcileAction::NoChange);
    }

    #[test]
    fn stable_file_identity_supports_rename() {
        let mut observation = observation();
        observation.relative_path = Some(path("new.rs"));
        let ReconcileOutcome::Planned(plan) = reconcile_source(
            &registered(),
            observation,
            context(),
            operation("rename", 2),
            DEFAULT_RECONCILE_LIMITS,
        )
        .expect("reconcile") else {
            panic!("rename should plan")
        };
        assert!(matches!(plan.action, ReconcileAction::RebindPath { .. }));
    }

    #[test]
    fn same_path_with_other_file_identity_nominates_replacement() {
        let mut observation = observation();
        observation.stable_file_identity_digest = Some(Blake3Digest32::from_bytes([9; 32]));
        let ReconcileOutcome::Planned(plan) = reconcile_source(
            &registered(),
            observation,
            context(),
            operation("replacement", 3),
            DEFAULT_RECONCILE_LIMITS,
        )
        .expect("reconcile") else {
            panic!("replacement should be explicit")
        };
        assert!(matches!(
            plan.action,
            ReconcileAction::RetireAndNominateReplacement { .. }
        ));
    }

    #[test]
    fn path_and_file_identity_change_is_never_silent_rename() {
        let mut observation = observation();
        observation.relative_path = Some(path("new.rs"));
        observation.stable_file_identity_digest = Some(Blake3Digest32::from_bytes([9; 32]));
        assert_eq!(
            reconcile_source(
                &registered(),
                observation,
                context(),
                operation("ambiguous", 4),
                DEFAULT_RECONCILE_LIMITS,
            ),
            Err(ReconcileError::ReasonSetInvalid)
        );
    }

    #[test]
    fn authoritative_missing_source_is_retired_explicitly() {
        let mut observation = observation();
        observation.presence = ObservationPresence::Missing;
        observation.relative_path = None;
        observation.stable_file_identity_digest = None;
        observation.content_digest = None;
        observation.content_bytes = None;
        let ReconcileOutcome::Planned(plan) = reconcile_source(
            &registered(),
            observation,
            context(),
            operation("missing", 5),
            DEFAULT_RECONCILE_LIMITS,
        )
        .expect("reconcile") else {
            panic!("missing should plan retirement")
        };
        assert!(matches!(plan.action, ReconcileAction::RetireMissing { .. }));
    }

    #[test]
    fn unknown_observation_quarantines() {
        let mut observation = observation();
        observation.presence = ObservationPresence::Unknown;
        assert!(matches!(
            reconcile_source(
                &registered(),
                observation,
                context(),
                operation("unknown", 6),
                DEFAULT_RECONCILE_LIMITS,
            )
            .expect("outcome"),
            ReconcileOutcome::Quarantined(_)
        ));
    }

    #[test]
    fn stale_owner_epoch_is_denied() {
        let mut observation = observation();
        observation.owner_epoch = OwnerEpoch::new(2).expect("epoch");
        let ReconcileOutcome::Denied(denial) = reconcile_source(
            &registered(),
            observation,
            context(),
            operation("stale-owner", 7),
            DEFAULT_RECONCILE_LIMITS,
        )
        .expect("outcome") else {
            panic!("stale owner must deny")
        };
        assert!(denial.reasons.contains(&ReconcileDenialReason::OwnerEpochMismatch));
    }

    #[test]
    fn exact_operation_replay_is_stable() {
        let outcome = reconcile_source(
            &registered(),
            observation(),
            context(),
            operation("replay", 8),
            DEFAULT_RECONCILE_LIMITS,
        )
        .expect("outcome");
        let mut ledger = ReconcileLedger::new(DEFAULT_RECONCILE_LIMITS)
            .expect("ledger");
        assert!(!ledger.record(outcome.clone()).expect("first").replayed);
        assert!(ledger.record(outcome).expect("replay").replayed);
    }
}
