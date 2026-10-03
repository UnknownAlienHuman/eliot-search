//! One focused Windows lifecycle proof against the exact qualified executable.
//!
//! It uses a short disposable root under `C:\qtmp`, an opaque test lease,
//! and OS-assigned loopback ports. A failed assertion preserves its scratch
//! tree; cleanup occurs only after the native shutdown receipt is returned.

#![cfg(windows)]

use std::net::TcpListener;
use std::num::{NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use search_contracts::{Blake3Digest32, InstallationIncarnationId, OpaqueId};
use search_qdrant_supervisor::{
    ArtifactArchitecture, ArtifactCandidate, ArtifactQualificationManifest, ExecutableExpectation,
    LaunchPlan, LoopbackEndpoint, LoopbackHost, OwnedChild, ProcessConfig, QdrantOwnerFence,
    QdrantSecretLease, SupervisorError, check_loopback_port_free, parse_loopback_host,
    qualify_artifact, validate_process_config,
};

const QUALIFIED_EXE: &str = "C:\\Tools\\Qdrant\\1.19.0\\qdrant.exe";
const QUALIFIED_VERSION: &str = "1.19.0";
const LIVE_API_KEY: &[u8] = b"t23-live-loopback-key-7f3a9c2e";
const SECRET_PURPOSE: Blake3Digest32 = Blake3Digest32::from_bytes([0xE1; 32]);
const SECRET_REFERENCE: Blake3Digest32 = Blake3Digest32::from_bytes([0xE0; 32]);

struct LiveLease {
    bytes: Vec<u8>,
    installation: InstallationIncarnationId,
    expires_at: NonZeroU64,
}

impl Drop for LiveLease {
    fn drop(&mut self) {
        self.bytes.fill(0);
        std::hint::black_box(self.bytes.as_mut_ptr());
    }
}

impl QdrantSecretLease for LiveLease {
    fn secret_reference_digest(&self) -> Blake3Digest32 {
        SECRET_REFERENCE
    }

    fn installation_incarnation_id(&self) -> InstallationIncarnationId {
        self.installation
    }

    fn purpose_digest(&self) -> Blake3Digest32 {
        SECRET_PURPOSE
    }

    fn expires_at_tick(&self) -> NonZeroU64 {
        self.expires_at
    }

    fn with_secret_bytes<R>(&self, use_bytes: impl FnOnce(&[u8]) -> R) -> R {
        use_bytes(&self.bytes)
    }
}

fn unique_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_nanos())
}

fn two_os_assigned_ports() -> (u16, u16) {
    let http = TcpListener::bind("127.0.0.1:0").expect("OS-assigned HTTP port");
    let grpc = TcpListener::bind("127.0.0.1:0").expect("OS-assigned gRPC port");
    let ports = (
        http.local_addr().expect("HTTP address").port(),
        grpc.local_addr().expect("gRPC address").port(),
    );
    drop((http, grpc));
    assert_ne!(ports.0, 0);
    assert_ne!(ports.1, 0);
    assert_ne!(ports.0, ports.1);
    ports
}

fn owner_fence(epoch: u64) -> QdrantOwnerFence {
    QdrantOwnerFence {
        data_root_id: search_contracts::DataRootId::from_bytes([0x31; 16]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([0xA0; 16]),
        owner_epoch: search_contracts::OwnerEpoch::new(epoch).expect("nonzero epoch"),
    }
}

fn live_artifact() -> search_qdrant_supervisor::QualifiedArtifact {
    let expectation = ExecutableExpectation::qualified_default(PathBuf::from(QUALIFIED_EXE))
        .expect("pinned artifact expectation");
    let verified =
        search_qdrant_supervisor::verify_executable_identity(&expectation, Duration::from_secs(60))
            .expect("pinned executable identity");
    assert_eq!(verified.version(), QUALIFIED_VERSION);
    assert_eq!(verified.bytes(), 84_184_576);
    let artifact_digest = search_contracts::ArtifactDigest::from_bytes(verified.sha256_bytes());
    let candidate = ArtifactCandidate {
        file_identity_digest: Blake3Digest32::from_bytes(*verified.sha256().as_bytes()),
        sha256: *verified.sha256(),
        artifact_digest,
        version: verified.version().to_owned(),
        build_identity: "74f3e85b".to_owned(),
        architecture: ArtifactArchitecture::X86_64Windows,
    };
    let manifest = ArtifactQualificationManifest {
        expected_sha256: *verified.sha256(),
        expected_artifact_digest: artifact_digest,
        expected_version: QUALIFIED_VERSION.to_owned(),
        expected_build_identity: "74f3e85b".to_owned(),
        expected_architecture: ArtifactArchitecture::X86_64Windows,
        source_receipt: search_contracts::ReceiptRef::new("t23-live-source").unwrap(),
        license_receipt: search_contracts::ReceiptRef::new("t23-live-license").unwrap(),
        probe_manifest_digest: Blake3Digest32::from_bytes([0xC0; 32]),
    };
    qualify_artifact(candidate, &manifest, Blake3Digest32::from_bytes([0xC1; 32]))
        .expect("exact artifact qualification fixture")
}

struct ScratchRoot(PathBuf);

impl ScratchRoot {
    fn create() -> Self {
        let base = Path::new("C:\\qtmp");
        std::fs::create_dir_all(base).expect("short scratch base exists or can be created");
        let path = base.join(format!("q-{}-{}", std::process::id(), unique_nanos()));
        std::fs::create_dir(&path).expect("unique scratch root");
        std::fs::create_dir_all(path.join("fixture\\nested")).expect("seed nested tree");
        std::fs::write(
            path.join("fixture\\nested\\seed.bin"),
            b"reopen-existing-tree",
        )
        .expect("seed existing file");
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().encode_utf16().count() <= 96);
        Self(path)
    }

    fn remove_after_verified_shutdown(&self) -> std::io::Result<()> {
        std::fs::remove_dir_all(&self.0)
    }
}

fn live_lease(fence: QdrantOwnerFence) -> LiveLease {
    LiveLease {
        bytes: LIVE_API_KEY.to_vec(),
        installation: fence.installation_incarnation_id,
        expires_at: NonZeroU64::new(u64::MAX / 2).expect("nonzero expiry"),
    }
}

fn prepare_live_launch(
    scratch: &ScratchRoot,
    epoch: u64,
    http_port: u16,
    grpc_port: u16,
) -> (LaunchPlan, LiveLease, LoopbackHost, u16, u16) {
    let host = parse_loopback_host("127.0.0.1").expect("literal loopback");
    check_loopback_port_free(&host, http_port).expect("HTTP endpoint is unowned");
    check_loopback_port_free(&host, grpc_port).expect("gRPC endpoint is unowned");

    let fence = owner_fence(epoch);
    let lease = live_lease(fence);
    let artifact = live_artifact();
    let config = ProcessConfig {
        owner_fence: fence,
        data_directory_digest: Blake3Digest32::from_bytes([0xD0; 32]),
        endpoint: LoopbackEndpoint {
            endpoint_digest: Blake3Digest32::from_bytes([0xD1; 32]),
            port: NonZeroU32::new(u32::from(http_port)).expect("nonzero HTTP port"),
        },
        bind_is_loopback: true,
        single_node: true,
        startup_timeout_ticks: NonZeroU64::new(1_200_000).unwrap(),
        shutdown_timeout_ticks: NonZeroU64::new(300_000).unwrap(),
        restart_window_ticks: NonZeroU64::new(600_000).unwrap(),
        max_restarts_per_window: 2,
        config_digest: Blake3Digest32::from_bytes([0xD2; 32]),
    };
    let qualified = validate_process_config(config, &artifact, &lease, NonZeroU64::MIN)
        .expect("loopback single-node config bound to lease");
    let plan = LaunchPlan::new(
        OpaqueId::new("t23-live-cycle").unwrap(),
        fence,
        artifact,
        qualified,
        &lease,
        SECRET_PURPOSE,
        PathBuf::from(QUALIFIED_EXE),
        84_184_576,
        scratch.0.clone(),
        host,
        http_port,
        grpc_port,
        Duration::from_secs(120),
        Duration::from_secs(30),
        NonZeroU64::MIN,
    )
    .expect("launch plan binds config, lease, root and both ports");
    (plan, lease, host, http_port, grpc_port)
}

#[test]
fn qualified_identity_is_verified_before_start() {
    let expectation = ExecutableExpectation::qualified_default(PathBuf::from(QUALIFIED_EXE))
        .expect("pinned expectation");
    let verified =
        search_qdrant_supervisor::verify_executable_identity(&expectation, Duration::from_secs(60))
            .expect("pinned artifact");
    assert_eq!(verified.version(), QUALIFIED_VERSION);
    assert_eq!(verified.bytes(), 84_184_576);
}

#[test]
fn wrong_version_against_real_executable_is_refused() {
    let expectation = ExecutableExpectation::custom(
        PathBuf::from(QUALIFIED_EXE),
        search_contracts::Sha256Digest32::parse_hex(
            search_qdrant_supervisor::QUALIFIED_EXE_SHA256_HEX,
        )
        .unwrap(),
        84_184_576,
        "9.9.9".to_owned(),
    )
    .unwrap();
    let result =
        search_qdrant_supervisor::verify_executable_identity(&expectation, Duration::from_secs(60));
    assert_eq!(
        result.unwrap_err(),
        SupervisorError::ArtifactVersionMismatch
    );
}

#[test]
fn owned_qdrant_spawn_identity_acl_job_and_shutdown_are_read_back() {
    let scratch = ScratchRoot::create();
    let (http_port, grpc_port) = two_os_assigned_ports();
    let (plan, lease, host, http_port, grpc_port) =
        prepare_live_launch(&scratch, 9001, http_port, grpc_port);
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let outcome =
        search_qdrant_supervisor::spawn_qualified(&plan, &lease, NonZeroU64::MIN, &cancel)
            .expect("native launch attempted without an owned-process attach path");
    let (mut owned, unknown_reason) = OwnedChild::from_spawn_outcome(outcome);
    if let Some(reason) = unknown_reason {
        if owned.verify_identity().is_ok() {
            let _ = owned.shutdown_bounded(Instant::now() + Duration::from_secs(30));
        } else {
            let _ = owned.recover_ambiguous_start(Instant::now() + Duration::from_secs(30));
        }
        panic!("native spawn outcome remained unknown: {reason}");
    }
    assert!(owned.containment_report().is_contained());
    let first_identity = owned.verify_identity().expect("handle-backed identity");
    assert_eq!(first_identity.endpoint().host(), host);
    assert_eq!(first_identity.endpoint().http_port().get(), http_port);
    assert_eq!(first_identity.endpoint().grpc_port().get(), grpc_port);
    assert_eq!(first_identity.owner_fence(), *plan.owner_fence());
    assert_eq!(
        first_identity.secret_binding().secret_reference_digest(),
        SECRET_REFERENCE
    );
    assert_eq!(
        first_identity
            .secret_binding()
            .installation_incarnation_id(),
        plan.owner_fence().installation_incarnation_id
    );
    assert_eq!(
        first_identity.secret_binding().purpose_digest(),
        SECRET_PURPOSE
    );

    cancel.store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(
        owned
            .wait_ready(
                &lease,
                NonZeroU64::MIN,
                Instant::now() + Duration::from_secs(5),
                &cancel,
            )
            .unwrap_err(),
        SupervisorError::StartupOutcomeUnknown
    );
    assert_eq!(
        owned
            .verify_identity()
            .expect("canceled startup retains native guard"),
        first_identity
    );
    cancel.store(false, std::sync::atomic::Ordering::Release);
    let readiness = owned
        .wait_ready(
            &lease,
            NonZeroU64::MIN,
            Instant::now() + Duration::from_secs(120),
            &cancel,
        )
        .expect("authenticated loopback readiness and anonymous auth denial");
    assert_eq!(readiness.identity(), first_identity);
    assert_eq!(owned.verify_identity().unwrap(), first_identity);

    let receipt = owned
        .shutdown_bounded(Instant::now() + Duration::from_secs(30))
        .expect("bounded Job shutdown and endpoint absence");
    assert_eq!(receipt.identity(), first_identity);
    assert!(check_loopback_port_free(&host, http_port).is_ok());
    assert!(check_loopback_port_free(&host, grpc_port).is_ok());
    drop(owned);

    // Re-open the same nonempty private tree under a new owner epoch. This
    // exercises handle-based reparse and ACL checks against existing children.
    let (second_plan, second_lease, _, _, _) =
        prepare_live_launch(&scratch, 9002, http_port, grpc_port);
    let second_outcome = search_qdrant_supervisor::spawn_qualified(
        &second_plan,
        &second_lease,
        NonZeroU64::MIN,
        &cancel,
    )
    .expect("restart on a previously populated tree");
    let (mut second_owned, second_reason) = OwnedChild::from_spawn_outcome(second_outcome);
    if let Some(reason) = second_reason {
        if second_owned.verify_identity().is_ok() {
            let _ = second_owned.shutdown_bounded(Instant::now() + Duration::from_secs(30));
        } else {
            let _ = second_owned.recover_ambiguous_start(Instant::now() + Duration::from_secs(30));
        }
        panic!("reopen spawn outcome remained unknown: {reason}");
    }
    let second_identity = second_owned
        .verify_identity()
        .expect("reopened tree process identity");
    second_owned
        .wait_ready(
            &second_lease,
            NonZeroU64::MIN,
            Instant::now() + Duration::from_secs(120),
            &cancel,
        )
        .expect("reopened Qdrant tree becomes ready");
    let second_receipt = second_owned
        .shutdown_bounded(Instant::now() + Duration::from_secs(30))
        .expect("reopened process tree is boundedly stopped");
    assert_eq!(second_receipt.identity(), second_identity);
    assert!(check_loopback_port_free(&host, http_port).is_ok());
    assert!(check_loopback_port_free(&host, grpc_port).is_ok());
    drop(second_owned);
    scratch
        .remove_after_verified_shutdown()
        .expect("clean only after both successful native shutdown receipts");
}
