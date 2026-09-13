//! Public executed live-suite report.

use crate::qualified::LiveProbeReceipt;

/// Executed suite report: the admission receipt plus human-readable evidence
/// lines (`PROBE <id> PASS <detail>`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveSuiteReport {
    /// Exact bridge-owned mandatory-probe receipt.
    pub receipt: LiveProbeReceipt,
    /// Ordered bounded human-readable evidence lines.
    pub log: Vec<String>,
}
