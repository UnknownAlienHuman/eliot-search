//! Live owned-process test against the exact qualified Qdrant executable.
//!
//! Runs `C:\Tools\Qdrant\1.19.0\qdrant.exe` (version 1.19.0, pinned SHA-256)
//! on disposable storage with ephemeral loopback ports. The test fails
//! closed when the qualified executable is absent: a missing artifact is a
//! hard error, never a silent skip.
//!
//! Windows-only: process control relies on inbox `cmd.exe` helpers and the
//! `netstat` loopback assertion.

#![cfg(windows)]

use std::net::TcpListener;
use std::num::{NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use search_contracts::{Blake3Digest32, OpaqueId};
use search_qdrant_supervisor::{
    ArtifactArchitecture, ArtifactCandidate, ArtifactQualificationManifest, ContainmentEvidence,
    ExecutableExpectation, LaunchPlan, LoopbackEndpoint, LoopbackHost, OwnedChild, ProcessConfig,
    ProcessIdentity, ProcessReadiness, QdrantOwnerFence, QdrantSupervisor, SecretLeaseEvidence,
    SecretMaterial, SupervisorError, check_loopback_port_free, parse_loopback_host,
    qualify_artifact, validate_process_config,
};

const QUALIFIED_EXE: &str = "C:\\Tools\\Qdrant\\1.19.0\\qdrant.exe";
const QUALIFIED_VERSION: &str = "1.19.0";
const LIVE_API_KEY: &str = "t23-live-loopback-key-7f3a9c2e";

fn qualified_expectation() -> ExecutableExpectation {
    ExecutableExpectation::qualified_default(PathBuf::from(QUALIFIED_EXE)).unwrap()
}

fn unique_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_nanos())
}

fn free_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    assert!(port != 0);
    port
}

fn owner_fence(data_root_seed: u8, epoch: u64) -> QdrantOwnerFence {
    QdrantOwnerFence {
        data_root_id: search_contracts::DataRootId::from_bytes([data_root_seed; 16]),
        installation_incarnation_id: search_contracts::InstallationIncarnationId::from_bytes([
            0xA0 + data_root_seed,
            0xA1,
            0xA2,
            0xA3,
            0xA4,
            0xA5,
            0xA6,
            0xA7,
            0xA8,
            0xA9,
            0xAA,
            0xAB,
            0xAC,
            0xAD,
            0xAE,
            0xAF,
        ]),
        owner_epoch: search_contracts::OwnerEpoch::new(epoch).unwrap(),
    }
}

fn live_artifact() -> search_qdrant_supervisor::QualifiedArtifact {
    let expectation = qualified_expectation();
    let verified =
        search_qdrant_supervisor::verify_executable_identity(&expectation, Duration::from_secs(60))
            .unwrap();
    let candidate = ArtifactCandidate {
        file_identity_digest: Blake3Digest32::from_bytes(*verified.sha256().as_bytes()),
        sha256: *verified.sha256(),
        artifact_digest: search_contracts::ArtifactDigest::from_bytes(verified.sha256_bytes()),
        version: verified.version().to_owned(),
        build_identity: "74f3e85b".to_owned(),
        architecture: ArtifactArchitecture::X86_64Windows,
    };
    let manifest = ArtifactQualificationManifest {
        expected_sha256: *verified.sha256(),
        expected_artifact_digest: search_contracts::ArtifactDigest::from_bytes(
            verified.sha256_bytes(),
        ),
        expected_version: QUALIFIED_VERSION.to_owned(),
        expected_build_identity: "74f3e85b".to_owned(),
        expected_architecture: ArtifactArchitecture::X86_64Windows,
        source_receipt: search_contracts::ReceiptRef::new("t23-live-source").unwrap(),
        license_receipt: search_contracts::ReceiptRef::new("t23-live-license").unwrap(),
        probe_manifest_digest: Blake3Digest32::from_bytes([0xC0; 32]),
    };
    qualify_artifact(candidate, &manifest, Blake3Digest32::from_bytes([0xC1; 32])).unwrap()
}

fn live_config(
    fence: QdrantOwnerFence,
    artifact: &search_qdrant_supervisor::QualifiedArtifact,
    http_port: u16,
) -> search_qdrant_supervisor::QualifiedProcessConfig {
    let config = ProcessConfig {
        owner_fence: fence,
        data_directory_digest: Blake3Digest32::from_bytes([0xD0; 32]),
        endpoint: LoopbackEndpoint {
            endpoint_digest: Blake3Digest32::from_bytes([0xD1; 32]),
            port: NonZeroU32::new(u32::from(http_port)).unwrap(),
        },
        bind_is_loopback: true,
        single_node: true,
        startup_timeout_ticks: NonZeroU64::new(1_200_000).unwrap(),
        shutdown_timeout_ticks: NonZeroU64::new(300_000).unwrap(),
        restart_window_ticks: NonZeroU64::new(600_000).unwrap(),
        max_restarts_per_window: 2,
        config_digest: Blake3Digest32::from_bytes([0xD2; 32]),
    };
    let lease = SecretLeaseEvidence {
        secret_reference_digest: Blake3Digest32::from_bytes([0xE0; 32]),
        installation_incarnation_id: fence.installation_incarnation_id,
        purpose_digest: Blake3Digest32::from_bytes([0xE1; 32]),
        expires_at_tick: NonZeroU64::new(u64::MAX / 2).unwrap(),
    };
    validate_process_config(config, artifact, lease, NonZeroU64::new(1).unwrap()).unwrap()
}

struct TempDataRoot {
    path: PathBuf,
}

impl TempDataRoot {
    fn create(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "eliot-t23-live-{label}-{}-{}",
            std::process::id(),
            unique_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDataRoot {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

fn readyz_allows_anonymous_but_collections_require_key(http_port: u16) {
    let base = format!("127.0.0.1:{http_port}");
    let readyz = http_get(&base, "/readyz", None, Duration::from_secs(5)).unwrap();
    assert_eq!(readyz.status, 200, "readyz must report ready");
    let denied = http_get(&base, "/collections", None, Duration::from_secs(5)).unwrap();
    assert_eq!(
        denied.status, 401,
        "collections without API key must be denied, got {}",
        denied.status
    );
}

struct HttpStatus {
    status: u16,
}

fn http_get(
    authority: &str,
    path: &str,
    api_key: Option<&str>,
    timeout: Duration,
) -> std::io::Result<HttpStatus> {
    use std::fmt::Write as _;
    use std::io::{Read as _, Write as _};
    let mut stream = std::net::TcpStream::connect_timeout(
        &authority.parse().map_err(std::io::Error::other)?,
        timeout,
    )?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let mut request = format!("GET {path} HTTP/1.0\r\nHost: {authority}\r\nConnection: close\r\n");
    if let Some(key) = api_key {
        write!(request, "api-key: {key}\r\n").unwrap();
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes())?;
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(read) if read > 0 => {
                raw.extend_from_slice(&chunk[..read]);
                if raw.len() > 65_536 {
                    break;
                }
            }
            _ => break,
        }
    }
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);
    Ok(HttpStatus { status })
}

fn assert_listening_is_loopback_only(http_port: u16, child_pid: u32) {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    let token = format!(":{http_port}");
    let mut matched = 0_usize;
    for line in text.lines() {
        if !line.contains(&token) {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        let local = fields[1];
        let pid: u32 = fields[4].parse().unwrap_or(0);
        if pid != child_pid {
            continue;
        }
        matched += 1;
        assert!(
            local.starts_with("127.0.0.1:") || local.starts_with("[::1]:"),
            "Qdrant must bind loopback only, observed local endpoint {local}"
        );
    }
    assert!(
        matched > 0,
        "expected at least one loopback listener for port {http_port} owned by pid {child_pid}"
    );
}

#[test]
fn qualified_identity_is_verified_before_start() {
    assert!(
        Path::new(QUALIFIED_EXE).is_file(),
        "qualified executable must exist at {QUALIFIED_EXE}; a missing artifact fails closed"
    );
    let verified = search_qdrant_supervisor::verify_executable_identity(
        &qualified_expectation(),
        Duration::from_secs(60),
    )
    .unwrap();
    assert_eq!(verified.version(), QUALIFIED_VERSION);
    assert_eq!(verified.bytes(), 84_184_576);
}

#[test]
fn wrong_version_against_real_executable_is_refused() {
    let mut expectation = qualified_expectation();
    expectation.set_expected_version_for_tests("9.9.9".to_owned());
    let result =
        search_qdrant_supervisor::verify_executable_identity(&expectation, Duration::from_secs(60));
    assert_eq!(
        result.unwrap_err(),
        SupervisorError::ArtifactVersionMismatch
    );
}

struct LiveLaunch {
    plan: LaunchPlan,
    secret: SecretMaterial,
    fence: QdrantOwnerFence,
    host: LoopbackHost,
    http_port: u16,
    _storage: TempDataRoot,
}

fn prepare_live_launch(label: &str, epoch: u64) -> LiveLaunch {
    let storage = TempDataRoot::create(label);
    let http_port = free_loopback_port();
    let mut grpc_port = free_loopback_port();
    while grpc_port == http_port {
        grpc_port = free_loopback_port();
    }
    let host = parse_loopback_host("127.0.0.1").unwrap();
    check_loopback_port_free(&host, http_port).unwrap();
    check_loopback_port_free(&host, grpc_port).unwrap();

    let fence = owner_fence(0x31, epoch);
    let artifact = live_artifact();
    let config = live_config(fence, &artifact, http_port);
    let lease = SecretLeaseEvidence {
        secret_reference_digest: Blake3Digest32::from_bytes([0xE0; 32]),
        installation_incarnation_id: fence.installation_incarnation_id,
        purpose_digest: Blake3Digest32::from_bytes([0xE1; 32]),
        expires_at_tick: NonZeroU64::new(u64::MAX / 2).unwrap(),
    };
    let secret = SecretMaterial::from_bytes(LIVE_API_KEY.as_bytes().to_vec()).unwrap();
    secret.validate_header_safe().unwrap();

    let plan = LaunchPlan::new(
        OpaqueId::new("t23-live-cycle").unwrap(),
        fence,
        artifact,
        config,
        lease,
        Blake3Digest32::from_bytes([0xE1; 32]),
        PathBuf::from(QUALIFIED_EXE),
        84_184_576,
        storage.path.clone(),
        host,
        http_port,
        grpc_port,
        Duration::from_secs(120),
        Duration::from_secs(30),
        ContainmentEvidence::explicit_uncontained_test_only(),
        NonZeroU64::new(1).unwrap(),
    )
    .unwrap();
    LiveLaunch {
        plan,
        secret,
        fence,
        host,
        http_port,
        _storage: storage,
    }
}

fn confirm_live_ready(
    supervisor: &mut QdrantSupervisor,
    owned: &OwnedChild,
    effect: &search_qdrant_supervisor::StartProcessEffect,
    fence: QdrantOwnerFence,
) -> ProcessIdentity {
    let identity = ProcessIdentity {
        process_id: owned.pid(),
        creation_marker: owned.spawn_unix_millis(),
        executable_file_digest: Blake3Digest32::from_bytes(*owned.verified().sha256().as_bytes()),
        artifact_digest: effect.artifact.candidate().artifact_digest,
        owner_fence: fence,
        endpoint: effect.config.config().endpoint,
    };
    let readiness = ProcessReadiness {
        identity,
        authenticated_health_ok: true,
        observed_config_digest: effect.config.config().config_digest,
        readiness_receipt: search_contracts::ReceiptRef::new("t23-live-ready").unwrap(),
    };
    supervisor.confirm_ready(&readiness).unwrap();
    assert!(matches!(
        supervisor.state(),
        search_qdrant_supervisor::SupervisorState::Ready { .. }
    ));
    identity
}

#[test]
fn owned_start_authenticated_health_and_bounded_stop() {
    assert!(
        Path::new(QUALIFIED_EXE).is_file(),
        "qualified executable must exist at {QUALIFIED_EXE}; a missing artifact fails closed"
    );
    let launch = prepare_live_launch("cycle", 9001);
    let LiveLaunch {
        plan,
        secret,
        fence,
        host,
        http_port,
        _storage,
    } = &launch;

    let mut supervisor = QdrantSupervisor::new();
    let effect = supervisor
        .prepare_start(
            OpaqueId::new("t23-live-cycle").unwrap(),
            plan.artifact().clone(),
            plan.config().clone(),
        )
        .unwrap();

    let spawned = search_qdrant_supervisor::spawn_qualified(plan, secret).unwrap_or_else(|error| {
        supervisor.abort_start().ok();
        panic!("qualified spawn must succeed, got {error}");
    });
    assert!(!spawned.containment_report().contained);
    let mut owned = OwnedChild::from(spawned);
    let child_pid = owned.pid().get();

    owned
        .wait_ready(
            host,
            *http_port,
            secret,
            Instant::now() + Duration::from_secs(120),
        )
        .unwrap_or_else(|error| {
            owned
                .terminate_bounded(Instant::now() + Duration::from_secs(30))
                .ok();
            panic!("qualified child must become ready, got {error}");
        });

    assert_listening_is_loopback_only(*http_port, child_pid);
    readyz_allows_anonymous_but_collections_require_key(*http_port);

    let identity = confirm_live_ready(&mut supervisor, &owned, &effect, *fence);

    let shutdown = supervisor
        .begin_shutdown(NonZeroU64::new(2).unwrap())
        .unwrap();
    let _ = shutdown;
    let code = owned
        .terminate_bounded(Instant::now() + Duration::from_secs(30))
        .unwrap();
    assert!(
        code.is_some(),
        "terminated child must report an exit code, got {code:?}"
    );
    supervisor
        .confirm_stopped(
            identity,
            !owned.is_running(),
            endpoint_absent(*host, *http_port),
        )
        .unwrap();
    assert!(matches!(
        supervisor.state(),
        search_qdrant_supervisor::SupervisorState::Stopped
    ));
    assert!(
        endpoint_absent(*host, *http_port),
        "no owned listener may remain after bounded stop"
    );
}

fn endpoint_absent(host: LoopbackHost, http_port: u16) -> bool {
    check_loopback_port_free(&host, http_port).is_ok()
}
