//! Mandatory A/B/C fault-injection and recovery matrix.

use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, EvalLimits, FingerprintBuilder, FrozenRunManifest};

use super::common::{HardBlocker, HardBlockerClass, bounded_reason};

/// Mandatory mutation/fault boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FaultPoint {
    /// Before runtime-owner acquisition mutation.
    BeforeOwnerAcquire,
    /// After OS lock but before durable owner readback.
    AfterOwnerLock,
    /// During control transaction commit.
    ControlCommit,
    /// During encrypted revision commit.
    RevisionCommit,
    /// During index stage mutation.
    IndexStage,
    /// During index close/retire mutation.
    IndexClose,
    /// During visible-epoch control CAS.
    VisibleEpochCommit,
    /// During handle/continuation mint.
    CapabilityMint,
    /// During purge live-deny commit.
    PurgeBarrier,
    /// During purge physical/object deletion.
    PurgeDelete,
    /// During graceful drain.
    Drain,
    /// During owner release.
    OwnerRelease,
}

impl FaultPoint {
    /// Baseline mandatory fault matrix.
    pub const MANDATORY: [Self; 12] = [
        Self::BeforeOwnerAcquire,
        Self::AfterOwnerLock,
        Self::ControlCommit,
        Self::RevisionCommit,
        Self::IndexStage,
        Self::IndexClose,
        Self::VisibleEpochCommit,
        Self::CapabilityMint,
        Self::PurgeBarrier,
        Self::PurgeDelete,
        Self::Drain,
        Self::OwnerRelease,
    ];
}

/// Terminal fault-cell state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FaultCellStatus {
    /// Exact recovery completed and all invariants held.
    Pass,
    /// Recovery completed with a violated invariant.
    Fail,
    /// Recovery observation remained unresolved.
    Unavailable,
}

/// One exact fault-injection/recovery cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaultCell {
    /// Fault boundary.
    pub fault_point: FaultPoint,
    /// Baseline/candidate identity.
    pub baseline_id: OpaqueId,
    /// Zero-based repetition.
    pub repetition: u32,
    /// Terminal status.
    pub status: FaultCellStatus,
    /// Authoritative-readback and replay invariants.
    pub readback: FaultReadback,
    /// Publication, identity, and barrier containment invariants.
    pub containment: FaultContainment,
    /// Immutable fault evidence.
    pub evidence_ref: ReceiptRef,
}

/// Authoritative-readback and replay invariants for one fault cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaultReadback {
    /// Whether authoritative readback resolved possible mutation.
    pub authoritative_readback: bool,
    /// Whether replay remained idempotent.
    pub idempotent_replay: bool,
}

/// Publication, identity, and barrier containment invariants for one fault cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaultContainment {
    /// Whether visible state linearized at most once.
    pub no_double_publish: bool,
    /// Whether source/root/membership identity remained exact.
    pub no_identity_widening: bool,
    /// Whether restrictive barriers survived restart.
    pub restrictive_barriers_preserved: bool,
}

/// Complete A/B/C fault-recovery matrix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaultMatrixReport {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Canonical cells.
    pub cells: Vec<FaultCell>,
    /// Hard blockers for failed/unavailable/incomplete cells.
    pub blockers: Vec<HardBlocker>,
    /// Whether every required cell passed.
    pub passed: bool,
    /// Deterministic matrix digest.
    pub matrix_digest: Blake3Digest32,
}

/// Validates every required fault point for every A/B/C identity and repetition.
pub fn audit_fault_matrix(
    run: &FrozenRunManifest,
    baseline_ids: &BTreeSet<OpaqueId>,
    repetitions: u32,
    mut cells: Vec<FaultCell>,
    limits: EvalLimits,
) -> Result<FaultMatrixReport, EvalError> {
    let limits = limits.validate()?;
    if baseline_ids.len() != 3
        || repetitions == 0
        || repetitions > limits.max_repetitions
        || cells.len() > limits.max_audit_items
    {
        return Err(EvalError::FaultMatrixIncomplete);
    }
    cells.sort_by(|left, right| {
        (left.fault_point, &left.baseline_id, left.repetition).cmp(&(
            right.fault_point,
            &right.baseline_id,
            right.repetition,
        ))
    });
    let mut observed = BTreeSet::new();
    let mut blockers = Vec::new();
    for cell in &cells {
        let key = (cell.fault_point, cell.baseline_id.clone(), cell.repetition);
        if !baseline_ids.contains(&cell.baseline_id)
            || cell.repetition >= repetitions
            || cell.evidence_ref.as_str().is_empty()
            || !observed.insert(key)
        {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        let invariant_pass = cell.status == FaultCellStatus::Pass
            && cell.readback.authoritative_readback
            && cell.readback.idempotent_replay
            && cell.containment.no_double_publish
            && cell.containment.no_identity_widening
            && cell.containment.restrictive_barriers_preserved;
        if !invariant_pass {
            blockers.push(HardBlocker {
                class: HardBlockerClass::FaultRecovery,
                check_id: bounded_reason(&format!(
                    "fault-{}-{}",
                    fault_tag(cell.fault_point),
                    cell.repetition
                ))?,
                reason: bounded_reason(match cell.status {
                    FaultCellStatus::Pass => "FAULT_INVARIANT_VIOLATION",
                    FaultCellStatus::Fail => "FAULT_RECOVERY_FAILED",
                    FaultCellStatus::Unavailable => "FAULT_RECOVERY_UNAVAILABLE",
                })?,
                evidence_ref: cell.evidence_ref.clone(),
            });
        }
    }
    for fault_point in FaultPoint::MANDATORY {
        for baseline_id in baseline_ids {
            for repetition in 0..repetitions {
                if !observed.contains(&(fault_point, baseline_id.clone(), repetition)) {
                    return Err(EvalError::FaultMatrixIncomplete);
                }
            }
        }
    }
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/fault-matrix/v1");
    fingerprint.push_digest(run.run_digest());
    for cell in &cells {
        fingerprint.push_u64(fault_tag(cell.fault_point));
        fingerprint.push_text(cell.baseline_id.as_str());
        fingerprint.push_u64(u64::from(cell.repetition));
        fingerprint.push_u64(fault_status_tag(cell.status));
        fingerprint.push_bool(cell.readback.authoritative_readback);
        fingerprint.push_bool(cell.readback.idempotent_replay);
        fingerprint.push_bool(cell.containment.no_double_publish);
        fingerprint.push_bool(cell.containment.no_identity_widening);
        fingerprint.push_bool(cell.containment.restrictive_barriers_preserved);
    }
    Ok(FaultMatrixReport {
        run_digest: run.run_digest(),
        cells,
        passed: blockers.is_empty(),
        blockers,
        matrix_digest: fingerprint.finish(),
    })
}

const fn fault_tag(value: FaultPoint) -> u64 {
    match value {
        FaultPoint::BeforeOwnerAcquire => 1,
        FaultPoint::AfterOwnerLock => 2,
        FaultPoint::ControlCommit => 3,
        FaultPoint::RevisionCommit => 4,
        FaultPoint::IndexStage => 5,
        FaultPoint::IndexClose => 6,
        FaultPoint::VisibleEpochCommit => 7,
        FaultPoint::CapabilityMint => 8,
        FaultPoint::PurgeBarrier => 9,
        FaultPoint::PurgeDelete => 10,
        FaultPoint::Drain => 11,
        FaultPoint::OwnerRelease => 12,
    }
}

const fn fault_status_tag(value: FaultCellStatus) -> u64 {
    match value {
        FaultCellStatus::Pass => 1,
        FaultCellStatus::Fail => 2,
        FaultCellStatus::Unavailable => 3,
    }
}
