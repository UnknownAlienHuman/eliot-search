//! Closed finite bounds for ephemeral DIRECT result handles.

use std::time::Duration;

/// Maximum simultaneous result handles.
pub const MAX_RESULT_HANDLES: usize = 50_000;
/// Maximum exact bytes returned by one handle expansion.
pub const MAX_HANDLE_EXPANSION_BYTES: u64 = 24 * 1024;
/// Finite process-local handle lifetime.
pub const RESULT_HANDLE_TTL: Duration = Duration::from_mins(15);
