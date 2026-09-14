//! Live current-workspace proof over independent source and index legs.

/// Empty registration blocks a current claim; it is not proof of emptiness.
pub const ROOTS_EMPTY_BLOCKS_CURRENTNESS: &str = "ROOTS_EMPTY_BLOCKS_CURRENTNESS";
/// Any unresolved observation gap blocks a current claim.
pub const ROOTS_GAP_BLOCKS_CURRENTNESS: &str = "ROOTS_GAP_BLOCKS_CURRENTNESS";
/// A workspace sync that did not cover the current generation blocks proof.
pub const ROOTS_SYNC_INCOMPLETE_BLOCKS_CURRENTNESS: &str =
    "ROOTS_SYNC_INCOMPLETE_BLOCKS_CURRENTNESS";
/// An unavailable index blocks the final proven claim, never the gap accounting.
pub const ROOTS_INDEX_UNAVAILABLE_BLOCKS_CURRENTNESS: &str =
    "ROOTS_INDEX_UNAVAILABLE_BLOCKS_CURRENTNESS";
/// All independent currentness legs hold.
pub const ROOTS_CURRENT_WORKSPACE_PROVEN: &str = "ROOTS_CURRENT_WORKSPACE_PROVEN";

/// Evaluates the final current-workspace claim from independent legs.
///
/// `workspace_current` is the catalog truth (probed active set plus a sync
/// covering the current generation with zero gaps). `index_available` is
/// index truth. Every leg must hold; any failure is an explicit fence, never
/// an empty success.
#[must_use]
pub const fn evaluate_current_workspace_proven(
    configured: usize,
    gap_count: usize,
    workspace_current: bool,
    index_available: bool,
) -> bool {
    configured > 0 && gap_count == 0 && workspace_current && index_available
}

/// Explains why the final proven claim does or does not hold. The order is
/// fixed: empty, then gaps, then sync coverage, then index. The proven code
/// is returned only when every leg holds.
#[must_use]
pub const fn current_workspace_proven_reason(
    configured: usize,
    gap_count: usize,
    workspace_current: bool,
    index_available: bool,
) -> &'static str {
    if configured == 0 {
        ROOTS_EMPTY_BLOCKS_CURRENTNESS
    } else if gap_count > 0 {
        ROOTS_GAP_BLOCKS_CURRENTNESS
    } else if !workspace_current {
        ROOTS_SYNC_INCOMPLETE_BLOCKS_CURRENTNESS
    } else if !index_available {
        ROOTS_INDEX_UNAVAILABLE_BLOCKS_CURRENTNESS
    } else {
        ROOTS_CURRENT_WORKSPACE_PROVEN
    }
}
