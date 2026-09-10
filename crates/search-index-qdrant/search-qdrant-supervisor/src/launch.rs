//! Qualified launch: from an accepted plan to a spawned owned child.
//!
//! [`spawn_qualified`] executes the pure [`StartProcessEffect`] on the exact
//! verified executable. Ordered checks, cheapest first:
//!
//! 1. containment evidence ([`evaluate_containment`]);
//! 2. loopback port availability (never adopt a foreign listener);
//! 3. secret lease binding and header-safe secret material;
//! 4. executable identity: size, SHA-256, `--version` ([`identity`]);
//! 5. owner-scoped storage canonicalization;
//! 6. secret-free config materialization and argv audit;
//! 7. spawn of the owned child.
//!
//! The secret travels only in the child environment block under
//! [`QDRANT_API_KEY_ENV`]. It never appears in argv, the config file,
//! logs, snapshots, or receipts.
//!
//! [`StartProcessEffect`]: crate::StartProcessEffect
//! [`evaluate_containment`]: crate::containment::evaluate_containment
//! [`identity`]: crate::identity

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::num::{NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use search_contracts::{Blake3Digest32, OpaqueId, Sha256Digest32};

use crate::containment::{
    ContainmentEvidence, ContainmentReport, LoopbackHost, evaluate_containment,
};
use crate::identity::{ExecutableExpectation, VerifiedExecutable, verify_executable_identity};
use crate::secret::SecretMaterial;
use crate::{
    QdrantOwnerFence, QualifiedArtifact, QualifiedProcessConfig, SecretLeaseEvidence,
    SupervisorError, spawn_unix_millis,
};

/// Environment variable carrying the API key into the child.
pub const QDRANT_API_KEY_ENV: &str = "QDRANT__SERVICE__API_KEY";
/// CLI flag carrying the materialized config path.
pub const QDRANT_CONFIG_ARG: &str = "--config-path";
/// Materialized config file name inside the data directory.
pub const CONFIG_FILE_NAME: &str = "config.yaml";
/// Lower bound for startup/shutdown timeouts.
pub const MIN_LAUNCH_TIMEOUT: Duration = Duration::from_secs(1);
/// Upper bound for startup/shutdown timeouts.
pub const MAX_LAUNCH_TIMEOUT: Duration = Duration::from_secs(600);
/// Deadline for the pre-spawn `--version` identity probe.
pub const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
/// Per-attempt TCP connect timeout for the pre-spawn port check.
pub const PORT_CHECK_TIMEOUT: Duration = Duration::from_millis(500);

/// Exact launch plan binding one qualified artifact, config, lease, and
/// owner fence to literal paths, ports, timeouts, and containment evidence.
pub struct LaunchPlan {
    operation_id: OpaqueId,
    owner_fence: QdrantOwnerFence,
    artifact: QualifiedArtifact,
    config: QualifiedProcessConfig,
    secret_lease: SecretLeaseEvidence,
    expected_secret_purpose: Blake3Digest32,
    executable_path: PathBuf,
    executable_sha256: Sha256Digest32,
    executable_bytes: u64,
    executable_version: String,
    data_dir: PathBuf,
    host: LoopbackHost,
    http_port: u16,
    grpc_port: u16,
    startup_timeout: Duration,
    shutdown_timeout: Duration,
    containment: ContainmentEvidence,
    observed_tick: NonZeroU64,
}

impl LaunchPlan {
    /// Builds a plan, rejecting incoherent bindings fail-closed.
    pub fn new(
        operation_id: OpaqueId,
        owner_fence: QdrantOwnerFence,
        artifact: QualifiedArtifact,
        config: QualifiedProcessConfig,
        secret_lease: SecretLeaseEvidence,
        expected_secret_purpose: Blake3Digest32,
        executable_path: PathBuf,
        executable_bytes: u64,
        data_dir: PathBuf,
        host: LoopbackHost,
        http_port: u16,
        grpc_port: u16,
        startup_timeout: Duration,
        shutdown_timeout: Duration,
        containment: ContainmentEvidence,
        observed_tick: NonZeroU64,
    ) -> Result<Self, SupervisorError> {
        if executable_path.as_os_str().is_empty()
            || data_dir.as_os_str().is_empty()
            || executable_bytes == 0
        {
            return Err(SupervisorError::InvalidProcessConfig);
        }
        if http_port == 0 || grpc_port == 0 || http_port == grpc_port {
            return Err(SupervisorError::InvalidProcessConfig);
        }
        if startup_timeout < MIN_LAUNCH_TIMEOUT
            || startup_timeout > MAX_LAUNCH_TIMEOUT
            || shutdown_timeout < MIN_LAUNCH_TIMEOUT
            || shutdown_timeout > MAX_LAUNCH_TIMEOUT
        {
            return Err(SupervisorError::InvalidProcessConfig);
        }
        let configured_port = config.config().endpoint.port.get();
        let http_port_wide = u32::from(http_port);
        if configured_port != http_port_wide {
            return Err(SupervisorError::EndpointIdentityMismatch);
        }
        let candidate = artifact.candidate();
        let executable_sha256 = candidate.sha256;
        let executable_version = candidate.version.clone();
        if secret_lease.installation_incarnation_id != owner_fence.installation_incarnation_id
            || secret_lease.installation_incarnation_id
                != config.config().owner_fence.installation_incarnation_id
            || secret_lease.expires_at_tick <= observed_tick
            || secret_lease.purpose_digest != expected_secret_purpose
        {
            return Err(SupervisorError::SecretLeaseInvalid);
        }
        if owner_fence != config.config().owner_fence {
            return Err(SupervisorError::OwnerFenceMismatch);
        }
        Ok(Self {
            operation_id,
            owner_fence,
            artifact,
            config,
            secret_lease,
            expected_secret_purpose,
            executable_path,
            executable_sha256,
            executable_bytes,
            executable_version,
            data_dir,
            host,
            http_port,
            grpc_port,
            startup_timeout,
            shutdown_timeout,
            containment,
            observed_tick,
        })
    }

    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }

    #[must_use]
    pub const fn owner_fence(&self) -> &QdrantOwnerFence {
        &self.owner_fence
    }

    #[must_use]
    pub const fn artifact(&self) -> &QualifiedArtifact {
        &self.artifact
    }

    #[must_use]
    pub const fn config(&self) -> &QualifiedProcessConfig {
        &self.config
    }

    #[must_use]
    pub const fn secret_lease(&self) -> &SecretLeaseEvidence {
        &self.secret_lease
    }

    #[must_use]
    pub const fn expected_secret_purpose(&self) -> Blake3Digest32 {
        self.expected_secret_purpose
    }

    #[must_use]
    pub fn executable_path(&self) -> &Path {
        &self.executable_path
    }

    #[must_use]
    pub const fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    #[must_use]
    pub const fn host(&self) -> LoopbackHost {
        self.host
    }

    #[must_use]
    pub const fn http_port(&self) -> u16 {
        self.http_port
    }

    #[must_use]
    pub const fn grpc_port(&self) -> u16 {
        self.grpc_port
    }

    #[must_use]
    pub const fn startup_timeout(&self) -> Duration {
        self.startup_timeout
    }

    #[must_use]
    pub const fn shutdown_timeout(&self) -> Duration {
        self.shutdown_timeout
    }

    #[must_use]
    pub const fn containment(&self) -> &ContainmentEvidence {
        &self.containment
    }

    #[must_use]
    pub const fn observed_tick(&self) -> NonZeroU64 {
        self.observed_tick
    }

    pub(crate) fn expectation(&self) -> Result<ExecutableExpectation, SupervisorError> {
        ExecutableExpectation::custom(
            self.executable_path.clone(),
            self.executable_sha256,
            self.executable_bytes,
            self.executable_version.clone(),
        )
    }
}

/// Printable argv without secrets: executable plus `--config-path` only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgvSnapshot(String);

impl ArgvSnapshot {
    /// Test-only snapshot for lifecycle fixtures. Never built from secrets.
    #[must_use]
    pub fn for_tests(label: &'static str) -> Self {
        Self(label.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Builds the auditable argv snapshot for one launch.
#[must_use]
pub fn build_argv_snapshot(exe: &Path, config_path: &Path) -> ArgvSnapshot {
    ArgvSnapshot(format!(
        "{} {QDRANT_CONFIG_ARG} {}",
        exe.display(),
        config_path.display()
    ))
}

/// Writes the secret-free Qdrant config file into the data directory.
///
/// The file carries storage location and loopback ports only. Authentication
/// travels exclusively through the leased secret in the child environment.
pub fn materialize_config_yaml(
    data_dir: &Path,
    host: LoopbackHost,
    http_port: u16,
    grpc_port: u16,
) -> Result<PathBuf, SupervisorError> {
    if http_port == 0 || grpc_port == 0 || http_port == grpc_port {
        return Err(SupervisorError::InvalidProcessConfig);
    }
    let text = format!(
        "storage:\n  storage_path: ./storage\nservice:\n  host: {}\n  http_port: {}\n  grpc_port: {}\n",
        host.as_str(),
        http_port,
        grpc_port
    );
    let path = data_dir.join(CONFIG_FILE_NAME);
    std::fs::write(&path, text).map_err(|_| SupervisorError::StartFailed)?;
    Ok(path)
}

/// Proves no listener currently answers on a loopback port.
///
/// A connectable port means a foreign process owns it: the supervisor must
/// never adopt it. Unreachable/refused means free. Any other observation
/// fails closed.
pub fn check_loopback_port_free(host: &LoopbackHost, port: u16) -> Result<(), SupervisorError> {
    if port == 0 {
        return Err(SupervisorError::InvalidProcessConfig);
    }
    let address: SocketAddr = match host {
        LoopbackHost::V4 => SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        LoopbackHost::V6 => SocketAddr::from((Ipv6Addr::LOCALHOST, port)),
    };
    match TcpStream::connect_timeout(&address, PORT_CHECK_TIMEOUT) {
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::TimedOut
            ) =>
        {
            Ok(())
        }
        _ => Err(SupervisorError::EndpointUnavailable),
    }
}

/// Spawned owned child plus the evidence that authorized the spawn.
pub struct SpawnedChild {
    child: Child,
    pid: NonZeroU32,
    verified: VerifiedExecutable,
    argv_snapshot: ArgvSnapshot,
    containment: ContainmentReport,
    spawn_unix_millis: NonZeroU64,
    canonical_data_dir: PathBuf,
    canonical_config_path: PathBuf,
}

impl SpawnedChild {
    #[must_use]
    pub const fn pid(&self) -> NonZeroU32 {
        self.pid
    }

    #[must_use]
    pub const fn verified(&self) -> &VerifiedExecutable {
        &self.verified
    }

    #[must_use]
    pub const fn argv_snapshot(&self) -> &ArgvSnapshot {
        &self.argv_snapshot
    }

    #[must_use]
    pub const fn containment_report(&self) -> ContainmentReport {
        self.containment
    }

    #[must_use]
    pub const fn spawn_unix_millis(&self) -> NonZeroU64 {
        self.spawn_unix_millis
    }

    #[must_use]
    pub const fn canonical_data_dir(&self) -> &PathBuf {
        &self.canonical_data_dir
    }

    #[must_use]
    pub const fn canonical_config_path(&self) -> &PathBuf {
        &self.canonical_config_path
    }

    pub(crate) fn into_child(self) -> Child {
        self.child
    }
}

/// Spawns the qualified executable after all ordered checks pass.
///
/// Returns the owned [`Child`] wrapper; the caller reaps it through
/// `owned` primitives with finite drain/reap deadlines. Never adopts an
/// existing process: only the handle just spawned is owned.
pub fn spawn_qualified(
    plan: &LaunchPlan,
    secret: &SecretMaterial,
) -> Result<SpawnedChild, SupervisorError> {
    let containment = evaluate_containment(plan.containment())?;
    check_loopback_port_free(&plan.host(), plan.http_port())?;
    check_loopback_port_free(&plan.host(), plan.grpc_port())?;
    secret.validate_header_safe()?;
    let verified = verify_executable_identity(&plan.expectation()?, VERSION_PROBE_TIMEOUT)?;
    let canonical_data_dir = canonical_data_dir(&plan.data_dir)?;
    let config_path = materialize_config_yaml(
        &canonical_data_dir,
        plan.host(),
        plan.http_port(),
        plan.grpc_port(),
    )?;
    let snapshot = build_argv_snapshot(verified.canonical_path(), &config_path);
    if secret.appears_in(snapshot.as_str()) {
        return Err(SupervisorError::SecretLeaseInvalid);
    }
    let secret_text = core::str::from_utf8(secret.secret_bytes())
        .map_err(|_| SupervisorError::SecretLeaseInvalid)?;
    let stdout_log = std::fs::File::create(canonical_data_dir.join("run-stdout.log"))
        .map_err(|_| SupervisorError::StartFailed)?;
    let stderr_log = std::fs::File::create(canonical_data_dir.join("run-stderr.log"))
        .map_err(|_| SupervisorError::StartFailed)?;
    let mut child = Command::new(verified.canonical_path())
        .arg(QDRANT_CONFIG_ARG)
        .arg(&config_path)
        .current_dir(&canonical_data_dir)
        .env(QDRANT_API_KEY_ENV, secret_text)
        .stdin(Stdio::null())
        .stdout(stdout_log)
        .stderr(stderr_log)
        .spawn()
        .map_err(|_| SupervisorError::StartFailed)?;
    // Re-check liveness immediately: a spawn that exited before we observe
    // it is still our owned child (same handle), never a foreign process.
    let pid = NonZeroU32::new(child.id()).ok_or(SupervisorError::StartFailed)?;
    let _ = child.try_wait().map_err(|_| SupervisorError::StartFailed)?;
    Ok(SpawnedChild {
        child,
        pid,
        verified,
        argv_snapshot: snapshot,
        containment,
        spawn_unix_millis: spawn_unix_millis(),
        canonical_data_dir,
        canonical_config_path: config_path,
    })
}

/// Canonicalizes the owner-scoped data directory; refuses missing/non-dir paths.
fn canonical_data_dir(data_dir: &Path) -> Result<PathBuf, SupervisorError> {
    let metadata = std::fs::metadata(data_dir).map_err(|_| SupervisorError::StartFailed)?;
    if !metadata.is_dir() {
        return Err(SupervisorError::StartFailed);
    }
    std::fs::canonicalize(data_dir).map_err(|_| SupervisorError::StartFailed)
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroU64};
    use std::path::PathBuf;
    use std::time::Duration;

    use search_contracts::{
        ArtifactDigest, Blake3Digest32, DataRootId, InstallationIncarnationId, OpaqueId,
        OwnerEpoch, ReceiptRef, Sha256Digest32,
    };

    use super::{
        LaunchPlan, build_argv_snapshot, check_loopback_port_free, materialize_config_yaml,
    };
    use crate::containment::{ContainmentEvidence, parse_loopback_host};
    use crate::secret::SecretMaterial;
    use crate::{
        ArtifactArchitecture, ArtifactCandidate, ArtifactQualificationManifest, LoopbackEndpoint,
        ProcessConfig, QdrantOwnerFence, SecretLeaseEvidence, SupervisorError, qualify_artifact,
        validate_process_config,
    };

    fn plan_for_ports(http_port: u16, grpc_port: u16) -> LaunchPlan {
        let fence = QdrantOwnerFence {
            data_root_id: DataRootId::from_bytes([1; 16]),
            installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
            owner_epoch: OwnerEpoch::new(3).unwrap(),
        };
        let candidate = ArtifactCandidate {
            file_identity_digest: Blake3Digest32::from_bytes([4; 32]),
            sha256: Sha256Digest32::from_bytes([5; 32]),
            artifact_digest: ArtifactDigest::from_bytes([6; 32]),
            version: "1.19.0".to_owned(),
            build_identity: "74f3e85b".to_owned(),
            architecture: ArtifactArchitecture::X86_64Windows,
        };
        let manifest = ArtifactQualificationManifest {
            expected_sha256: Sha256Digest32::from_bytes([5; 32]),
            expected_artifact_digest: ArtifactDigest::from_bytes([6; 32]),
            expected_version: "1.19.0".to_owned(),
            expected_build_identity: "74f3e85b".to_owned(),
            expected_architecture: ArtifactArchitecture::X86_64Windows,
            source_receipt: ReceiptRef::new("source").unwrap(),
            license_receipt: ReceiptRef::new("license").unwrap(),
            probe_manifest_digest: Blake3Digest32::from_bytes([7; 32]),
        };
        let artifact =
            qualify_artifact(candidate, &manifest, Blake3Digest32::from_bytes([8; 32])).unwrap();
        let config = ProcessConfig {
            owner_fence: fence,
            data_directory_digest: Blake3Digest32::from_bytes([9; 32]),
            endpoint: LoopbackEndpoint {
                endpoint_digest: Blake3Digest32::from_bytes([10; 32]),
                port: NonZeroU32::new(u32::from(http_port)).unwrap(),
            },
            bind_is_loopback: true,
            single_node: true,
            startup_timeout_ticks: NonZeroU64::new(100).unwrap(),
            shutdown_timeout_ticks: NonZeroU64::new(100).unwrap(),
            restart_window_ticks: NonZeroU64::new(1000).unwrap(),
            max_restarts_per_window: 2,
            config_digest: Blake3Digest32::from_bytes([11; 32]),
        };
        let lease = SecretLeaseEvidence {
            secret_reference_digest: Blake3Digest32::from_bytes([12; 32]),
            installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
            purpose_digest: Blake3Digest32::from_bytes([13; 32]),
            expires_at_tick: NonZeroU64::new(1_000_000).unwrap(),
        };
        let qualified =
            validate_process_config(config, &artifact, lease, NonZeroU64::new(7).unwrap()).unwrap();
        LaunchPlan::new(
            OpaqueId::new("plan-unit").unwrap(),
            fence,
            artifact,
            qualified,
            lease,
            Blake3Digest32::from_bytes([13; 32]),
            PathBuf::from("qdrant.exe"),
            84_184_576,
            PathBuf::from("data"),
            parse_loopback_host("127.0.0.1").unwrap(),
            http_port,
            grpc_port,
            Duration::from_secs(60),
            Duration::from_secs(20),
            ContainmentEvidence::explicit_uncontained_test_only(),
            NonZeroU64::new(7).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn materialized_config_carries_no_secret_and_pins_loopback() {
        let dir = std::env::temp_dir().join(format!(
            "eliot-t23-cfg-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let secret = SecretMaterial::from_bytes(b"config-audit-key".to_vec()).unwrap();
        let path =
            materialize_config_yaml(&dir, parse_loopback_host("127.0.0.1").unwrap(), 6333, 6334)
                .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("host: 127.0.0.1"));
        assert!(text.contains("http_port: 6333"));
        assert!(!secret.appears_in(&text));
        let snapshot = build_argv_snapshot(PathBuf::from("qdrant.exe").as_path(), &path);
        assert!(!secret.appears_in(snapshot.as_str()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn plan_with_distinct_ports_is_accepted() {
        let plan = plan_for_ports(6333, 6334);
        assert_eq!(plan.http_port(), 6333);
        assert_eq!(plan.grpc_port(), 6334);
    }

    #[test]
    fn invalid_port_combinations_are_rejected() {
        assert_eq!(
            materialize_config_yaml(
                std::path::Path::new("data"),
                parse_loopback_host("127.0.0.1").unwrap(),
                6333,
                6333
            )
            .unwrap_err(),
            SupervisorError::InvalidProcessConfig
        );
        assert_eq!(
            check_loopback_port_free(&parse_loopback_host("127.0.0.1").unwrap(), 0).unwrap_err(),
            SupervisorError::InvalidProcessConfig
        );
    }
}
