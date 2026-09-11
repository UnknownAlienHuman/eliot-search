//! Ordinary CAS retention sweep decisions.
//!
//! This module decides what Search-owned CAS objects may be collected. It
//! never deletes anything itself: a vendor-neutral [`CasAdmin`] port executes
//! exact-ID deletion and proves absence through readback. Purge, restore and
//! physical secure erasure are out of scope; receipts explicitly state
//! Search-owned CAS/cache deletion only.
//!
//! Decision inputs are one coherent control generation plus explicit retention
//! roots and a fresh pin-protection snapshot. Reference counts may be
//! observed by callers but never authorize deletion here.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, ObjectResidencyKeyDigest, OpaqueId};

use crate::{RetentionError, RetentionOperation};

/// Closed durable retention-root kind (architecture baseline, 11 kinds).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RetentionRootKind {
    ActiveProjectionManifest,
    PublicationIntent,
    CompensationIntent,
    RetainedSourceRevisionLease,
    DurableSourceHandle,
    ClientPinImportExportContract,
    PairedRecoveryManifest,
    RetentionPolicy,
    LegalHold,
    PurgeTombstone,
    RestoreQuarantineManifest,
}

impl RetentionRootKind {
    /// All closed root kinds in canonical order.
    pub const ALL: &'static [Self] = &[
        Self::ActiveProjectionManifest,
        Self::PublicationIntent,
        Self::CompensationIntent,
        Self::RetainedSourceRevisionLease,
        Self::DurableSourceHandle,
        Self::ClientPinImportExportContract,
        Self::PairedRecoveryManifest,
        Self::RetentionPolicy,
        Self::LegalHold,
        Self::PurgeTombstone,
        Self::RestoreQuarantineManifest,
    ];

    /// Stable wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActiveProjectionManifest => "active_projection_manifest",
            Self::PublicationIntent => "publication_intent",
            Self::CompensationIntent => "compensation_intent",
            Self::RetainedSourceRevisionLease => "retained_source_revision_lease",
            Self::DurableSourceHandle => "durable_source_handle",
            Self::ClientPinImportExportContract => "client_pin_import_export_contract",
            Self::PairedRecoveryManifest => "paired_recovery_manifest",
            Self::RetentionPolicy => "retention_policy",
            Self::LegalHold => "legal_hold",
            Self::PurgeTombstone => "purge_tombstone",
            Self::RestoreQuarantineManifest => "restore_quarantine_manifest",
        }
    }

    /// Parses a closed root kind; anything else is rejected.
    pub fn parse(value: &str) -> Result<Self, RetentionError> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == value)
            .ok_or(RetentionError::RootIncomplete)
    }
}

/// One exact durable retention root bound to a control generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableRoot {
    /// Stable Search-owned object identity.
    pub object_id: OpaqueId,
    /// Closed root kind.
    pub kind: RetentionRootKind,
    /// Residency domain digest.
    pub residency_digest: ObjectResidencyKeyDigest,
    /// Coherent control generation that admitted this root.
    pub control_generation: u64,
}

/// Finite sweep budgets. Zero never means unlimited.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SweepLimits {
    /// Maximum durable roots in one protection set.
    pub max_roots: usize,
    /// Maximum objects across mark inventory and plan.
    pub max_objects: usize,
    /// Maximum outgoing edges per object in the mark graph.
    pub max_edges_per_object: usize,
    /// Maximum deletion batches in one plan.
    pub max_batches: usize,
    /// Maximum object IDs per batch.
    pub max_batch_objects: usize,
}

impl SweepLimits {
    /// Conservative local baseline.
    pub const BASELINE: Self = Self {
        max_roots: 1_024,
        max_objects: 100_000,
        max_edges_per_object: 64,
        max_batches: 10_000,
        max_batch_objects: 1_000,
    };

    /// Validates finite non-zero limits.
    pub const fn validate(self) -> Result<Self, RetentionError> {
        if self.max_roots == 0
            || self.max_objects == 0
            || self.max_edges_per_object == 0
            || self.max_batches == 0
            || self.max_batch_objects == 0
        {
            Err(RetentionError::InvalidPolicy)
        } else {
            Ok(self)
        }
    }
}

/// Fresh active-pin protection snapshot captured for one sweep.
///
/// Query, continuation, route and epoch pins are transient; the caller
/// captures them atomically with the control generation. A stale or
/// mismatched snapshot fails closed and never authorizes deletion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinEvidence {
    /// Exact pinned Search-owned object IDs.
    pub pinned_ids: BTreeSet<OpaqueId>,
    /// Pin-snapshot generation (monotone per capture).
    pub pin_generation: u64,
    /// Caller-supplied capture time (opaque wall clock, not a decision TTL).
    pub capture_time_ms: u64,
    /// False means the snapshot is stale, mismatched or unknown.
    pub fresh: bool,
    /// Control generation the pins were captured against.
    pub control_generation: u64,
    /// Publication generation the pins were captured against.
    pub publication_generation: u64,
}

/// Frozen protection set: roots plus pinned, leased and tombstoned objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectionSet {
    /// Canonically ordered durable roots.
    pub roots: Vec<DurableRoot>,
    /// Exact pinned object IDs.
    pub pinned_ids: BTreeSet<OpaqueId>,
    /// Exact tombstoned/audit-retained IDs (kept as non-content records).
    pub tombstoned_ids: BTreeSet<OpaqueId>,
    /// Exact actively leased IDs resolved from the lease catalog.
    pub leased_ids: BTreeSet<OpaqueId>,
    /// Coherent control generation.
    pub control_generation: u64,
    /// Pin-snapshot generation.
    pub pin_generation: u64,
    /// Publication generation.
    pub publication_generation: u64,
    /// Binding digest over the exact frozen set.
    pub protection_digest: Blake3Digest32,
}

/// Durably recorded sweep intent binding operation to protection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SweepIntent {
    /// Stable operation identity with its canonical request digest.
    pub operation: RetentionOperation,
    /// Bound control generation.
    pub control_generation: u64,
    /// Bound pin generation.
    pub pin_generation: u64,
    /// Bound publication generation.
    pub publication_generation: u64,
    /// Bound protection-set digest.
    pub protection_digest: Blake3Digest32,
}

/// Complete mark manifest: exact reachable IDs under one intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkManifest {
    /// Intent operation identity.
    pub operation_id: OpaqueId,
    /// Exact reachable Search-owned object IDs.
    pub reachable: BTreeSet<OpaqueId>,
    /// Binding digest over intent, reachable set and generations.
    pub mark_digest: Blake3Digest32,
    /// Bound control generation.
    pub control_generation: u64,
    /// Bound pin generation.
    pub pin_generation: u64,
    /// Bound publication generation.
    pub publication_generation: u64,
    /// Always true; a partial mark never authorizes a plan.
    pub complete: bool,
}

/// One exact finite deletion batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SweepBatch {
    /// Zero-based deterministic batch index.
    pub batch_index: usize,
    /// Immutable operation identity for this exact object set.
    pub operation_id: OpaqueId,
    /// Canonically ordered exact object IDs.
    pub object_ids: Vec<OpaqueId>,
}

/// Complete exact sweep plan over unreachable, unprotected objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SweepPlan {
    /// Intent operation identity.
    pub operation_id: OpaqueId,
    /// Bound mark digest.
    pub mark_digest: Blake3Digest32,
    /// Bound object-inventory generation.
    pub inventory_generation: u64,
    /// Canonically ordered candidates (unreachable and unprotected).
    pub candidates: Vec<OpaqueId>,
    /// Deterministic batches partitioning the candidates.
    pub batches: Vec<SweepBatch>,
    /// Bound control generation.
    pub control_generation: u64,
    /// Bound pin generation.
    pub pin_generation: u64,
    /// Bound publication generation.
    pub publication_generation: u64,
}

/// Immutable exact mutation identity for one batch execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CasMutation {
    /// Caller-assigned operation identity; must equal the batch identity.
    pub operation_id: OpaqueId,
    /// Digest of the exact canonical execution input.
    pub input_digest: [u8; 32],
}

/// Exact acknowledgement returned by the CAS-admin delete path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CasDeleteAck {
    /// Echo of the executed operation identity.
    pub operation_id: OpaqueId,
    /// Identifiers the admin reports deleted.
    pub deleted_ids: Vec<OpaqueId>,
    /// Whether the receipt came from idempotent replay.
    pub replayed: bool,
}

/// Exact readback over the batch identifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CasReadback {
    /// Requested identifiers with no stored object.
    pub missing_ids: Vec<OpaqueId>,
    /// Identifiers returned for unrequested objects.
    pub unexpected_ids: Vec<OpaqueId>,
}

/// Closed CAS-admin failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CasAdminError {
    /// The delete may have committed; only an exact readback resolves it.
    Unknown,
    /// The same identity was reused with different identifiers.
    Conflict,
    /// The admin deterministically contradicts the planned expectation.
    Mismatch,
    /// The transport failed without a usable acknowledgement.
    Transport,
}

/// Vendor-neutral exact-ID CAS administration.
///
/// The implementor deletes only the batch's exact identifiers and reads back
/// only requested identifiers. No broad-prefix operation exists on this port.
pub trait CasAdmin {
    /// Deletes only the batch's exact identifiers.
    fn delete_exact(
        &mut self,
        batch: &SweepBatch,
        mutation: &CasMutation,
    ) -> Result<CasDeleteAck, CasAdminError>;

    /// Reads back exactly the requested identifiers.
    fn readback_exact(&self, ids: &[OpaqueId]) -> Result<CasReadback, CasAdminError>;
}

/// Exact batch receipt proving absence after deletion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SweepBatchReceipt {
    /// Intent operation identity.
    pub operation_id: OpaqueId,
    /// Executed batch index.
    pub batch_index: usize,
    /// Batch operation identity.
    pub batch_operation_id: OpaqueId,
    /// Identifiers proven absent.
    pub missing_ids: Vec<OpaqueId>,
    /// Unexpected identifiers observed (always empty on success).
    pub unexpected_ids: Vec<OpaqueId>,
}

/// Final sweep receipt stating Search-owned CAS deletion only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SweepReceipt {
    /// Intent operation identity.
    pub operation_id: OpaqueId,
    /// Exact deleted object IDs in canonical order.
    pub deleted: Vec<OpaqueId>,
    /// Number of executed batches.
    pub batch_count: usize,
    /// Always false: logical CAS removal is never physical secure erasure.
    pub secure_erase_claimed: bool,
}

/// Freezes one coherent protection set.
///
/// Duplicate roots collapse only by exact identity; unreadable or drifted
/// generations fail closed with no complete set.
pub fn collect_protection(
    roots: Vec<DurableRoot>,
    pins: &PinEvidence,
    tombstoned_ids: &BTreeSet<OpaqueId>,
    leased_ids: &BTreeSet<OpaqueId>,
    limits: SweepLimits,
) -> Result<ProtectionSet, RetentionError> {
    let limits = limits.validate()?;
    if roots.is_empty() || roots.len() > limits.max_roots {
        return Err(RetentionError::RootIncomplete);
    }
    if !pins.fresh {
        return Err(RetentionError::PinProtectionUnknown);
    }
    if pins.pinned_ids.len() > limits.max_objects
        || tombstoned_ids.len() > limits.max_objects
        || leased_ids.len() > limits.max_objects
    {
        return Err(RetentionError::CapacityExceeded);
    }
    let control_generation = roots[0].control_generation;
    if control_generation == 0 {
        return Err(RetentionError::RootIncomplete);
    }
    for root in &roots {
        if root.control_generation != control_generation {
            return Err(RetentionError::RootGenerationChanged);
        }
    }
    if pins.control_generation != control_generation {
        return Err(RetentionError::RootGenerationChanged);
    }
    // Canonical order: strictly increasing object IDs, no duplicate identity.
    for pair in roots.windows(2) {
        if pair[0].object_id >= pair[1].object_id {
            return Err(RetentionError::RootIncomplete);
        }
    }
    let protection_digest = derive_protection_digest(
        &roots,
        &pins.pinned_ids,
        tombstoned_ids,
        leased_ids,
        control_generation,
        pins.pin_generation,
        pins.publication_generation,
    );
    Ok(ProtectionSet {
        roots,
        pinned_ids: pins.pinned_ids.clone(),
        tombstoned_ids: tombstoned_ids.clone(),
        leased_ids: leased_ids.clone(),
        control_generation,
        pin_generation: pins.pin_generation,
        publication_generation: pins.publication_generation,
        protection_digest,
    })
}

/// Records the sweep intent binding operation to one frozen protection set.
///
/// The same operation identity with equal input reconstructs the intent.
#[allow(clippy::missing_const_for_fn)]
pub fn begin_sweep(
    operation: RetentionOperation,
    protection: &ProtectionSet,
) -> Result<SweepIntent, RetentionError> {
    Ok(SweepIntent {
        operation,
        control_generation: protection.control_generation,
        pin_generation: protection.pin_generation,
        publication_generation: protection.publication_generation,
        protection_digest: protection.protection_digest,
    })
}

/// Marks all reachable Search-owned objects.
///
/// Traversal starts from roots, pins and leases in canonical order. Missing,
/// corrupt or out-of-inventory edges, budget exhaustion and generation drift
/// prevent a complete mark.
pub fn mark_reachable(
    intent: &SweepIntent,
    protection: &ProtectionSet,
    graph: &BTreeMap<OpaqueId, Vec<OpaqueId>>,
    inventory: &BTreeSet<OpaqueId>,
    limits: SweepLimits,
) -> Result<MarkManifest, RetentionError> {
    let limits = limits.validate()?;
    if intent.protection_digest != protection.protection_digest
        || intent.control_generation != protection.control_generation
        || intent.pin_generation != protection.pin_generation
        || intent.publication_generation != protection.publication_generation
    {
        return Err(RetentionError::SweepGenerationMismatch);
    }
    if inventory.len() > limits.max_objects || graph.len() > limits.max_objects {
        return Err(RetentionError::MarkIncomplete);
    }
    for (node, edges) in graph {
        if !inventory.contains(node) {
            return Err(RetentionError::MarkIncomplete);
        }
        if edges.len() > limits.max_edges_per_object {
            return Err(RetentionError::MarkIncomplete);
        }
        for target in edges {
            if !inventory.contains(target) {
                return Err(RetentionError::MarkIncomplete);
            }
        }
    }
    // Seeds are the durable roots plus pinned and leased objects. Tombstoned
    // IDs alone seed nothing: they protect from deletion but imply no
    // reachability.
    let mut seeds = BTreeSet::new();
    for root in &protection.roots {
        if !inventory.contains(&root.object_id) {
            return Err(RetentionError::MarkIncomplete);
        }
        seeds.insert(root.object_id.clone());
    }
    for id in protection
        .pinned_ids
        .iter()
        .chain(protection.leased_ids.iter())
    {
        if !inventory.contains(id) {
            return Err(RetentionError::MarkIncomplete);
        }
        seeds.insert(id.clone());
    }
    // Bounded canonical traversal: always expand the smallest unvisited ID
    // with its edges in sorted order.
    let mut reachable = BTreeSet::new();
    let mut frontier = seeds;
    while let Some(next) = frontier.iter().next().cloned() {
        frontier.remove(&next);
        if !reachable.insert(next.clone()) {
            continue;
        }
        if reachable.len() > limits.max_objects {
            return Err(RetentionError::MarkIncomplete);
        }
        if let Some(edges) = graph.get(&next) {
            let mut sorted = edges.clone();
            sorted.sort();
            sorted.dedup();
            for target in sorted {
                if !reachable.contains(&target) {
                    frontier.insert(target);
                }
            }
        }
    }
    let mark_digest = derive_mark_digest(
        &intent.operation.operation_id,
        &reachable,
        intent.control_generation,
        intent.pin_generation,
        intent.publication_generation,
    );
    Ok(MarkManifest {
        operation_id: intent.operation.operation_id.clone(),
        reachable,
        mark_digest,
        control_generation: intent.control_generation,
        pin_generation: intent.pin_generation,
        publication_generation: intent.publication_generation,
        complete: true,
    })
}

/// Plans exact deletion of unmarked, unprotected objects.
///
/// Tombstoned and leased objects remain as explicit non-content records and
/// are never candidates. Reference counts are not consulted: only the mark
/// plus explicit protection authorize deletion.
pub fn plan_sweep(
    intent: &SweepIntent,
    mark: &MarkManifest,
    inventory: &[OpaqueId],
    inventory_generation: u64,
    protection: &ProtectionSet,
    limits: SweepLimits,
) -> Result<SweepPlan, RetentionError> {
    let limits = limits.validate()?;
    if !mark.complete {
        return Err(RetentionError::MarkIncomplete);
    }
    if mark.operation_id != intent.operation.operation_id {
        return Err(RetentionError::SweepGenerationMismatch);
    }
    if mark.control_generation != intent.control_generation
        || mark.pin_generation != intent.pin_generation
        || mark.publication_generation != intent.publication_generation
    {
        return Err(RetentionError::RootGenerationChanged);
    }
    if mark.mark_digest
        != derive_mark_digest(
            &intent.operation.operation_id,
            &mark.reachable,
            mark.control_generation,
            mark.pin_generation,
            mark.publication_generation,
        )
    {
        return Err(RetentionError::MarkManifestInvalid);
    }
    if inventory_generation == 0 {
        return Err(RetentionError::SweepPlanInvalid);
    }
    if inventory.len() > limits.max_objects {
        return Err(RetentionError::SweepPlanInvalid);
    }
    if inventory.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RetentionError::SweepPlanInvalid);
    }
    let inventory_set: BTreeSet<OpaqueId> = inventory.iter().cloned().collect();
    if inventory_set.len() != inventory.len() {
        return Err(RetentionError::SweepPlanInvalid);
    }
    for id in &mark.reachable {
        if !inventory_set.contains(id) {
            return Err(RetentionError::MarkManifestInvalid);
        }
    }
    let mut candidates = Vec::new();
    for id in inventory {
        if mark.reachable.contains(id)
            || protection.pinned_ids.contains(id)
            || protection.leased_ids.contains(id)
            || protection.tombstoned_ids.contains(id)
        {
            continue;
        }
        candidates.push(id.clone());
    }
    if candidates.len() > limits.max_objects {
        return Err(RetentionError::SweepPlanInvalid);
    }
    if limits.max_batch_objects == 0 {
        return Err(RetentionError::InvalidPolicy);
    }
    let batch_count = candidates.len().div_ceil(limits.max_batch_objects);
    if batch_count > limits.max_batches {
        return Err(RetentionError::SweepPlanInvalid);
    }
    let prefix = hex_prefix(mark.mark_digest.as_bytes());
    let mut batches = Vec::with_capacity(batch_count);
    for (batch_index, chunk) in candidates.chunks(limits.max_batch_objects).enumerate() {
        let operation_id = OpaqueId::new(format!("sweep:{prefix}:{batch_index}"))
            .map_err(|_| RetentionError::ContractExhausted)?;
        batches.push(SweepBatch {
            batch_index,
            operation_id,
            object_ids: chunk.to_vec(),
        });
    }
    Ok(SweepPlan {
        operation_id: intent.operation.operation_id.clone(),
        mark_digest: mark.mark_digest,
        inventory_generation,
        candidates,
        batches,
        control_generation: intent.control_generation,
        pin_generation: intent.pin_generation,
        publication_generation: intent.publication_generation,
    })
}

/// Executes one exact batch.
///
/// Revalidates generations and fresh protection, deletes exact IDs only,
/// then proves absence through readback.
///
/// A new root, hold, pin or publication generation invalidates the sweep
/// instead of being ignored. Timeout or disconnect after dispatch becomes
/// [`RetentionError::SweepDeleteOutcomeUnknown`] until exact readback
/// resolves it. This receipt never claims purge, backup deletion or physical
/// secure erasure.
pub fn execute_sweep_batch(
    plan: &SweepPlan,
    batch_index: usize,
    admin: &mut impl CasAdmin,
    mutation: &CasMutation,
    current: &ProtectionSet,
) -> Result<SweepBatchReceipt, RetentionError> {
    let batch = plan
        .batches
        .get(batch_index)
        .ok_or(RetentionError::SweepGenerationMismatch)?;
    if mutation.operation_id != batch.operation_id {
        return Err(RetentionError::SweepGenerationMismatch);
    }
    if current.control_generation != plan.control_generation
        || current.publication_generation != plan.publication_generation
    {
        return Err(RetentionError::RootGenerationChanged);
    }
    if current.pin_generation != plan.pin_generation {
        return Err(RetentionError::RootGenerationChanged);
    }
    for id in &batch.object_ids {
        if current.pinned_ids.contains(id)
            || current.leased_ids.contains(id)
            || current.tombstoned_ids.contains(id)
        {
            return Err(RetentionError::SweepProtectedObjectConflict);
        }
    }
    let delete_acked = match admin.delete_exact(batch, mutation) {
        Ok(ack) => {
            if ack.operation_id != batch.operation_id {
                return Err(RetentionError::SweepProtectedObjectConflict);
            }
            if ack.deleted_ids != batch.object_ids {
                return Err(RetentionError::SweepProtectedObjectConflict);
            }
            true
        }
        Err(error) => match error {
            CasAdminError::Unknown | CasAdminError::Transport => false,
            CasAdminError::Conflict => return Err(RetentionError::SweepGenerationMismatch),
            CasAdminError::Mismatch => return Err(RetentionError::SweepProtectedObjectConflict),
        },
    };
    let readback = admin
        .readback_exact(&batch.object_ids)
        .map_err(|_| RetentionError::SweepDeleteOutcomeUnknown)?;
    if !readback.unexpected_ids.is_empty() {
        return Err(RetentionError::SweepProtectedObjectConflict);
    }
    if readback.missing_ids == batch.object_ids {
        return Ok(SweepBatchReceipt {
            operation_id: plan.operation_id.clone(),
            batch_index: batch.batch_index,
            batch_operation_id: batch.operation_id.clone(),
            missing_ids: readback.missing_ids,
            unexpected_ids: Vec::new(),
        });
    }
    if delete_acked {
        return Err(RetentionError::SweepDeletePartial);
    }
    Err(RetentionError::SweepDeleteOutcomeUnknown)
}

/// Completes a sweep.
///
/// Succeeds only when every planned object is exactly accounted and no
/// protected object was deleted. The receipt states Search-owned CAS
/// deletion only.
pub fn complete_sweep(
    intent: &SweepIntent,
    plan: &SweepPlan,
    receipts: &[SweepBatchReceipt],
    final_inventory: &BTreeSet<OpaqueId>,
) -> Result<SweepReceipt, RetentionError> {
    if intent.operation.operation_id != plan.operation_id {
        return Err(RetentionError::SweepGenerationMismatch);
    }
    if receipts.len() != plan.batches.len() {
        return Err(RetentionError::SweepDeletePartial);
    }
    let mut deleted = Vec::new();
    for (receipt, batch) in receipts.iter().zip(plan.batches.iter()) {
        if receipt.operation_id != plan.operation_id {
            return Err(RetentionError::SweepDeletePartial);
        }
        if receipt.batch_index != batch.batch_index {
            return Err(RetentionError::SweepDeletePartial);
        }
        if receipt.batch_operation_id != batch.operation_id {
            return Err(RetentionError::SweepDeletePartial);
        }
        if receipt.missing_ids != batch.object_ids {
            return Err(RetentionError::SweepDeletePartial);
        }
        if !receipt.unexpected_ids.is_empty() {
            return Err(RetentionError::SweepDeletePartial);
        }
        deleted.extend(receipt.missing_ids.iter().cloned());
    }
    deleted.sort();
    deleted.dedup();
    let mut expected = plan.candidates.clone();
    expected.sort();
    if deleted != expected {
        return Err(RetentionError::SweepDeletePartial);
    }
    for id in &deleted {
        if final_inventory.contains(id) {
            return Err(RetentionError::SweepDeletePartial);
        }
    }
    Ok(SweepReceipt {
        operation_id: plan.operation_id.clone(),
        deleted,
        batch_count: plan.batches.len(),
        secure_erase_claimed: false,
    })
}

/// Returns the remaining exact batches.
///
/// Generation drift invalidates the remainder instead of being ignored.
pub fn remaining_batches(
    plan: &SweepPlan,
    completed: &[SweepBatchReceipt],
    current: &ProtectionSet,
) -> Result<Vec<SweepBatch>, RetentionError> {
    if current.control_generation != plan.control_generation {
        return Err(RetentionError::RootGenerationChanged);
    }
    if current.publication_generation != plan.publication_generation {
        return Err(RetentionError::RootGenerationChanged);
    }
    if current.pin_generation != plan.pin_generation {
        return Err(RetentionError::RootGenerationChanged);
    }
    let done: BTreeSet<usize> = completed.iter().map(|r| r.batch_index).collect();
    if done.len() != completed.len() {
        return Err(RetentionError::SweepDeletePartial);
    }
    let mut out = Vec::new();
    for batch in &plan.batches {
        if done.contains(&batch.batch_index) {
            continue;
        }
        for id in &batch.object_ids {
            if current.pinned_ids.contains(id)
                || current.leased_ids.contains(id)
                || current.tombstoned_ids.contains(id)
            {
                return Err(RetentionError::SweepProtectedObjectConflict);
            }
        }
        out.push(batch.clone());
    }
    Ok(out)
}

fn derive_protection_digest(
    roots: &[DurableRoot],
    pinned: &BTreeSet<OpaqueId>,
    tombstoned: &BTreeSet<OpaqueId>,
    leased: &BTreeSet<OpaqueId>,
    control_generation: u64,
    pin_generation: u64,
    publication_generation: u64,
) -> Blake3Digest32 {
    let mut state = [
        0xcbf2_9ce4_8422_2325_u64,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0xc2b2_ae3d_27d4_eb4f,
    ];
    mix(&mut state, b"eliot-search/sweep-protection/v1");
    mix(&mut state, &control_generation.to_be_bytes());
    mix(&mut state, &pin_generation.to_be_bytes());
    mix(&mut state, &publication_generation.to_be_bytes());
    for root in roots {
        mix(&mut state, root.object_id.as_str().as_bytes());
        mix(&mut state, root.kind.as_str().as_bytes());
        mix(&mut state, root.residency_digest.as_bytes());
        mix(&mut state, &root.control_generation.to_be_bytes());
    }
    for id in pinned {
        mix(&mut state, b"pin:");
        mix(&mut state, id.as_str().as_bytes());
    }
    for id in tombstoned {
        mix(&mut state, b"tombstone:");
        mix(&mut state, id.as_str().as_bytes());
    }
    for id in leased {
        mix(&mut state, b"lease:");
        mix(&mut state, id.as_str().as_bytes());
    }
    fold_state(state)
}

fn derive_mark_digest(
    operation_id: &OpaqueId,
    reachable: &BTreeSet<OpaqueId>,
    control_generation: u64,
    pin_generation: u64,
    publication_generation: u64,
) -> Blake3Digest32 {
    let mut state = [
        0x243f_6a88_85a3_08d3_u64,
        0x4528_21e6_38d0_1377,
        0x1319_8a2e_0370_7344,
        0xa409_3822_299f_31d0,
    ];
    mix(&mut state, b"eliot-search/sweep-mark/v1");
    mix(&mut state, operation_id.as_str().as_bytes());
    mix(&mut state, &control_generation.to_be_bytes());
    mix(&mut state, &pin_generation.to_be_bytes());
    mix(&mut state, &publication_generation.to_be_bytes());
    for id in reachable {
        mix(&mut state, id.as_str().as_bytes());
    }
    fold_state(state)
}

fn mix(state: &mut [u64; 4], bytes: &[u8]) {
    for (index, byte) in bytes.iter().copied().enumerate() {
        let lane = index % state.len();
        state[lane] ^= u64::from(byte);
        state[lane] = state[lane]
            .wrapping_mul(0x0000_0100_0000_01b3)
            .rotate_left(u32::try_from(11 + lane * 7).unwrap_or(11));
    }
}

fn fold_state(state: [u64; 4]) -> Blake3Digest32 {
    let mut output = [0_u8; 32];
    for (index, lane) in state.into_iter().enumerate() {
        output[index * 8..index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    Blake3Digest32::from_bytes(output)
}

fn hex_prefix(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(16);
    for byte in &bytes[..8] {
        use core::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
