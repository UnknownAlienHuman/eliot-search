//! Mutable live-suite state and final mandatory-receipt construction.

use crate::qualified::{
    LiveProbeOutcome, LiveProbeReceipt, MANDATORY_LIVE_PROBES, QUALIFIED_CLIENT_VERSION,
};
use crate::real::LiveProbeClient;

use super::super::LiveError;
use super::report::LiveSuiteReport;

pub(in crate::live) struct Suite<'a> {
    pub(in crate::live) client: &'a LiveProbeClient,
    pub(in crate::live) log: Vec<String>,
    outcomes: Vec<LiveProbeOutcome>,
    failures: usize,
    server_identity: Option<(String, String)>,
}

impl<'a> Suite<'a> {
    pub(super) fn new(client: &'a LiveProbeClient, start_line: String) -> Self {
        Self {
            client,
            log: vec![start_line],
            outcomes: Vec::new(),
            failures: 0,
            server_identity: None,
        }
    }

    pub(in crate::live) fn observe_server_identity(&mut self, version: String, build: String) {
        self.server_identity = Some((version, build));
    }

    pub(in crate::live) fn record(&mut self, probe_id: &'static str, passed: bool, detail: String) {
        let status = if passed { "PASS" } else { "FAIL" };
        self.log.push(format!("PROBE {probe_id} {status} {detail}"));
        if !passed {
            self.failures += 1;
        }
        self.outcomes.push(LiveProbeOutcome {
            probe_id: probe_id.to_owned(),
            passed,
            detail,
        });
    }

    pub(super) fn finish(mut self) -> Result<(LiveSuiteReport, ExecutedProbeReceipt), LiveError> {
        self.log.push(format!(
            "SUITE end failures={} probes={}",
            self.failures,
            self.outcomes.len()
        ));
        validate_probe_outcomes(&self.outcomes, self.failures)?;
        let (server_version, server_build) =
            self.server_identity.take().ok_or(LiveError::ProbeFailed {
                probe: "live_server_identity",
            })?;
        let receipt = LiveProbeReceipt {
            server_version,
            server_build,
            client_version: QUALIFIED_CLIENT_VERSION.to_owned(),
            collection: self.client.collection_name().to_owned(),
            outcomes: self.outcomes,
        };
        let executed = ExecutedProbeReceipt {
            receipt: receipt.clone(),
        };
        Ok((
            LiveSuiteReport {
                receipt,
                log: self.log,
            },
            executed,
        ))
    }
}

/// Unforgeable in-crate proof that the ordered mandatory probes ran and passed.
pub struct ExecutedProbeReceipt {
    receipt: LiveProbeReceipt,
}

impl ExecutedProbeReceipt {
    pub(crate) const fn receipt(&self) -> &LiveProbeReceipt {
        &self.receipt
    }
}

fn validate_probe_outcomes(
    outcomes: &[LiveProbeOutcome],
    failures: usize,
) -> Result<(), LiveError> {
    if failures > 0 {
        return Err(LiveError::ProbeFailed { probe: "suite" });
    }
    for required in MANDATORY_LIVE_PROBES {
        let passed = outcomes
            .iter()
            .any(|outcome| outcome.probe_id == required && outcome.passed);
        if !passed {
            return Err(LiveError::ProbeFailed { probe: required });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_probe_never_finishes_as_a_receipt() {
        let outcomes = [LiveProbeOutcome {
            probe_id: "synthetic_failure".to_owned(),
            passed: false,
            detail: "failed".to_owned(),
        }];
        assert_eq!(
            validate_probe_outcomes(&outcomes, 1),
            Err(LiveError::ProbeFailed { probe: "suite" })
        );
    }
}
