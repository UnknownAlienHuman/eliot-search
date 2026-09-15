//! Deterministic content-safe staged restore receipt.

use search_retention::RestorePhase;

use super::model::{StagedRestore, phase};

/// Deterministic staged receipt: no timestamps, no randomness.
#[must_use]
pub fn staged_receipt(staged: &StagedRestore) -> String {
    let (phase_name, pending) = match phase(staged) {
        RestorePhase::RestorePendingRevalidation => ("restore-pending-revalidation", true),
        RestorePhase::ControlReadbackVerified => ("control-readback-verified", true),
        RestorePhase::ObjectsReadbackVerified => ("objects-readback-verified", true),
        RestorePhase::DirectOnly => ("direct-only", false),
        RestorePhase::IndexReadbackVerified => ("index-readback-verified", false),
        RestorePhase::IndexedAdmitted => ("indexed-admitted", false),
        RestorePhase::Quarantined => ("quarantined", false),
    };
    format!(
        concat!(
            "{{\"event\":\"restore-staged\",\"schema\":\"eliot.restore-staging.v1\",",
            "\"export_id\":\"{}\",\"phase\":\"{}\",\"pending_validation\":{},\"ready\":{},",
            "\"migrated\":{},\"cutover\":{},\"source_present\":{},\"source_deleted\":{},",
            "\"interrupted\":{}}}"
        ),
        staged.export.export_id.as_str(),
        phase_name,
        pending,
        matches!(phase(staged), RestorePhase::IndexedAdmitted),
        staged.migrated,
        staged.cutover.is_some(),
        staged.source_present,
        staged.source_deleted,
        staged.interrupted,
    )
}
