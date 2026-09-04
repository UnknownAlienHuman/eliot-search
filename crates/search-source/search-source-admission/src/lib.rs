//! Fail-closed source admission for the W2 direct-source spine.
//!
//! This package performs no filesystem, process, network, secret, or database
//! I/O. Callers supply exact policy, owner, security, observation, and optional
//! virtual-snapshot evidence. Admission is terminal, bounded, replay-fenced,
//! and never inferred from configuration presence alone.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

use core::fmt;
use std::collections::BTreeSet;

use search_contracts::{
    Blake3Digest32, NonZeroRevision, OpaqueId, OwnerEpoch, ReceiptRef,
};
use search_ports::{MonotonicInstant, MutationIdentity};

/// Conservative finite admission limits.
pub const DEFAULT_ADMISSION_LIMITS: AdmissionLimits = AdmissionLimits {
    max_batch_items: 1_024,
    max_evidence_receipts: 32,
    max_ledger_entries: 65_536,
    max_source_bytes: 8 * 1024 * 1024 * 1024,
};

/// Closed content-free admission failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdmissionError {
    /// Admission limits are zero or internally inconsistent.
    InvalidLimits,
    /// Batch is empty or exceeds its finite item ceiling.
    BatchSizeInvalid,
    /// Batch contains a duplicate candidate identity.
    DuplicateCandidate,
    /// Batch contains a duplicate operation identity.
    DuplicateOperation,
    /// One operation identity was reused with another request digest.
    OperationConflict,
    /// Finite idempotency ledger is full.
    LedgerCapacityExceeded,
    /// Evidence set is empty, duplicated, or exceeds its finite ceiling.
    EvidenceInvalid,
    /// Candidate size is zero or exceeds policy/implementation ceilings.
    SourceSizeInvalid,
    /// Policy has no enabled profile, modality, or residency class.
    EmptyPolicy,
}

impl AdmissionError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "ADMISSION_INVALID_LIMITS",
            Self::BatchSizeInvalid => "ADMISSION_BATCH_SIZE_INVALID",
            Self::DuplicateCandidate => "ADMISSION_DUPLICATE_CANDIDATE",
            Self::DuplicateOperation => "ADMISSION_DUPLICATE_OPERATION",
            Self::OperationConflict => "ADMISSION_OPERATION_CONFLICT",
            Self::LedgerCapacityExceeded => "ADMISSION_LEDGER_CAPACITY_EXCEEDED",
            Self::EvidenceInvalid => "ADMISSION_EVIDENCE_INVALID",
            Self::SourceSizeInvalid => "ADMISSION_SOURCE_SIZE_INVALID",
            Self::EmptyPolicy => "ADMISSION_EMPTY_POLICY",
        }
    }
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for AdmissionError {}

/// Finite admission limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionLimits {
    /// Maximum candidates in one atomic evaluation batch.
    pub max_batch_items: usize,
    /// Maximum content-free evidence receipts per candidate.
    pub max_evidence_receipts: usize,
    /// Maximum retained idempotency entries.
    pub max_ledger_entries: usize,
    /// Absolute implementation ceiling for one source observation.
    pub max_source_bytes: u64,
}

impl AdmissionLimits {
    /// Validates all finite dimensions as non-zero.
    pub const fn validate(self) -> Result<Self, AdmissionError> {
        if self.max_batch_items == 0
            || self.max_evidence_receipts == 0
            || self.max_ledger_entries == 0
            || self.max_source_bytes == 0
        {
            Err(AdmissionError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Build/profile capability requested for a source.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdmissionProfile {
    /// Retained direct-source reading and preparation.
    Direct,
    /// Lexical indexing and retrieval.
    Lexical,
    /// Current authenticated workspace overlays.
    CurrentWorkspace,
    /// Optional isolated model/document depth.
    OptionalDepth,
}

/// Closed source modality.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceModality {
    /// One ordinary filesystem file.
    RegularFile,
    /// A bounded directory or repository root whose children are admitted later.
    CollectionRoot,
    /// Authenticated unsaved in-memory content.
    VirtualSnapshot,
}

/// Closed data-residency class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResidencyClass {
    /// Stable local fixed storage.
    LocalFixed,
    /// Explicitly permitted local removable storage.
    LocalRemovable,
    /// Process-local or authenticated client memory.
    MemoryOnly,
    /// Remote, network, cloud, or otherwise non-local storage.
    Remote,
}

/// Live restrictive-security disposition at the admission boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SecurityDisposition {
    /// Current barrier permits admission.
    Permitted,
    /// Current barrier denies admission without asserting corruption.
    Restricted,
    /// Contradictory or unresolved security evidence requires quarantine.
    Quarantined,
}

/// Closed immutable admission policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionPolicy {
    revision: NonZeroRevision,
    enabled_profiles: BTreeSet<AdmissionProfile>,
    enabled_modalities: BTreeSet<SourceModality>,
    enabled_residencies: BTreeSet<ResidencyClass>,
    max_source_bytes: u64,
    require_authenticated_virtual_snapshots: bool,
}

impl AdmissionPolicy {
    /// Creates a non-empty finite policy.
    pub fn new(
        revision: NonZeroRevision,
        enabled_profiles: impl IntoIterator<Item = AdmissionProfile>,
        enabled_modalities: impl IntoIterator<Item = SourceModality>,
        enabled_residencies: impl IntoIterator<Item = ResidencyClass>,
        max_source_bytes: u64,
        require_authenticated_virtual_snapshots: bool,
        limits: AdmissionLimits,
    ) -> Result<Self, AdmissionError> {
        let limits = limits.validate()?;
        let enabled_profiles = enabled_profiles.into_iter().collect::<BTreeSet<_>>();
        let enabled_modalities = enabled_modalities.into_iter().collect::<BTreeSet<_>>();
        let enabled_residencies = enabled_residencies.into_iter().collect::<BTreeSet<_>>();
        if enabled_profiles.is_empty()
            || enabled_modalities.is_empty()
            || enabled_residencies.is_empty()
        {
            return Err(AdmissionError::EmptyPolicy);
        }
        if max_source_bytes == 0 || max_source_bytes > limits.max_source_bytes {
            return Err(AdmissionError::SourceSizeInvalid);
        }
        Ok(Self {
            revision,
            enabled_profiles,
            enabled_modalities,
            enabled_residencies,
            max_source_bytes,
            require_authenticated_virtual_snapshots,
        })
    }

    /// Exact policy revision.
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Maximum admitted source bytes.
    pub const fn max_source_bytes(&self) -> u64 {
        self.max_source_bytes
    }

    /// Returns whether a profile is enabled.
    pub fn profile_enabled(&self, profile: AdmissionProfile) -> bool {
        self.enabled_profiles.contains(&profile)
    }

    /// Returns whether a modality is enabled.
    pub fn modality_enabled(&self, modality: SourceModality) -> bool {
        self.enabled_modalities.contains(&modality)
    }

    /// Returns whether a residency class is enabled.
    pub fn residency_enabled(&self, residency: ResidencyClass) -> bool {
        self.enabled_residencies.contains(&residency)
    }

    /// Returns whether virtual snapshots require authentication proof.
    pub const fn requires_authenticated_virtual_snapshots(&self) -> bool {
        self.require_authenticated_virtual_snapshots
    }
}

/// Immutable operation identity plus canonical request digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionOperation {
    mutation: MutationIdentity,
    request_digest: Blake3Digest32,
}

impl AdmissionOperation {
    /// Creates an admission replay fence.
    pub const fn new(
        mutation: MutationIdentity,
        request_digest: Blake3Digest32,
    ) -> Self {
        Self {
            mutation,
            request_digest,
        }
    }

    /// Shared immutable mutation identity.
    pub const fn mutation(&self) -> &MutationIdentity {
        &self.mutation
    }

    /// Digest of exact canonical request bytes.
    pub const fn request_digest(&self) -> Blake3Digest32 {
        self.request_digest
    }

    /// Returns whether operation identity and request digest both match.
    pub fn is_same_request(&self, other: &Self) -> bool {
        self == other
    }
}

/// Exact content-free evidence required for ordinary admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionEvidence {
    /// Receipt for exact source/root observation.
    pub source_observation_receipt: ReceiptRef,
    /// Receipt for the current data-root owner epoch.
    pub owner_verification_receipt: ReceiptRef,
    /// Receipt for current restrictive-security barrier state.
    pub security_barrier_receipt: ReceiptRef,
    /// Digest binding the complete evidence set and canonical observations.
    pub evidence_digest: Blake3Digest32,
    /// Additional finite package-specific evidence receipts.
    pub additional_receipts: Vec<ReceiptRef>,
}

impl AdmissionEvidence {
    /// Validates receipt uniqueness and finite cardinality.
    pub fn validate(&self, limits: AdmissionLimits) -> Result<(), AdmissionError> {
        let limits = limits.validate()?;
        let total = 3_usize
            .checked_add(self.additional_receipts.len())
            .ok_or(AdmissionError::EvidenceInvalid)?;
        if total > limits.max_evidence_receipts {
            return Err(AdmissionError::EvidenceInvalid);
        }
        let mut receipts = Vec::with_capacity(total);
        receipts.push(&self.source_observation_receipt);
        receipts.push(&self.owner_verification_receipt);
        receipts.push(&self.security_barrier_receipt);
        for receipt in &self.additional_receipts {
            if receipts.iter().any(|existing| *existing == receipt) {
                return Err(AdmissionError::EvidenceInvalid);
            }
            receipts.push(receipt);
        }
        for (index, receipt) in receipts.iter().enumerate() {
            if receipts[..index].iter().any(|existing| existing == receipt) {
                return Err(AdmissionError::EvidenceInvalid);
            }
        }
        Ok(())
    }
}

/// Authenticated binding for an unsaved virtual snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtualSnapshotAttestation {
    /// Candidate identity covered by the attestation.
    pub candidate_id: OpaqueId,
    /// Authenticated client or editor incarnation.
    pub client_incarnation_id: OpaqueId,
    /// Monotone client buffer revision.
    pub buffer_revision: NonZeroRevision,
    /// Exact snapshot content digest.
    pub content_digest: Blake3Digest32,
    /// Owner epoch to which the snapshot was submitted.
    pub owner_epoch: OwnerEpoch,
    /// Issue time from the shared monotonic clock domain.
    pub issued_at: MonotonicInstant,
    /// Finite expiration time.
    pub expires_at: MonotonicInstant,
    /// Digest of the authentication/binding proof.
    pub proof_digest: Blake3Digest32,
    /// Whether the proof was verified by the authenticating adapter.
    pub proof_verified: bool,
}

impl VirtualSnapshotAttestation {
    /// Returns whether the attestation is valid at an explicit instant.
    pub const fn is_valid_at(&self, now: MonotonicInstant) -> bool {
        self.expires_at.ticks() > self.issued_at.ticks()
            && now.ticks() >= self.issued_at.ticks()
            && now.ticks() < self.expires_at.ticks()
    }
}

/// Complete finite source candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCandidate {
    /// Stable candidate identity before source-ID assignment.
    pub candidate_id: OpaqueId,
    /// Requested capability profile.
    pub profile: AdmissionProfile,
    /// Source modality.
    pub modality: SourceModality,
    /// Residency classification from the source-observation adapter.
    pub residency: ResidencyClass,
    /// Exact observed byte length.
    pub source_bytes: u64,
    /// Digest of canonical root identity.
    pub root_identity_digest: Blake3Digest32,
    /// Digest of exact observed content or collection descriptor.
    pub content_digest: Blake3Digest32,
}

/// Complete immutable admission request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionRequest {
    /// Replay-fenced mutation identity.
    pub operation: AdmissionOperation,
    /// Exact source candidate.
    pub candidate: SourceCandidate,
    /// Policy revision observed by the caller.
    pub policy_revision: NonZeroRevision,
    /// Current owner epoch observed by the caller.
    pub owner_epoch: OwnerEpoch,
    /// Restrictive-security barrier revision observed by the caller.
    pub security_barrier_revision: NonZeroRevision,
    /// Current security disposition.
    pub security_disposition: SecurityDisposition,
    /// Exact content-free evidence.
    pub evidence: AdmissionEvidence,
    /// Required only for virtual snapshots.
    pub virtual_attestation: Option<VirtualSnapshotAttestation>,
}

/// Closed non-quarantine denial reason.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdmissionDenialReason {
    /// Request policy revision is not current.
    PolicyRevisionMismatch,
    /// Request owner epoch is not current.
    OwnerEpochMismatch,
    /// Request security-barrier revision is not current.
    SecurityBarrierRevisionMismatch,
    /// Requested profile is disabled.
    ProfileDisabled,
    /// Requested modality is disabled.
    ModalityDisabled,
    /// Residency class is disabled or remote.
    ResidencyDenied,
    /// Observed source size is zero.
    EmptySource,
    /// Observed source size exceeds policy or implementation limits.
    SourceTooLarge,
    /// Current restrictive-security state denies admission.
    SecurityRestricted,
    /// Virtual snapshot requires an attestation.
    VirtualAttestationRequired,
    /// Virtual snapshot attestation is expired or not yet valid.
    VirtualAttestationExpired,
    /// Ordinary sources must not carry virtual snapshot attestations.
    UnexpectedVirtualAttestation,
}

/// Closed quarantine reason.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdmissionQuarantineReason {
    /// Security evidence is contradictory or unresolved.
    SecurityEvidenceQuarantined,
    /// Virtual snapshot proof was not authenticated.
    VirtualAttestationUnauthenticated,
    /// Virtual snapshot attestation does not bind exact request identity/content.
    VirtualAttestationMismatch,
    /// Evidence receipts are duplicated, malformed, or over limit.
    EvidenceInvalid,
}

/// Terminal admitted grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionGrant {
    /// Exact candidate identity.
    pub candidate_id: OpaqueId,
    /// Accepted profile.
    pub profile: AdmissionProfile,
    /// Accepted modality.
    pub modality: SourceModality,
    /// Accepted residency class.
    pub residency: ResidencyClass,
    /// Policy revision that authorized admission.
    pub policy_revision: NonZeroRevision,
    /// Owner epoch that authorized admission.
    pub owner_epoch: OwnerEpoch,
    /// Security-barrier revision that authorized admission.
    pub security_barrier_revision: NonZeroRevision,
    /// Digest binding exact evidence.
    pub evidence_digest: Blake3Digest32,
    /// Immutable operation.
    pub operation: AdmissionOperation,
}

/// Terminal denied result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionDenial {
    /// Exact candidate identity.
    pub candidate_id: OpaqueId,
    /// Distinct denial reasons in canonical order.
    pub reasons: BTreeSet<AdmissionDenialReason>,
    /// Immutable operation.
    pub operation: AdmissionOperation,
}

/// Terminal quarantined result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionQuarantine {
    /// Exact candidate identity.
    pub candidate_id: OpaqueId,
    /// Distinct quarantine reasons in canonical order.
    pub reasons: BTreeSet<AdmissionQuarantineReason>,
    /// Immutable operation.
    pub operation: AdmissionOperation,
}

/// Terminal source-admission outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionOutcome {
    /// Candidate passed all exact checks.
    Admitted(AdmissionGrant),
    /// Candidate is not admitted under current policy/evidence.
    Denied(AdmissionDenial),
    /// Candidate evidence is contradictory or unsafe.
    Quarantined(AdmissionQuarantine),
}

impl AdmissionOutcome {
    /// Exact operation that produced this terminal outcome.
    pub const fn operation(&self) -> &AdmissionOperation {
        match self {
            Self::Admitted(grant) => &grant.operation,
            Self::Denied(denial) => &denial.operation,
            Self::Quarantined(quarantine) => &quarantine.operation,
        }
    }

    /// Exact candidate identity.
    pub const fn candidate_id(&self) -> &OpaqueId {
        match self {
            Self::Admitted(grant) => &grant.candidate_id,
            Self::Denied(denial) => &denial.candidate_id,
            Self::Quarantined(quarantine) => &quarantine.candidate_id,
        }
    }
}

/// Evaluates one exact source-admission request.
#[must_use]
pub fn evaluate_admission(
    policy: &AdmissionPolicy,
    request: AdmissionRequest,
    current_owner_epoch: OwnerEpoch,
    current_security_barrier_revision: NonZeroRevision,
    now: MonotonicInstant,
    limits: AdmissionLimits,
) -> AdmissionOutcome {
    let mut denial = BTreeSet::new();
    let mut quarantine = BTreeSet::new();

    if request.policy_revision != policy.revision() {
        denial.insert(AdmissionDenialReason::PolicyRevisionMismatch);
    }
    if request.owner_epoch != current_owner_epoch {
        denial.insert(AdmissionDenialReason::OwnerEpochMismatch);
    }
    if request.security_barrier_revision != current_security_barrier_revision {
        denial.insert(AdmissionDenialReason::SecurityBarrierRevisionMismatch);
    }
    if !policy.profile_enabled(request.candidate.profile) {
        denial.insert(AdmissionDenialReason::ProfileDisabled);
    }
    if !policy.modality_enabled(request.candidate.modality) {
        denial.insert(AdmissionDenialReason::ModalityDisabled);
    }
    if request.candidate.residency == ResidencyClass::Remote
        || !policy.residency_enabled(request.candidate.residency)
    {
        denial.insert(AdmissionDenialReason::ResidencyDenied);
    }
    if request.candidate.source_bytes == 0 {
        denial.insert(AdmissionDenialReason::EmptySource);
    } else if request.candidate.source_bytes > policy.max_source_bytes()
        || request.candidate.source_bytes > limits.max_source_bytes
    {
        denial.insert(AdmissionDenialReason::SourceTooLarge);
    }

    match request.security_disposition {
        SecurityDisposition::Permitted => {}
        SecurityDisposition::Restricted => {
            denial.insert(AdmissionDenialReason::SecurityRestricted);
        }
        SecurityDisposition::Quarantined => {
            quarantine.insert(AdmissionQuarantineReason::SecurityEvidenceQuarantined);
        }
    }

    if request.evidence.validate(limits).is_err() {
        quarantine.insert(AdmissionQuarantineReason::EvidenceInvalid);
    }

    match request.candidate.modality {
        SourceModality::VirtualSnapshot => match &request.virtual_attestation {
            None if policy.requires_authenticated_virtual_snapshots() => {
                denial.insert(AdmissionDenialReason::VirtualAttestationRequired);
            }
            None => {}
            Some(attestation) => {
                if !attestation.proof_verified {
                    quarantine.insert(
                        AdmissionQuarantineReason::VirtualAttestationUnauthenticated,
                    );
                }
                if attestation.candidate_id != request.candidate.candidate_id
                    || attestation.content_digest != request.candidate.content_digest
                    || attestation.owner_epoch != request.owner_epoch
                {
                    quarantine.insert(AdmissionQuarantineReason::VirtualAttestationMismatch);
                }
                if !attestation.is_valid_at(now) {
                    denial.insert(AdmissionDenialReason::VirtualAttestationExpired);
                }
            }
        },
        SourceModality::RegularFile | SourceModality::CollectionRoot => {
            if request.virtual_attestation.is_some() {
                denial.insert(AdmissionDenialReason::UnexpectedVirtualAttestation);
            }
        }
    }

    let candidate_id = request.candidate.candidate_id.clone();
    let operation = request.operation.clone();
    if !quarantine.is_empty() {
        AdmissionOutcome::Quarantined(AdmissionQuarantine {
            candidate_id,
            reasons: quarantine,
            operation,
        })
    } else if !denial.is_empty() {
        AdmissionOutcome::Denied(AdmissionDenial {
            candidate_id,
            reasons: denial,
            operation,
        })
    } else {
        AdmissionOutcome::Admitted(AdmissionGrant {
            candidate_id,
            profile: request.candidate.profile,
            modality: request.candidate.modality,
            residency: request.candidate.residency,
            policy_revision: request.policy_revision,
            owner_epoch: request.owner_epoch,
            security_barrier_revision: request.security_barrier_revision,
            evidence_digest: request.evidence.evidence_digest,
            operation,
        })
    }
}

/// Finite duplicate-free atomic evaluation batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionBatch {
    requests: Vec<AdmissionRequest>,
}

impl AdmissionBatch {
    /// Creates a non-empty finite batch with unique candidate and operation identities.
    pub fn new(
        requests: Vec<AdmissionRequest>,
        limits: AdmissionLimits,
    ) -> Result<Self, AdmissionError> {
        let limits = limits.validate()?;
        if requests.is_empty() || requests.len() > limits.max_batch_items {
            return Err(AdmissionError::BatchSizeInvalid);
        }
        for (index, request) in requests.iter().enumerate() {
            if requests[..index]
                .iter()
                .any(|earlier| earlier.candidate.candidate_id == request.candidate.candidate_id)
            {
                return Err(AdmissionError::DuplicateCandidate);
            }
            if requests[..index]
                .iter()
                .any(|earlier| earlier.operation.mutation() == request.operation.mutation())
            {
                return Err(AdmissionError::DuplicateOperation);
            }
        }
        Ok(Self { requests })
    }

    /// Requests in caller order.
    pub fn requests(&self) -> &[AdmissionRequest] {
        &self.requests
    }

    /// Batch cardinality.
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Returns whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
}

/// Replay-aware terminal outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LedgerOutcome {
    /// Terminal admission result.
    pub outcome: AdmissionOutcome,
    /// Whether exact prior outcome was replayed.
    pub replayed: bool,
}

/// Finite idempotency ledger for terminal admission outcomes.
#[derive(Clone, Debug)]
pub struct AdmissionLedger {
    max_entries: usize,
    entries: Vec<(MutationIdentity, Blake3Digest32, AdmissionOutcome)>,
}

impl AdmissionLedger {
    /// Creates a finite ledger.
    pub fn new(limits: AdmissionLimits) -> Result<Self, AdmissionError> {
        let limits = limits.validate()?;
        Ok(Self {
            max_entries: limits.max_ledger_entries,
            entries: Vec::new(),
        })
    }

    /// Records or replays one terminal outcome.
    pub fn record(&mut self, outcome: AdmissionOutcome) -> Result<LedgerOutcome, AdmissionError> {
        let operation = outcome.operation();
        if let Some((_, digest, existing)) = self
            .entries
            .iter()
            .find(|(identity, _, _)| identity == operation.mutation())
        {
            if *digest != operation.request_digest() {
                return Err(AdmissionError::OperationConflict);
            }
            return Ok(LedgerOutcome {
                outcome: existing.clone(),
                replayed: true,
            });
        }
        if self.entries.len() >= self.max_entries {
            return Err(AdmissionError::LedgerCapacityExceeded);
        }
        self.entries.push((
            operation.mutation().clone(),
            operation.request_digest(),
            outcome.clone(),
        ));
        Ok(LedgerOutcome {
            outcome,
            replayed: false,
        })
    }

    /// Number of retained operation outcomes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether no outcomes are retained.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_ports::IdempotencyClass;

    fn operation(name: &str, digest: u8) -> AdmissionOperation {
        AdmissionOperation::new(
            MutationIdentity::new(
                OpaqueId::new(format!("admission-operation:{name}"))
                    .expect("operation"),
                IdempotencyClass::RetrySameIdentity,
            ),
            Blake3Digest32::from_bytes([digest; 32]),
        )
    }

    fn policy() -> AdmissionPolicy {
        AdmissionPolicy::new(
            NonZeroRevision::new(3).expect("revision"),
            [AdmissionProfile::Direct],
            [SourceModality::RegularFile, SourceModality::VirtualSnapshot],
            [ResidencyClass::LocalFixed, ResidencyClass::MemoryOnly],
            1_024,
            true,
            DEFAULT_ADMISSION_LIMITS,
        )
        .expect("policy")
    }

    fn evidence() -> AdmissionEvidence {
        AdmissionEvidence {
            source_observation_receipt: ReceiptRef::new("receipt:source")
                .expect("receipt"),
            owner_verification_receipt: ReceiptRef::new("receipt:owner")
                .expect("receipt"),
            security_barrier_receipt: ReceiptRef::new("receipt:security")
                .expect("receipt"),
            evidence_digest: Blake3Digest32::from_bytes([1; 32]),
            additional_receipts: Vec::new(),
        }
    }

    fn request(candidate: &str) -> AdmissionRequest {
        AdmissionRequest {
            operation: operation(candidate, 2),
            candidate: SourceCandidate {
                candidate_id: OpaqueId::new(format!("candidate:{candidate}"))
                    .expect("candidate"),
                profile: AdmissionProfile::Direct,
                modality: SourceModality::RegularFile,
                residency: ResidencyClass::LocalFixed,
                source_bytes: 10,
                root_identity_digest: Blake3Digest32::from_bytes([3; 32]),
                content_digest: Blake3Digest32::from_bytes([4; 32]),
            },
            policy_revision: NonZeroRevision::new(3).expect("revision"),
            owner_epoch: OwnerEpoch::new(7).expect("epoch"),
            security_barrier_revision: NonZeroRevision::new(5).expect("revision"),
            security_disposition: SecurityDisposition::Permitted,
            evidence: evidence(),
            virtual_attestation: None,
        }
    }

    fn evaluate(request: AdmissionRequest) -> AdmissionOutcome {
        evaluate_admission(
            &policy(),
            request,
            OwnerEpoch::new(7).expect("epoch"),
            NonZeroRevision::new(5).expect("revision"),
            MonotonicInstant::from_ticks(10),
            DEFAULT_ADMISSION_LIMITS,
        )
    }

    #[test]
    fn exact_current_request_is_admitted() {
        assert!(matches!(evaluate(request("one")), AdmissionOutcome::Admitted(_)));
    }

    #[test]
    fn stale_owner_epoch_is_denied() {
        let mut request = request("one");
        request.owner_epoch = OwnerEpoch::new(6).expect("epoch");
        let AdmissionOutcome::Denied(denial) = evaluate(request) else {
            panic!("stale epoch must deny")
        };
        assert!(denial.reasons.contains(&AdmissionDenialReason::OwnerEpochMismatch));
    }

    #[test]
    fn remote_residency_is_never_silently_admitted() {
        let mut request = request("one");
        request.candidate.residency = ResidencyClass::Remote;
        let AdmissionOutcome::Denied(denial) = evaluate(request) else {
            panic!("remote residency must deny")
        };
        assert!(denial.reasons.contains(&AdmissionDenialReason::ResidencyDenied));
    }

    #[test]
    fn contradictory_security_evidence_quarantines() {
        let mut request = request("one");
        request.security_disposition = SecurityDisposition::Quarantined;
        assert!(matches!(evaluate(request), AdmissionOutcome::Quarantined(_)));
    }

    #[test]
    fn virtual_snapshot_requires_exact_authenticated_binding() {
        let mut request = request("virtual");
        request.candidate.modality = SourceModality::VirtualSnapshot;
        request.candidate.residency = ResidencyClass::MemoryOnly;
        request.virtual_attestation = Some(VirtualSnapshotAttestation {
            candidate_id: request.candidate.candidate_id.clone(),
            client_incarnation_id: OpaqueId::new("client:editor-1").expect("client"),
            buffer_revision: NonZeroRevision::new(9).expect("revision"),
            content_digest: request.candidate.content_digest,
            owner_epoch: request.owner_epoch,
            issued_at: MonotonicInstant::from_ticks(1),
            expires_at: MonotonicInstant::from_ticks(20),
            proof_digest: Blake3Digest32::from_bytes([5; 32]),
            proof_verified: true,
        });
        assert!(matches!(evaluate(request), AdmissionOutcome::Admitted(_)));
    }

    #[test]
    fn unauthenticated_virtual_snapshot_quarantines() {
        let mut request = request("virtual");
        request.candidate.modality = SourceModality::VirtualSnapshot;
        request.candidate.residency = ResidencyClass::MemoryOnly;
        request.virtual_attestation = Some(VirtualSnapshotAttestation {
            candidate_id: request.candidate.candidate_id.clone(),
            client_incarnation_id: OpaqueId::new("client:editor-1").expect("client"),
            buffer_revision: NonZeroRevision::new(9).expect("revision"),
            content_digest: request.candidate.content_digest,
            owner_epoch: request.owner_epoch,
            issued_at: MonotonicInstant::from_ticks(1),
            expires_at: MonotonicInstant::from_ticks(20),
            proof_digest: Blake3Digest32::from_bytes([5; 32]),
            proof_verified: false,
        });
        assert!(matches!(evaluate(request), AdmissionOutcome::Quarantined(_)));
    }

    #[test]
    fn duplicate_batch_candidate_is_rejected() {
        assert_eq!(
            AdmissionBatch::new(
                vec![request("same"), request("same")],
                DEFAULT_ADMISSION_LIMITS,
            ),
            Err(AdmissionError::DuplicateCandidate)
        );
    }

    #[test]
    fn exact_operation_replay_returns_same_terminal_outcome() {
        let outcome = evaluate(request("one"));
        let mut ledger = AdmissionLedger::new(DEFAULT_ADMISSION_LIMITS)
            .expect("ledger");
        assert!(!ledger.record(outcome.clone()).expect("first").replayed);
        let replay = ledger.record(outcome).expect("replay");
        assert!(replay.replayed);
        assert!(matches!(replay.outcome, AdmissionOutcome::Admitted(_)));
    }

    #[test]
    fn operation_identity_reuse_with_other_payload_is_rejected() {
        let first = evaluate(request("one"));
        let mut second_request = request("two");
        second_request.operation = AdmissionOperation::new(
            first.operation().mutation().clone(),
            Blake3Digest32::from_bytes([99; 32]),
        );
        let second = evaluate(second_request);
        let mut ledger = AdmissionLedger::new(DEFAULT_ADMISSION_LIMITS)
            .expect("ledger");
        ledger.record(first).expect("first");
        assert_eq!(
            ledger.record(second),
            Err(AdmissionError::OperationConflict)
        );
    }
}
