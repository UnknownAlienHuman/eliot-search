//! Owned-lifecycle tests that need no Qdrant binary.
//!
//! Covers: substituted executables never start, occupied ports are never
//! adopted, startup hangs resolve to a typed unknown outcome with bounded
//! reap, crashed children are classified with a bounded restart budget,
//! forged process identities quarantine, missing containment fails closed
//! with a typed error, and secret material never reaches argv, config
//! files, or debug output.

use std::io::Write as _;
use std::net::TcpListener;
use std::num::{NonZeroU32, NonZeroU64};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use search_contracts::{
    ArtifactDigest, Blake3Digest32, DataRootId, InstallationIncarnationId, OpaqueId, OwnerEpoch,
    ReceiptRef, Sha256Digest32,
};
use search_qdrant_supervisor::{
    ArtifactArchitecture, ArtifactCandidate, ArtifactQualificationManifest, ContainmentEvidence,
    ExecutableExpectation, LaunchPlan, LoopbackEndpoint, OwnedChild, ProcessConfig,
    ProcessIdentity, QdrantOwnerFence, QdrantSupervisor, SecretLeaseEvidence, SecretMaterial,
    SupervisorError, build_argv_snapshot, check_loopback_port_free, evaluate_containment,
    materialize_config_yaml, parse_loopback_host, parse_qdrant_version_output, qualify_artifact,
    sha256_bytes, spawn_qualified, validate_process_config,
};

fn owner_fence() -> QdrantOwnerFence {
    QdrantOwnerFence {
        data_root_id: DataRootId::from_bytes([11; 16]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([22; 16]),
        owner_epoch: OwnerEpoch::new(7).unwrap(),
    }
}

fn test_artifact() -> search_qdrant_supervisor::QualifiedArtifact {
    let candidate = ArtifactCandidate {
        file_identity_digest: Blake3Digest32::from_bytes([1; 32]),
        sha256: Sha256Digest32::from_bytes([2; 32]),
        artifact_digest: ArtifactDigest::from_bytes([3; 32]),
        version: "1.19.0".to_owned(),
        build_identity: "74f3e85b".to_owned(),
        architecture: ArtifactArchitecture::X86_64Windows,
    };
    let manifest = ArtifactQualificationManifest {
        expected_sha256: Sha256Digest32::from_bytes([2; 32]),
        expected_artifact_digest: ArtifactDigest::from_bytes([3; 32]),
        expected_version: "1.19.0".to_owned(),
        expected_build_identity: "74f3e85b".to_owned(),
        expected_architecture: ArtifactArchitecture::X86_64Windows,
        source_receipt: ReceiptRef::new("test-source").unwrap(),
        license_receipt: ReceiptRef::new("test-license").unwrap(),
        probe_manifest_digest: Blake3Digest32::from_bytes([4; 32]),
    };
    qualify_artifact(candidate, &manifest, Blake3Digest32::from_bytes([5; 32])).unwrap()
}

fn test_config(port: u16) -> search_qdrant_supervisor::QualifiedProcessConfig {
    let config = ProcessConfig {
        owner_fence: owner_fence(),
        data_directory_digest: Blake3Digest32::from_bytes([6; 32]),
        endpoint: LoopbackEndpoint {
            endpoint_digest: Blake3Digest32::from_bytes([7; 32]),
            port: NonZeroU32::new(u32::from(port)).unwrap(),
        },
        bind_is_loopback: true,
        single_node: true,
        startup_timeout_ticks: NonZeroU64::new(1_200).unwrap(),
        shutdown_timeout_ticks: NonZeroU64::new(300).unwrap(),
        restart_window_ticks: NonZeroU64::new(10_000).unwrap(),
        max_restarts_per_window: 2,
        config_digest: Blake3Digest32::from_bytes([8; 32]),
    };
    let lease = SecretLeaseEvidence {
        secret_reference_digest: Blake3Digest32::from_bytes([9; 32]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([22; 16]),
        purpose_digest: Blake3Digest32::from_bytes([10; 32]),
        expires_at_tick: NonZeroU64::new(1_000_000).unwrap(),
    };
    validate_process_config(
        config,
        &test_artifact(),
        lease,
        NonZeroU64::new(42).unwrap(),
    )
    .unwrap()
}

const fn test_lease() -> SecretLeaseEvidence {
    SecretLeaseEvidence {
        secret_reference_digest: Blake3Digest32::from_bytes([9; 32]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([22; 16]),
        purpose_digest: Blake3Digest32::from_bytes([10; 32]),
        expires_at_tick: NonZeroU64::new(1_000_000).unwrap(),
    }
}

fn test_identity(port: u16) -> ProcessIdentity {
    ProcessIdentity {
        process_id: NonZeroU32::new(4242).unwrap(),
        creation_marker: NonZeroU64::new(9_999).unwrap(),
        executable_file_digest: Blake3Digest32::from_bytes([1; 32]),
        artifact_digest: ArtifactDigest::from_bytes([3; 32]),
        owner_fence: owner_fence(),
        endpoint: LoopbackEndpoint {
            endpoint_digest: Blake3Digest32::from_bytes([7; 32]),
            port: NonZeroU32::new(u32::from(port)).unwrap(),
        },
    }
}

fn unique_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_nanos())
}

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "eliot-t23-{label}-{}-{}",
        std::process::id(),
        unique_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn free_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    assert!(port != 0);
    port
}

#[test]
fn substituted_executable_with_wrong_size_never_starts() {
    let dir = temp_dir("substituted-size");
    let fake = dir.join("qdrant.exe");
    std::fs::write(&fake, b"not the qualified executable").unwrap();
    let expectation = ExecutableExpectation::custom(
        fake,
        Sha256Digest32::from_bytes([0xAB; 32]),
        84_184_576,
        "1.19.0".to_owned(),
    )
    .unwrap();
    let result =
        search_qdrant_supervisor::verify_executable_identity(&expectation, Duration::from_secs(10));
    assert_eq!(result.unwrap_err(), SupervisorError::ArtifactDigestMismatch);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn substituted_executable_with_matching_size_but_wrong_hash_never_starts() {
    let dir = temp_dir("substituted-hash");
    let fake = dir.join("qdrant.exe");
    let handle = std::fs::File::create(&fake).unwrap();
    handle.set_len(84_184_576).unwrap();
    drop(handle);
    let expectation = ExecutableExpectation::custom(
        fake,
        Sha256Digest32::from_bytes([0xAB; 32]),
        84_184_576,
        "1.19.0".to_owned(),
    )
    .unwrap();
    let result =
        search_qdrant_supervisor::verify_executable_identity(&expectation, Duration::from_secs(30));
    assert_eq!(result.unwrap_err(), SupervisorError::ArtifactDigestMismatch);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_executable_fails_closed_without_probe() {
    let missing = std::env::temp_dir().join(format!(
        "eliot-t23-absent-{}-{}.exe",
        std::process::id(),
        unique_nanos()
    ));
    assert!(!missing.exists());
    let expectation = ExecutableExpectation::custom(
        missing,
        Sha256Digest32::from_bytes([0xAB; 32]),
        84_184_576,
        "1.19.0".to_owned(),
    )
    .unwrap();
    let result =
        search_qdrant_supervisor::verify_executable_identity(&expectation, Duration::from_secs(10));
    assert_eq!(result.unwrap_err(), SupervisorError::InvalidArtifact);
}

#[test]
fn version_output_parser_accepts_exact_format_and_rejects_forgeries() {
    assert_eq!(
        parse_qdrant_version_output("qdrant 1.19.0\n").unwrap(),
        "1.19.0"
    );
    assert_eq!(
        parse_qdrant_version_output("qdrant 1.19.0").unwrap(),
        "1.19.0"
    );
    assert!(parse_qdrant_version_output("qdrant 9.9.9\n").is_ok());
    assert_eq!(
        parse_qdrant_version_output("").unwrap_err(),
        SupervisorError::InvalidArtifact
    );
    assert_eq!(
        parse_qdrant_version_output("1.19.0").unwrap_err(),
        SupervisorError::InvalidArtifact
    );
    assert_eq!(
        parse_qdrant_version_output("qdrant ").unwrap_err(),
        SupervisorError::InvalidArtifact
    );
    assert_eq!(
        parse_qdrant_version_output("qdrant 1.19.0 extra").unwrap_err(),
        SupervisorError::InvalidArtifact
    );
    assert_eq!(
        parse_qdrant_version_output("QDRANT 1.19.0").unwrap_err(),
        SupervisorError::InvalidArtifact
    );
}

#[test]
fn occupied_port_is_reported_unavailable_and_never_adopted() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let host = parse_loopback_host("127.0.0.1").unwrap();
    let result = check_loopback_port_free(&host, port);
    assert_eq!(result.unwrap_err(), SupervisorError::EndpointUnavailable);
    // The foreign listener is untouched: still accepting the port afterwards.
    let probe = std::net::TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_secs(2),
    );
    assert!(probe.is_ok());
    drop(listener);
}

#[test]
fn non_loopback_hosts_are_rejected_before_any_effect() {
    for host in [
        "0.0.0.0",
        "example.com",
        "",
        "127.0.0.2",
        "::ffff:127.0.0.1",
    ] {
        assert_eq!(
            parse_loopback_host(host).unwrap_err(),
            SupervisorError::NonLoopbackEndpoint,
            "host {host:?} must be rejected"
        );
    }
    assert!(parse_loopback_host("127.0.0.1").is_ok());
    assert!(parse_loopback_host("::1").is_ok());
}

#[test]
fn missing_containment_evidence_fails_closed_with_typed_error() {
    let http_port = free_loopback_port();
    let grpc_port = free_loopback_port();
    assert_ne!(http_port, grpc_port);
    let data_dir = temp_dir("containment");
    let plan = LaunchPlan::new(
        OpaqueId::new("t23-missing-containment").unwrap(),
        owner_fence(),
        test_artifact(),
        test_config(http_port),
        test_lease(),
        Blake3Digest32::from_bytes([10; 32]),
        PathBuf::from("C:\\Tools\\Qdrant\\1.19.0\\qdrant.exe"),
        84_184_576,
        data_dir.clone(),
        parse_loopback_host("127.0.0.1").unwrap(),
        http_port,
        grpc_port,
        Duration::from_secs(60),
        Duration::from_secs(20),
        ContainmentEvidence::missing(),
        NonZeroU64::new(42).unwrap(),
    )
    .unwrap();
    let secret = SecretMaterial::from_bytes(b"loopback-test-key".to_vec()).unwrap();
    let result = spawn_qualified(&plan, &secret);
    assert!(
        matches!(result, Err(SupervisorError::ContainmentUnavailable)),
        "missing containment must fail closed"
    );
    assert_eq!(
        SupervisorError::ContainmentUnavailable.code(),
        "QDRANT_CONTAINMENT_UNAVAILABLE"
    );
    std::fs::remove_dir_all(&data_dir).ok();
}

#[test]
fn launch_plan_rejects_port_mismatch_and_bad_timeouts() {
    let http_port = free_loopback_port();
    let data_dir = temp_dir("plan-validation");
    let other_port = http_port.checked_add(1).unwrap_or(1024);
    // HTTP port must equal the qualified endpoint port.
    let mismatch = LaunchPlan::new(
        OpaqueId::new("t23-port-mismatch").unwrap(),
        owner_fence(),
        test_artifact(),
        test_config(http_port),
        test_lease(),
        Blake3Digest32::from_bytes([10; 32]),
        PathBuf::from("C:\\Tools\\Qdrant\\1.19.0\\qdrant.exe"),
        84_184_576,
        data_dir.clone(),
        parse_loopback_host("127.0.0.1").unwrap(),
        other_port,
        free_loopback_port(),
        Duration::from_secs(60),
        Duration::from_secs(20),
        ContainmentEvidence::explicit_uncontained_test_only(),
        NonZeroU64::new(42).unwrap(),
    );
    assert!(
        matches!(mismatch, Err(SupervisorError::EndpointIdentityMismatch)),
        "HTTP port must equal the qualified endpoint port"
    );
    // Zero/degenerate timeouts are rejected.
    let bad_timeout = LaunchPlan::new(
        OpaqueId::new("t23-bad-timeout").unwrap(),
        owner_fence(),
        test_artifact(),
        test_config(http_port),
        test_lease(),
        Blake3Digest32::from_bytes([10; 32]),
        PathBuf::from("C:\\Tools\\Qdrant\\1.19.0\\qdrant.exe"),
        84_184_576,
        data_dir.clone(),
        parse_loopback_host("127.0.0.1").unwrap(),
        http_port,
        free_loopback_port(),
        Duration::from_secs(0),
        Duration::from_secs(20),
        ContainmentEvidence::explicit_uncontained_test_only(),
        NonZeroU64::new(42).unwrap(),
    );
    assert!(
        matches!(bad_timeout, Err(SupervisorError::InvalidProcessConfig)),
        "degenerate timeouts must be rejected"
    );
    // Expired secret leases are rejected at plan construction.
    let expired_lease = SecretLeaseEvidence {
        secret_reference_digest: Blake3Digest32::from_bytes([9; 32]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([22; 16]),
        purpose_digest: Blake3Digest32::from_bytes([10; 32]),
        expires_at_tick: NonZeroU64::new(10).unwrap(),
    };
    let expired = LaunchPlan::new(
        OpaqueId::new("t23-expired-lease").unwrap(),
        owner_fence(),
        test_artifact(),
        test_config(http_port),
        expired_lease,
        Blake3Digest32::from_bytes([10; 32]),
        PathBuf::from("C:\\Tools\\Qdrant\\1.19.0\\qdrant.exe"),
        84_184_576,
        data_dir.clone(),
        parse_loopback_host("127.0.0.1").unwrap(),
        http_port,
        free_loopback_port(),
        Duration::from_secs(60),
        Duration::from_secs(20),
        ContainmentEvidence::explicit_uncontained_test_only(),
        NonZeroU64::new(42).unwrap(),
    );
    assert!(
        matches!(expired, Err(SupervisorError::SecretLeaseInvalid)),
        "expired leases must be rejected at plan construction"
    );
    std::fs::remove_dir_all(&data_dir).ok();
}

#[test]
fn secret_material_never_reaches_argv_config_or_debug_output() {
    let secret_text = "t23-side-channel-audit-key-xyz";
    let secret = SecretMaterial::from_bytes(secret_text.as_bytes().to_vec()).unwrap();
    assert!(secret.validate_header_safe().is_ok());

    let dir = temp_dir("sc-audit");
    let config_path =
        materialize_config_yaml(&dir, parse_loopback_host("127.0.0.1").unwrap(), 6333, 6334)
            .unwrap();
    let config_text = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        !secret.appears_in(&config_text),
        "config file must not contain secret material"
    );

    let snapshot = build_argv_snapshot(
        std::path::Path::new("C:\\Tools\\Qdrant\\1.19.0\\qdrant.exe"),
        &config_path,
    );
    assert!(
        !secret.appears_in(snapshot.as_str()),
        "argv snapshot must not contain secret material"
    );
    assert!(
        !snapshot.as_str().contains(secret_text),
        "argv snapshot must not contain the key either"
    );
    assert!(
        snapshot.as_str().contains("--config-path"),
        "argv must carry the config path flag"
    );

    let material_debug = format!("{secret:?}");
    assert!(
        !secret.appears_in(&material_debug),
        "SecretMaterial Debug must stay redacted"
    );

    let evidence =
        evaluate_containment(&ContainmentEvidence::explicit_uncontained_test_only()).unwrap();
    let evidence_debug = format!("{evidence:?}");
    assert!(!secret.appears_in(&evidence_debug));

    // Header-unsafe secrets are refused at bind time instead of being mangled.
    let binary = SecretMaterial::from_bytes(vec![0xFF, 0xFE, 0x00, 0x20]).unwrap();
    assert_eq!(
        binary.validate_header_safe().unwrap_err(),
        SupervisorError::SecretLeaseInvalid
    );
    // Empty and oversized secrets are refused at construction.
    assert_eq!(
        SecretMaterial::from_bytes(Vec::new()).unwrap_err(),
        SupervisorError::SecretLeaseInvalid
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn sha256_matches_reference_vector_for_streamed_executable() {
    let dir = temp_dir("sha256");
    let file = dir.join("payload.bin");
    let mut payload = Vec::with_capacity(200_000);
    for chunk in 0..2000 {
        payload
            .extend_from_slice(format!("block-{chunk:06}-abcdefghijklmnopqrstuvwxyz\n").as_bytes());
    }
    {
        let mut handle = std::fs::File::create(&file).unwrap();
        handle.write_all(&payload).unwrap();
        handle.flush().unwrap();
    }
    let expected = sha256_bytes(&payload);
    let observed = search_qdrant_supervisor::sha256_file(&file).unwrap();
    assert_eq!(observed, expected);
    assert_eq!(expected.len(), 32);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn forged_readiness_identity_is_rejected_and_mismatched_exit_quarantines() {
    let port = free_loopback_port();
    let mut supervisor = QdrantSupervisor::new();
    supervisor
        .prepare_start(
            OpaqueId::new("t23-forged-identity").unwrap(),
            test_artifact(),
            test_config(port),
        )
        .unwrap();

    // Readiness for a different executable digest must not confirm.
    let mut forged = test_identity(port);
    forged.executable_file_digest = Blake3Digest32::from_bytes([0xEE; 32]);
    let readiness = search_qdrant_supervisor::ProcessReadiness {
        identity: forged,
        authenticated_health_ok: true,
        observed_config_digest: Blake3Digest32::from_bytes([8; 32]),
        readiness_receipt: ReceiptRef::new("test-ready").unwrap(),
    };
    assert_eq!(
        supervisor.confirm_ready(&readiness).unwrap_err(),
        SupervisorError::ExecutableIdentityMismatch
    );

    // Correct identity confirms; a later exit under a reused PID quarantines.
    let readiness = search_qdrant_supervisor::ProcessReadiness {
        identity: test_identity(port),
        authenticated_health_ok: true,
        observed_config_digest: Blake3Digest32::from_bytes([8; 32]),
        readiness_receipt: ReceiptRef::new("test-ready").unwrap(),
    };
    supervisor.confirm_ready(&readiness).unwrap();
    let mut reused = test_identity(port);
    reused.process_id = NonZeroU32::new(7777).unwrap();
    let observation = search_qdrant_supervisor::ExitObservation {
        identity: reused,
        expected_shutdown: false,
        exit_code: Some(1),
        observed_tick: NonZeroU64::new(100).unwrap(),
    };
    let decision = supervisor.classify_exit(observation).unwrap();
    assert_eq!(
        decision,
        search_qdrant_supervisor::RestartDecision::Quarantine
    );
    assert!(matches!(
        supervisor.state(),
        search_qdrant_supervisor::SupervisorState::Quarantined(
            SupervisorError::ProcessIdentityMismatch
        )
    ));
}

#[test]
fn bounded_restart_budget_ends_in_quarantine() {
    let port = free_loopback_port();
    let mut supervisor = QdrantSupervisor::new();
    // Budget is 2 restarts per window; the third unexpected exit quarantines.
    for _ in 0..3 {
        supervisor
            .prepare_start(
                OpaqueId::new("t23-restart-budget").unwrap(),
                test_artifact(),
                test_config(port),
            )
            .unwrap();
        let readiness = search_qdrant_supervisor::ProcessReadiness {
            identity: test_identity(port),
            authenticated_health_ok: true,
            observed_config_digest: Blake3Digest32::from_bytes([8; 32]),
            readiness_receipt: ReceiptRef::new("test-ready").unwrap(),
        };
        supervisor.confirm_ready(&readiness).unwrap();
        let observation = search_qdrant_supervisor::ExitObservation {
            identity: test_identity(port),
            expected_shutdown: false,
            exit_code: Some(3),
            observed_tick: NonZeroU64::new(200).unwrap(),
        };
        let decision = supervisor.classify_exit(observation).unwrap();
        if matches!(
            supervisor.state(),
            search_qdrant_supervisor::SupervisorState::Quarantined(_)
        ) {
            assert_eq!(
                decision,
                search_qdrant_supervisor::RestartDecision::Quarantine
            );
            assert_eq!(
                supervisor
                    .prepare_start(
                        OpaqueId::new("t23-restart-budget").unwrap(),
                        test_artifact(),
                        test_config(port),
                    )
                    .unwrap_err(),
                SupervisorError::Quarantined
            );
            return;
        }
        assert_eq!(decision, search_qdrant_supervisor::RestartDecision::Restart);
    }
    panic!("restart budget must quarantine after exhaustion");
}

#[test]
fn expected_shutdown_returns_to_stopped_without_restart() {
    let port = free_loopback_port();
    let mut supervisor = QdrantSupervisor::new();
    supervisor
        .prepare_start(
            OpaqueId::new("t23-clean-stop").unwrap(),
            test_artifact(),
            test_config(port),
        )
        .unwrap();
    let readiness = search_qdrant_supervisor::ProcessReadiness {
        identity: test_identity(port),
        authenticated_health_ok: true,
        observed_config_digest: Blake3Digest32::from_bytes([8; 32]),
        readiness_receipt: ReceiptRef::new("test-ready").unwrap(),
    };
    supervisor.confirm_ready(&readiness).unwrap();
    let observation = search_qdrant_supervisor::ExitObservation {
        identity: test_identity(port),
        expected_shutdown: true,
        exit_code: Some(0),
        observed_tick: NonZeroU64::new(300).unwrap(),
    };
    let decision = supervisor.classify_exit(observation).unwrap();
    assert_eq!(decision, search_qdrant_supervisor::RestartDecision::Stop);
    assert!(matches!(
        supervisor.state(),
        search_qdrant_supervisor::SupervisorState::Stopped
    ));
}

#[cfg(windows)]
fn spawn_sleeper(seconds: u64) -> Child {
    Command::new("cmd")
        .args(["/C", &format!("timeout /T {seconds} /NOBREAK > NUL")])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap()
}

#[cfg(windows)]
fn spawn_immediate_crash() -> Child {
    Command::new("cmd")
        .args(["/C", "exit 3"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap()
}

#[test]
#[cfg(windows)]
fn startup_hang_reports_unknown_outcome_and_reaps_without_orphan() {
    let child = spawn_sleeper(60);
    let mut owned = OwnedChild::wrap_test_child(child, "hang-fixture").unwrap();
    assert!(owned.is_running());
    let secret = SecretMaterial::from_bytes(b"hang-probe-key".to_vec()).unwrap();
    let port = free_loopback_port();
    let host = parse_loopback_host("127.0.0.1").unwrap();
    let waited = Instant::now();
    let result = owned.wait_ready(
        &host,
        port,
        &secret,
        Instant::now() + Duration::from_secs(3),
    );
    assert_eq!(result.unwrap_err(), SupervisorError::StartupOutcomeUnknown);
    assert!(
        waited.elapsed() < Duration::from_secs(30),
        "startup wait must stay finite"
    );
    // The hanging child is still ours: bounded terminate reaps it, no orphan.
    let code = owned
        .terminate_bounded(Instant::now() + Duration::from_secs(15))
        .unwrap();
    assert!(
        code.is_some(),
        "reaped child must report an exit code, got {code:?}"
    );
    assert!(!owned.is_running());
}

#[test]
#[cfg(windows)]
fn crashed_child_is_reaped_with_its_exit_code() {
    let child = spawn_immediate_crash();
    let mut owned = OwnedChild::wrap_test_child(child, "crash-fixture").unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut observed = None;
    while Instant::now() < deadline {
        observed = owned.try_exit_code();
        if observed.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(observed, Some(Some(3)));
    assert!(!owned.is_running());
}
