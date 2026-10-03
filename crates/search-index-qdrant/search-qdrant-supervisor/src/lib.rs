//! Qualified Qdrant artifact and native process-lifecycle supervision.
//!
//! Windows process handles, Job Objects and ACL APIs are private to the
//! platform adapter. Public callers receive Eliot-owned identities and
//! content-free receipts only.

#![deny(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use core::num::{NonZeroU16, NonZeroU32, NonZeroU64};

use search_contracts::{
    ArtifactDigest, Blake3Digest32, DataRootId, InstallationIncarnationId, OpaqueId, OwnerEpoch,
    ReceiptRef, Sha256Digest32,
};

pub mod containment;
pub mod identity;
pub mod launch;
pub mod owned;
pub mod secret;
pub mod sha256;
#[cfg(windows)]
#[allow(unsafe_code)]
mod win32;
#[cfg(not(windows))]
mod win32;

pub use containment::{ContainmentReport, LoopbackHost, parse_loopback_host};
pub use identity::{
    ExecutableExpectation, MAX_PROBE_TIMEOUT, MIN_PROBE_TIMEOUT, PROBE_POLL_INTERVAL,
    QUALIFIED_EXE_BYTES, QUALIFIED_EXE_SHA256_HEX, QUALIFIED_QDRANT_VERSION, VERSION_PROBE_ARG,
    VerifiedExecutable, parse_qdrant_version_output, verify_executable_identity,
};
pub use launch::{
    ArgvSnapshot, CONFIG_FILE_NAME, LaunchPlan, MAX_DATA_ROOT_PATH_UTF16_UNITS, MAX_LAUNCH_TIMEOUT,
    MIN_LAUNCH_TIMEOUT, PORT_CHECK_TIMEOUT, QDRANT_API_KEY_ENV, QDRANT_CONFIG_ARG, SpawnOutcome,
    SpawnedChild, build_argv_snapshot, check_loopback_port_free, spawn_qualified,
};
pub use owned::{
    MAX_PROBE_RESPONSE_BYTES, OwnedChild, PROBE_SOCKET_TIMEOUT, READINESS_POLL_INTERVAL,
};
pub use secret::{MAX_SECRET_BYTES, QdrantSecretLease, SecretLeaseBinding, SecretMaterial};
pub use sha256::{hex_lower, sha256_bytes, sha256_file};

/// Endpoint tuple fixed by one validated launch plan and carried by its
/// OS-owned process identity. The digest is opaque and equality-only; the
/// host and both ports remain explicit for connection and shutdown checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QdrantEndpointIdentity {
    host: LoopbackHost,
    http_port: NonZeroU16,
    grpc_port: NonZeroU16,
    endpoint_digest: Blake3Digest32,
}

impl QdrantEndpointIdentity {
    /// Creates one exact loopback endpoint identity for process planning.
    pub fn new(
        host: LoopbackHost,
        http_port: u16,
        grpc_port: u16,
        endpoint_digest: Blake3Digest32,
    ) -> Result<Self, SupervisorError> {
        let http_port = NonZeroU16::new(http_port).ok_or(SupervisorError::InvalidProcessConfig)?;
        let grpc_port = NonZeroU16::new(grpc_port).ok_or(SupervisorError::InvalidProcessConfig)?;
        if http_port == grpc_port {
            return Err(SupervisorError::InvalidProcessConfig);
        }
        Ok(Self {
            host,
            http_port,
            grpc_port,
            endpoint_digest,
        })
    }

    pub(crate) fn from_launch(
        host: LoopbackHost,
        http_port: u16,
        grpc_port: u16,
        configured: LoopbackEndpoint,
    ) -> Result<Self, SupervisorError> {
        if u32::from(http_port) != configured.port.get() {
            return Err(SupervisorError::EndpointIdentityMismatch);
        }
        Self::new(host, http_port, grpc_port, configured.endpoint_digest)
    }

    /// Exact loopback literal fixed by the launch plan.
    #[must_use]
    pub const fn host(self) -> LoopbackHost {
        self.host
    }

    /// HTTP port fixed by the launch plan.
    #[must_use]
    pub const fn http_port(self) -> NonZeroU16 {
        self.http_port
    }

    /// gRPC port fixed by the launch plan.
    #[must_use]
    pub const fn grpc_port(self) -> NonZeroU16 {
        self.grpc_port
    }

    /// Existing opaque endpoint token from the validated process config.
    #[must_use]
    pub const fn endpoint_digest(self) -> Blake3Digest32 {
        self.endpoint_digest
    }
}

/// Configured HTTP endpoint identity. The launch adapter adds its selected
/// gRPC port and exact loopback literal to the OS-backed process identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopbackEndpoint {
    pub endpoint_digest: Blake3Digest32,
    pub port: NonZeroU32,
}

/// Closed Qdrant-supervisor failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SupervisorError {
    InvalidArtifact,
    ArtifactDigestMismatch,
    ArtifactVersionMismatch,
    ArtifactArchitectureMismatch,
    QualificationEvidenceMissing,
    NonLoopbackEndpoint,
    MultiNodeTopologyDenied,
    DataRootMismatch,
    OwnerFenceMismatch,
    SecretLeaseInvalid,
    InvalidProcessConfig,
    InvalidLifecycleTransition,
    ProcessIdentityMismatch,
    ExecutableIdentityMismatch,
    EndpointIdentityMismatch,
    ProcessNotReady,
    StartupOutcomeUnknown,
    ShutdownOutcomeUnknown,
    RestartBudgetExceeded,
    Quarantined,
    ContainmentUnavailable,
    PlatformUnavailable,
    ExecutableProbeFailed,
    EndpointUnavailable,
    StartFailed,
}

impl SupervisorError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidArtifact => "QDRANT_ARTIFACT_INVALID",
            Self::ArtifactDigestMismatch => "QDRANT_ARTIFACT_DIGEST_MISMATCH",
            Self::ArtifactVersionMismatch => "QDRANT_ARTIFACT_VERSION_MISMATCH",
            Self::ArtifactArchitectureMismatch => "QDRANT_ARTIFACT_ARCH_MISMATCH",
            Self::QualificationEvidenceMissing => "QDRANT_QUALIFICATION_EVIDENCE_MISSING",
            Self::NonLoopbackEndpoint => "QDRANT_NON_LOOPBACK_ENDPOINT",
            Self::MultiNodeTopologyDenied => "QDRANT_MULTI_NODE_TOPOLOGY_DENIED",
            Self::DataRootMismatch => "QDRANT_DATA_ROOT_MISMATCH",
            Self::OwnerFenceMismatch => "QDRANT_OWNER_FENCE_MISMATCH",
            Self::SecretLeaseInvalid => "QDRANT_SECRET_LEASE_INVALID",
            Self::InvalidProcessConfig => "QDRANT_PROCESS_CONFIG_INVALID",
            Self::InvalidLifecycleTransition => "QDRANT_LIFECYCLE_INVALID",
            Self::ProcessIdentityMismatch => "QDRANT_PROCESS_IDENTITY_MISMATCH",
            Self::ExecutableIdentityMismatch => "QDRANT_EXECUTABLE_IDENTITY_MISMATCH",
            Self::EndpointIdentityMismatch => "QDRANT_ENDPOINT_IDENTITY_MISMATCH",
            Self::ProcessNotReady => "QDRANT_PROCESS_NOT_READY",
            Self::StartupOutcomeUnknown => "QDRANT_STARTUP_OUTCOME_UNKNOWN",
            Self::ShutdownOutcomeUnknown => "QDRANT_SHUTDOWN_OUTCOME_UNKNOWN",
            Self::RestartBudgetExceeded => "QDRANT_RESTART_BUDGET_EXCEEDED",
            Self::Quarantined => "QDRANT_PROCESS_QUARANTINED",
            Self::ContainmentUnavailable => "QDRANT_CONTAINMENT_UNAVAILABLE",
            Self::PlatformUnavailable => "QDRANT_PLATFORM_UNAVAILABLE",
            Self::ExecutableProbeFailed => "QDRANT_EXECUTABLE_PROBE_FAILED",
            Self::EndpointUnavailable => "QDRANT_ENDPOINT_UNAVAILABLE",
            Self::StartFailed => "QDRANT_START_FAILED",
        }
    }
}

impl fmt::Display for SupervisorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SupervisorError {}

/// Supported executable architecture.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ArtifactArchitecture {
    X86_64Windows,
    X86_64Linux,
    Aarch64Windows,
    Aarch64Linux,
}

/// Observed immutable local executable candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactCandidate {
    pub file_identity_digest: Blake3Digest32,
    pub sha256: Sha256Digest32,
    pub artifact_digest: ArtifactDigest,
    pub version: String,
    pub build_identity: String,
    pub architecture: ArtifactArchitecture,
}

/// Accepted immutable qualification manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactQualificationManifest {
    pub expected_sha256: Sha256Digest32,
    pub expected_artifact_digest: ArtifactDigest,
    pub expected_version: String,
    pub expected_build_identity: String,
    pub expected_architecture: ArtifactArchitecture,
    pub source_receipt: ReceiptRef,
    pub license_receipt: ReceiptRef,
    pub probe_manifest_digest: Blake3Digest32,
}

/// Exact qualified Qdrant artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedArtifact {
    candidate: ArtifactCandidate,
    qualification_digest: Blake3Digest32,
}

impl QualifiedArtifact {
    #[must_use]
    pub const fn candidate(&self) -> &ArtifactCandidate {
        &self.candidate
    }

    #[must_use]
    pub const fn qualification_digest(&self) -> Blake3Digest32 {
        self.qualification_digest
    }
}

/// Verifies one exact local artifact against an accepted manifest.
pub fn qualify_artifact(
    candidate: ArtifactCandidate,
    manifest: &ArtifactQualificationManifest,
    qualification_digest: Blake3Digest32,
) -> Result<QualifiedArtifact, SupervisorError> {
    if candidate.version.is_empty()
        || candidate.version.len() > 128
        || candidate.build_identity.is_empty()
        || candidate.build_identity.len() > 256
    {
        return Err(SupervisorError::InvalidArtifact);
    }
    if candidate.sha256 != manifest.expected_sha256
        || candidate.artifact_digest != manifest.expected_artifact_digest
    {
        return Err(SupervisorError::ArtifactDigestMismatch);
    }
    if candidate.version != manifest.expected_version
        || candidate.build_identity != manifest.expected_build_identity
    {
        return Err(SupervisorError::ArtifactVersionMismatch);
    }
    if candidate.architecture != manifest.expected_architecture {
        return Err(SupervisorError::ArtifactArchitectureMismatch);
    }
    Ok(QualifiedArtifact {
        candidate,
        qualification_digest,
    })
}

/// Exact owner fence inherited from the Search daemon.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QdrantOwnerFence {
    pub data_root_id: DataRootId,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub owner_epoch: OwnerEpoch,
}

/// Candidate process configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessConfig {
    pub owner_fence: QdrantOwnerFence,
    pub data_directory_digest: Blake3Digest32,
    pub endpoint: LoopbackEndpoint,
    pub bind_is_loopback: bool,
    pub single_node: bool,
    pub startup_timeout_ticks: NonZeroU64,
    pub shutdown_timeout_ticks: NonZeroU64,
    pub restart_window_ticks: NonZeroU64,
    pub max_restarts_per_window: usize,
    pub config_digest: Blake3Digest32,
}

/// Process configuration accepted for one artifact and secret lease.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedProcessConfig {
    config: ProcessConfig,
    artifact_digest: ArtifactDigest,
    secret_binding: SecretLeaseBinding,
}

impl QualifiedProcessConfig {
    #[must_use]
    pub const fn config(&self) -> &ProcessConfig {
        &self.config
    }

    pub(crate) const fn artifact_digest(&self) -> ArtifactDigest {
        self.artifact_digest
    }

    pub(crate) const fn secret_binding(&self) -> SecretLeaseBinding {
        self.secret_binding
    }
}

/// Validates loopback-only one-node process configuration.
pub fn validate_process_config(
    config: ProcessConfig,
    artifact: &QualifiedArtifact,
    secret: &impl QdrantSecretLease,
    observed_tick: NonZeroU64,
) -> Result<QualifiedProcessConfig, SupervisorError> {
    let secret_binding = SecretLeaseBinding::from_lease(secret);
    if !config.bind_is_loopback {
        return Err(SupervisorError::NonLoopbackEndpoint);
    }
    if !config.single_node {
        return Err(SupervisorError::MultiNodeTopologyDenied);
    }
    if config.max_restarts_per_window == 0 || config.max_restarts_per_window > 100 {
        return Err(SupervisorError::InvalidProcessConfig);
    }
    if secret_binding.installation_incarnation_id()
        != config.owner_fence.installation_incarnation_id
        || secret_binding.expires_at_tick() <= observed_tick
    {
        return Err(SupervisorError::SecretLeaseInvalid);
    }
    Ok(QualifiedProcessConfig {
        config,
        artifact_digest: artifact.candidate.artifact_digest,
        secret_binding,
    })
}

/// Reuse-resistant observed process identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessIdentity {
    process_id: NonZeroU32,
    creation_marker: NonZeroU64,
    executable_sha256: Sha256Digest32,
    artifact_digest: ArtifactDigest,
    owner_fence: QdrantOwnerFence,
    endpoint: QdrantEndpointIdentity,
    secret_binding: SecretLeaseBinding,
}

impl ProcessIdentity {
    pub(crate) const fn from_platform(
        process_id: NonZeroU32,
        creation_marker: NonZeroU64,
        executable_sha256: Sha256Digest32,
        artifact_digest: ArtifactDigest,
        owner_fence: QdrantOwnerFence,
        endpoint: QdrantEndpointIdentity,
        secret_binding: SecretLeaseBinding,
    ) -> Self {
        Self {
            process_id,
            creation_marker,
            executable_sha256,
            artifact_digest,
            owner_fence,
            endpoint,
            secret_binding,
        }
    }

    /// OS process identifier.
    #[must_use]
    pub const fn process_id(self) -> NonZeroU32 {
        self.process_id
    }

    /// OS process creation time in the platform identity domain.
    #[must_use]
    pub const fn creation_marker(self) -> NonZeroU64 {
        self.creation_marker
    }

    /// SHA-256 of the executable image bound to the process handle.
    #[must_use]
    pub const fn executable_sha256(self) -> Sha256Digest32 {
        self.executable_sha256
    }

    /// Qualified server artifact identity.
    #[must_use]
    pub const fn artifact_digest(self) -> ArtifactDigest {
        self.artifact_digest
    }

    /// Owner fence supplied to the exact launch operation.
    #[must_use]
    pub const fn owner_fence(self) -> QdrantOwnerFence {
        self.owner_fence
    }

    /// Authenticated loopback endpoint configured for this process.
    #[must_use]
    pub const fn endpoint(self) -> QdrantEndpointIdentity {
        self.endpoint
    }

    /// Content-free binding to the process API-key lease.
    #[must_use]
    pub const fn secret_binding(self) -> SecretLeaseBinding {
        self.secret_binding
    }
}

/// Exact process startup effect for a platform adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartProcessEffect {
    pub operation_id: OpaqueId,
    pub artifact: QualifiedArtifact,
    pub config: QualifiedProcessConfig,
}

/// Exact process shutdown effect for a platform adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownProcessEffect {
    pub identity: ProcessIdentity,
    pub force_after_tick: NonZeroU64,
}

/// Truthful readiness observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessReadiness {
    identity: ProcessIdentity,
    observed_config_digest: Blake3Digest32,
}

impl ProcessReadiness {
    pub(crate) const fn from_owned_process(
        identity: ProcessIdentity,
        observed_config_digest: Blake3Digest32,
    ) -> Self {
        Self {
            identity,
            observed_config_digest,
        }
    }

    /// OS-verified process identity bound to authenticated loopback health.
    #[must_use]
    pub const fn identity(&self) -> ProcessIdentity {
        self.identity
    }

    /// Exact process configuration digest observed during readiness.
    #[must_use]
    pub const fn observed_config_digest(&self) -> Blake3Digest32 {
        self.observed_config_digest
    }
}

/// Child exit observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExitObservation {
    identity: ProcessIdentity,
    expected_shutdown: bool,
    exit_code: Option<i32>,
    observed_tick: NonZeroU64,
    job_empty: bool,
}

impl ExitObservation {
    pub(crate) const fn from_process_handle(
        identity: ProcessIdentity,
        expected_shutdown: bool,
        exit_code: Option<i32>,
        observed_tick: NonZeroU64,
        job_empty: bool,
    ) -> Self {
        Self {
            identity,
            expected_shutdown,
            exit_code,
            observed_tick,
            job_empty,
        }
    }
}

/// Confirmed process-tree and endpoint absence after bounded shutdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownReceipt {
    identity: ProcessIdentity,
    forced: bool,
    graceful_signal_sent: bool,
}

impl ShutdownReceipt {
    pub(crate) const fn from_process_handle(
        identity: ProcessIdentity,
        forced: bool,
        graceful_signal_sent: bool,
    ) -> Self {
        Self {
            identity,
            forced,
            graceful_signal_sent,
        }
    }

    /// Identity of the stopped root process.
    #[must_use]
    pub const fn identity(self) -> ProcessIdentity {
        self.identity
    }

    /// Whether shutdown required forced Job Object termination.
    #[must_use]
    pub const fn forced(self) -> bool {
        self.forced
    }

    /// Whether Windows accepted a process-group graceful signal.
    #[must_use]
    pub const fn graceful_signal_sent(self) -> bool {
        self.graceful_signal_sent
    }
}

/// Recovery proof for an ambiguous CreateProcessW result.
///
/// It is emitted only after the private Job Object reports zero active
/// processes and both originally planned loopback endpoints refuse connects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartRecoveryReceipt {
    operation_id: OpaqueId,
    process_was_created: bool,
    forced: bool,
}

impl StartRecoveryReceipt {
    pub(crate) const fn from_empty_job(
        operation_id: OpaqueId,
        process_was_created: bool,
        forced: bool,
    ) -> Self {
        Self {
            operation_id,
            process_was_created,
            forced,
        }
    }

    /// Whether the Job Object ever observed a process in this launch.
    #[must_use]
    pub const fn process_was_created(&self) -> bool {
        self.process_was_created
    }

    /// Whether recovery needed hard Job Object termination.
    #[must_use]
    pub const fn forced(&self) -> bool {
        self.forced
    }
}

/// Restart policy decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartDecision {
    Stop,
    Restart,
    Quarantine,
}

/// Closed process lifecycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupervisorState {
    Stopped,
    Starting(StartProcessEffect),
    StartupOutcomeUnknown {
        effect: StartProcessEffect,
        identity: Option<ProcessIdentity>,
    },
    Ready {
        identity: ProcessIdentity,
        artifact: QualifiedArtifact,
        config: QualifiedProcessConfig,
    },
    Draining {
        identity: ProcessIdentity,
        artifact: QualifiedArtifact,
        config: QualifiedProcessConfig,
    },
    ShutdownOutcomeUnknown {
        identity: ProcessIdentity,
        artifact: QualifiedArtifact,
        config: QualifiedProcessConfig,
    },
    Quarantined(SupervisorError),
}

/// Process-local Qdrant lifecycle supervisor.
#[derive(Clone, Debug)]
pub struct QdrantSupervisor {
    state: SupervisorState,
    restart_window_started_at: Option<NonZeroU64>,
    restart_count: usize,
}

impl Default for QdrantSupervisor {
    fn default() -> Self {
        Self {
            state: SupervisorState::Stopped,
            restart_window_started_at: None,
            restart_count: 0,
        }
    }
}

impl QdrantSupervisor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn state(&self) -> &SupervisorState {
        &self.state
    }

    pub fn prepare_start(
        &mut self,
        operation_id: OpaqueId,
        artifact: QualifiedArtifact,
        config: QualifiedProcessConfig,
    ) -> Result<StartProcessEffect, SupervisorError> {
        if !matches!(self.state, SupervisorState::Stopped) {
            return Err(match self.state {
                SupervisorState::Quarantined(_) => SupervisorError::Quarantined,
                _ => SupervisorError::InvalidLifecycleTransition,
            });
        }
        let effect = StartProcessEffect {
            operation_id,
            artifact,
            config,
        };
        self.state = SupervisorState::Starting(effect.clone());
        Ok(effect)
    }

    /// Abandons a start that never produced a child (identity, containment,
    /// or spawn refused before any process existed). Returns to `Stopped`
    /// without consuming restart budget; quarantine is sticky.
    pub fn abort_start(&mut self) -> Result<(), SupervisorError> {
        match &self.state {
            SupervisorState::Starting(_) => {
                self.state = SupervisorState::Stopped;
                Ok(())
            }
            SupervisorState::Quarantined(_) => Err(SupervisorError::Quarantined),
            _ => Err(SupervisorError::InvalidLifecycleTransition),
        }
    }

    pub fn mark_startup_unknown(&mut self) -> Result<(), SupervisorError> {
        let SupervisorState::Starting(effect) = &self.state else {
            return Err(SupervisorError::InvalidLifecycleTransition);
        };
        self.state = SupervisorState::StartupOutcomeUnknown {
            effect: effect.clone(),
            identity: None,
        };
        Ok(())
    }

    /// Records the identity returned by the native spawn adapter. Startup is
    /// still outcome-unknown until the authenticated readiness probes pass.
    pub fn record_spawned(&mut self, identity: ProcessIdentity) -> Result<(), SupervisorError> {
        let SupervisorState::Starting(effect) = &self.state else {
            return Err(SupervisorError::InvalidLifecycleTransition);
        };
        verify_process_identity(effect, identity)?;
        self.state = SupervisorState::StartupOutcomeUnknown {
            effect: effect.clone(),
            identity: Some(identity),
        };
        Ok(())
    }

    pub fn confirm_ready(&mut self, readiness: ProcessReadiness) -> Result<(), SupervisorError> {
        let (effect, expected_identity) = match &self.state {
            SupervisorState::StartupOutcomeUnknown { effect, identity } => {
                (effect.clone(), *identity)
            }
            _ => return Err(SupervisorError::InvalidLifecycleTransition),
        };
        let identity = readiness.identity;
        if expected_identity != Some(identity) {
            return Err(SupervisorError::ProcessIdentityMismatch);
        }
        verify_process_identity(&effect, identity)?;
        if readiness.observed_config_digest != effect.config.config.config_digest {
            return Err(SupervisorError::InvalidProcessConfig);
        }
        self.state = SupervisorState::Ready {
            identity,
            artifact: effect.artifact,
            config: effect.config,
        };
        Ok(())
    }

    /// Resolves a startup whose process handle could not be returned. Only
    /// the guard's exact operation identity and verified empty Job receipt can
    /// clear the unknown outcome.
    pub fn confirm_start_recovered(
        &mut self,
        receipt: StartRecoveryReceipt,
    ) -> Result<(), SupervisorError> {
        let SupervisorState::StartupOutcomeUnknown {
            effect,
            identity: None,
        } = &self.state
        else {
            return Err(SupervisorError::InvalidLifecycleTransition);
        };
        if effect.operation_id != receipt.operation_id {
            return Err(SupervisorError::ProcessIdentityMismatch);
        }
        self.state = SupervisorState::Stopped;
        Ok(())
    }

    pub fn classify_exit(
        &mut self,
        observation: ExitObservation,
    ) -> Result<RestartDecision, SupervisorError> {
        let (identity, config) = match &self.state {
            SupervisorState::Ready {
                identity, config, ..
            }
            | SupervisorState::Draining {
                identity, config, ..
            }
            | SupervisorState::StartupOutcomeUnknown {
                effect: StartProcessEffect { config, .. },
                identity: Some(identity),
            } => (*identity, config),
            _ => return Err(SupervisorError::InvalidLifecycleTransition),
        };
        if observation.identity != identity {
            self.state = SupervisorState::Quarantined(SupervisorError::ProcessIdentityMismatch);
            return Ok(RestartDecision::Quarantine);
        }
        if !observation.job_empty {
            self.state = SupervisorState::Quarantined(SupervisorError::ShutdownOutcomeUnknown);
            return Ok(RestartDecision::Quarantine);
        }
        if observation.expected_shutdown {
            self.state = SupervisorState::Stopped;
            return Ok(RestartDecision::Stop);
        }

        let window = config.config.restart_window_ticks.get();
        let now = observation.observed_tick.get();
        let reset_window = self
            .restart_window_started_at
            .is_none_or(|start| now.saturating_sub(start.get()) >= window);
        if reset_window {
            self.restart_window_started_at = Some(observation.observed_tick);
            self.restart_count = 0;
        }
        self.restart_count = self.restart_count.saturating_add(1);
        if self.restart_count > config.config.max_restarts_per_window {
            self.state = SupervisorState::Quarantined(SupervisorError::RestartBudgetExceeded);
            Ok(RestartDecision::Quarantine)
        } else {
            self.state = SupervisorState::Stopped;
            Ok(RestartDecision::Restart)
        }
    }

    pub fn begin_shutdown(
        &mut self,
        now: NonZeroU64,
    ) -> Result<ShutdownProcessEffect, SupervisorError> {
        let (identity, artifact, config) = match &self.state {
            SupervisorState::Ready {
                identity,
                artifact,
                config,
            } => (*identity, artifact.clone(), config.clone()),
            SupervisorState::StartupOutcomeUnknown {
                effect,
                identity: Some(identity),
            } => (*identity, effect.artifact.clone(), effect.config.clone()),
            _ => return Err(SupervisorError::InvalidLifecycleTransition),
        };
        let force_after = now
            .get()
            .checked_add(config.config.shutdown_timeout_ticks.get())
            .and_then(NonZeroU64::new)
            .ok_or(SupervisorError::InvalidProcessConfig)?;
        self.state = SupervisorState::Draining {
            identity,
            artifact,
            config,
        };
        Ok(ShutdownProcessEffect {
            identity,
            force_after_tick: force_after,
        })
    }

    pub fn mark_shutdown_unknown(&mut self) -> Result<(), SupervisorError> {
        let SupervisorState::Draining {
            identity,
            artifact,
            config,
        } = &self.state
        else {
            return Err(SupervisorError::InvalidLifecycleTransition);
        };
        self.state = SupervisorState::ShutdownOutcomeUnknown {
            identity: *identity,
            artifact: artifact.clone(),
            config: config.clone(),
        };
        Ok(())
    }

    pub fn confirm_stopped(&mut self, receipt: ShutdownReceipt) -> Result<(), SupervisorError> {
        let expected = match &self.state {
            SupervisorState::Draining { identity, .. }
            | SupervisorState::ShutdownOutcomeUnknown { identity, .. } => *identity,
            _ => return Err(SupervisorError::InvalidLifecycleTransition),
        };
        if receipt.identity != expected {
            return Err(SupervisorError::ProcessIdentityMismatch);
        }
        self.state = SupervisorState::Stopped;
        Ok(())
    }

    pub fn quarantine(&mut self, reason: SupervisorError) {
        self.state = SupervisorState::Quarantined(reason);
    }
}

fn verify_process_identity(
    effect: &StartProcessEffect,
    identity: ProcessIdentity,
) -> Result<(), SupervisorError> {
    let expected_artifact = effect.artifact.candidate();
    let expected_config = effect.config.config();
    if identity.owner_fence != expected_config.owner_fence {
        return Err(SupervisorError::OwnerFenceMismatch);
    }
    if identity.artifact_digest != expected_artifact.artifact_digest
        || identity.executable_sha256 != expected_artifact.sha256
        || identity.secret_binding != effect.config.secret_binding
    {
        return Err(SupervisorError::ExecutableIdentityMismatch);
    }
    if identity.endpoint.endpoint_digest() != expected_config.endpoint.endpoint_digest
        || u32::from(identity.endpoint.http_port().get()) != expected_config.endpoint.port.get()
    {
        return Err(SupervisorError::EndpointIdentityMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    struct FixtureLease;

    impl QdrantSecretLease for FixtureLease {
        fn secret_reference_digest(&self) -> Blake3Digest32 {
            Blake3Digest32::from_bytes([0x41; 32])
        }

        fn installation_incarnation_id(&self) -> InstallationIncarnationId {
            InstallationIncarnationId::from_bytes([0x42; 16])
        }

        fn purpose_digest(&self) -> Blake3Digest32 {
            Blake3Digest32::from_bytes([0x43; 32])
        }

        fn expires_at_tick(&self) -> NonZeroU64 {
            NonZeroU64::new(10_000).expect("nonzero")
        }

        fn with_secret_bytes<R>(&self, use_bytes: impl FnOnce(&[u8]) -> R) -> R {
            use_bytes(b"state-machine-fixture-key")
        }
    }

    fn fixture() -> (QualifiedArtifact, QualifiedProcessConfig, QdrantOwnerFence) {
        let owner = QdrantOwnerFence {
            data_root_id: DataRootId::from_bytes([0x44; 16]),
            installation_incarnation_id: InstallationIncarnationId::from_bytes([0x42; 16]),
            owner_epoch: OwnerEpoch::new(1).expect("nonzero"),
        };
        let sha256 = Sha256Digest32::from_bytes([0x45; 32]);
        let artifact_digest = ArtifactDigest::from_bytes([0x46; 32]);
        let candidate = ArtifactCandidate {
            file_identity_digest: Blake3Digest32::from_bytes([0x47; 32]),
            sha256,
            artifact_digest,
            version: "1.19.0".to_owned(),
            build_identity: "fixture".to_owned(),
            architecture: ArtifactArchitecture::X86_64Windows,
        };
        let manifest = ArtifactQualificationManifest {
            expected_sha256: sha256,
            expected_artifact_digest: artifact_digest,
            expected_version: "1.19.0".to_owned(),
            expected_build_identity: "fixture".to_owned(),
            expected_architecture: ArtifactArchitecture::X86_64Windows,
            source_receipt: ReceiptRef::new("fixture-source").unwrap(),
            license_receipt: ReceiptRef::new("fixture-license").unwrap(),
            probe_manifest_digest: Blake3Digest32::from_bytes([0x48; 32]),
        };
        let artifact =
            qualify_artifact(candidate, &manifest, Blake3Digest32::from_bytes([0x49; 32])).unwrap();
        let lease = FixtureLease;
        let config = ProcessConfig {
            owner_fence: owner,
            data_directory_digest: Blake3Digest32::from_bytes([0x4A; 32]),
            endpoint: LoopbackEndpoint {
                endpoint_digest: Blake3Digest32::from_bytes([0x4B; 32]),
                port: NonZeroU32::new(6333).unwrap(),
            },
            bind_is_loopback: true,
            single_node: true,
            startup_timeout_ticks: NonZeroU64::new(1_200).unwrap(),
            shutdown_timeout_ticks: NonZeroU64::new(300).unwrap(),
            restart_window_ticks: NonZeroU64::new(600).unwrap(),
            max_restarts_per_window: 2,
            config_digest: Blake3Digest32::from_bytes([0x4C; 32]),
        };
        let qualified =
            validate_process_config(config, &artifact, &lease, NonZeroU64::new(1).unwrap())
                .unwrap();
        (artifact, qualified, owner)
    }

    fn identity(
        artifact: &QualifiedArtifact,
        config: &QualifiedProcessConfig,
        owner: QdrantOwnerFence,
        pid: u32,
        created: u64,
    ) -> ProcessIdentity {
        let endpoint = QdrantEndpointIdentity::new(
            LoopbackHost::V4,
            6333,
            6334,
            config.config.endpoint.endpoint_digest,
        )
        .unwrap();
        ProcessIdentity::from_platform(
            NonZeroU32::new(pid).unwrap(),
            NonZeroU64::new(created).unwrap(),
            artifact.candidate.sha256,
            artifact.candidate.artifact_digest,
            owner,
            endpoint,
            config.secret_binding,
        )
    }

    fn enter_ready(
        supervisor: &mut QdrantSupervisor,
        artifact: &QualifiedArtifact,
        config: &QualifiedProcessConfig,
        identity: ProcessIdentity,
        operation: &'static str,
    ) {
        supervisor
            .prepare_start(
                OpaqueId::new(operation).unwrap(),
                artifact.clone(),
                config.clone(),
            )
            .unwrap();
        supervisor.mark_startup_unknown().unwrap();
        supervisor.record_spawned(identity).unwrap();
        supervisor
            .confirm_ready(ProcessReadiness::from_owned_process(
                identity,
                config.config.config_digest,
            ))
            .unwrap();
    }

    fn unexpected_exit(
        supervisor: &mut QdrantSupervisor,
        identity: ProcessIdentity,
        tick: u64,
        job_empty: bool,
    ) -> RestartDecision {
        supervisor
            .classify_exit(ExitObservation::from_process_handle(
                identity,
                false,
                Some(1),
                NonZeroU64::new(tick).unwrap(),
                job_empty,
            ))
            .unwrap()
    }

    #[test]
    fn restart_budget_is_bounded_and_nonempty_job_quarantines() {
        let (artifact, config, owner) = fixture();
        let mut supervisor = QdrantSupervisor::new();
        let first = identity(&artifact, &config, owner, 701, 101);
        enter_ready(&mut supervisor, &artifact, &config, first, "restart-1");
        assert_eq!(
            unexpected_exit(&mut supervisor, first, 100, true),
            RestartDecision::Restart
        );

        let second = identity(&artifact, &config, owner, 702, 102);
        enter_ready(&mut supervisor, &artifact, &config, second, "restart-2");
        assert_eq!(
            unexpected_exit(&mut supervisor, second, 200, true),
            RestartDecision::Restart
        );

        let third = identity(&artifact, &config, owner, 703, 103);
        enter_ready(&mut supervisor, &artifact, &config, third, "restart-3");
        assert_eq!(
            unexpected_exit(&mut supervisor, third, 300, true),
            RestartDecision::Quarantine
        );
        assert!(matches!(
            supervisor.state(),
            SupervisorState::Quarantined(SupervisorError::RestartBudgetExceeded)
        ));

        let mut another = QdrantSupervisor::new();
        let identity = identity(&artifact, &config, owner, 704, 104);
        enter_ready(&mut another, &artifact, &config, identity, "nonempty-job");
        assert_eq!(
            unexpected_exit(&mut another, identity, 400, false),
            RestartDecision::Quarantine
        );
        assert!(matches!(
            another.state(),
            SupervisorState::Quarantined(SupervisorError::ShutdownOutcomeUnknown)
        ));
    }

    #[test]
    fn uncertain_start_forged_identity_exit_and_stop_stay_fenced() {
        let (artifact, config, owner) = fixture();
        let good = identity(&artifact, &config, owner, 801, 201);

        let mut hanging_start = QdrantSupervisor::new();
        hanging_start
            .prepare_start(
                OpaqueId::new("startup-hang").unwrap(),
                artifact.clone(),
                config.clone(),
            )
            .unwrap();
        hanging_start.mark_startup_unknown().unwrap();
        assert_eq!(
            hanging_start.abort_start().unwrap_err(),
            SupervisorError::InvalidLifecycleTransition
        );
        assert!(matches!(
            hanging_start.state(),
            SupervisorState::StartupOutcomeUnknown { identity: None, .. }
        ));

        let mut forged = QdrantSupervisor::new();
        forged
            .prepare_start(
                OpaqueId::new("forged-process").unwrap(),
                artifact.clone(),
                config.clone(),
            )
            .unwrap();
        let mut wrong_executable = good;
        wrong_executable.executable_sha256 = Sha256Digest32::from_bytes([0xEE; 32]);
        assert_eq!(
            forged.record_spawned(wrong_executable).unwrap_err(),
            SupervisorError::ExecutableIdentityMismatch
        );
        forged.abort_start().unwrap();

        let mut exited_under_reused_identity = QdrantSupervisor::new();
        enter_ready(
            &mut exited_under_reused_identity,
            &artifact,
            &config,
            good,
            "exit-reuse",
        );
        let reused = identity(&artifact, &config, owner, 802, 202);
        assert_eq!(
            unexpected_exit(&mut exited_under_reused_identity, reused, 500, true),
            RestartDecision::Quarantine
        );
        assert!(matches!(
            exited_under_reused_identity.state(),
            SupervisorState::Quarantined(SupervisorError::ProcessIdentityMismatch)
        ));

        let mut clean_stop = QdrantSupervisor::new();
        let stopped_identity = identity(&artifact, &config, owner, 803, 203);
        enter_ready(
            &mut clean_stop,
            &artifact,
            &config,
            stopped_identity,
            "clean-stop",
        );
        clean_stop
            .begin_shutdown(NonZeroU64::new(600).unwrap())
            .unwrap();
        clean_stop
            .confirm_stopped(ShutdownReceipt::from_process_handle(
                stopped_identity,
                false,
                true,
            ))
            .unwrap();
        assert!(matches!(clean_stop.state(), SupervisorState::Stopped));
    }

    #[test]
    fn readiness_rejects_a_different_process_or_config_digest() {
        let (artifact, config, owner) = fixture();
        let expected = identity(&artifact, &config, owner, 901, 301);
        let wrong_process = identity(&artifact, &config, owner, 902, 302);
        let mut supervisor = QdrantSupervisor::new();
        supervisor
            .prepare_start(
                OpaqueId::new("readiness-binding").unwrap(),
                artifact,
                config.clone(),
            )
            .unwrap();
        supervisor.mark_startup_unknown().unwrap();
        supervisor.record_spawned(expected).unwrap();
        assert_eq!(
            supervisor
                .confirm_ready(ProcessReadiness::from_owned_process(
                    wrong_process,
                    config.config.config_digest,
                ))
                .unwrap_err(),
            SupervisorError::ProcessIdentityMismatch
        );
        assert!(matches!(
            supervisor.state(),
            SupervisorState::StartupOutcomeUnknown { identity: Some(observed), .. }
                if *observed == expected
        ));
        assert_eq!(
            supervisor
                .confirm_ready(ProcessReadiness::from_owned_process(
                    expected,
                    Blake3Digest32::from_bytes([0xFE; 32]),
                ))
                .unwrap_err(),
            SupervisorError::InvalidProcessConfig
        );
        assert!(matches!(
            supervisor.state(),
            SupervisorState::StartupOutcomeUnknown { identity: Some(observed), .. }
                if *observed == expected
        ));
    }

    #[test]
    fn crashed_root_is_restartable_only_after_empty_job_observation() {
        let (artifact, config, owner) = fixture();
        let process = identity(&artifact, &config, owner, 903, 303);
        let mut supervisor = QdrantSupervisor::new();
        enter_ready(
            &mut supervisor,
            &artifact,
            &config,
            process,
            "crash-cleanup",
        );
        assert_eq!(
            supervisor
                .classify_exit(ExitObservation::from_process_handle(
                    process,
                    false,
                    Some(1),
                    NonZeroU64::new(700).unwrap(),
                    true,
                ))
                .unwrap(),
            RestartDecision::Restart
        );
        assert!(matches!(supervisor.state(), SupervisorState::Stopped));
    }
}
