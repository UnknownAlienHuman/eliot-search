//! Explicit mandatory live-suite execution order.

use crate::qualified::QUALIFIED_CLIENT_VERSION;
use crate::real::{CanonicalRequestContext, LiveProbeClient, OpContext, OperationalProbeFailure};
use crate::{BridgeError, BridgeLimits};

use super::super::LiveError;
use super::super::probes::{
    probe_count_and_readback, probe_create_and_topology, probe_independent_idf,
    probe_ingest_batch_a, probe_missing_upper_bound, probe_payload_indexes, probe_schema_digest,
    probe_server_identity, probe_signed_range, probe_sparse_modifier, probe_strict_negatives,
};
use super::super::server::DisposableServer;
use super::report::LiveSuiteReport;
use super::state::{ExecutedProbeReceipt, Suite};

pub struct ProbeRunComplete {
    pub(crate) report: LiveSuiteReport,
    pub(crate) executed: ExecutedProbeReceipt,
}

/// Runs the actual owner RPC probes, then performs one bounded exact-scope cleanup.
pub async fn run_live_probe_suite(
    client: &LiveProbeClient,
) -> Result<ProbeRunComplete, OperationalProbeFailure> {
    let start = format!(
        "SUITE start attempt={} collection={} client=qdrant-client/{QUALIFIED_CLIENT_VERSION}",
        client.attempt().id().as_str(),
        client.collection_name(),
    );
    let mut suite = Suite::new(client, start);
    let probes = run_mandatory_probes(&mut suite).await;
    let finished = suite.finish();
    if let Err(error) = probes {
        return Err(cleanup_failed_probe(client, bridge_error(&error)).await);
    }
    let (report, executed) = match finished {
        Ok(completed) => completed,
        Err(error) => return Err(cleanup_failed_probe(client, bridge_error(&error)).await),
    };
    if let Err(error) = client.cleanup_owned_collection().await {
        return Err(client.failure(error));
    }
    Ok(ProbeRunComplete { report, executed })
}

async fn cleanup_failed_probe(
    client: &LiveProbeClient,
    error: BridgeError,
) -> OperationalProbeFailure {
    if client.unknown_mutation().is_some() {
        return client.failure(BridgeError::MutationOutcomeUnknown);
    }
    match client.cleanup_owned_collection().await {
        Ok(()) => client.failure(error),
        Err(cleanup_error) => client.failure(cleanup_error),
    }
}

const fn bridge_error(error: &LiveError) -> BridgeError {
    match error {
        LiveError::Bridge(error) => *error,
        LiveError::ProbeFailed { .. } => BridgeError::CapabilityProbeFailed,
        LiveError::ServerVersionUnexpected | LiveError::ServerBuildUnexpected => {
            BridgeError::CapabilityReceiptMismatch
        }
        LiveError::FixtureNotRepresentable => BridgeError::CollectionSchemaMismatch,
        _ => BridgeError::TransportFailed,
    }
}

async fn run_mandatory_probes(suite: &mut Suite<'_>) -> Result<(), LiveError> {
    probe_server_identity(suite).await?;
    suite.client.verify_list_collections_access().await?;
    suite
        .log
        .push("AUTH protected list_collections PASS".to_owned());
    suite.client.refuse_existing_collection().await?;
    probe_create_and_topology(suite).await?;
    probe_payload_indexes(suite).await?;
    probe_ingest_batch_a(suite).await?;
    probe_strict_negatives(suite).await?;
    probe_signed_range(suite).await?;
    probe_independent_idf(suite).await?;
    probe_sparse_modifier(suite).await?;
    probe_missing_upper_bound(suite).await?;
    probe_count_and_readback(suite).await?;
    probe_schema_digest(suite).await?;
    Ok(())
}

/// Executes every bridge-owned mandatory probe against a disposable server.
///
/// The private disposable server receives a fixed test-only API key through
/// its child environment; the suite never handles a production lease or key
/// and binds only loopback. Any transport failure aborts with [`LiveError`];
/// per-probe expectations that do not hold are recorded as failed outcomes
/// and reject final receipt construction — never as silent success.
pub async fn run_qualification_suite(
    server: &DisposableServer,
) -> Result<LiveSuiteReport, LiveError> {
    let context = CanonicalRequestContext::new(OpContext::new(std::time::Duration::from_secs(180)))
        .map_err(LiveError::Bridge)?;
    let client = LiveProbeClient::connect(
        server.endpoint(),
        server.fixture_connection_binding(),
        server.fixture_api_key_lease(),
        context,
        BridgeLimits::BASELINE,
        std::time::Duration::from_secs(10),
    )
    .map_err(|failure| LiveError::Bridge(failure.error()))?;
    run_live_probe_suite(&client)
        .await
        .map(|completed| completed.report)
        .map_err(|failure| LiveError::Bridge(failure.error()))
}
