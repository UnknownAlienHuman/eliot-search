//! Atomic revisioned source registry for the W2 direct-source spine.
//!
//! The registry consumes terminal admission grants and externally assigned
//! stable source identities. It performs no filesystem, network, process, or
//! database I/O. Every batch is finite, exact-revision guarded, replay-fenced,
//! staged before commit, and either applies completely or not at all.

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
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};
use search_source_admission::AdmissionGrant;
use search_source_identity::{SourceBinding, SourceIdentity};

/// Conservative finite source-registry limits.
pub const DEFAULT_REGISTRY_LIMITS: RegistryLimits = RegistryLimits {
    max_sources: 1_000_000,
    max_memberships: 4_000_000,
    max_batch_changes: 4_096,
    max_operations: 2_000_000,
    max_portfolio_items: 100_000,
    max_cutover_inventory: 1_000_000,
};

/// Closed content-free registry failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RegistryError {
    /// Registry limits are zero or internally inconsistent.
    InvalidLimits,
    /// Batch is empty or exceeds its finite change ceiling.
    BatchSizeInvalid,
    /// Expected registry revision is stale.
    RegistryRevisionConflict,
    /// Registry revision cannot advance.
    RegistryRevisionOverflow,
    /// Operation identity was reused with another full-payload digest.
    OperationConflict,
    /// Finite operation ledger is full.
    OperationCapacityExceeded,
    /// One batch touches the same source or membership more than once.
    DuplicateBatchTarget,
    /// Stable source identity is already registered.
    SourceAlreadyRegistered,
    /// Stable source identity is absent.
    SourceNotFound,
    /// Admission candidate and assignment proof differ.
    AdmissionBindingMismatch,
    /// Admission assignment receipt or exact binding digest is absent.
    AdmissionBindingEvidenceMissing,
    /// Existing source binding differs from the registration request.
    SourceBindingConflict,
    /// Source lifecycle or revision does not permit the mutation.
    SourceRevisionConflict,
    /// Membership is already active or conflicts with another generation.
    MembershipCollision,
    /// Membership is absent.
    MembershipNotFound,
    /// Membership revision differs.
    MembershipRevisionConflict,
    /// Source is retired and cannot enter an active portfolio.
    SourceRetired,
    /// Membership references another namespace.
    NamespaceMismatch,
    /// Cutover inventory is empty, duplicated, or exceeds its finite ceiling.
    CutoverInventoryInvalid,
    /// Cutover generation is stale, reused, or not strictly newer.
    CutoverGenerationConflict,
    /// Cutover authorization or authoritative readback is missing.
    CutoverEvidenceMissing,
    /// Cutover inventory contains an absent or retired source.
    CutoverSourceUnavailable,
    /// Portfolio request is zero or exceeds its finite ceiling.
    PortfolioLimitInvalid,
    /// Registry would exceed finite source or membership capacity.
    CapacityExceeded,
    /// Shared source or membership revision cannot advance.
    ContractExhausted,
}

impl RegistryError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "REGISTRY_INVALID_LIMITS",
            Self::BatchSizeInvalid => "REGISTRY_BATCH_SIZE_INVALID",
            Self::RegistryRevisionConflict => "REGISTRY_REVISION_CONFLICT",
            Self::RegistryRevisionOverflow => "REGISTRY_REVISION_OVERFLOW",
            Self::OperationConflict => "REGISTRY_OPERATION_CONFLICT",
            Self::OperationCapacityExceeded => "REGISTRY_OPERATION_CAPACITY_EXCEEDED",
            Self::DuplicateBatchTarget => "REGISTRY_DUPLICATE_BATCH_TARGET",
            Self::SourceAlreadyRegistered => "REGISTRY_SOURCE_ALREADY_REGISTERED",
            Self::SourceNotFound => "REGISTRY_SOURCE_NOT_FOUND",
            Self::AdmissionBindingMismatch => "REGISTRY_ADMISSION_BINDING_MISMATCH",
            Self::AdmissionBindingEvidenceMissing => "REGISTRY_ADMISSION_BINDING_EVIDENCE_MISSING",
            Self::SourceBindingConflict => "REGISTRY_SOURCE_BINDING_CONFLICT",
            Self::SourceRevisionConflict => "REGISTRY_SOURCE_REVISION_CONFLICT",
            Self::MembershipCollision => "REGISTRY_MEMBERSHIP_COLLISION",
            Self::MembershipNotFound => "REGISTRY_MEMBERSHIP_NOT_FOUND",
            Self::MembershipRevisionConflict => "REGISTRY_MEMBERSHIP_REVISION_CONFLICT",
            Self::SourceRetired => "REGISTRY_SOURCE_RETIRED",
            Self::NamespaceMismatch => "REGISTRY_NAMESPACE_MISMATCH",
            Self::CutoverInventoryInvalid => "REGISTRY_CUTOVER_INVENTORY_INVALID",
            Self::CutoverGenerationConflict => "REGISTRY_CUTOVER_GENERATION_CONFLICT",
            Self::CutoverEvidenceMissing => "REGISTRY_CUTOVER_EVIDENCE_MISSING",
            Self::CutoverSourceUnavailable => "REGISTRY_CUTOVER_SOURCE_UNAVAILABLE",
            Self::PortfolioLimitInvalid => "REGISTRY_PORTFOLIO_LIMIT_INVALID",
            Self::CapacityExceeded => "REGISTRY_CAPACITY_EXCEEDED",
            Self::ContractExhausted => "REGISTRY_CONTRACT_EXHAUSTED",
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RegistryError {}

/// Finite source-registry limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryLimits {
    /// Maximum retained stable sources.
    pub max_sources: usize,
    /// Maximum retained source/corpus membership records.
    pub max_memberships: usize,
    /// Maximum changes in one atomic batch.
    pub max_batch_changes: usize,
    /// Maximum retained full-payload operation identities.
    pub max_operations: usize,
    /// Maximum active portfolio results.
    pub max_portfolio_items: usize,
    /// Maximum stable identities in one namespace cutover inventory.
    pub max_cutover_inventory: usize,
}

impl RegistryLimits {
    /// Validates all finite dimensions as non-zero.
    pub const fn validate(self) -> Result<Self, RegistryError> {
        if self.max_sources == 0
            || self.max_memberships == 0
            || self.max_batch_changes == 0
            || self.max_operations == 0
            || self.max_portfolio_items == 0
            || self.max_cutover_inventory == 0
        {
            Err(RegistryError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Full-payload immutable registry operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryOperation {
    operation_id: OpaqueId,
    mutation_digest: Blake3Digest32,
}

impl RegistryOperation {
    /// Creates an operation from immutable identity and digest of the complete
    /// canonical batch payload.
    #[must_use]
    pub const fn new(operation_id: OpaqueId, mutation_digest: Blake3Digest32) -> Self {
        Self {
            operation_id,
            mutation_digest,
        }
    }

    /// Immutable operation identity.
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }

    /// Digest of the complete canonical batch payload.
    pub const fn mutation_digest(&self) -> Blake3Digest32 {
        self.mutation_digest
    }
}

/// Exact proof binding a terminal admission candidate to a registry-assigned
/// stable source identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionBindingProof {
    /// Candidate identity from the terminal admission grant.
    pub candidate_id: OpaqueId,
    /// Registry-assigned stable source identity.
    pub source_identity: SourceIdentity,
    /// Digest of candidate, stable identity, admission grant, and source binding.
    pub assignment_digest: Blake3Digest32,
    /// Content-free assignment receipt.
    pub assignment_receipt: ReceiptRef,
    /// Whether exact authoritative assignment readback was verified.
    pub readback_verified: bool,
}

/// Source lifecycle visible to portfolios.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceLifecycle {
    /// Source is active and may participate in admitted portfolios.
    Active,
    /// Source is retired and excluded from new portfolios.
    Retired,
}

/// Registry-owned source record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredSource {
    binding: SourceBinding,
    admission: AdmissionGrant,
    assignment: AdmissionBindingProof,
    lifecycle: SourceLifecycle,
    source_revision: NonZeroRevision,
    registry_revision: u64,
    last_receipt: ReceiptRef,
}

impl RegisteredSource {
    /// Stable source identity.
    pub const fn identity(&self) -> &SourceIdentity {
        self.binding.identity()
    }

    /// Current source binding.
    pub const fn binding(&self) -> &SourceBinding {
        &self.binding
    }

    /// Terminal admission grant bound to this registration.
    pub const fn admission(&self) -> &AdmissionGrant {
        &self.admission
    }

    /// Exact candidate-to-source assignment proof.
    pub const fn assignment(&self) -> &AdmissionBindingProof {
        &self.assignment
    }

    /// Current lifecycle.
    pub const fn lifecycle(&self) -> SourceLifecycle {
        self.lifecycle
    }

    /// Monotone source-record revision.
    pub const fn source_revision(&self) -> NonZeroRevision {
        self.source_revision
    }

    /// Registry revision that last changed this record.
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }

    /// Content-free receipt for the last source-record mutation.
    pub const fn last_receipt(&self) -> &ReceiptRef {
        &self.last_receipt
    }
}

/// Stable source/corpus membership key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MembershipKey {
    /// Corpus identity.
    pub corpus_id: OpaqueId,
    /// Stable source identity.
    pub source_identity: SourceIdentity,
}

/// Membership lifecycle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MembershipLifecycle {
    /// Membership is active in its generation.
    Active,
    /// Membership is retired but retained as a tombstone.
    Retired,
}

/// Registry-owned source/corpus membership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipRecord {
    key: MembershipKey,
    generation: NonZeroRevision,
    membership_revision: NonZeroRevision,
    lifecycle: MembershipLifecycle,
    registry_revision: u64,
    last_receipt: ReceiptRef,
}

impl MembershipRecord {
    /// Stable membership key.
    pub const fn key(&self) -> &MembershipKey {
        &self.key
    }

    /// Active or retired namespace generation.
    pub const fn generation(&self) -> NonZeroRevision {
        self.generation
    }

    /// Monotone membership revision.
    pub const fn membership_revision(&self) -> NonZeroRevision {
        self.membership_revision
    }

    /// Membership lifecycle.
    pub const fn lifecycle(&self) -> MembershipLifecycle {
        self.lifecycle
    }

    /// Registry revision that last changed the membership.
    pub const fn registry_revision(&self) -> u64 {
        self.registry_revision
    }

    /// Content-free receipt for the last membership mutation.
    pub const fn last_receipt(&self) -> &ReceiptRef {
        &self.last_receipt
    }
}

/// New membership request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewMembership {
    /// Corpus/source key.
    pub key: MembershipKey,
    /// Active namespace generation.
    pub generation: NonZeroRevision,
    /// Initial content-free membership receipt.
    pub receipt: ReceiptRef,
}

/// Exact namespace cutover request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceCutover {
    /// Corpus whose active inventory changes atomically.
    pub corpus_id: OpaqueId,
    /// Exact current active generation.
    pub expected_generation: Option<NonZeroRevision>,
    /// Strictly newer generation.
    pub next_generation: NonZeroRevision,
    /// Complete frozen active inventory in canonical source-identity order.
    pub frozen_inventory: Vec<SourceIdentity>,
    /// Digest of exact canonical frozen inventory.
    pub inventory_digest: Blake3Digest32,
    /// Whether mutation authorization was verified.
    pub authorization_verified: bool,
    /// Whether authoritative post-cutover readback is required and available.
    pub readback_verified: bool,
    /// Content-free cutover receipt.
    pub receipt: ReceiptRef,
}

/// One source-registry change inside an atomic batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryChange {
    /// Register a newly assigned admitted source.
    RegisterSource {
        /// Terminal admission grant.
        admission: AdmissionGrant,
        /// First current source binding.
        binding: SourceBinding,
        /// Candidate-to-stable-identity assignment proof.
        assignment: AdmissionBindingProof,
        /// Content-free registration receipt.
        receipt: ReceiptRef,
    },
    /// Replace only the current source binding under exact source revision.
    UpdateSourceBinding {
        /// Stable source identity.
        identity: SourceIdentity,
        /// Expected current source-record revision.
        expected_source_revision: NonZeroRevision,
        /// Exact replacement source binding.
        binding: SourceBinding,
        /// Content-free update receipt.
        receipt: ReceiptRef,
    },
    /// Retire one active source.
    RetireSource {
        /// Stable source identity.
        identity: SourceIdentity,
        /// Expected current source-record revision.
        expected_source_revision: NonZeroRevision,
        /// Content-free retirement receipt.
        receipt: ReceiptRef,
    },
    /// Add one source/corpus membership.
    AddMembership(NewMembership),
    /// Retire one active source/corpus membership.
    RetireMembership {
        /// Exact membership key.
        key: MembershipKey,
        /// Expected current membership revision.
        expected_membership_revision: NonZeroRevision,
        /// Content-free retirement receipt.
        receipt: ReceiptRef,
    },
    /// Atomically replace one corpus active inventory and generation.
    CutoverNamespace(NamespaceCutover),
}

impl RegistryChange {
    fn source_target(&self) -> Option<&SourceIdentity> {
        match self {
            Self::RegisterSource { binding, .. } => Some(binding.identity()),
            Self::UpdateSourceBinding { identity, .. } | Self::RetireSource { identity, .. } => {
                Some(identity)
            }
            Self::AddMembership(_) | Self::RetireMembership { .. } | Self::CutoverNamespace(_) => {
                None
            }
        }
    }

    fn membership_target(&self) -> Option<&MembershipKey> {
        match self {
            Self::AddMembership(membership) => Some(&membership.key),
            Self::RetireMembership { key, .. } => Some(key),
            Self::RegisterSource { .. }
            | Self::UpdateSourceBinding { .. }
            | Self::RetireSource { .. }
            | Self::CutoverNamespace(_) => None,
        }
    }

    fn cutover_target(&self) -> Option<&OpaqueId> {
        match self {
            Self::CutoverNamespace(cutover) => Some(&cutover.corpus_id),
            _ => None,
        }
    }
}

/// Finite exact-revision guarded atomic registry batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryBatch {
    /// Exact registry revision required before application.
    pub expected_registry_revision: u64,
    /// Full-payload immutable operation.
    pub operation: RegistryOperation,
    /// Finite ordered changes.
    pub changes: Vec<RegistryChange>,
}

impl RegistryBatch {
    /// Validates finite size and duplicate mutation targets.
    pub fn validate(&self, limits: RegistryLimits) -> Result<(), RegistryError> {
        let limits = limits.validate()?;
        if self.changes.is_empty() || self.changes.len() > limits.max_batch_changes {
            return Err(RegistryError::BatchSizeInvalid);
        }
        let mut sources = BTreeSet::new();
        let mut memberships = BTreeSet::new();
        let mut cutovers = BTreeSet::new();
        for change in &self.changes {
            if let Some(source) = change.source_target() {
                if !sources.insert(source.clone()) {
                    return Err(RegistryError::DuplicateBatchTarget);
                }
            }
            if let Some(membership) = change.membership_target() {
                if !memberships.insert(membership.clone()) {
                    return Err(RegistryError::DuplicateBatchTarget);
                }
            }
            if let Some(corpus) = change.cutover_target() {
                if !cutovers.insert(corpus.clone()) {
                    return Err(RegistryError::DuplicateBatchTarget);
                }
                if memberships
                    .iter()
                    .any(|membership| &membership.corpus_id == corpus)
                {
                    return Err(RegistryError::DuplicateBatchTarget);
                }
            }
        }
        for membership in &memberships {
            if cutovers.contains(&membership.corpus_id) {
                return Err(RegistryError::DuplicateBatchTarget);
            }
        }
        Ok(())
    }
}

/// Content-free atomic registry receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryReceipt {
    /// Full-payload operation.
    pub operation: RegistryOperation,
    /// Registry revision before the atomic batch.
    pub before_revision: u64,
    /// Registry revision after the atomic batch.
    pub after_revision: u64,
    /// Number of committed changes.
    pub change_count: usize,
    /// Digest of complete canonical batch payload.
    pub mutation_digest: Blake3Digest32,
    /// Whether this receipt came from exact idempotency replay.
    pub replayed: bool,
}

/// One active admitted portfolio item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioItem {
    /// Stable source record.
    pub source: RegisteredSource,
    /// Active membership record.
    pub membership: MembershipRecord,
}

/// Finite atomic source registry.
#[derive(Clone, Debug)]
pub struct SourceRegistry {
    limits: RegistryLimits,
    revision: u64,
    sources: BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: BTreeMap<MembershipKey, MembershipRecord>,
    active_generations: BTreeMap<OpaqueId, NonZeroRevision>,
    operations: Vec<(OpaqueId, Blake3Digest32, RegistryReceipt)>,
}

impl SourceRegistry {
    /// Creates an empty finite registry.
    pub fn new(limits: RegistryLimits) -> Result<Self, RegistryError> {
        Ok(Self {
            limits: limits.validate()?,
            revision: 0,
            sources: BTreeMap::new(),
            memberships: BTreeMap::new(),
            active_generations: BTreeMap::new(),
            operations: Vec::new(),
        })
    }

    /// Exact current registry revision.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns one exact source record.
    pub fn source(&self, identity: &SourceIdentity) -> Result<&RegisteredSource, RegistryError> {
        self.sources
            .get(identity)
            .ok_or(RegistryError::SourceNotFound)
    }

    /// Returns one exact membership record.
    pub fn membership(&self, key: &MembershipKey) -> Result<&MembershipRecord, RegistryError> {
        self.memberships
            .get(key)
            .ok_or(RegistryError::MembershipNotFound)
    }

    /// Current active generation for a corpus.
    pub fn active_generation(&self, corpus_id: &OpaqueId) -> Option<NonZeroRevision> {
        self.active_generations.get(corpus_id).copied()
    }

    /// Applies one finite atomic batch or returns the exact prior replay receipt.
    pub fn apply(&mut self, batch: RegistryBatch) -> Result<RegistryReceipt, RegistryError> {
        batch.validate(self.limits)?;
        if let Some((_, digest, receipt)) = self
            .operations
            .iter()
            .find(|(operation_id, _, _)| operation_id == batch.operation.operation_id())
        {
            if *digest != batch.operation.mutation_digest() {
                return Err(RegistryError::OperationConflict);
            }
            let mut replay = receipt.clone();
            replay.replayed = true;
            return Ok(replay);
        }
        if self.operations.len() >= self.limits.max_operations {
            return Err(RegistryError::OperationCapacityExceeded);
        }
        if batch.expected_registry_revision != self.revision {
            return Err(RegistryError::RegistryRevisionConflict);
        }
        let after_revision = self
            .revision
            .checked_add(1)
            .ok_or(RegistryError::RegistryRevisionOverflow)?;

        let mut staged_sources = self.sources.clone();
        let mut staged_memberships = self.memberships.clone();
        let mut staged_generations = self.active_generations.clone();
        for change in &batch.changes {
            apply_change(
                change,
                after_revision,
                self.limits,
                &mut staged_sources,
                &mut staged_memberships,
                &mut staged_generations,
            )?;
        }
        if staged_sources.len() > self.limits.max_sources
            || staged_memberships.len() > self.limits.max_memberships
        {
            return Err(RegistryError::CapacityExceeded);
        }

        let receipt = RegistryReceipt {
            operation: batch.operation.clone(),
            before_revision: self.revision,
            after_revision,
            change_count: batch.changes.len(),
            mutation_digest: batch.operation.mutation_digest(),
            replayed: false,
        };
        self.sources = staged_sources;
        self.memberships = staged_memberships;
        self.active_generations = staged_generations;
        self.revision = after_revision;
        self.operations.push((
            batch.operation.operation_id().clone(),
            batch.operation.mutation_digest(),
            receipt.clone(),
        ));
        Ok(receipt)
    }

    /// Returns the active admitted portfolio for one exact corpus generation.
    pub fn active_portfolio(
        &self,
        corpus_id: &OpaqueId,
        generation: NonZeroRevision,
        max_items: usize,
    ) -> Result<Vec<PortfolioItem>, RegistryError> {
        if max_items == 0 || max_items > self.limits.max_portfolio_items {
            return Err(RegistryError::PortfolioLimitInvalid);
        }
        if self.active_generation(corpus_id) != Some(generation) {
            return Err(RegistryError::CutoverGenerationConflict);
        }
        let mut result = Vec::new();
        for (key, membership) in &self.memberships {
            if &key.corpus_id != corpus_id
                || membership.generation != generation
                || membership.lifecycle != MembershipLifecycle::Active
            {
                continue;
            }
            let source = self.source(&key.source_identity)?;
            if source.lifecycle != SourceLifecycle::Active {
                return Err(RegistryError::SourceRetired);
            }
            if result.len() >= max_items {
                return Err(RegistryError::PortfolioLimitInvalid);
            }
            result.push(PortfolioItem {
                source: source.clone(),
                membership: membership.clone(),
            });
        }
        Ok(result)
    }
}

fn apply_change(
    change: &RegistryChange,
    registry_revision: u64,
    limits: RegistryLimits,
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
    active_generations: &mut BTreeMap<OpaqueId, NonZeroRevision>,
) -> Result<(), RegistryError> {
    match change {
        RegistryChange::RegisterSource {
            admission,
            binding,
            assignment,
            receipt,
        } => register_source(
            admission,
            binding,
            assignment,
            receipt,
            registry_revision,
            sources,
        ),
        RegistryChange::UpdateSourceBinding {
            identity,
            expected_source_revision,
            binding,
            receipt,
        } => update_source_binding(
            identity,
            *expected_source_revision,
            binding,
            receipt,
            registry_revision,
            sources,
        ),
        RegistryChange::RetireSource {
            identity,
            expected_source_revision,
            receipt,
        } => retire_source(
            identity,
            *expected_source_revision,
            receipt,
            registry_revision,
            sources,
            memberships,
        ),
        RegistryChange::AddMembership(membership) => add_membership(
            membership,
            registry_revision,
            sources,
            memberships,
            active_generations,
        ),
        RegistryChange::RetireMembership {
            key,
            expected_membership_revision,
            receipt,
        } => retire_membership(
            key,
            *expected_membership_revision,
            receipt,
            registry_revision,
            memberships,
        ),
        RegistryChange::CutoverNamespace(cutover) => apply_cutover(
            cutover,
            registry_revision,
            limits,
            sources,
            memberships,
            active_generations,
        ),
    }
}

fn register_source(
    admission: &AdmissionGrant,
    binding: &SourceBinding,
    assignment: &AdmissionBindingProof,
    receipt: &ReceiptRef,
    registry_revision: u64,
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
) -> Result<(), RegistryError> {
    if sources.contains_key(binding.identity()) {
        return Err(RegistryError::SourceAlreadyRegistered);
    }
    if admission.candidate_id != assignment.candidate_id
        || binding.identity() != &assignment.source_identity
    {
        return Err(RegistryError::AdmissionBindingMismatch);
    }
    if !assignment.readback_verified {
        return Err(RegistryError::AdmissionBindingEvidenceMissing);
    }
    let source_revision = NonZeroRevision::new(1).map_err(|_| RegistryError::ContractExhausted)?;
    sources.insert(
        binding.identity().clone(),
        RegisteredSource {
            binding: binding.clone(),
            admission: admission.clone(),
            assignment: assignment.clone(),
            lifecycle: SourceLifecycle::Active,
            source_revision,
            registry_revision,
            last_receipt: receipt.clone(),
        },
    );
    Ok(())
}

fn update_source_binding(
    identity: &SourceIdentity,
    expected_source_revision: NonZeroRevision,
    binding: &SourceBinding,
    receipt: &ReceiptRef,
    registry_revision: u64,
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
) -> Result<(), RegistryError> {
    let source = sources
        .get_mut(identity)
        .ok_or(RegistryError::SourceNotFound)?;
    if source.lifecycle != SourceLifecycle::Active {
        return Err(RegistryError::SourceRetired);
    }
    if source.source_revision != expected_source_revision {
        return Err(RegistryError::SourceRevisionConflict);
    }
    if binding.identity() != identity {
        return Err(RegistryError::SourceBindingConflict);
    }
    source.source_revision = source
        .source_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    source.binding = binding.clone();
    source.registry_revision = registry_revision;
    source.last_receipt = receipt.clone();
    Ok(())
}

fn retire_source(
    identity: &SourceIdentity,
    expected_source_revision: NonZeroRevision,
    receipt: &ReceiptRef,
    registry_revision: u64,
    sources: &mut BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
) -> Result<(), RegistryError> {
    let source = sources
        .get_mut(identity)
        .ok_or(RegistryError::SourceNotFound)?;
    if source.lifecycle != SourceLifecycle::Active
        || source.source_revision != expected_source_revision
    {
        return Err(RegistryError::SourceRevisionConflict);
    }
    source.source_revision = source
        .source_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    source.lifecycle = SourceLifecycle::Retired;
    source.registry_revision = registry_revision;
    source.last_receipt = receipt.clone();
    for membership in memberships.values_mut() {
        if &membership.key.source_identity == identity
            && membership.lifecycle == MembershipLifecycle::Active
        {
            membership.membership_revision = membership
                .membership_revision
                .checked_next()
                .map_err(|_| RegistryError::ContractExhausted)?;
            membership.lifecycle = MembershipLifecycle::Retired;
            membership.registry_revision = registry_revision;
            membership.last_receipt = receipt.clone();
        }
    }
    Ok(())
}

fn add_membership(
    membership: &NewMembership,
    registry_revision: u64,
    sources: &BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
    active_generations: &mut BTreeMap<OpaqueId, NonZeroRevision>,
) -> Result<(), RegistryError> {
    let source = sources
        .get(&membership.key.source_identity)
        .ok_or(RegistryError::SourceNotFound)?;
    if source.lifecycle != SourceLifecycle::Active {
        return Err(RegistryError::SourceRetired);
    }
    if memberships.contains_key(&membership.key) {
        return Err(RegistryError::MembershipCollision);
    }
    if let Some(active) = active_generations.get(&membership.key.corpus_id) {
        if *active != membership.generation {
            return Err(RegistryError::CutoverGenerationConflict);
        }
    } else {
        active_generations.insert(membership.key.corpus_id.clone(), membership.generation);
    }
    memberships.insert(
        membership.key.clone(),
        MembershipRecord {
            key: membership.key.clone(),
            generation: membership.generation,
            membership_revision: NonZeroRevision::new(1)
                .map_err(|_| RegistryError::ContractExhausted)?,
            lifecycle: MembershipLifecycle::Active,
            registry_revision,
            last_receipt: membership.receipt.clone(),
        },
    );
    Ok(())
}

fn retire_membership(
    key: &MembershipKey,
    expected_membership_revision: NonZeroRevision,
    receipt: &ReceiptRef,
    registry_revision: u64,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
) -> Result<(), RegistryError> {
    let membership = memberships
        .get_mut(key)
        .ok_or(RegistryError::MembershipNotFound)?;
    if membership.lifecycle != MembershipLifecycle::Active
        || membership.membership_revision != expected_membership_revision
    {
        return Err(RegistryError::MembershipRevisionConflict);
    }
    membership.membership_revision = membership
        .membership_revision
        .checked_next()
        .map_err(|_| RegistryError::ContractExhausted)?;
    membership.lifecycle = MembershipLifecycle::Retired;
    membership.registry_revision = registry_revision;
    membership.last_receipt = receipt.clone();
    Ok(())
}

fn apply_cutover(
    cutover: &NamespaceCutover,
    registry_revision: u64,
    limits: RegistryLimits,
    sources: &BTreeMap<SourceIdentity, RegisteredSource>,
    memberships: &mut BTreeMap<MembershipKey, MembershipRecord>,
    active_generations: &mut BTreeMap<OpaqueId, NonZeroRevision>,
) -> Result<(), RegistryError> {
    if !cutover.authorization_verified || !cutover.readback_verified {
        return Err(RegistryError::CutoverEvidenceMissing);
    }
    if cutover.frozen_inventory.is_empty()
        || cutover.frozen_inventory.len() > limits.max_cutover_inventory
    {
        return Err(RegistryError::CutoverInventoryInvalid);
    }
    let inventory = cutover
        .frozen_inventory
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if inventory.len() != cutover.frozen_inventory.len() {
        return Err(RegistryError::CutoverInventoryInvalid);
    }
    if active_generations.get(&cutover.corpus_id).copied() != cutover.expected_generation {
        return Err(RegistryError::CutoverGenerationConflict);
    }
    if let Some(current) = cutover.expected_generation {
        if cutover.next_generation <= current {
            return Err(RegistryError::CutoverGenerationConflict);
        }
    }
    for identity in &inventory {
        let source = sources
            .get(identity)
            .ok_or(RegistryError::CutoverSourceUnavailable)?;
        if source.lifecycle != SourceLifecycle::Active {
            return Err(RegistryError::CutoverSourceUnavailable);
        }
    }

    for membership in memberships.values_mut() {
        if membership.key.corpus_id != cutover.corpus_id {
            continue;
        }
        membership.membership_revision = membership
            .membership_revision
            .checked_next()
            .map_err(|_| RegistryError::ContractExhausted)?;
        membership.lifecycle = MembershipLifecycle::Retired;
        membership.registry_revision = registry_revision;
        membership.last_receipt = cutover.receipt.clone();
    }

    for identity in inventory {
        let key = MembershipKey {
            corpus_id: cutover.corpus_id.clone(),
            source_identity: identity,
        };
        match memberships.get_mut(&key) {
            Some(membership) => {
                membership.generation = cutover.next_generation;
                membership.lifecycle = MembershipLifecycle::Active;
                membership.registry_revision = registry_revision;
                membership.last_receipt = cutover.receipt.clone();
            }
            None => {
                if memberships.len() >= limits.max_memberships {
                    return Err(RegistryError::CapacityExceeded);
                }
                memberships.insert(
                    key.clone(),
                    MembershipRecord {
                        key,
                        generation: cutover.next_generation,
                        membership_revision: NonZeroRevision::new(1)
                            .map_err(|_| RegistryError::ContractExhausted)?,
                        lifecycle: MembershipLifecycle::Active,
                        registry_revision,
                        last_receipt: cutover.receipt.clone(),
                    },
                );
            }
        }
    }
    active_generations.insert(cutover.corpus_id.clone(), cutover.next_generation);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::{
        OpaqueCanonicalBytes, OwnerEpoch, SourceId, SourceIdentityKind, SourceNamespaceId,
    };
    use search_ports::{IdempotencyClass, MutationIdentity};
    use search_source_admission::{
        AdmissionOperation, AdmissionProfile, ResidencyClass, SourceModality,
    };
    use search_source_identity::{CanonicalRelativePath, RootBindingId, SourceObservation};

    fn source_identity(name: &str) -> SourceIdentity {
        let mut source_id = [0_u8; 16];
        for (index, byte) in name.bytes().enumerate() {
            let slot = index % source_id.len();
            source_id[slot] ^= byte;
        }
        SourceIdentity {
            source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
            source_id: SourceId::from_bytes(source_id),
            identity_kind: SourceIdentityKind::NtfsFile,
            stable_identity_components: OpaqueCanonicalBytes::from_validated(
                format!("registry-test:{name}").into_bytes(),
            )
            .expect("stable identity components"),
        }
    }

    fn source_binding(name: &str) -> SourceBinding {
        SourceBinding::new(
            source_identity(name),
            SourceObservation {
                root_binding_id: RootBindingId::from_bytes([1; 16]),
                relative_path: CanonicalRelativePath::new(
                    format!("{name}.rs"),
                    search_source_identity::DEFAULT_IDENTITY_LIMITS,
                )
                .expect("path"),
                stable_file_identity_digest: Some(Blake3Digest32::from_bytes([2; 32])),
                content_digest: Blake3Digest32::from_bytes([3; 32]),
                content_bytes: 10,
                observation_receipt: ReceiptRef::new(format!("receipt:observation:{name}"))
                    .expect("receipt"),
            },
            NonZeroRevision::new(1).expect("revision"),
            NonZeroRevision::new(1).expect("revision"),
        )
    }

    fn admission(name: &str) -> AdmissionGrant {
        AdmissionGrant {
            candidate_id: OpaqueId::new(format!("candidate:{name}")).expect("candidate"),
            profile: AdmissionProfile::Direct,
            modality: SourceModality::RegularFile,
            residency: ResidencyClass::LocalFixed,
            policy_revision: NonZeroRevision::new(1).expect("revision"),
            owner_epoch: OwnerEpoch::new(1).expect("epoch"),
            security_barrier_revision: NonZeroRevision::new(1).expect("revision"),
            evidence_digest: Blake3Digest32::from_bytes([4; 32]),
            operation: AdmissionOperation::new(
                MutationIdentity::new(
                    OpaqueId::new(format!("admission-operation:{name}")).expect("operation"),
                    IdempotencyClass::RetrySameIdentity,
                ),
                Blake3Digest32::from_bytes([5; 32]),
            ),
        }
    }

    fn assignment(name: &str) -> AdmissionBindingProof {
        AdmissionBindingProof {
            candidate_id: OpaqueId::new(format!("candidate:{name}")).expect("candidate"),
            source_identity: source_identity(name),
            assignment_digest: Blake3Digest32::from_bytes([6; 32]),
            assignment_receipt: ReceiptRef::new(format!("receipt:assignment:{name}"))
                .expect("receipt"),
            readback_verified: true,
        }
    }

    fn operation(name: &str, digest: u8) -> RegistryOperation {
        RegistryOperation::new(
            OpaqueId::new(format!("registry-operation:{name}")).expect("operation"),
            Blake3Digest32::from_bytes([digest; 32]),
        )
    }

    fn register_change(name: &str) -> RegistryChange {
        RegistryChange::RegisterSource {
            admission: admission(name),
            binding: source_binding(name),
            assignment: assignment(name),
            receipt: ReceiptRef::new(format!("receipt:register:{name}")).expect("receipt"),
        }
    }

    fn registry_with_source(name: &str) -> SourceRegistry {
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
        registry
            .apply(RegistryBatch {
                expected_registry_revision: 0,
                operation: operation("register", 1),
                changes: vec![register_change(name)],
            })
            .expect("register");
        registry
    }

    #[test]
    fn registration_is_exact_revision_guarded() {
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
        assert_eq!(
            registry.apply(RegistryBatch {
                expected_registry_revision: 1,
                operation: operation("register", 1),
                changes: vec![register_change("one")],
            }),
            Err(RegistryError::RegistryRevisionConflict)
        );
        assert_eq!(registry.revision(), 0);
    }

    #[test]
    fn full_payload_operation_replay_is_exact() {
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
        let batch = RegistryBatch {
            expected_registry_revision: 0,
            operation: operation("register", 1),
            changes: vec![register_change("one")],
        };
        let first = registry.apply(batch.clone()).expect("first");
        let replay = registry.apply(batch).expect("replay");
        assert_eq!(first.after_revision, replay.after_revision);
        assert!(replay.replayed);
        assert_eq!(registry.revision(), 1);
    }

    #[test]
    fn operation_identity_reuse_with_other_payload_is_rejected() {
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
        registry
            .apply(RegistryBatch {
                expected_registry_revision: 0,
                operation: operation("same", 1),
                changes: vec![register_change("one")],
            })
            .expect("first");
        assert_eq!(
            registry.apply(RegistryBatch {
                expected_registry_revision: 1,
                operation: operation("same", 2),
                changes: vec![register_change("two")],
            }),
            Err(RegistryError::OperationConflict)
        );
    }

    #[test]
    fn candidate_assignment_mismatch_is_atomic_failure() {
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
        let mut proof = assignment("one");
        proof.candidate_id = OpaqueId::new("candidate:other").expect("candidate");
        assert_eq!(
            registry.apply(RegistryBatch {
                expected_registry_revision: 0,
                operation: operation("register", 1),
                changes: vec![RegistryChange::RegisterSource {
                    admission: admission("one"),
                    binding: source_binding("one"),
                    assignment: proof,
                    receipt: ReceiptRef::new("receipt:register").expect("receipt"),
                }],
            }),
            Err(RegistryError::AdmissionBindingMismatch)
        );
        assert_eq!(registry.revision(), 0);
    }

    #[test]
    fn duplicate_source_corpus_membership_is_rejected() {
        let mut registry = registry_with_source("one");
        let membership = NewMembership {
            key: MembershipKey {
                corpus_id: OpaqueId::new("corpus:test").expect("corpus"),
                source_identity: source_identity("one"),
            },
            generation: NonZeroRevision::new(1).expect("generation"),
            receipt: ReceiptRef::new("receipt:membership").expect("receipt"),
        };
        registry
            .apply(RegistryBatch {
                expected_registry_revision: 1,
                operation: operation("membership-one", 2),
                changes: vec![RegistryChange::AddMembership(membership.clone())],
            })
            .expect("membership");
        assert_eq!(
            registry.apply(RegistryBatch {
                expected_registry_revision: 2,
                operation: operation("membership-two", 3),
                changes: vec![RegistryChange::AddMembership(membership)],
            }),
            Err(RegistryError::MembershipCollision)
        );
    }

    #[test]
    fn retiring_source_removes_it_from_active_portfolio() {
        let mut registry = registry_with_source("one");
        let corpus = OpaqueId::new("corpus:test").expect("corpus");
        registry
            .apply(RegistryBatch {
                expected_registry_revision: 1,
                operation: operation("membership", 2),
                changes: vec![RegistryChange::AddMembership(NewMembership {
                    key: MembershipKey {
                        corpus_id: corpus.clone(),
                        source_identity: source_identity("one"),
                    },
                    generation: NonZeroRevision::new(1).expect("generation"),
                    receipt: ReceiptRef::new("receipt:membership").expect("receipt"),
                })],
            })
            .expect("membership");
        registry
            .apply(RegistryBatch {
                expected_registry_revision: 2,
                operation: operation("retire", 3),
                changes: vec![RegistryChange::RetireSource {
                    identity: source_identity("one"),
                    expected_source_revision: NonZeroRevision::new(1).expect("revision"),
                    receipt: ReceiptRef::new("receipt:retire").expect("receipt"),
                }],
            })
            .expect("retire");
        let portfolio = registry
            .active_portfolio(&corpus, NonZeroRevision::new(1).expect("generation"), 10)
            .expect("portfolio");
        assert!(portfolio.is_empty());
    }

    #[test]
    fn namespace_cutover_is_atomic_and_inventory_bound() {
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
        registry
            .apply(RegistryBatch {
                expected_registry_revision: 0,
                operation: operation("register", 1),
                changes: vec![register_change("one"), register_change("two")],
            })
            .expect("register");
        let corpus = OpaqueId::new("corpus:test").expect("corpus");
        registry
            .apply(RegistryBatch {
                expected_registry_revision: 1,
                operation: operation("cutover", 2),
                changes: vec![RegistryChange::CutoverNamespace(NamespaceCutover {
                    corpus_id: corpus.clone(),
                    expected_generation: None,
                    next_generation: NonZeroRevision::new(1).expect("generation"),
                    frozen_inventory: vec![source_identity("one"), source_identity("two")],
                    inventory_digest: Blake3Digest32::from_bytes([7; 32]),
                    authorization_verified: true,
                    readback_verified: true,
                    receipt: ReceiptRef::new("receipt:cutover").expect("receipt"),
                })],
            })
            .expect("cutover");
        let portfolio = registry
            .active_portfolio(&corpus, NonZeroRevision::new(1).expect("generation"), 10)
            .expect("portfolio");
        assert_eq!(portfolio.len(), 2);
    }

    #[test]
    fn failed_multi_change_batch_does_not_partially_register() {
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
        let mut invalid = assignment("two");
        invalid.readback_verified = false;
        assert_eq!(
            registry.apply(RegistryBatch {
                expected_registry_revision: 0,
                operation: operation("batch", 9),
                changes: vec![
                    register_change("one"),
                    RegistryChange::RegisterSource {
                        admission: admission("two"),
                        binding: source_binding("two"),
                        assignment: invalid,
                        receipt: ReceiptRef::new("receipt:two").expect("receipt"),
                    },
                ],
            }),
            Err(RegistryError::AdmissionBindingEvidenceMissing)
        );
        assert_eq!(registry.revision(), 0);
        assert_eq!(
            registry.source(&source_identity("one")),
            Err(RegistryError::SourceNotFound)
        );
    }
}
