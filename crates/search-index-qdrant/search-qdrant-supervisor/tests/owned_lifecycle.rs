//! Plan validation for the native, lease-bound process wrapper.

use std::net::{TcpListener, TcpStream};
use std::num::{NonZeroU32, NonZeroU64};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use search_contracts::{Blake3Digest32, InstallationIncarnationId, OpaqueId};
use search_qdrant_supervisor::{
    ArtifactArchitecture, ArtifactCandidate, ArtifactQualificationManifest, ExecutableExpectation,
    LaunchPlan, LoopbackEndpoint, LoopbackHost, ProcessConfig, QdrantOwnerFence, QdrantSecretLease,
    SupervisorError, check_loopback_port_free, parse_loopback_host, qualify_artifact,
    validate_process_config,
};

const SECRET_PURPOSE: Blake3Digest32 = Blake3Digest32::from_bytes([0x10; 32]);

struct Lease {
    installation: InstallationIncarnationId,
    purpose: Blake3Digest32,
    expiry: NonZeroU64,
}

impl QdrantSecretLease for Lease {
    fn secret_reference_digest(&self) -> Blake3Digest32 {
        Blake3Digest32::from_bytes([0x11; 32])
    }

    fn installation_incarnation_id(&self) -> InstallationIncarnationId {
        self.installation
    }

    fn purpose_digest(&self) -> Blake3Digest32 {
        self.purpose
    }

    fn expires_at_tick(&self) -> NonZeroU64 {
        self.expiry
    }

    fn with_secret_bytes<R>(&self, use_bytes: impl FnOnce(&[u8]) -> R) -> R {
        use_bytes(b"contract-fixture-key-47")
    }
}

fn owner_fence() -> QdrantOwnerFence {
    QdrantOwnerFence {
        data_root_id: search_contracts::DataRootId::from_bytes([0x21; 16]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([0x22; 16]),
        owner_epoch: search_contracts::OwnerEpoch::new(47).unwrap(),
    }
}

fn artifact() -> search_qdrant_supervisor::QualifiedArtifact {
    let sha256 = search_contracts::Sha256Digest32::from_bytes([0x23; 32]);
    let digest = search_contracts::ArtifactDigest::from_bytes([0x24; 32]);
    let candidate = ArtifactCandidate {
        file_identity_digest: Blake3Digest32::from_bytes([0x25; 32]),
        sha256,
        artifact_digest: digest,
        version: "1.19.0".to_owned(),
        build_identity: "fixture-build".to_owned(),
        architecture: ArtifactArchitecture::X86_64Windows,
    };
    let manifest = ArtifactQualificationManifest {
        expected_sha256: sha256,
        expected_artifact_digest: digest,
        expected_version: "1.19.0".to_owned(),
        expected_build_identity: "fixture-build".to_owned(),
        expected_architecture: ArtifactArchitecture::X86_64Windows,
        source_receipt: search_contracts::ReceiptRef::new("fixture-source").unwrap(),
        license_receipt: search_contracts::ReceiptRef::new("fixture-license").unwrap(),
        probe_manifest_digest: Blake3Digest32::from_bytes([0x26; 32]),
    };
    qualify_artifact(candidate, &manifest, Blake3Digest32::from_bytes([0x27; 32])).unwrap()
}

fn lease(fence: QdrantOwnerFence) -> Lease {
    Lease {
        installation: fence.installation_incarnation_id,
        purpose: SECRET_PURPOSE,
        expiry: NonZeroU64::new(10_000).unwrap(),
    }
}

fn process_config(fence: QdrantOwnerFence, http_port: u16) -> ProcessConfig {
    ProcessConfig {
        owner_fence: fence,
        data_directory_digest: Blake3Digest32::from_bytes([0x30; 32]),
        endpoint: LoopbackEndpoint {
            endpoint_digest: Blake3Digest32::from_bytes([0x31; 32]),
            port: NonZeroU32::new(u32::from(http_port)).unwrap(),
        },
        bind_is_loopback: true,
        single_node: true,
        startup_timeout_ticks: NonZeroU64::new(1_200).unwrap(),
        shutdown_timeout_ticks: NonZeroU64::new(300).unwrap(),
        restart_window_ticks: NonZeroU64::new(600).unwrap(),
        max_restarts_per_window: 2,
        config_digest: Blake3Digest32::from_bytes([0x32; 32]),
    }
}

fn short_absolute_data_root() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from("C:\\qtmp\\supervisor-unit")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/tmp/supervisor-unit")
    }
}

fn plan_with_configured_http(
    configured_http_port: u16,
    launch_http_port: u16,
    grpc_port: u16,
) -> Result<LaunchPlan, SupervisorError> {
    let fence = owner_fence();
    let lease = lease(fence);
    let artifact = artifact();
    let config = validate_process_config(
        process_config(fence, configured_http_port),
        &artifact,
        &lease,
        NonZeroU64::new(1).unwrap(),
    )?;
    LaunchPlan::new(
        OpaqueId::new("supervisor-plan-fixture")
            .map_err(|_| SupervisorError::InvalidProcessConfig)?,
        fence,
        artifact,
        config,
        &lease,
        SECRET_PURPOSE,
        PathBuf::from("C:\\Tools\\Qdrant\\1.19.0\\qdrant.exe"),
        84_184_576,
        short_absolute_data_root(),
        parse_loopback_host("127.0.0.1")?,
        launch_http_port,
        grpc_port,
        Duration::from_secs(60),
        Duration::from_secs(20),
        NonZeroU64::new(1).unwrap(),
    )
}

fn plan(http_port: u16, grpc_port: u16) -> Result<LaunchPlan, SupervisorError> {
    plan_with_configured_http(http_port, http_port, grpc_port)
}

#[test]
fn launch_plan_binds_the_configured_http_port_and_selected_grpc_port() {
    let plan = plan(6333, 6334).expect("coherent loopback plan");
    assert_eq!(plan.endpoint_identity().host(), LoopbackHost::V4);
    assert_eq!(plan.endpoint_identity().http_port().get(), 6333);
    assert_eq!(plan.endpoint_identity().grpc_port().get(), 6334);
    assert_eq!(
        plan.endpoint_identity().endpoint_digest(),
        Blake3Digest32::from_bytes([0x31; 32])
    );
}

#[test]
fn launch_plan_rejects_http_mismatch_equal_ports_and_long_roots() {
    assert_eq!(
        plan_with_configured_http(6333, 6334, 6335)
            .err()
            .expect("HTTP mismatch must fail"),
        SupervisorError::EndpointIdentityMismatch
    );
    assert_eq!(
        plan(6333, 6333).err().expect("equal ports must fail"),
        SupervisorError::InvalidProcessConfig
    );

    let fence = owner_fence();
    let lease = lease(fence);
    let artifact = artifact();
    let config = validate_process_config(
        process_config(fence, 6333),
        &artifact,
        &lease,
        NonZeroU64::new(1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        LaunchPlan::new(
            OpaqueId::new("supervisor-short-timeout").unwrap(),
            fence,
            artifact.clone(),
            config.clone(),
            &lease,
            SECRET_PURPOSE,
            PathBuf::from("C:\\Tools\\Qdrant\\1.19.0\\qdrant.exe"),
            84_184_576,
            short_absolute_data_root(),
            parse_loopback_host("127.0.0.1").unwrap(),
            6333,
            6334,
            Duration::from_millis(500),
            Duration::from_secs(20),
            NonZeroU64::new(1).unwrap(),
        )
        .err()
        .expect("short startup timeout must fail"),
        SupervisorError::InvalidProcessConfig
    );
    let long_root = PathBuf::from(format!("C:\\{}", "x".repeat(120)));
    assert_eq!(
        LaunchPlan::new(
            OpaqueId::new("supervisor-long-path").unwrap(),
            fence,
            artifact,
            config,
            &lease,
            SECRET_PURPOSE,
            PathBuf::from("C:\\Tools\\Qdrant\\1.19.0\\qdrant.exe"),
            84_184_576,
            long_root,
            parse_loopback_host("127.0.0.1").unwrap(),
            6333,
            6334,
            Duration::from_secs(60),
            Duration::from_secs(20),
            NonZeroU64::new(1).unwrap(),
        )
        .err()
        .expect("long data root must fail"),
        SupervisorError::InvalidProcessConfig
    );
}

#[test]
fn wrong_and_missing_executables_fail_closed_before_spawn() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let scratch = std::env::temp_dir().join(format!(
        "qdrant-supervisor-identity-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir(&scratch).expect("unique identity scratch");
    let wrong_path = scratch.join("wrong.exe");
    std::fs::write(&wrong_path, b"wrong").expect("write small mismatched image");
    let wrong = ExecutableExpectation::custom(
        wrong_path.clone(),
        search_contracts::Sha256Digest32::from_bytes([0x61; 32]),
        5,
        "1.19.0".to_owned(),
    )
    .unwrap();
    assert_eq!(
        search_qdrant_supervisor::verify_executable_identity(&wrong, Duration::from_secs(1))
            .unwrap_err(),
        SupervisorError::ArtifactDigestMismatch
    );

    let wrong_size = ExecutableExpectation::custom(
        wrong_path,
        search_contracts::Sha256Digest32::from_bytes(search_qdrant_supervisor::sha256_bytes(
            b"wrong",
        )),
        4,
        "1.19.0".to_owned(),
    )
    .unwrap();
    assert_eq!(
        search_qdrant_supervisor::verify_executable_identity(&wrong_size, Duration::from_secs(1))
            .unwrap_err(),
        SupervisorError::ArtifactDigestMismatch
    );
    let missing = ExecutableExpectation::custom(
        scratch.join("missing.exe"),
        search_contracts::Sha256Digest32::from_bytes([0x62; 32]),
        5,
        "1.19.0".to_owned(),
    )
    .unwrap();
    assert_eq!(
        search_qdrant_supervisor::verify_executable_identity(&missing, Duration::from_secs(1))
            .unwrap_err(),
        SupervisorError::InvalidArtifact
    );
    std::fs::remove_dir_all(&scratch).expect("remove only the exact test scratch");
}

#[test]
fn occupied_loopback_port_is_not_adopted_or_disturbed() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("OS-assigned test listener");
    let port = listener.local_addr().expect("listener address").port();
    let host = parse_loopback_host("127.0.0.1").unwrap();
    assert_eq!(
        check_loopback_port_free(&host, port).unwrap_err(),
        SupervisorError::EndpointUnavailable
    );
    let address = format!("127.0.0.1:{port}").parse().unwrap();
    assert!(TcpStream::connect_timeout(&address, Duration::from_secs(1)).is_ok());
}

#[test]
fn non_loopback_host_literals_are_rejected() {
    assert_eq!(
        parse_loopback_host("0.0.0.0").unwrap_err(),
        SupervisorError::NonLoopbackEndpoint
    );
    assert_eq!(
        parse_loopback_host("localhost").unwrap_err(),
        SupervisorError::NonLoopbackEndpoint
    );
    assert_eq!(parse_loopback_host("127.0.0.1").unwrap(), LoopbackHost::V4);
    assert_eq!(parse_loopback_host("::1").unwrap(), LoopbackHost::V6);
}
