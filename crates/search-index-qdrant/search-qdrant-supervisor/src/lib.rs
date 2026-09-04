//! Pure Qdrant artifact qualification and process-lifecycle supervision.
//!
//! The package never downloads, starts, kills, or probes a process itself.
//! Platform adapters execute the explicit effects produced here and return
//! exact process, executable, owner, endpoint, and readiness observations.

#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use core::num::{NonZeroU32, NonZeroU64};

use search_contracts::{
    ArtifactDigest, Blake3Digest32, DataRootId, InstallationIncarnationId,
    OpaqueId, OwnerEpoch, ReceiptRef, Sha256Digest32,
};

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

/// Loopback endpoint identity without unrestricted address disclosure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopbackEndpoint {
    pub endpoint_digest: Blake3Digest32,
    pub port: NonZeroU32,
}

/// Purpose/incarnation-bound secret-lease evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecretLeaseEvidence {
    pub secret_reference_digest: Blake3Digest32,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub purpose_digest: Blake3Digest32,
    pub expires_at_tick: NonZeroU64,
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
    secret_reference_digest: Blake3Digest32,
}

impl QualifiedProcessConfig {
    #[must_use]
    pub const fn config(&self) -> &ProcessConfig {
        &self.config
    }
}

/// Validates loopback-only one-node process configuration.
pub fn validate_process_config(
    config: ProcessConfig,
    artifact: &QualifiedArtifact,
    secret: SecretLeaseEvidence,
    observed_tick: NonZeroU64,
) -> Result<QualifiedProcessConfig, SupervisorError> {
    if !config.bind_is_loopback {
        return Err(SupervisorError::NonLoopbackEndpoint);
    }
    if !config.single_node {
        return Err(SupervisorError::MultiNodeTopologyDenied);
    }
    if config.max_restarts_per_window == 0 || config.max_restarts_per_window > 100 {
        return Err(SupervisorError::InvalidProcessConfig);
    }
    if secret.installation_incarnation_id != config.owner_fence.installation_incarnation_id
        || secret.expires_at_tick <= observed_tick
    {
        return Err(SupervisorError::SecretLeaseInvalid);
    }
    Ok(QualifiedProcessConfig {
        config,
        artifact_digest: artifact.candidate.artifact_digest,
        secret_reference_digest: secret.secret_reference_digest,
    })
}

/// Reuse-resistant observed process identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessIdentity {
    pub process_id: NonZeroU32,
    pub creation_marker: NonZeroU64,
    pub executable_file_digest: Blake3Digest32,
    pub artifact_digest: ArtifactDigest,
    pub owner_fence: QdrantOwnerFence,
    pub endpoint: LoopbackEndpoint,
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
    pub identity: ProcessIdentity,
    pub authenticated_health_ok: bool,
    pub observed_config_digest: Blake3Digest32,
    pub readiness_receipt: ReceiptRef,
}

/// Child exit observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExitObservation {
    pub identity: ProcessIdentity,
    pub expected_shutdown: bool,
    pub exit_code: Option<i32>,
    pub observed_tick: NonZeroU64,
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
    StartupOutcomeUnknown(StartProcessEffect),
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

    pub fn mark_startup_unknown(&mut self) -> Result<(), SupervisorError> {
        let SupervisorState::Starting(effect) = &self.state else {
            return Err(SupervisorError::InvalidLifecycleTransition);
        };
        self.state = SupervisorState::StartupOutcomeUnknown(effect.clone());
        Ok(())
    }

    pub fn confirm_ready(
        &mut self,
        readiness: &ProcessReadiness,
    ) -> Result<(), SupervisorError> {
        let effect = match &self.state {
            SupervisorState::Starting(effect)
            | SupervisorState::StartupOutcomeUnknown(effect) => effect.clone(),
            _ => return Err(SupervisorError::InvalidLifecycleTransition),
        };
        verify_process_identity(&effect, readiness)?;
        if !readiness.authenticated_health_ok {
            return Err(SupervisorError::ProcessNotReady);
        }
        self.state = SupervisorState::Ready {
            identity: readiness.identity,
            artifact: effect.artifact,
            config: effect.config,
        };
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
            } => (*identity, config),
            _ => return Err(SupervisorError::InvalidLifecycleTransition),
        };
        if observation.identity != identity {
            self.state = SupervisorState::Quarantined(
                SupervisorError::ProcessIdentityMismatch,
            );
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
            self.state = SupervisorState::Quarantined(
                SupervisorError::RestartBudgetExceeded,
            );
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

    pub fn confirm_stopped(
        &mut self,
        observed_identity: ProcessIdentity,
        process_absent: bool,
        endpoint_absent: bool,
    ) -> Result<(), SupervisorError> {
        let expected = match &self.state {
            SupervisorState::Draining { identity, .. }
            | SupervisorState::ShutdownOutcomeUnknown { identity, .. } => *identity,
            _ => return Err(SupervisorError::InvalidLifecycleTransition),
        };
        if observed_identity != expected {
            return Err(SupervisorError::ProcessIdentityMismatch);
        }
        if !process_absent || !endpoint_absent {
            return Err(SupervisorError::ShutdownOutcomeUnknown);
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
    readiness: &ProcessReadiness,
) -> Result<(), SupervisorError> {
    let expected_artifact = effect.artifact.candidate();
    let expected_config = effect.config.config();
    if readiness.identity.owner_fence != expected_config.owner_fence {
        return Err(SupervisorError::OwnerFenceMismatch);
    }
    if readiness.identity.artifact_digest != expected_artifact.artifact_digest
        || readiness.identity.executable_file_digest != expected_artifact.file_identity_digest
    {
        return Err(SupervisorError::ExecutableIdentityMismatch);
    }
    if readiness.identity.endpoint != expected_config.endpoint {
        return Err(SupervisorError::EndpointIdentityMismatch);
    }
    if readiness.observed_config_digest != expected_config.config_digest {
        return Err(SupervisorError::InvalidProcessConfig);
    }
    Ok(())
}
