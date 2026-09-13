//! Mutable live-suite state and final mandatory-receipt construction.

use qdrant_client::Qdrant;

use crate::qualified::{
    LiveProbeOutcome, LiveProbeReceipt, MANDATORY_LIVE_PROBES,
    QUALIFIED_CLIENT_VERSION, QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION,
};

use super::super::fixtures::QUALIFICATION_COLLECTION;
use super::super::LiveError;
use super::report::LiveSuiteReport;

pub(super) struct Suite {
    pub(super) client: Qdrant,
    pub(super) log: Vec<String>,
    outcomes: Vec<LiveProbeOutcome>,
    failures: usize,
}

impl Suite {
    pub(super) fn new(client: Qdrant, start_line: String) -> Self {
        Self {
            client,
            log: vec![start_line],
            outcomes: Vec::new(),
            failures: 0,
        }
    }

    pub(super) fn record(
        &mut self,
        probe_id: &'static str,
        passed: bool,
        detail: String,
    ) {
        let status = if passed { "PASS" } else { "FAIL" };
        self.log
            .push(format!("PROBE {probe_id} {status} {detail}"));
        if !passed {
            self.failures += 1;
        }
        self.outcomes.push(LiveProbeOutcome {
            probe_id: probe_id.to_owned(),
            passed,
            detail,
        });
    }

    pub(super) fn finish(mut self) -> Result<LiveSuiteReport, LiveError> {
        self.log.push(format!(
            "SUITE end failures={} probes={}",
            self.failures,
            self.outcomes.len()
        ));
        if self.failures > 0 {
            return Err(LiveError::ProbeFailed { probe: "suite" });
        }
        for required in MANDATORY_LIVE_PROBES {
            let passed = self
                .outcomes
                .iter()
                .any(|outcome| outcome.probe_id == required && outcome.passed);
            if !passed {
                return Err(LiveError::ProbeFailed { probe: required });
            }
        }
        Ok(LiveSuiteReport {
            receipt: LiveProbeReceipt {
                server_version: QUALIFIED_SERVER_VERSION.to_owned(),
                server_build: QUALIFIED_SERVER_BUILD.to_owned(),
                client_version: QUALIFIED_CLIENT_VERSION.to_owned(),
                collection: QUALIFICATION_COLLECTION.to_owned(),
                outcomes: self.outcomes,
            },
            log: self.log,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_probe_never_finishes_as_a_receipt() {
        let endpoint = qdrant_client::Qdrant::from_url("http://127.0.0.1:1")
            .skip_compatibility_check()
            .build()
            .expect("client construction does not connect");
        let mut suite = Suite::new(endpoint, "SUITE start".to_owned());
        suite.record("synthetic_failure", false, "failed".to_owned());
        assert_eq!(
            suite.finish(),
            Err(LiveError::ProbeFailed { probe: "suite" })
        );
    }
}
