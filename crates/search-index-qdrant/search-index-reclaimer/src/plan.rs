//! Deterministic finite exact-ID reclaim planning.

use search_contracts::OpaqueId;
use search_epoch_pins::ReclamationWatermark;

use crate::{CommittedRetiredManifest, ReclaimError, ReclaimPointId};

/// Finite reclaim tuning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReclaimSettings {
    /// Exact point identifiers per delete/readback batch.
    pub batch_size: usize,
}

/// Finite plan budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReclaimBudget {
    /// Maximum identifiers in one plan.
    pub max_points: usize,
    /// Maximum batches in one plan.
    pub max_batches: usize,
}

impl ReclaimBudget {
    /// Conservative local baseline.
    pub const BASELINE: Self = Self {
        max_points: 1_000_000,
        max_batches: 10_000,
    };
}

/// Frozen package-local deterministic plan digest.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReclaimPlanDigest(pub [u8; 32]);

/// One exact finite deletion batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclaimBatch {
    /// Zero-based deterministic batch index.
    pub batch_index: usize,
    /// Immutable operation identity for this exact point set.
    pub operation_id: OpaqueId,
    /// Canonically ordered exact identifiers.
    pub point_ids: Vec<ReclaimPointId>,
}

/// Complete exact reclaim plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclaimPlan {
    /// Exact committed retired manifest.
    pub manifest: CommittedRetiredManifest,
    /// Deterministic plan digest.
    pub plan_digest: ReclaimPlanDigest,
    /// Exact deterministic batches.
    pub batches: Vec<ReclaimBatch>,
}

/// Plans deterministic exact-ID batches only after all active pins permit it.
///
/// # Errors
///
/// Rejects pinned state, zero limits, oversized manifests, or unrepresentable
/// operation identities. No partial plan is returned.
pub fn plan(
    manifest: CommittedRetiredManifest,
    watermark: ReclamationWatermark,
    settings: ReclaimSettings,
    budget: ReclaimBudget,
) -> Result<ReclaimPlan, ReclaimError> {
    if !watermark.reclaimable
        || watermark.blocking_epoch_pins != 0
        || watermark.blocking_route_pins != 0
    {
        return Err(ReclaimError::StillPinned);
    }
    if settings.batch_size == 0 || budget.max_points == 0 || budget.max_batches == 0 {
        return Err(ReclaimError::BudgetExceeded);
    }
    let points = &manifest.manifest().point_ids;
    if points.is_empty() || points.len() > budget.max_points {
        return Err(ReclaimError::BudgetExceeded);
    }
    let batch_count = points.len().div_ceil(settings.batch_size);
    if batch_count == 0 || batch_count > budget.max_batches {
        return Err(ReclaimError::BudgetExceeded);
    }

    let plan_digest = derive_plan_digest(&manifest, settings.batch_size)?;
    let prefix = hex_prefix(&plan_digest.0);
    let mut batches = Vec::with_capacity(batch_count);
    for (batch_index, point_ids) in points.chunks(settings.batch_size).enumerate() {
        let operation_id = OpaqueId::new(format!("reclaim:{prefix}:{batch_index}"))
            .map_err(|_| ReclaimError::IdentityEncoding)?;
        batches.push(ReclaimBatch {
            batch_index,
            operation_id,
            point_ids: point_ids.to_vec(),
        });
    }
    Ok(ReclaimPlan {
        manifest,
        plan_digest,
        batches,
    })
}

fn derive_plan_digest(
    manifest: &CommittedRetiredManifest,
    batch_size: usize,
) -> Result<ReclaimPlanDigest, ReclaimError> {
    let manifest = manifest.manifest();
    let mut state = [
        0xcbf2_9ce4_8422_2325_u64,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0xc2b2_ae3d_27d4_eb4f,
    ];
    mix(&mut state, b"eliot-search/reclaim-plan/v1");
    mix(&mut state, manifest.collection_generation_id.as_bytes());
    mix(&mut state, manifest.manifest_digest.as_bytes());
    mix(
        &mut state,
        &u64::try_from(batch_size)
            .map_err(|_| ReclaimError::IdentityEncoding)?
            .to_be_bytes(),
    );
    for point_id in &manifest.point_ids {
        mix(&mut state, &point_id.0);
    }
    let mut output = [0_u8; 32];
    for (index, lane) in state.into_iter().enumerate() {
        output[index * 8..index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    Ok(ReclaimPlanDigest(output))
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

fn hex_prefix(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(16);
    for byte in &bytes[..8] {
        use core::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
