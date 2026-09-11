//! Measured resource budgets on a frozen representative corpus (T40).
//!
//! Ceilings are declared upfront from the T30-measured base (fresh DIRECT
//! index ~1.5s, live write ~3.5s, single-op harness ceiling 15s) and every
//! report carries wall-clock figures measured on the real product path —
//! never inferred from code constants. Resident memory has no honest
//! in-process source on this path without a platform API, so reports carry
//! [`MEMORY_BYTES_STATUS`] (`UNAVAILABLE`) instead of an invented number.
//! Cancellation latency is measured as bounded kill-to-reap (or typed early
//! completion) on a real oversized batch, and control-store deltas prove
//! that repeated queries append no application control records.

/// Single daemon invocation ceiling (T30 harness bound, kept verbatim).
pub const CEIL_SINGLE_OP_MS: u128 = 15_000;
/// Whole-batch total-work ceiling: adversarial batches must drain inside it.
pub const CEIL_TOTAL_BATCH_MS: u128 = 600_000;
/// Whole-batch total response ceiling across all invocations.
pub const CEIL_TOTAL_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
/// Bounded kill-to-reap ceiling for a cancelled batch unit.
pub const CEIL_CANCELLATION_MS: u128 = 5_000;
/// Honest memory status: no platform RSS source is linked on this path.
pub const MEMORY_BYTES_STATUS: &str = "UNAVAILABLE";
/// Maximum samples retained by one report; beyond it ingestion fails closed.
pub const MAX_BUDGET_SAMPLES: usize = 4096;

/// One measured unit of daemon work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetSample {
    /// Stable operation identity (`ingest`, `query`, `currentness`, `rebuild`).
    pub op: &'static str,
    /// Measured wall milliseconds for the unit.
    pub elapsed_ms: u128,
    /// Measured response bytes drained from the child.
    pub response_bytes: u64,
    /// Measured `control/source-events.log` byte delta across the unit.
    pub control_bytes_delta: u64,
}

/// Nearest-rank percentile over measured wall milliseconds.
///
/// `sorted_ms` must be sorted ascending; returns `None` for an empty slice
/// or a zero/over-100 rank instead of inventing a value.
#[must_use]
pub fn percentile_sorted(sorted_ms: &[u128], percent: u8) -> Option<u128> {
    if sorted_ms.is_empty() || percent == 0 || percent > 100 {
        return None;
    }
    let rank = (u128::from(percent) * sorted_ms.len() as u128).div_ceil(100);
    sorted_ms
        .get(usize::try_from(rank.saturating_sub(1)).unwrap_or(usize::MAX))
        .copied()
}

/// Bounded ledger of measured samples for one frozen-corpus run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BudgetReport {
    samples: Vec<BudgetSample>,
}

impl BudgetReport {
    /// Empty report.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    /// Records one measured sample.
    ///
    /// # Errors
    ///
    /// Returns `RESOURCE_BUDGET_SAMPLE_LIMIT` once [`MAX_BUDGET_SAMPLES`] is
    /// reached instead of growing without bound.
    pub fn push(&mut self, sample: BudgetSample) -> Result<(), &'static str> {
        if self.samples.len() >= MAX_BUDGET_SAMPLES {
            return Err("RESOURCE_BUDGET_SAMPLE_LIMIT");
        }
        self.samples.push(sample);
        Ok(())
    }

    /// Number of recorded samples.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether any sample was recorded.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Nearest-rank percentile of wall milliseconds for one operation.
    #[must_use]
    pub fn percentile(&self, op: &str, percent: u8) -> Option<u128> {
        let mut values = self
            .samples
            .iter()
            .filter(|sample| sample.op == op)
            .map(|sample| sample.elapsed_ms)
            .collect::<Vec<_>>();
        values.sort_unstable();
        percentile_sorted(&values, percent)
    }

    /// Total measured wall milliseconds across all samples (saturating).
    #[must_use]
    pub fn total_ms(&self) -> u128 {
        self.samples.iter().fold(0_u128, |total, sample| {
            total.saturating_add(sample.elapsed_ms)
        })
    }

    /// Total measured response bytes across all samples (saturating).
    #[must_use]
    pub fn total_response_bytes(&self) -> u64 {
        self.samples.iter().fold(0_u64, |total, sample| {
            total.saturating_add(sample.response_bytes)
        })
    }

    /// Total measured control-store byte delta across all samples.
    #[must_use]
    pub fn total_control_bytes_delta(&self) -> u64 {
        self.samples.iter().fold(0_u64, |total, sample| {
            total.saturating_add(sample.control_bytes_delta)
        })
    }

    /// Enforces the declared total-work ceilings on measured totals.
    ///
    /// # Errors
    ///
    /// Returns `RESOURCE_BUDGET_EXCEEDED` when measured total time or total
    /// response bytes breach [`CEIL_TOTAL_BATCH_MS`] /
    /// [`CEIL_TOTAL_RESPONSE_BYTES`].
    pub fn check_totals(&self) -> Result<(), &'static str> {
        if self.total_ms() > CEIL_TOTAL_BATCH_MS
            || self.total_response_bytes() > CEIL_TOTAL_RESPONSE_BYTES
        {
            return Err("RESOURCE_BUDGET_EXCEEDED");
        }
        Ok(())
    }

    /// Bounded JSON report with measured figures and honest memory status.
    #[must_use]
    pub fn json(&self) -> String {
        use std::fmt::Write as _;
        let mut body = String::from("{\"event\":\"t40_budget_report\",");
        for op in ["ingest", "query", "currentness", "rebuild"] {
            let count = self.samples.iter().filter(|sample| sample.op == op).count();
            let p50 = self
                .percentile(op, 50)
                .map_or_else(|| "null".to_owned(), |value| value.to_string());
            let p95 = self
                .percentile(op, 95)
                .map_or_else(|| "null".to_owned(), |value| value.to_string());
            let _ = write!(
                body,
                "\"{op}_count\":{count},\"{op}_p50_ms\":{p50},\"{op}_p95_ms\":{p95},"
            );
        }
        let _ = write!(
            body,
            "\"total_ms\":{},\"total_response_bytes\":{},\"total_control_bytes_delta\":{},\"memory_bytes\":\"{MEMORY_BYTES_STATUS}\"}}",
            self.total_ms(),
            self.total_response_bytes(),
            self.total_control_bytes_delta(),
        );
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_is_nearest_rank_and_honest_on_empty() {
        assert_eq!(percentile_sorted(&[], 50), None);
        assert_eq!(percentile_sorted(&[7], 0), None);
        assert_eq!(percentile_sorted(&[7], 101), None);
        assert_eq!(percentile_sorted(&[7], 50), Some(7));
        let sorted = [10, 20, 30, 40];
        assert_eq!(percentile_sorted(&sorted, 50), Some(20));
        assert_eq!(percentile_sorted(&sorted, 95), Some(40));
        assert_eq!(percentile_sorted(&sorted, 100), Some(40));
        assert_eq!(percentile_sorted(&sorted, 25), Some(10));
    }

    #[test]
    fn report_percentiles_totals_and_total_ceilings() {
        let mut report = BudgetReport::new();
        assert!(report.is_empty());
        for elapsed in [100_u128, 200, 300, 400] {
            report
                .push(BudgetSample {
                    op: "query",
                    elapsed_ms: elapsed,
                    response_bytes: 512,
                    control_bytes_delta: 0,
                })
                .expect("bounded ledger admits samples");
        }
        assert_eq!(report.len(), 4);
        assert_eq!(report.percentile("query", 50), Some(200));
        assert_eq!(report.percentile("query", 95), Some(400));
        assert_eq!(report.percentile("ingest", 50), None);
        assert_eq!(report.total_ms(), 1000);
        assert_eq!(report.total_response_bytes(), 2048);
        assert_eq!(report.total_control_bytes_delta(), 0);
        report.check_totals().expect("well under ceilings");
        let rendered = report.json();
        assert!(
            rendered.contains("\"memory_bytes\":\"UNAVAILABLE\""),
            "{rendered}"
        );
        assert!(rendered.contains("\"query_p50_ms\":200"), "{rendered}");
    }

    #[test]
    fn totals_breach_fails_closed_and_samples_are_bounded() {
        let mut report = BudgetReport::new();
        report
            .push(BudgetSample {
                op: "ingest",
                elapsed_ms: CEIL_TOTAL_BATCH_MS + 1,
                response_bytes: 0,
                control_bytes_delta: 0,
            })
            .expect("single sample fits");
        assert_eq!(report.check_totals(), Err("RESOURCE_BUDGET_EXCEEDED"));

        let mut full = BudgetReport::new();
        for _ in 0..MAX_BUDGET_SAMPLES {
            full.push(BudgetSample {
                op: "query",
                elapsed_ms: 0,
                response_bytes: 0,
                control_bytes_delta: 0,
            })
            .expect("ledger admits up to the bound");
        }
        assert_eq!(
            full.push(BudgetSample {
                op: "query",
                elapsed_ms: 0,
                response_bytes: 0,
                control_bytes_delta: 0,
            }),
            Err("RESOURCE_BUDGET_SAMPLE_LIMIT")
        );
    }
}
