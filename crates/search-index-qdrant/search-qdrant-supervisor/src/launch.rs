//! Lease-bound Qdrant launch planning and native process creation.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use search_contracts::{Blake3Digest32, OpaqueId};

use crate::containment::{ContainmentReport, LoopbackHost};
use crate::secret::{QdrantSecretLease, SecretLeaseBinding, SecretMaterial};
use crate::win32::{NativeProcess, NativeSpawnError};
use crate::{
    QdrantEndpointIdentity, QdrantOwnerFence, QualifiedArtifact, QualifiedProcessConfig,
    SupervisorError,
};

/// Environment variable carrying the API key into the child.
pub const QDRANT_API_KEY_ENV: &str = "QDRANT__SERVICE__API_KEY";
/// CLI flag carrying the materialized config path.
pub const QDRANT_CONFIG_ARG: &str = "--config-path";
/// Materialized Qdrant config file name inside the data directory.
pub const CONFIG_FILE_NAME: &str = "config.yaml";
/// Lower bound for startup/shutdown timeouts.
pub const MIN_LAUNCH_TIMEOUT: Duration = Duration::from_secs(1);
/// Upper bound for startup/shutdown timeouts.
pub const MAX_LAUNCH_TIMEOUT: Duration = Duration::from_secs(600);
/// Maximum absolute data-root path length before the native adapter is invoked.
pub const MAX_DATA_ROOT_PATH_UTF16_UNITS: usize = 96;
/// Per-attempt TCP connect timeout for the pre-spawn port check.
pub const PORT_CHECK_TIMEOUT: Duration = Duration::from_millis(500);

/// Exact launch plan binding one qualified artifact, process config, secret
/// lease and owner fence to one executable, root, endpoint pair and deadline.
pub struct LaunchPlan {
    operation_id: OpaqueId,
    owner_fence: QdrantOwnerFence,
    artifact: QualifiedArtifact,
    config: QualifiedProcessConfig,
    endpoint_identity: QdrantEndpointIdentity,
    secret_binding: SecretLeaseBinding,
    expected_secret_purpose: Blake3Digest32,
    executable_path: PathBuf,
    executable_bytes: u64,
    data_dir: PathBuf,
    host: LoopbackHost,
    http_port: u16,
    grpc_port: u16,
    startup_timeout: Duration,
    shutdown_timeout: Duration,
    observed_tick: NonZeroU64,
}

impl LaunchPlan {
    /// Builds a coherent plan from the exact opaque lease capability.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation_id: OpaqueId,
        owner_fence: QdrantOwnerFence,
        artifact: QualifiedArtifact,
        config: QualifiedProcessConfig,
        secret_lease: &impl QdrantSecretLease,
        expected_secret_purpose: Blake3Digest32,
        executable_path: PathBuf,
        executable_bytes: u64,
        data_dir: PathBuf,
        host: LoopbackHost,
        http_port: u16,
        grpc_port: u16,
        startup_timeout: Duration,
        shutdown_timeout: Duration,
        observed_tick: NonZeroU64,
    ) -> Result<Self, SupervisorError> {
        if executable_path.as_os_str().is_empty()
            || data_dir.as_os_str().is_empty()
            || executable_bytes == 0
            || !data_dir.is_absolute()
            || data_dir.to_string_lossy().encode_utf16().count() > MAX_DATA_ROOT_PATH_UTF16_UNITS
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
        let candidate = artifact.candidate();
        let lease_binding = SecretLeaseBinding::from_lease(secret_lease);
        if config.secret_binding() != lease_binding
            || lease_binding.purpose_digest() != expected_secret_purpose
            || lease_binding.installation_incarnation_id()
                != owner_fence.installation_incarnation_id
            || lease_binding.expires_at_tick() <= observed_tick
        {
            return Err(SupervisorError::SecretLeaseInvalid);
        }
        if owner_fence != config.config().owner_fence
            || config.artifact_digest() != candidate.artifact_digest
        {
            return Err(SupervisorError::OwnerFenceMismatch);
        }
        if !config.config().bind_is_loopback || !config.config().single_node {
            return Err(if config.config().bind_is_loopback {
                SupervisorError::MultiNodeTopologyDenied
            } else {
                SupervisorError::NonLoopbackEndpoint
            });
        }
        let endpoint_identity = QdrantEndpointIdentity::from_launch(
            host,
            http_port,
            grpc_port,
            config.config().endpoint,
        )?;
        Ok(Self {
            operation_id,
            owner_fence,
            artifact,
            config,
            endpoint_identity,
            secret_binding: lease_binding,
            expected_secret_purpose,
            executable_path,
            executable_bytes,
            data_dir,
            host,
            http_port,
            grpc_port,
            startup_timeout,
            shutdown_timeout,
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

    /// Endpoint tuple sealed into the process identity and owned guard.
    #[must_use]
    pub const fn endpoint_identity(&self) -> QdrantEndpointIdentity {
        self.endpoint_identity
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
    pub const fn observed_tick(&self) -> NonZeroU64 {
        self.observed_tick
    }

    pub(crate) const fn secret_binding(&self) -> SecretLeaseBinding {
        self.secret_binding
    }

    pub(crate) const fn expected_secret_purpose(&self) -> Blake3Digest32 {
        self.expected_secret_purpose
    }

    pub(crate) fn executable_path(&self) -> &Path {
        &self.executable_path
    }

    pub(crate) const fn executable_bytes(&self) -> u64 {
        self.executable_bytes
    }

    pub(crate) const fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }
}

/// Printable argv without secrets: executable plus config path only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgvSnapshot(String);

impl ArgvSnapshot {
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

/// Process-owning guard returned immediately after CreateProcessW succeeds.
///
/// All post-create observations remain methods on this retained guard; an
/// uncertain identity/readiness outcome never discards it.
pub struct SpawnedChild {
    native: NativeProcess,
    argv_snapshot: ArgvSnapshot,
    containment: ContainmentReport,
}

impl SpawnedChild {
    #[must_use]
    pub const fn argv_snapshot(&self) -> &ArgvSnapshot {
        &self.argv_snapshot
    }

    #[must_use]
    pub const fn containment_report(&self) -> ContainmentReport {
        self.containment
    }

    pub(crate) fn into_native(self) -> NativeProcess {
        self.native
    }
}

/// Native start result. `OutcomeUnknown` owns the recovery guard and must not
/// be converted to an ordinary pre-spawn failure or dropped before recovery.
pub enum SpawnOutcome {
    Started(SpawnedChild),
    OutcomeUnknown {
        reason: SupervisorError,
        guard: SpawnedChild,
    },
}

impl SpawnOutcome {
    /// Reason attached to an ambiguous CreateProcessW return, if any.
    #[must_use]
    pub const fn unknown_reason(&self) -> Option<SupervisorError> {
        match self {
            Self::Started(_) => None,
            Self::OutcomeUnknown { reason, .. } => Some(*reason),
        }
    }

    /// Consumes the result while preserving its process/job guard.
    #[must_use]
    pub fn into_guard(self) -> SpawnedChild {
        match self {
            Self::Started(guard) | Self::OutcomeUnknown { guard, .. } => guard,
        }
    }

    /// Preserves both the owned guard and any ambiguous post-create result.
    #[must_use]
    pub fn into_parts(self) -> (SpawnedChild, Option<SupervisorError>) {
        match self {
            Self::Started(guard) => (guard, None),
            Self::OutcomeUnknown { reason, guard } => (guard, Some(reason)),
        }
    }
}

/// Spawns the exact qualified artifact after loopback, lease, config, ACL,
/// executable-handle and Job Object checks succeed.
pub fn spawn_qualified(
    plan: &LaunchPlan,
    secret_lease: &impl QdrantSecretLease,
    now_tick: NonZeroU64,
    cancel: &AtomicBool,
) -> Result<SpawnOutcome, SupervisorError> {
    if cancel.load(Ordering::Acquire) {
        return Err(SupervisorError::StartFailed);
    }
    let binding = SecretLeaseBinding::from_lease(secret_lease);
    if binding != plan.secret_binding
        || binding.purpose_digest() != plan.expected_secret_purpose
        || binding.installation_incarnation_id() != plan.owner_fence.installation_incarnation_id
        || binding.expires_at_tick() <= now_tick
    {
        return Err(SupervisorError::SecretLeaseInvalid);
    }
    check_loopback_port_free(&plan.host, plan.http_port)?;
    check_loopback_port_free(&plan.host, plan.grpc_port)?;
    let secret = SecretMaterial::from_lease(secret_lease, plan.secret_binding, now_tick)?;
    secret.validate_header_safe()?;
    let requested_argv =
        build_argv_snapshot(&plan.executable_path, &plan.data_dir.join(CONFIG_FILE_NAME));
    if secret.appears_in(requested_argv.as_str()) {
        return Err(SupervisorError::SecretLeaseInvalid);
    }
    let config_bytes = render_config_yaml(plan.host, plan.http_port, plan.grpc_port)?;
    if secret.appears_in(core::str::from_utf8(&config_bytes).unwrap_or_default()) {
        return Err(SupervisorError::SecretLeaseInvalid);
    }
    let config_path = plan.data_dir.join(CONFIG_FILE_NAME);
    let candidate = plan.artifact.candidate();
    if cancel.load(Ordering::Acquire) {
        return Err(SupervisorError::StartFailed);
    }
    let native = match NativeProcess::spawn(
        &plan.executable_path,
        candidate.sha256,
        plan.executable_bytes,
        &candidate.version,
        plan.operation_id.clone(),
        candidate.artifact_digest,
        plan.owner_fence,
        plan.endpoint_identity,
        plan.config.config().config_digest,
        plan.expected_secret_purpose,
        &plan.data_dir,
        &config_path,
        &config_bytes,
        secret,
    ) {
        Ok(native) => native,
        Err(NativeSpawnError::Definite(error)) => return Err(error),
        Err(NativeSpawnError::Unknown {
            reason,
            guard,
            containment,
        }) => {
            let guard = SpawnedChild {
                native: guard,
                argv_snapshot: build_argv_snapshot(&plan.executable_path, &config_path),
                containment,
            };
            return Ok(SpawnOutcome::OutcomeUnknown { reason, guard });
        }
    };
    let argv_snapshot = build_argv_snapshot(&plan.executable_path, native.config_path());
    if cancel.load(Ordering::Acquire) || native.secret_appears_in(argv_snapshot.as_str()) {
        // This is post-CreateProcess. Preserve the native guard in the typed
        // unknown result instead of dropping a possibly running process.
        let guard = SpawnedChild {
            native,
            argv_snapshot,
            containment: ContainmentReport::windows_verified(),
        };
        return Ok(SpawnOutcome::OutcomeUnknown {
            reason: if cancel.load(Ordering::Acquire) {
                SupervisorError::StartupOutcomeUnknown
            } else {
                SupervisorError::SecretLeaseInvalid
            },
            guard,
        });
    }
    Ok(SpawnOutcome::Started(SpawnedChild {
        native,
        argv_snapshot,
        containment: ContainmentReport::windows_verified(),
    }))
}

fn render_config_yaml(
    host: LoopbackHost,
    http_port: u16,
    grpc_port: u16,
) -> Result<Vec<u8>, SupervisorError> {
    if http_port == 0 || grpc_port == 0 || http_port == grpc_port {
        return Err(SupervisorError::InvalidProcessConfig);
    }
    Ok(format!(
        "storage:\n  storage_path: ./storage\nservice:\n  host: {}\n  http_port: {}\n  grpc_port: {}\n",
        host.as_str(),
        http_port,
        grpc_port
    )
    .into_bytes())
}

/// Proves no listener currently answers on a loopback port. A connectable or
/// inconclusive port is foreign/unknown and is never adopted.
pub fn check_loopback_port_free(host: &LoopbackHost, port: u16) -> Result<(), SupervisorError> {
    if port == 0 {
        return Err(SupervisorError::InvalidProcessConfig);
    }
    let address = socket_for(*host, port);
    match TcpStream::connect_timeout(&address, PORT_CHECK_TIMEOUT) {
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => Ok(()),
        _ => Err(SupervisorError::EndpointUnavailable),
    }
}

fn socket_for(host: LoopbackHost, port: u16) -> SocketAddr {
    match host {
        LoopbackHost::V4 => SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        LoopbackHost::V6 => SocketAddr::from((Ipv6Addr::LOCALHOST, port)),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{build_argv_snapshot, render_config_yaml};
    use crate::LoopbackHost;

    #[test]
    fn argv_and_materialized_config_have_no_secret_channel() {
        let secret = "fixture-api-key-that-must-not-appear";
        let argv = build_argv_snapshot(
            Path::new("C:\\Tools\\Qdrant\\qdrant.exe"),
            Path::new("C:\\qtmp\\owned\\config.yaml"),
        );
        let config = render_config_yaml(LoopbackHost::V4, 6333, 6334).unwrap();
        let config = String::from_utf8(config).unwrap();
        assert!(!argv.as_str().contains(secret));
        assert!(!config.contains(secret));
        assert!(!config.contains("api_key"));
        assert!(config.contains("host: 127.0.0.1"));
        assert!(config.contains("http_port: 6333"));
        assert!(config.contains("grpc_port: 6334"));
    }
}
