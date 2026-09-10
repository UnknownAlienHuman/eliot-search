//! Live T22 qualification path beside the in-memory oracle.
//!
//! This module spawns the exact qualified native server on disposable
//! storage with OS-assigned loopback ports, executes every bridge-owned
//! mandatory probe through the pinned `qdrant-client` 1.19.0 gRPC transport,
//! then kills the server and removes the storage. The only way to use any
//! result of this module for indexed admission is
//! [`QualifiedGate`](crate::qualified::QualifiedGate), which rechecks the
//! executed [`LiveProbeReceipt`](crate::qualified::LiveProbeReceipt).
//!
//! Deliberately out of scope (supervisor/T23-owned): API-key secret leases,
//! ACL/Job Object containment and PID-reuse guards. This suite runs without
//! authentication on a disposable loopback instance and never handles
//! secrets, so there is nothing secret-bearing to leak into logs or
//! receipts.
//!
//! Vendor (`qdrant_client`) types never appear in public signatures: every
//! public function consumes/returns std/bridge types only.

use std::collections::HashMap;
use std::fmt;
use std::fmt::Write as _;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CollectionStatus, Condition, CountPoints, CreateCollection, CreateFieldIndexCollection,
    DeletePoints, FieldCondition, FieldType, Filter, GetPoints, IdfParams, Match, Modifier,
    PointId, PointStruct, PointsIdsList, PointsSelector, Query, QueryPoints, Range, SearchParams,
    SparseVectorConfig, SparseVectorParams, StrictModeConfig, UpsertPoints, Value, Vector,
    VectorInput, Vectors, WriteOrdering, WriteOrderingType, condition, r#match, point_id,
    points_selector, value, vectors,
};
use sha2::{Digest, Sha256};

use crate::qualified::{
    BaseEligibility, IndependentIdfProfile, LiveProbeOutcome, LiveProbeReceipt,
    MANDATORY_LIVE_PROBES, ObservedClient, QUALIFIED_CLIENT_VERSION, QUALIFIED_EXE_BYTES,
    QUALIFIED_EXE_SHA256_HEX, QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION, QualificationError,
    admit_independent_idf, verify_client,
};

/// Pinned native server under qualification.
pub const NATIVE_EXE_PATH: &str = r"C:\Tools\Qdrant\1.19.0\qdrant.exe";
/// Disposable qualification collection. The server itself is disposable, so a
/// fixed name is collision-free by construction.
pub const QUALIFICATION_COLLECTION: &str = "t22_qual_probe";
/// Qualified sparse vector names (frozen lexical legs).
pub const VECTOR_CODE: &str = "lex_code_v1";
pub const VECTOR_TEXT: &str = "lex_text_neutral_v1";
/// Payload fields.
pub const FIELD_TENANT: &str = "tenant";
pub const FIELD_ACCESS: &str = "access_partition";
pub const FIELD_FROM: &str = "valid_from_epoch";
pub const FIELD_UNTIL: &str = "valid_until_epoch_exclusive";
/// Payload field that is ingested but deliberately never indexed: the strict
/// negative fixture.
pub const FIELD_UNINDEXED: &str = "unit_kind";
/// Tenant populations.
pub const TENANT_A: &str = "tenant-a";
pub const TENANT_B: &str = "tenant-b";
pub const ACCESS_A: &str = "partition-a";
/// Visible epoch shared by the retrieval plan and the IDF corpus plan.
pub const VISIBLE_EPOCH: u64 = 42;
/// Same epoch as `i64` for integer payload/range construction without casts.
pub const VISIBLE_EPOCH_I64: i64 = 42;
/// Signed epoch extremes as integer payload.
///
/// `Range` bounds travel as `double`, so extremes stay within exactly
/// representable `f64` integers (|v| < 2^53). [`exact_f64`] rechecks
/// exactness instead of assuming it.
pub const EPOCH_MIN: i64 = -9_007_199_254_740_000;
pub const EPOCH_MAX: i64 = 9_007_199_254_740_000;
/// UUID point proving UUID transport without the client `uuid` feature.
pub const UUID_POINT: &str = "550e8400-e29b-41d4-a716-446655440000";

/// Closed live-path failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveError {
    EndpointNotLoopback,
    ExecutableUnreadable,
    ArtifactDigestMismatch,
    ArtifactSizeMismatch,
    StorageSetupFailed,
    SpawnFailed,
    ServerNotReady,
    TransportFailed,
    ServerVersionUnexpected,
    ServerBuildUnexpected,
    FixtureNotRepresentable,
    ProbeFailed { probe: &'static str },
}

impl LiveError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EndpointNotLoopback => "QDRANT_LIVE_ENDPOINT_NOT_LOOPBACK",
            Self::ExecutableUnreadable => "QDRANT_LIVE_EXECUTABLE_UNREADABLE",
            Self::ArtifactDigestMismatch => QualificationError::ArtifactDigestMismatch.code(),
            Self::ArtifactSizeMismatch => QualificationError::ArtifactSizeMismatch.code(),
            Self::StorageSetupFailed => "QDRANT_LIVE_STORAGE_SETUP_FAILED",
            Self::SpawnFailed => "QDRANT_LIVE_SPAWN_FAILED",
            Self::ServerNotReady => "QDRANT_LIVE_SERVER_NOT_READY",
            Self::TransportFailed => "QDRANT_LIVE_TRANSPORT_FAILED",
            Self::ServerVersionUnexpected => "QDRANT_LIVE_SERVER_VERSION_UNEXPECTED",
            Self::ServerBuildUnexpected => "QDRANT_LIVE_SERVER_BUILD_UNEXPECTED",
            Self::FixtureNotRepresentable => "QDRANT_LIVE_FIXTURE_NOT_REPRESENTABLE",
            Self::ProbeFailed { .. } => "QDRANT_LIVE_PROBE_FAILED",
        }
    }
}

impl fmt::Display for LiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LiveError {}

/// Loopback-only endpoint. Construction rejects anything that is not an
/// explicit loopback host before any socket is opened.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveEndpoint {
    host: String,
    http_port: u16,
    grpc_port: u16,
}

impl LiveEndpoint {
    /// Builds an endpoint pair on one host port, rejecting non-loopback hosts.
    pub fn grpc(host: &str, grpc_port: u16) -> Result<Self, LiveError> {
        Self::loopback(host, grpc_port, grpc_port)
    }

    /// Builds an endpoint pair with distinct HTTP/gRPC ports.
    pub fn loopback(host: &str, http_port: u16, grpc_port: u16) -> Result<Self, LiveError> {
        let normalized = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();
        let is_loopback =
            normalized == "127.0.0.1" || normalized == "::1" || normalized == "localhost";
        if !is_loopback {
            return Err(LiveError::EndpointNotLoopback);
        }
        Ok(Self {
            host: normalized,
            http_port,
            grpc_port,
        })
    }

    /// gRPC URL of the disposable server.
    #[must_use]
    pub fn grpc_url(&self) -> String {
        format!("http://{}:{}", self.host, self.grpc_port)
    }

    /// Reserved HTTP port (server config evidence).
    #[must_use]
    pub const fn http_port(&self) -> u16 {
        self.http_port
    }

    /// Reserved gRPC port.
    #[must_use]
    pub const fn grpc_port(&self) -> u16 {
        self.grpc_port
    }
}

/// Reserves two distinct free loopback ports from the OS.
pub fn free_loopback_ports() -> Result<(u16, u16), LiveError> {
    let http = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .local_addr()
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .port();
    let grpc = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .local_addr()
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .port();
    if http == 0 || grpc == 0 || http == grpc {
        return Err(LiveError::EndpointNotLoopback);
    }
    Ok((http, grpc))
}

/// Measures the executable and verifies the exact qualified identity before
/// any process starts. A wrong hash or size fails here; the server is never
/// spawned from unqualified bytes.
pub fn verify_executable(path: &str) -> Result<(), LiveError> {
    let metadata = std::fs::metadata(path).map_err(|_| LiveError::ExecutableUnreadable)?;
    if metadata.len() != QUALIFIED_EXE_BYTES {
        return Err(LiveError::ArtifactSizeMismatch);
    }
    let mut file = std::fs::File::open(path).map_err(|_| LiveError::ExecutableUnreadable)?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0_u8; 65_536];
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|_| LiveError::ExecutableUnreadable)?;
        if read == 0 {
            break;
        }
        hasher.update(&chunk[..read]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        for nibble in [byte >> 4, byte & 0x0F] {
            hex.push(char::from_digit(u32::from(nibble), 16).unwrap_or('?'));
        }
    }
    if hex.to_ascii_uppercase() != QUALIFIED_EXE_SHA256_HEX {
        return Err(LiveError::ArtifactDigestMismatch);
    }
    Ok(())
}

/// A spawned disposable server. [`Drop`] kills the child and removes the
/// storage directory (best effort); qualification storage never escapes the
/// temp directory.
pub struct DisposableServer {
    child: Child,
    dir: PathBuf,
    endpoint: LiveEndpoint,
}

impl DisposableServer {
    /// Reserved loopback endpoint of the child.
    #[must_use]
    pub const fn endpoint(&self) -> &LiveEndpoint {
        &self.endpoint
    }

    /// Disposable storage directory (removed on drop).
    #[must_use]
    pub fn storage_dir(&self) -> &std::path::Path {
        &self.dir
    }

    /// Child process ID for evidence lines.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for DisposableServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        for _ in 0..20 {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Spawns the qualified server on disposable storage with the given loopback
/// ports and waits for gRPC readiness. Only loopback endpoints are accepted.
pub async fn spawn_disposable_server(
    exe_path: &str,
    http_port: u16,
    grpc_port: u16,
) -> Result<DisposableServer, LiveError> {
    verify_executable(exe_path)?;
    let endpoint = LiveEndpoint::loopback("127.0.0.1", http_port, grpc_port)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let dir = std::env::temp_dir().join(format!("eliot-t22-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(dir.join("storage")).map_err(|_| LiveError::StorageSetupFailed)?;
    let config = format!(
        "storage:\n  storage_path: ./storage\nservice:\n  host: 127.0.0.1\n  http_port: \
         {http_port}\n  grpc_port: {grpc_port}\n"
    );
    std::fs::write(dir.join("config.yaml"), config).map_err(|_| LiveError::StorageSetupFailed)?;
    let stdout = std::fs::File::create(dir.join("qdrant-out.log"))
        .map_err(|_| LiveError::StorageSetupFailed)?;
    let stderr = std::fs::File::create(dir.join("qdrant-err.log"))
        .map_err(|_| LiveError::StorageSetupFailed)?;
    let child = Command::new(exe_path)
        .arg("--config-path")
        .arg(dir.join("config.yaml"))
        .current_dir(&dir)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|_| LiveError::SpawnFailed)?;
    let mut server = DisposableServer {
        child,
        dir,
        endpoint,
    };
    let client = connect(server.endpoint())?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        if client.health_check().await.is_ok() {
            return Ok(server);
        }
        match server.child.try_wait() {
            Ok(Some(status)) => {
                eprintln!("qdrant exited during startup: {status}");
                eprintln!("{}", read_log_tail(&server.dir));
                return Err(LiveError::ServerNotReady);
            }
            Ok(None) => {}
            Err(_) => return Err(LiveError::ServerNotReady),
        }
        if tokio::time::Instant::now() >= deadline {
            eprintln!("qdrant not ready in 60s");
            eprintln!("{}", read_log_tail(&server.dir));
            return Err(LiveError::ServerNotReady);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn read_log_tail(dir: &std::path::Path) -> String {
    let mut combined = String::new();
    for name in ["qdrant-out.log", "qdrant-err.log"] {
        if let Ok(content) = std::fs::read_to_string(dir.join(name)) {
            let start = content.len().saturating_sub(1024);
            let _ = write!(combined, "--- {name} (tail) ---\n{}\n", &content[start..]);
        }
    }
    combined
}

fn connect(endpoint: &LiveEndpoint) -> Result<Qdrant, LiveError> {
    // The client's own compatibility check is warn-only and tolerates ±1
    // minor, so it is skipped: the bridge enforces the exact 1.19.0 pair in
    // `probe_server_identity` and `QualifiedGate::admit` instead.
    Qdrant::from_url(&endpoint.grpc_url())
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .skip_compatibility_check()
        .build()
        .map_err(|_| LiveError::TransportFailed)
}

/// Executed suite report: the admission receipt plus human-readable evidence
/// lines (`PROBE <id> PASS <detail>`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveSuiteReport {
    pub receipt: LiveProbeReceipt,
    pub log: Vec<String>,
}

struct Suite {
    client: Qdrant,
    log: Vec<String>,
    outcomes: Vec<LiveProbeOutcome>,
    failures: usize,
}

impl Suite {
    fn record(&mut self, probe_id: &'static str, passed: bool, detail: String) {
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

fn base_eligibility() -> BaseEligibility {
    BaseEligibility {
        access_partition: ACCESS_A.to_owned(),
        tenant: TENANT_A.to_owned(),
        visible_epoch: VISIBLE_EPOCH,
    }
}

const fn exact_f64(value: i64) -> Result<f64, LiveError> {
    #[allow(clippy::cast_precision_loss)]
    let as_float = value as f64;
    // Rechecked below: only exactly representable integers pass, so the
    // truncation lint cannot fire on an admitted value.
    #[allow(clippy::cast_possible_truncation)]
    if as_float as i64 != value {
        return Err(LiveError::FixtureNotRepresentable);
    }
    Ok(as_float)
}

fn keyword_condition(key: &str, value: &str) -> Condition {
    Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(FieldCondition {
            key: key.to_owned(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Keyword(value.to_owned())),
            }),
            ..Default::default()
        })),
    }
}

fn range_condition(key: &str, gte: Option<f64>, lte: Option<f64>) -> Condition {
    Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(FieldCondition {
            key: key.to_owned(),
            range: Some(Range {
                gte,
                lte,
                ..Default::default()
            }),
            ..Default::default()
        })),
    }
}

/// Canonical base eligibility plan, shared verbatim by retrieval and the IDF
/// corpus: tenant-a, access partition A, `valid_from <= 42`, with the
/// open-ended upper bound expressed as `must_not(valid_until <= 42)`.
fn base_filter() -> Result<Filter, LiveError> {
    Ok(Filter {
        must: vec![
            keyword_condition(FIELD_TENANT, TENANT_A),
            keyword_condition(FIELD_ACCESS, ACCESS_A),
            range_condition(FIELD_FROM, None, Some(exact_f64(VISIBLE_EPOCH_I64)?)),
        ],
        must_not: vec![range_condition(
            FIELD_UNTIL,
            None,
            Some(exact_f64(VISIBLE_EPOCH_I64)?),
        )],
        ..Default::default()
    })
}

const fn strong_ordering() -> WriteOrdering {
    WriteOrdering {
        r#type: WriteOrderingType::Strong as i32,
    }
}

const fn int_value(value: i64) -> Value {
    Value {
        kind: Some(value::Kind::IntegerValue(value)),
    }
}

fn string_value(value: &str) -> Value {
    Value {
        kind: Some(value::Kind::StringValue(value.to_owned())),
    }
}

fn sparse_named(code: Vec<(u32, f32)>, text: Vec<(u32, f32)>) -> Vectors {
    let (code_idx, code_val): (Vec<u32>, Vec<f32>) = code.into_iter().unzip();
    let (text_idx, text_val): (Vec<u32>, Vec<f32>) = text.into_iter().unzip();
    let mut map = HashMap::new();
    map.insert(
        VECTOR_CODE.to_owned(),
        Vector::new_sparse(code_idx, code_val),
    );
    map.insert(
        VECTOR_TEXT.to_owned(),
        Vector::new_sparse(text_idx, text_val),
    );
    Vectors {
        vectors_options: Some(vectors::VectorsOptions::Vectors(
            qdrant_client::qdrant::NamedVectors { vectors: map },
        )),
    }
}

fn point(
    id: u64,
    tenant: &str,
    from: i64,
    until: Option<i64>,
    code: Vec<(u32, f32)>,
    text: Vec<(u32, f32)>,
) -> PointStruct {
    let mut payload = HashMap::new();
    payload.insert(FIELD_TENANT.to_owned(), string_value(tenant));
    payload.insert(FIELD_ACCESS.to_owned(), string_value(ACCESS_A));
    payload.insert(FIELD_FROM.to_owned(), int_value(from));
    if let Some(until) = until {
        payload.insert(FIELD_UNTIL.to_owned(), int_value(until));
    }
    payload.insert(FIELD_UNINDEXED.to_owned(), string_value("code_unit"));
    PointStruct {
        id: Some(PointId {
            point_id_options: Some(point_id::PointIdOptions::Num(id)),
        }),
        payload,
        vectors: Some(sparse_named(code, text)),
    }
}

const fn num_point_id(id: u64) -> PointId {
    PointId {
        point_id_options: Some(point_id::PointIdOptions::Num(id)),
    }
}

async fn probe_server_identity(suite: &mut Suite) -> Result<(), LiveError> {
    let reply = suite
        .client
        .health_check()
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let version_ok = reply.version == QUALIFIED_SERVER_VERSION;
    let commit_ok = reply
        .commit
        .as_deref()
        .is_some_and(|commit| commit.starts_with(QUALIFIED_SERVER_BUILD));
    suite.record(
        "live_server_identity",
        version_ok && commit_ok,
        format!(
            "health version={} commit={:?} want={QUALIFIED_SERVER_VERSION}/{QUALIFIED_SERVER_BUILD}",
            reply.version, reply.commit,
        ),
    );
    if !version_ok {
        return Err(LiveError::ServerVersionUnexpected);
    }
    if !commit_ok {
        return Err(LiveError::ServerBuildUnexpected);
    }
    Ok(())
}

async fn probe_create_and_topology(suite: &mut Suite) -> Result<(), LiveError> {
    if suite
        .client
        .collection_exists(QUALIFICATION_COLLECTION)
        .await
        .map_err(|_| LiveError::TransportFailed)?
    {
        let deleted = suite
            .client
            .delete_collection(QUALIFICATION_COLLECTION)
            .await
            .map_err(|_| LiveError::TransportFailed)?;
        if !deleted.result {
            return Err(LiveError::ProbeFailed {
                probe: "one_shard_topology",
            });
        }
    }
    let mut sparse = HashMap::new();
    for name in [VECTOR_CODE, VECTOR_TEXT] {
        sparse.insert(
            name.to_owned(),
            SparseVectorParams {
                modifier: Some(Modifier::Idf as i32),
                ..Default::default()
            },
        );
    }
    let created = suite
        .client
        .create_collection(CreateCollection {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            shard_number: Some(1),
            replication_factor: Some(1),
            write_consistency_factor: Some(1),
            sparse_vectors_config: Some(SparseVectorConfig { map: sparse }),
            strict_mode_config: Some(StrictModeConfig {
                enabled: Some(true),
                unindexed_filtering_retrieve: Some(false),
                unindexed_filtering_update: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    if !created.result {
        return Err(LiveError::ProbeFailed {
            probe: "one_shard_topology",
        });
    }
    let info = suite
        .client
        .collection_info(QUALIFICATION_COLLECTION)
        .await
        .map_err(|_| LiveError::TransportFailed)?
        .result
        .ok_or(LiveError::TransportFailed)?;
    let shards = info
        .config
        .as_ref()
        .and_then(|config| config.params.as_ref())
        .map(|params| params.shard_number);
    suite.record(
        "one_shard_topology",
        shards == Some(1),
        format!("shard_number={shards:?}"),
    );
    Ok(())
}

async fn probe_payload_indexes(suite: &mut Suite) -> Result<(), LiveError> {
    for (field, field_type) in [
        (FIELD_TENANT, FieldType::Keyword),
        (FIELD_ACCESS, FieldType::Keyword),
        (FIELD_FROM, FieldType::Integer),
        (FIELD_UNTIL, FieldType::Integer),
    ] {
        let indexed = suite
            .client
            .create_field_index(CreateFieldIndexCollection {
                collection_name: QUALIFICATION_COLLECTION.to_owned(),
                field_name: field.to_owned(),
                field_type: Some(field_type as i32),
                wait: Some(true),
                ordering: Some(strong_ordering()),
                ..Default::default()
            })
            .await
            .map_err(|_| LiveError::TransportFailed)?;
        let index_ready = indexed
            .result
            .as_ref()
            .is_some_and(|result| update_completed(result.status));
        if !index_ready {
            return Err(LiveError::ProbeFailed {
                probe: "payload_index_completeness",
            });
        }
    }
    let info = suite
        .client
        .collection_info(QUALIFICATION_COLLECTION)
        .await
        .map_err(|_| LiveError::TransportFailed)?
        .result
        .ok_or(LiveError::TransportFailed)?;
    let missing: Vec<&str> = [FIELD_TENANT, FIELD_ACCESS, FIELD_FROM, FIELD_UNTIL]
        .into_iter()
        .filter(|field| !info.payload_schema.contains_key(*field))
        .collect();
    suite.record(
        "payload_index_completeness",
        missing.is_empty(),
        format!("missing={missing:?}"),
    );
    Ok(())
}

fn update_completed(status: i32) -> bool {
    use qdrant_client::qdrant::UpdateStatus;
    UpdateStatus::try_from(status).is_ok_and(|parsed| parsed == UpdateStatus::Completed)
}

async fn probe_ingest_batch_a(suite: &mut Suite) -> Result<(), LiveError> {
    let mut uuid_payload = HashMap::new();
    uuid_payload.insert(FIELD_TENANT.to_owned(), string_value(TENANT_A));
    uuid_payload.insert(FIELD_ACCESS.to_owned(), string_value(ACCESS_A));
    uuid_payload.insert(FIELD_FROM.to_owned(), int_value(10));
    uuid_payload.insert(FIELD_UNINDEXED.to_owned(), string_value("code_unit"));
    let points = vec![
        point(
            1,
            TENANT_A,
            10,
            None,
            vec![(0, 1.0), (1, 1.0)],
            vec![(100, 1.0)],
        ),
        point(2, TENANT_A, 10, Some(50), vec![(0, 1.0)], vec![(100, 1.0)]),
        point(
            9,
            TENANT_A,
            EPOCH_MIN,
            Some(EPOCH_MAX),
            vec![(0, 1.0), (2, 1.0)],
            vec![(100, 1.0)],
        ),
        PointStruct {
            id: Some(PointId {
                point_id_options: Some(point_id::PointIdOptions::Uuid(UUID_POINT.to_owned())),
            }),
            payload: uuid_payload,
            vectors: Some(sparse_named(vec![(1, 1.0)], vec![(101, 1.0)])),
        },
    ];
    let response = suite
        .client
        .upsert_points(UpsertPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            wait: Some(true),
            ordering: Some(strong_ordering()),
            points,
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let acked = response
        .result
        .as_ref()
        .is_some_and(|result| update_completed(result.status));
    let readback = suite
        .client
        .get_points(GetPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            ids: vec![
                num_point_id(1),
                num_point_id(2),
                num_point_id(9),
                PointId {
                    point_id_options: Some(point_id::PointIdOptions::Uuid(UUID_POINT.to_owned())),
                },
            ],
            with_payload: Some(true.into()),
            with_vectors: Some(true.into()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let visible = readback.result.len() == 4
        && readback.result.iter().all(|point| {
            point.payload.get(FIELD_TENANT).is_some_and(|tenant| {
                tenant.kind == Some(value::Kind::StringValue(TENANT_A.to_owned()))
            })
        });
    suite.record(
        "wait_true_mutation_ack",
        acked && visible,
        format!("acked={acked} readback={}/4", readback.result.len()),
    );
    suite.record(
        "strong_write_ordering",
        acked && visible,
        "wait=true + WriteOrdering::Strong acknowledged and immediately readable".to_owned(),
    );
    Ok(())
}

async fn probe_strict_negatives(suite: &mut Suite) -> Result<(), LiveError> {
    let unindexed = Filter {
        must: vec![keyword_condition(FIELD_UNINDEXED, "code_unit")],
        ..Default::default()
    };
    let retrieve_rejected = suite
        .client
        .query(QueryPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            query: Some(Query::new_nearest(VectorInput::new_sparse(
                vec![0_u32],
                vec![1.0_f32],
            ))),
            using: Some(VECTOR_CODE.to_owned()),
            filter: Some(unindexed.clone()),
            limit: Some(10),
            ..Default::default()
        })
        .await
        .is_err();
    suite.record(
        "strict_unindexed_retrieve_rejected",
        retrieve_rejected,
        "filter on never-indexed unit_kind".to_owned(),
    );
    let update_rejected = suite
        .client
        .delete_points(DeletePoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            wait: Some(true),
            points: Some(PointsSelector {
                points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Filter(
                    unindexed,
                )),
            }),
            ordering: Some(strong_ordering()),
            ..Default::default()
        })
        .await
        .is_err();
    suite.record(
        "strict_unindexed_update_rejected",
        update_rejected,
        "delete-by-filter on never-indexed unit_kind".to_owned(),
    );
    Ok(())
}

async fn probe_signed_range(suite: &mut Suite) -> Result<(), LiveError> {
    let lower = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(Filter {
                must: vec![range_condition(
                    FIELD_FROM,
                    None,
                    Some(exact_f64(EPOCH_MIN)?),
                )],
                ..Default::default()
            }),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let upper = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(Filter {
                must: vec![range_condition(
                    FIELD_UNTIL,
                    Some(exact_f64(EPOCH_MAX)?),
                    None,
                )],
                ..Default::default()
            }),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let lower_count = lower.result.as_ref().map(|result| result.count);
    let upper_count = upper.result.as_ref().map(|result| result.count);
    suite.record(
        "signed_i64_epoch_range",
        lower_count == Some(1) && upper_count == Some(1),
        format!("from<=MIN count={lower_count:?} until>=MAX count={upper_count:?}"),
    );
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct ScoredSnapshot {
    id: String,
    score: f32,
}

fn snapshot_id(id: Option<&PointId>) -> String {
    match id.and_then(|point| point.point_id_options.as_ref()) {
        Some(point_id::PointIdOptions::Num(number)) => number.to_string(),
        Some(point_id::PointIdOptions::Uuid(uuid)) => uuid.clone(),
        None => "<missing>".to_owned(),
    }
}

async fn query_tenant_a(
    suite: &Suite,
    term: u32,
    with_idf_corpus: bool,
) -> Result<Vec<ScoredSnapshot>, LiveError> {
    let params = SearchParams {
        exact: Some(true),
        idf: with_idf_corpus
            .then(base_filter)
            .transpose()?
            .map(|corpus| IdfParams {
                corpus: Some(corpus),
            }),
        ..Default::default()
    };
    let response = suite
        .client
        .query(QueryPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            query: Some(Query::new_nearest(VectorInput::new_sparse(
                vec![term],
                vec![1.0_f32],
            ))),
            using: Some(VECTOR_CODE.to_owned()),
            filter: Some(base_filter()?),
            params: Some(params),
            limit: Some(10),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    Ok(response
        .result
        .into_iter()
        .map(|scored| ScoredSnapshot {
            id: snapshot_id(scored.id.as_ref()),
            score: scored.score,
        })
        .collect())
}

async fn probe_independent_idf(suite: &mut Suite) -> Result<(), LiveError> {
    // Pre-flight: the pure gate admits exactly the delegated shape the suite
    // executes (TF-only vectors, Qdrant-side idf modifier, shared plan).
    let plan = base_eligibility();
    let gate_ok = admit_independent_idf(&IndependentIdfProfile {
        vector_name: VECTOR_CODE.to_owned(),
        local_idf_factor: 1.0,
        local_statistics_present: false,
        qdrant_modifier_idf: true,
        retrieval_eligibility: plan.clone(),
        idf_corpus_eligibility: plan,
    })
    .is_ok();
    let global_before = query_tenant_a(suite, 0, false).await?;
    let scoped_before = query_tenant_a(suite, 0, true).await?;
    let same_population = global_before == scoped_before;
    suite.log.push(format!(
        "IDF pre-insert global={global_before:?} scoped={scoped_before:?} gate={gate_ok}"
    ));
    // Forbidden population: six tenant-b documents sharing term 0. They must
    // move the global IDF denominators but never the tenant-a corpus.
    let forbidden: Vec<PointStruct> = (3..=8)
        .map(|id| point(id, TENANT_B, 10, None, vec![(0, 1.0)], vec![(100, 1.0)]))
        .collect();
    let inserted = suite
        .client
        .upsert_points(UpsertPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            wait: Some(true),
            ordering: Some(strong_ordering()),
            points: forbidden,
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    if !inserted
        .result
        .as_ref()
        .is_some_and(|result| update_completed(result.status))
    {
        suite.record(
            "independent_idf_population_filter",
            false,
            "forbidden-population insert not acknowledged".to_owned(),
        );
        return Ok(());
    }
    let global_after = query_tenant_a(suite, 0, false).await?;
    let scoped_after = query_tenant_a(suite, 0, true).await?;
    let noninterference = scoped_before == scoped_after
        && scoped_after
            .iter()
            .all(|snapshot| snapshot.score.is_finite());
    let discrimination = global_before != global_after && global_after != scoped_after;
    suite.record(
        "independent_idf_population_filter",
        gate_ok && same_population && noninterference && discrimination,
        format!(
            "scoped_stable={noninterference} global_moved={discrimination} \
             global_after={global_after:?} scoped_after={scoped_after:?}"
        ),
    );
    Ok(())
}

async fn probe_sparse_modifier(suite: &mut Suite) -> Result<(), LiveError> {
    // Term 1 occurs in 2 of 10 documents, term 0 in 9 of 10: with the idf
    // modifier the rarer term must score higher on the document holding both.
    let rare = query_tenant_a(suite, 1, true).await?;
    let common = query_tenant_a(suite, 0, true).await?;
    let rare_top = rare.first().map(|snapshot| snapshot.id.clone());
    let rare_score = rare
        .iter()
        .find(|snapshot| snapshot.id == "1")
        .map(|snapshot| snapshot.score);
    let common_score = common
        .iter()
        .find(|snapshot| snapshot.id == "1")
        .map(|snapshot| snapshot.score);
    let rarer_scores_higher = match (rare_score, common_score) {
        (Some(rare_score), Some(common_score)) => {
            rare_score.is_finite() && common_score.is_finite() && rare_score > common_score
        }
        _ => false,
    };
    let passed = matches!(rare_top.as_deref(), Some("1" | UUID_POINT)) && rarer_scores_higher;
    suite.record(
        "sparse_idf_modifier",
        passed,
        format!("t1 top={rare_top:?} score_p1(t1)={rare_score:?} score_p1(t0)={common_score:?}"),
    );
    Ok(())
}

async fn probe_missing_upper_bound(suite: &mut Suite) -> Result<(), LiveError> {
    let open = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(base_filter()?),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let closed = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(Filter {
                must: vec![
                    keyword_condition(FIELD_TENANT, TENANT_A),
                    range_condition(FIELD_UNTIL, None, Some(exact_f64(VISIBLE_EPOCH_I64)?)),
                ],
                ..Default::default()
            }),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let open_count = open.result.as_ref().map(|result| result.count);
    let closed_count = closed.result.as_ref().map(|result| result.count);
    // Tenant-a holds points 1 (missing upper bound), 2 (until 50), 9 (until
    // EPOCH_MAX) and the UUID point (missing): all four pass the open-ended
    // filter, none passes the closed one.
    suite.record(
        "missing_valid_until_open_end",
        open_count == Some(4) && closed_count == Some(0),
        format!("must_not(until<=42)={open_count:?} must(until<=42)={closed_count:?}"),
    );
    Ok(())
}

async fn probe_count_and_readback(suite: &mut Suite) -> Result<(), LiveError> {
    let count = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(base_filter()?),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let readback = suite
        .client
        .get_points(GetPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            ids: vec![
                num_point_id(1),
                num_point_id(2),
                num_point_id(9),
                PointId {
                    point_id_options: Some(point_id::PointIdOptions::Uuid(UUID_POINT.to_owned())),
                },
            ],
            with_payload: Some(true.into()),
            with_vectors: Some(true.into()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let mut found: Vec<String> = readback
        .result
        .iter()
        .map(|point| snapshot_id(point.id.as_ref()))
        .collect();
    found.sort();
    let payload_ok = readback.result.iter().all(|point| {
        point.payload.get(FIELD_TENANT).is_some_and(|tenant| {
            tenant.kind == Some(value::Kind::StringValue(TENANT_A.to_owned()))
        }) && point
            .payload
            .get(FIELD_FROM)
            .is_some_and(|from| matches!(from.kind, Some(value::Kind::IntegerValue(_))))
    });
    let unknown = suite
        .client
        .get_points(GetPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            ids: vec![num_point_id(777)],
            with_payload: Some(false.into()),
            with_vectors: Some(false.into()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let passed = count.result.as_ref().map(|result| result.count) == Some(4)
        && found == ["1", "2", "550e8400-e29b-41d4-a716-446655440000", "9"]
        && payload_ok
        && unknown.result.is_empty();
    suite.record(
        "exact_count_and_readback",
        passed,
        format!(
            "count={:?} ids={found:?} unknown_empty={}",
            count.result.as_ref().map(|result| result.count),
            unknown.result.is_empty()
        ),
    );
    // Exact-ID reclaim on the live path: delete point 2, prove the eligible
    // count drops by exactly one.
    let deleted = suite
        .client
        .delete_points(DeletePoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            wait: Some(true),
            points: Some(PointsSelector {
                points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Points(
                    PointsIdsList {
                        ids: vec![num_point_id(2)],
                    },
                )),
            }),
            ordering: Some(strong_ordering()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    if !deleted
        .result
        .as_ref()
        .is_some_and(|result| update_completed(result.status))
    {
        return Err(LiveError::ProbeFailed {
            probe: "exact_count_and_readback",
        });
    }
    let recount = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(base_filter()?),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    suite.log.push(format!(
        "RECLAIM point=2 post_count={:?}",
        recount.result.as_ref().map(|result| result.count)
    ));
    if recount.result.as_ref().map(|result| result.count) != Some(3) {
        return Err(LiveError::ProbeFailed {
            probe: "exact_count_and_readback",
        });
    }
    Ok(())
}

async fn probe_schema_digest(suite: &mut Suite) -> Result<(), LiveError> {
    let info = suite
        .client
        .collection_info(QUALIFICATION_COLLECTION)
        .await
        .map_err(|_| LiveError::TransportFailed)?
        .result
        .ok_or(LiveError::TransportFailed)?;
    let mut digest_parts = Vec::new();
    let mut sparse_ok = false;
    let mut shard_ok = false;
    if let Some(params) = info
        .config
        .as_ref()
        .and_then(|config| config.params.as_ref())
    {
        digest_parts.push(format!("shards={}", params.shard_number));
        shard_ok = params.shard_number == 1;
        if let Some(sparse) = params.sparse_vectors_config.as_ref() {
            let mut names: Vec<(&str, i32)> = sparse
                .map
                .iter()
                .map(|(name, vector_params)| {
                    (name.as_str(), vector_params.modifier.unwrap_or_default())
                })
                .collect();
            names.sort_unstable();
            digest_parts.push(format!("sparse={names:?}"));
            sparse_ok = names
                == [
                    (VECTOR_CODE, Modifier::Idf as i32),
                    (VECTOR_TEXT, Modifier::Idf as i32),
                ];
        }
    }
    let mut payload_ok = true;
    for field in [FIELD_TENANT, FIELD_ACCESS, FIELD_FROM, FIELD_UNTIL] {
        if info.payload_schema.contains_key(field) {
            digest_parts.push(format!("index:{field}=present"));
        } else {
            payload_ok = false;
        }
    }
    let (strict_enabled, retrieve_open, update_open) = info
        .config
        .as_ref()
        .and_then(|config| config.strict_mode_config.as_ref())
        .map(|strict| {
            (
                strict.enabled.unwrap_or_default(),
                strict.unindexed_filtering_retrieve.unwrap_or(true),
                strict.unindexed_filtering_update.unwrap_or(true),
            )
        })
        .unwrap_or_default();
    digest_parts.push(format!(
        "strict={strict_enabled}/{retrieve_open}/{update_open}"
    ));
    let status = CollectionStatus::try_from(info.status)
        .map_or_else(|_| info.status.to_string(), |parsed| format!("{parsed:?}"));
    digest_parts.push(format!("status={status}"));
    let digest = digest_parts.join("|");
    suite.record(
        "schema_digest_equality",
        sparse_ok && payload_ok && shard_ok && strict_enabled && !retrieve_open && !update_open,
        digest,
    );
    Ok(())
}

/// Verifies the compiled-in client pin.
///
/// Compares the registry/lockfile record against the pin before any live
/// call. Callers pass the checksum recorded in
/// `qualification/qdrant/artifact.toml`; a mismatch fails without touching
/// the network.
#[must_use]
pub fn verify_compiled_client(source_checksum: &str) -> Option<QualificationError> {
    verify_client(&ObservedClient {
        crate_name: "qdrant-client".to_owned(),
        version: QUALIFIED_CLIENT_VERSION.to_owned(),
        source_checksum: source_checksum.to_owned(),
    })
    .err()
}
