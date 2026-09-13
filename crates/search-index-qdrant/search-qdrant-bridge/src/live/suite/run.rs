//! Explicit mandatory live-suite execution order.

use crate::qualified::QUALIFIED_CLIENT_VERSION;

use super::super::fixtures::QUALIFICATION_COLLECTION;
use super::super::probes::{
    probe_count_and_readback, probe_create_and_topology,
    probe_independent_idf, probe_ingest_batch_a, probe_missing_upper_bound,
    probe_payload_indexes, probe_schema_digest, probe_server_identity,
    probe_signed_range, probe_sparse_modifier, probe_strict_negatives,
};
use super::super::server::{DisposableServer, connect};
use super::super::LiveError;
use super::report::LiveSuiteReport;
use super::state::Suite;

/// Executes every bridge-owned mandatory probe against a disposable server.
///
/// The suite owns no secrets and binds only loopback. Any transport failure
/// aborts with [`LiveError`]; per-probe expectations that do not hold are
/// recorded as failed outcomes and reject final receipt construction — never
/// as silent success.
pub async fn run_qualification_suite(
    server: &DisposableServer,
) -> Result<LiveSuiteReport, LiveError> {
    let client = connect(server.endpoint())?;
    let mut suite = Suite::new(
        client,
        format!(
            "SUITE start pid={} http_port={} grpc_port={} collection={QUALIFICATION_COLLECTION} \
             client=qdrant-client/{QUALIFIED_CLIENT_VERSION}",
            server.pid(),
            server.endpoint().http_port(),
            server.endpoint().grpc_port(),
        ),
    );
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
    suite.finish()
}
