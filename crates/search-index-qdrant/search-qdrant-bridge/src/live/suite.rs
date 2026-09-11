use qdrant_client::Qdrant;

use crate::qualified::{
    LiveProbeOutcome, LiveProbeReceipt, MANDATORY_LIVE_PROBES, ObservedClient,
    QUALIFIED_CLIENT_VERSION, QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION,
    QualificationError, verify_client,
};

use super::fixtures::QUALIFICATION_COLLECTION;
use super::probes::{
    probe_count_and_readback, probe_create_and_topology,
    probe_independent_idf, probe_ingest_batch_a, probe_missing_upper_bound,
    probe_payload_indexes, probe_schema_digest, probe_server_identity,
    probe_signed_range, probe_sparse_modifier, probe_strict_negatives,
};
use super::server::{DisposableServer, connect};
use super::LiveError;

/// Executed suite report: the admission receipt plus human-readable evidence
/// lines (`PROBE <id> PASS <detail>`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveSuiteReport {
    pub receipt: LiveProbeReceipt,
    pub log: Vec<String>,
}

pub(super) struct Suite {
    pub(super) client: Qdrant,
    pub(super) log: Vec<String>,
    outcomes: Vec<LiveProbeOutcome>,
    failures: usize,
}

impl Suite {
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
}

/// Executes every bridge-owned mandatory probe against a disposable server.
///
/// The suite owns no secrets and binds only loopback. Any transport failure
/// aborts with [`LiveError`]; per-probe expectations that do not hold are
/// recorded as failed outcomes and surface as [`LiveError::ProbeFailed`]
/// after the run — never as silent success.
pub async fn run_qualification_suite(
    server: &DisposableServer,
) -> Result<LiveSuiteReport, LiveError> {
    let client = connect(server.endpoint())?;
    let mut suite = Suite {
        client,
        log: vec![format!(
            "SUITE start pid={} http_port={} grpc_port={} collection={QUALIFICATION_COLLECTION} \
             client=qdrant-client/{QUALIFIED_CLIENT_VERSION}",
            server.pid(),
            server.endpoint().http_port(),
            server.endpoint().grpc_port(),
        )],
        outcomes: Vec::new(),
        failures: 0,
    };
    probe_server_identity(&mut suite).await?;
    probe_create_and_topology(&mut suite).await?;
    probe_payload_indexes(&mut suite).await?;
    probe_ingest_batch_a(&mut suite).await?;
    probe_strict_negatives(&mut suite).await?;
    probe_signed_range(&mut suite).await?;
    probe_independent_idf(&mut suite).await?;
    probe_sparse_modifier(&mut suite).await?;
    probe_missing_upper_bound(&mut suite).await?;
    probe_count_and_readback(&mut suite).await?;
    probe_schema_digest(&mut suite).await?;
    suite.log.push(format!(
        "SUITE end failures={} probes={}",
        suite.failures,
        suite.outcomes.len()
    ));
    if suite.failures > 0 {
        return Err(LiveError::ProbeFailed { probe: "suite" });
    }
    for required in MANDATORY_LIVE_PROBES {
        let ok = suite
            .outcomes
            .iter()
            .any(|outcome| outcome.probe_id == required && outcome.passed);
        if !ok {
            return Err(LiveError::ProbeFailed { probe: required });
        }
    }
    let receipt = LiveProbeReceipt {
        server_version: QUALIFIED_SERVER_VERSION.to_owned(),
        server_build: QUALIFIED_SERVER_BUILD.to_owned(),
        client_version: QUALIFIED_CLIENT_VERSION.to_owned(),
        collection: QUALIFICATION_COLLECTION.to_owned(),
        outcomes: suite.outcomes.clone(),
    };
    Ok(LiveSuiteReport {
        receipt,
        log: suite.log,
    })
}

/// Verifies the compiled-in client pin.
///
/// Compares the registry/lockfile record against the pin before any live call.
/// Callers pass the checksum recorded in
/// `qualification/qdrant/artifact.toml`; a mismatch fails without touching the
/// network.
#[must_use]
pub fn verify_compiled_client(
    source_checksum: &str,
) -> Option<QualificationError> {
    verify_client(&ObservedClient {
        crate_name: "qdrant-client".to_owned(),
        version: QUALIFIED_CLIENT_VERSION.to_owned(),
        source_checksum: source_checksum.to_owned(),
    })
    .err()
}
