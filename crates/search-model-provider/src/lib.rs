//! Bounded optional model-provider lifecycle and selected-candidate protocol.
//!
//! The provider receives only explicitly selected, already authorized candidate
//! inputs. It cannot enumerate sources, open handles, widen scope, mutate index
//! visibility, or create evidence. Embeddings and rerank scores remain derived
//! observations bound to exact model/artifact/profile identities.

#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::cmp::Ordering;
use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId, ProfileId, ReceiptRef};

/// Closed optional-model failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ModelError {
    /// Provider artifact is not qualified for the requested capability.
    ArtifactUnqualified,
    /// Provider/model/tokenizer identity differs from the accepted manifest.
    ArtifactMismatch,
    /// Requested task is not supported by the qualified model.
    CapabilityUnavailable,
    /// Provider is not in the ready state.
    ProviderNotReady,
    /// Provider is draining and rejects new work.
    ProviderDraining,
    /// Provider is quarantined.
    ProviderQuarantined,
    /// Request identity or profile is malformed.
    RequestInvalid,
    /// Candidate input set is empty, duplicated, or exceeds finite limits.
    CandidateSetInvalid,
    /// Selected-candidate input exceeds byte/token budget.
    InputBudgetExceeded,
    /// Deadline is invalid or elapsed.
    DeadlineExceeded,
    /// Cancellation occurred before dispatch.
    CancelledBeforeDispatch,
    /// Cancellation or timeout occurred after dispatch.
    OutcomeUnknown,
    /// Response request/model/profile identity differs.
    ResponseBindingMismatch,
    /// Response contains a missing, duplicated, or unexpected candidate.
    ResponseCandidateMismatch,
    /// Embedding dimensions or sparse indices are invalid.
    EmbeddingInvalid,
    /// Rerank score is non-finite or outside the accepted range.
    RerankScoreInvalid,
    /// Response bytes/items exceed admitted output limits.
    OutputBudgetExceeded,
    /// Progress regressed or contradicted the terminal response.
    ProgressInvalid,
    /// Lifecycle transition is invalid.
    InvalidTransition,
    /// Provider reported degraded operation.
    ProviderDegraded,
    /// Exact response/readback receipt is absent or mismatched.
    ReceiptMismatch,
}

impl ModelError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ArtifactUnqualified => "MODEL_ARTIFACT_UNQUALIFIED",
            Self::ArtifactMismatch => "MODEL_ARTIFACT_MISMATCH",
            Self::CapabilityUnavailable => "MODEL_CAPABILITY_UNAVAILABLE",
            Self::ProviderNotReady => "MODEL_PROVIDER_NOT_READY",
            Self::ProviderDraining => "MODEL_PROVIDER_DRAINING",
            Self::ProviderQuarantined => "MODEL_PROVIDER_QUARANTINED",
            Self::RequestInvalid => "MODEL_REQUEST_INVALID",
            Self::CandidateSetInvalid => "MODEL_CANDIDATE_SET_INVALID",
            Self::InputBudgetExceeded => "MODEL_INPUT_BUDGET_EXCEEDED",
            Self::DeadlineExceeded => "MODEL_DEADLINE_EXCEEDED",
            Self::CancelledBeforeDispatch => "MODEL_CANCELLED_BEFORE_DISPATCH",
            Self::OutcomeUnknown => "MODEL_OUTCOME_UNKNOWN",
            Self::ResponseBindingMismatch => "MODEL_RESPONSE_BINDING_MISMATCH",
            Self::ResponseCandidateMismatch => "MODEL_RESPONSE_CANDIDATE_MISMATCH",
            Self::EmbeddingInvalid => "MODEL_EMBEDDING_INVALID",
            Self::RerankScoreInvalid => "MODEL_RERANK_SCORE_INVALID",
            Self::OutputBudgetExceeded => "MODEL_OUTPUT_BUDGET_EXCEEDED",
            Self::ProgressInvalid => "MODEL_PROGRESS_INVALID",
            Self::InvalidTransition => "MODEL_TRANSITION_INVALID",
            Self::ProviderDegraded => "MODEL_PROVIDER_DEGRADED",
            Self::ReceiptMismatch => "MODEL_RECEIPT_MISMATCH",
        }
    }
}

impl fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ModelError {}

/// Closed optional model capability.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ModelCapability {
    /// Produce a dense embedding for each selected candidate.
    DenseEmbedding,
    /// Produce a sparse embedding for each selected candidate.
    SparseEmbedding,
    /// Reorder selected candidates relative to one bounded query.
    Rerank,
    /// Classify selected candidate metadata into a closed label set.
    Classification,
}

/// Exact immutable model artifact observed locally.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelArtifact {
    /// Provider implementation identity.
    pub provider_id: OpaqueId,
    /// Model identity.
    pub model_id: OpaqueId,
    /// Exact executable/model artifact digest.
    pub artifact_digest: Blake3Digest32,
    /// Exact tokenizer/preprocessor digest.
    pub tokenizer_digest: Blake3Digest32,
    /// Runtime protocol/profile digest.
    pub runtime_profile_digest: Blake3Digest32,
    /// Capabilities supplied by this artifact.
    pub capabilities: BTreeSet<ModelCapability>,
    /// Dense embedding dimension when supported.
    pub dense_dimensions: Option<u32>,
    /// Sparse vocabulary ceiling when supported.
    pub sparse_vocabulary_size: Option<u32>,
}

/// Accepted qualification manifest for an exact model artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelQualificationManifest {
    /// Expected provider identity.
    pub provider_id: OpaqueId,
    /// Expected model identity.
    pub model_id: OpaqueId,
    /// Expected artifact digest.
    pub artifact_digest: Blake3Digest32,
    /// Expected tokenizer digest.
    pub tokenizer_digest: Blake3Digest32,
    /// Expected runtime profile digest.
    pub runtime_profile_digest: Blake3Digest32,
    /// Accepted capabilities.
    pub capabilities: BTreeSet<ModelCapability>,
    /// Accepted dense dimensions.
    pub dense_dimensions: Option<u32>,
    /// Accepted sparse vocabulary ceiling.
    pub sparse_vocabulary_size: Option<u32>,
    /// Qualification evidence.
    pub qualification_receipt: ReceiptRef,
}

/// Model artifact accepted for optional use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedModelArtifact {
    artifact: ModelArtifact,
    qualification_receipt: ReceiptRef,
}

impl QualifiedModelArtifact {
    /// Exact qualified artifact.
    #[must_use]
    pub const fn artifact(&self) -> &ModelArtifact {
        &self.artifact
    }

    /// Qualification receipt.
    #[must_use]
    pub const fn qualification_receipt(&self) -> &ReceiptRef {
        &self.qualification_receipt
    }
}

/// Qualifies exact artifact identity and capability shape.
pub fn qualify_model_artifact(
    artifact: ModelArtifact,
    manifest: &ModelQualificationManifest,
) -> Result<QualifiedModelArtifact, ModelError> {
    if artifact.provider_id != manifest.provider_id
        || artifact.model_id != manifest.model_id
        || artifact.artifact_digest != manifest.artifact_digest
        || artifact.tokenizer_digest != manifest.tokenizer_digest
        || artifact.runtime_profile_digest != manifest.runtime_profile_digest
    {
        return Err(ModelError::ArtifactMismatch);
    }
    if artifact.capabilities != manifest.capabilities
        || artifact.dense_dimensions != manifest.dense_dimensions
        || artifact.sparse_vocabulary_size != manifest.sparse_vocabulary_size
        || artifact.capabilities.is_empty()
    {
        return Err(ModelError::ArtifactUnqualified);
    }
    if artifact
        .capabilities
        .contains(&ModelCapability::DenseEmbedding)
        && artifact.dense_dimensions.is_none_or(|dimensions| dimensions == 0)
    {
        return Err(ModelError::ArtifactUnqualified);
    }
    if artifact
        .capabilities
        .contains(&ModelCapability::SparseEmbedding)
        && artifact
            .sparse_vocabulary_size
            .is_none_or(|dimensions| dimensions == 0)
    {
        return Err(ModelError::ArtifactUnqualified);
    }
    Ok(QualifiedModelArtifact {
        artifact,
        qualification_receipt: manifest.qualification_receipt.clone(),
    })
}

/// Optional provider lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderState {
    Absent,
    Stopped,
    Starting,
    Ready,
    Degraded,
    Draining,
    Quarantined,
}

/// Exact provider session identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSessionIdentity {
    /// Process incarnation identity.
    pub process_identity_digest: Blake3Digest32,
    /// Qualified artifact digest.
    pub artifact_digest: Blake3Digest32,
    /// Runtime profile digest.
    pub runtime_profile_digest: Blake3Digest32,
    /// Authentication/pairing receipt.
    pub authentication_receipt: ReceiptRef,
}

/// Content-free provider readiness receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderReadyReceipt {
    /// Exact session identity.
    pub session: ProviderSessionIdentity,
    /// Model identity proven by live capability probe.
    pub model_id: OpaqueId,
    /// Capability probe digest.
    pub capability_probe_digest: Blake3Digest32,
    /// Live capabilities.
    pub capabilities: BTreeSet<ModelCapability>,
    /// Readiness evidence.
    pub readiness_receipt: ReceiptRef,
}

/// Stateful optional provider session with explicit transitions.
#[derive(Clone, Debug)]
pub struct ProviderSession {
    state: ProviderState,
    qualified: QualifiedModelArtifact,
    session: Option<ProviderSessionIdentity>,
    ready: Option<ProviderReadyReceipt>,
    degradation_generation: u64,
}

impl ProviderSession {
    /// Creates a stopped session around a qualified artifact.
    #[must_use]
    pub fn new(qualified: QualifiedModelArtifact) -> Self {
        Self {
            state: ProviderState::Stopped,
            qualified,
            session: None,
            ready: None,
            degradation_generation: 0,
        }
    }

    /// Current lifecycle state.
    #[must_use]
    pub const fn state(&self) -> ProviderState {
        self.state
    }

    /// Begins process start.
    pub fn begin_start(&mut self) -> Result<(), ModelError> {
        if !matches!(self.state, ProviderState::Stopped | ProviderState::Degraded) {
            return Err(ModelError::InvalidTransition);
        }
        self.state = ProviderState::Starting;
        self.session = None;
        self.ready = None;
        Ok(())
    }

    /// Accepts exact authenticated readiness.
    pub fn accept_ready(
        &mut self,
        session: ProviderSessionIdentity,
        receipt: ProviderReadyReceipt,
    ) -> Result<(), ModelError> {
        if self.state != ProviderState::Starting {
            return Err(ModelError::InvalidTransition);
        }
        let artifact = self.qualified.artifact();
        if session.artifact_digest != artifact.artifact_digest
            || session.runtime_profile_digest != artifact.runtime_profile_digest
            || receipt.session != session
            || receipt.model_id != artifact.model_id
            || receipt.capabilities != artifact.capabilities
        {
            return Err(ModelError::ArtifactMismatch);
        }
        self.session = Some(session);
        self.ready = Some(receipt);
        self.state = ProviderState::Ready;
        Ok(())
    }

    /// Marks provider degradation and removes request admission authority.
    pub fn degrade(&mut self) {
        self.degradation_generation = self.degradation_generation.saturating_add(1);
        self.state = ProviderState::Degraded;
        self.ready = None;
    }

    /// Begins graceful drain.
    pub fn begin_drain(&mut self) -> Result<(), ModelError> {
        if !matches!(self.state, ProviderState::Ready | ProviderState::Degraded) {
            return Err(ModelError::InvalidTransition);
        }
        self.state = ProviderState::Draining;
        self.ready = None;
        Ok(())
    }

    /// Completes stop after external process shutdown readback.
    pub fn stop(&mut self, shutdown_verified: bool) -> Result<(), ModelError> {
        if self.state != ProviderState::Draining || !shutdown_verified {
            return Err(ModelError::InvalidTransition);
        }
        self.state = ProviderState::Stopped;
        self.session = None;
        self.ready = None;
        Ok(())
    }

    /// Quarantines contradictory provider state.
    pub fn quarantine(&mut self) {
        self.state = ProviderState::Quarantined;
        self.ready = None;
    }

    /// Returns current request-admission receipt.
    pub fn ready_receipt(&self) -> Result<&ProviderReadyReceipt, ModelError> {
        match self.state {
            ProviderState::Ready => self.ready.as_ref().ok_or(ModelError::ProviderNotReady),
            ProviderState::Draining => Err(ModelError::ProviderDraining),
            ProviderState::Quarantined => Err(ModelError::ProviderQuarantined),
            ProviderState::Degraded => Err(ModelError::ProviderDegraded),
            ProviderState::Absent | ProviderState::Stopped | ProviderState::Starting => {
                Err(ModelError::ProviderNotReady)
            }
        }
    }
}

/// Finite request/output limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelLimits {
    /// Maximum selected candidates per request.
    pub max_candidates: usize,
    /// Maximum bytes per candidate.
    pub max_candidate_bytes: usize,
    /// Maximum aggregate input bytes.
    pub max_input_bytes: usize,
    /// Maximum output vector values across the request.
    pub max_output_values: usize,
    /// Maximum output labels per candidate.
    pub max_labels_per_candidate: usize,
}

impl ModelLimits {
    /// Conservative baseline.
    pub const BASELINE: Self = Self {
        max_candidates: 1_024,
        max_candidate_bytes: 512 * 1_024,
        max_input_bytes: 32 * 1_024 * 1_024,
        max_output_values: 16 * 1_024 * 1_024,
        max_labels_per_candidate: 64,
    };

    /// Validates finite non-zero limits.
    pub const fn validate(self) -> Result<Self, ModelError> {
        if self.max_candidates == 0
            || self.max_candidate_bytes == 0
            || self.max_input_bytes == 0
            || self.max_output_values == 0
            || self.max_labels_per_candidate == 0
        {
            Err(ModelError::InputBudgetExceeded)
        } else {
            Ok(self)
        }
    }
}

/// Model task for one request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelTask {
    DenseEmbedding,
    SparseEmbedding,
    Rerank,
    Classification,
}

impl ModelTask {
    #[must_use]
    pub const fn capability(self) -> ModelCapability {
        match self {
            Self::DenseEmbedding => ModelCapability::DenseEmbedding,
            Self::SparseEmbedding => ModelCapability::SparseEmbedding,
            Self::Rerank => ModelCapability::Rerank,
            Self::Classification => ModelCapability::Classification,
        }
    }
}

/// Selected authorized candidate input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedCandidateInput {
    /// Stable request-local candidate identity.
    pub candidate_id: OpaqueId,
    /// Digest of exact validated source bytes.
    pub source_digest: Blake3Digest32,
    /// Digest of candidate identity/range metadata.
    pub identity_digest: Blake3Digest32,
    /// Bounded exact bytes supplied by the caller.
    pub bytes: Vec<u8>,
}

/// One bounded optional-model request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRequest {
    /// Request identity.
    pub request_id: OpaqueId,
    /// Closed task.
    pub task: ModelTask,
    /// Exact request profile.
    pub request_profile_id: ProfileId,
    /// Digest of request/query/profile inputs.
    pub request_profile_digest: Blake3Digest32,
    /// Optional query bytes for reranking/classification.
    pub query_bytes: Vec<u8>,
    /// Exact selected candidate set.
    pub candidates: Vec<SelectedCandidateInput>,
    /// Finite absolute process-local deadline tick.
    pub deadline_tick: u64,
    /// Digest of exact canonical request identity.
    pub request_digest: Blake3Digest32,
}

/// Request accepted for dispatch to one exact ready session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedModelRequest {
    /// Original bounded request.
    pub request: ModelRequest,
    /// Exact process/session identity.
    pub session: ProviderSessionIdentity,
    /// Qualification receipt.
    pub qualification_receipt: ReceiptRef,
    /// Readiness receipt.
    pub readiness_receipt: ReceiptRef,
}

/// Validates selected-candidate scope, capability, bytes, and deadline.
pub fn admit_request(
    session: &ProviderSession,
    request: ModelRequest,
    limits: ModelLimits,
    now_tick: u64,
    cancelled: bool,
) -> Result<AdmittedModelRequest, ModelError> {
    let limits = limits.validate()?;
    if cancelled {
        return Err(ModelError::CancelledBeforeDispatch);
    }
    if request.deadline_tick <= now_tick
        || request.request_profile_id.as_str().is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > limits.max_candidates
    {
        return Err(ModelError::RequestInvalid);
    }
    let ready = session.ready_receipt()?;
    let artifact = session.qualified.artifact();
    if !artifact.capabilities.contains(&request.task.capability())
        || !ready.capabilities.contains(&request.task.capability())
    {
        return Err(ModelError::CapabilityUnavailable);
    }
    let mut candidate_ids = BTreeSet::new();
    let mut input_bytes = request.query_bytes.len();
    for candidate in &request.candidates {
        if candidate.bytes.is_empty()
            || candidate.bytes.len() > limits.max_candidate_bytes
            || !candidate_ids.insert(candidate.candidate_id.clone())
        {
            return Err(ModelError::CandidateSetInvalid);
        }
        input_bytes = input_bytes
            .checked_add(candidate.bytes.len())
            .ok_or(ModelError::InputBudgetExceeded)?;
    }
    if input_bytes > limits.max_input_bytes {
        return Err(ModelError::InputBudgetExceeded);
    }
    Ok(AdmittedModelRequest {
        request,
        session: ready.session.clone(),
        qualification_receipt: session.qualified.qualification_receipt.clone(),
        readiness_receipt: ready.readiness_receipt.clone(),
    })
}

/// Monotone provider progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelProgress {
    /// Total selected candidates.
    pub total: usize,
    /// Completed selected candidates.
    pub completed: usize,
    /// Whether a terminal response was emitted.
    pub terminal: bool,
}

impl ModelProgress {
    /// Creates initial finite progress.
    #[must_use]
    pub const fn new(total: usize) -> Self {
        Self {
            total,
            completed: 0,
            terminal: false,
        }
    }

    /// Advances monotonically.
    pub fn advance(&mut self, completed: usize) -> Result<(), ModelError> {
        if self.terminal || completed < self.completed || completed > self.total {
            return Err(ModelError::ProgressInvalid);
        }
        self.completed = completed;
        Ok(())
    }

    /// Marks exactly one terminal response.
    pub fn finish(&mut self, complete: bool) -> Result<(), ModelError> {
        if self.terminal || (complete && self.completed != self.total) {
            return Err(ModelError::ProgressInvalid);
        }
        self.terminal = true;
        Ok(())
    }
}

/// Dense embedding output.
#[derive(Clone, Debug, PartialEq)]
pub struct DenseEmbedding {
    /// Candidate identity.
    pub candidate_id: OpaqueId,
    /// Finite dense vector.
    pub values: Vec<f32>,
    /// Digest of exact vector serialization.
    pub vector_digest: Blake3Digest32,
}

/// Sparse embedding output.
#[derive(Clone, Debug, PartialEq)]
pub struct SparseEmbedding {
    /// Candidate identity.
    pub candidate_id: OpaqueId,
    /// Strictly increasing sparse index/value pairs.
    pub values: Vec<(u32, f32)>,
    /// Digest of exact vector serialization.
    pub vector_digest: Blake3Digest32,
}

/// Rerank output.
#[derive(Clone, Debug, PartialEq)]
pub struct RerankScore {
    /// Candidate identity.
    pub candidate_id: OpaqueId,
    /// Finite model score. Cross-model comparability is not implied.
    pub score: f32,
    /// Optional deterministic rank returned by the provider.
    pub rank: Option<u32>,
}

/// Closed classification output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassificationOutput {
    /// Candidate identity.
    pub candidate_id: OpaqueId,
    /// Closed/bounded labels selected by the configured profile.
    pub labels: BTreeSet<OpaqueId>,
}

/// One exact provider response.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelResponseBody {
    DenseEmbeddings(Vec<DenseEmbedding>),
    SparseEmbeddings(Vec<SparseEmbedding>),
    Rerank(Vec<RerankScore>),
    Classifications(Vec<ClassificationOutput>),
}

/// Response bound to exact request and session identity.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelResponse {
    /// Matching request identity.
    pub request_id: OpaqueId,
    /// Matching request digest.
    pub request_digest: Blake3Digest32,
    /// Exact provider process identity.
    pub process_identity_digest: Blake3Digest32,
    /// Exact model artifact digest.
    pub artifact_digest: Blake3Digest32,
    /// Exact runtime profile digest.
    pub runtime_profile_digest: Blake3Digest32,
    /// Exact response body.
    pub body: ModelResponseBody,
    /// Content-free response readback receipt.
    pub response_receipt: ReceiptRef,
}

/// Validated derived model output.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedModelOutput {
    /// Request identity.
    pub request_id: OpaqueId,
    /// Request digest.
    pub request_digest: Blake3Digest32,
    /// Task.
    pub task: ModelTask,
    /// Model identity.
    pub model_id: OpaqueId,
    /// Artifact digest.
    pub artifact_digest: Blake3Digest32,
    /// Tokenizer digest.
    pub tokenizer_digest: Blake3Digest32,
    /// Request profile digest.
    pub request_profile_digest: Blake3Digest32,
    /// Validated body.
    pub body: ModelResponseBody,
    /// Response receipt.
    pub response_receipt: ReceiptRef,
}

/// Validates exact response identity, candidate set, dimensions, and limits.
pub fn validate_response(
    admitted: &AdmittedModelRequest,
    response: ModelResponse,
    qualified: &QualifiedModelArtifact,
    limits: ModelLimits,
) -> Result<ValidatedModelOutput, ModelError> {
    let limits = limits.validate()?;
    if response.request_id != admitted.request.request_id
        || response.request_digest != admitted.request.request_digest
        || response.process_identity_digest != admitted.session.process_identity_digest
        || response.artifact_digest != admitted.session.artifact_digest
        || response.runtime_profile_digest != admitted.session.runtime_profile_digest
        || response.artifact_digest != qualified.artifact.artifact_digest
    {
        return Err(ModelError::ResponseBindingMismatch);
    }
    let expected_candidates = admitted
        .request
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<BTreeSet<_>>();
    let observed_candidates = response_candidate_ids(&response.body)?;
    if observed_candidates != expected_candidates {
        return Err(ModelError::ResponseCandidateMismatch);
    }
    validate_body(
        admitted.request.task,
        &response.body,
        &qualified.artifact,
        limits,
    )?;
    Ok(ValidatedModelOutput {
        request_id: response.request_id,
        request_digest: response.request_digest,
        task: admitted.request.task,
        model_id: qualified.artifact.model_id.clone(),
        artifact_digest: qualified.artifact.artifact_digest,
        tokenizer_digest: qualified.artifact.tokenizer_digest,
        request_profile_digest: admitted.request.request_profile_digest,
        body: response.body,
        response_receipt: response.response_receipt,
    })
}

/// Orders rerank output deterministically inside one exact model/profile only.
pub fn order_rerank_scores(
    output: &ValidatedModelOutput,
) -> Result<Vec<RerankScore>, ModelError> {
    let ModelResponseBody::Rerank(scores) = &output.body else {
        return Err(ModelError::CapabilityUnavailable);
    };
    let mut ordered = scores.clone();
    ordered.sort_by(|left, right| {
        left.rank
            .unwrap_or(u32::MAX)
            .cmp(&right.rank.unwrap_or(u32::MAX))
            .then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    Ok(ordered)
}

fn response_candidate_ids(
    body: &ModelResponseBody,
) -> Result<BTreeSet<OpaqueId>, ModelError> {
    let ids = match body {
        ModelResponseBody::DenseEmbeddings(values) => values
            .iter()
            .map(|value| value.candidate_id.clone())
            .collect::<Vec<_>>(),
        ModelResponseBody::SparseEmbeddings(values) => values
            .iter()
            .map(|value| value.candidate_id.clone())
            .collect::<Vec<_>>(),
        ModelResponseBody::Rerank(values) => values
            .iter()
            .map(|value| value.candidate_id.clone())
            .collect::<Vec<_>>(),
        ModelResponseBody::Classifications(values) => values
            .iter()
            .map(|value| value.candidate_id.clone())
            .collect::<Vec<_>>(),
    };
    let set = ids.iter().cloned().collect::<BTreeSet<_>>();
    if set.len() != ids.len() {
        return Err(ModelError::ResponseCandidateMismatch);
    }
    Ok(set)
}

fn validate_body(
    task: ModelTask,
    body: &ModelResponseBody,
    artifact: &ModelArtifact,
    limits: ModelLimits,
) -> Result<(), ModelError> {
    match (task, body) {
        (ModelTask::DenseEmbedding, ModelResponseBody::DenseEmbeddings(values)) => {
            let dimensions = usize::try_from(
                artifact
                    .dense_dimensions
                    .ok_or(ModelError::EmbeddingInvalid)?,
            )
            .map_err(|_| ModelError::EmbeddingInvalid)?;
            let total = values.iter().try_fold(0_usize, |total, embedding| {
                if embedding.values.len() != dimensions
                    || embedding.values.iter().any(|value| !value.is_finite())
                {
                    return Err(ModelError::EmbeddingInvalid);
                }
                total
                    .checked_add(embedding.values.len())
                    .ok_or(ModelError::OutputBudgetExceeded)
            })?;
            if total > limits.max_output_values {
                return Err(ModelError::OutputBudgetExceeded);
            }
        }
        (ModelTask::SparseEmbedding, ModelResponseBody::SparseEmbeddings(values)) => {
            let vocabulary = artifact
                .sparse_vocabulary_size
                .ok_or(ModelError::EmbeddingInvalid)?;
            let total = values.iter().try_fold(0_usize, |total, embedding| {
                if embedding.values.is_empty()
                    || embedding
                        .values
                        .iter()
                        .any(|(index, value)| *index >= vocabulary || !value.is_finite())
                    || embedding
                        .values
                        .windows(2)
                        .any(|pair| pair[0].0 >= pair[1].0)
                {
                    return Err(ModelError::EmbeddingInvalid);
                }
                total
                    .checked_add(embedding.values.len())
                    .ok_or(ModelError::OutputBudgetExceeded)
            })?;
            if total > limits.max_output_values {
                return Err(ModelError::OutputBudgetExceeded);
            }
        }
        (ModelTask::Rerank, ModelResponseBody::Rerank(values)) => {
            if values.iter().any(|value| !value.score.is_finite()) {
                return Err(ModelError::RerankScoreInvalid);
            }
        }
        (ModelTask::Classification, ModelResponseBody::Classifications(values)) => {
            if values
                .iter()
                .any(|value| value.labels.len() > limits.max_labels_per_candidate)
            {
                return Err(ModelError::OutputBudgetExceeded);
            }
        }
        _ => return Err(ModelError::CapabilityUnavailable),
    }
    Ok(())
}
