//! Materialization request validation, finite budgets and cancellation.
//!
//! A [`MaterializationRequest`] names one exact retained revision, its
//! residency, digests, declared kind/encoding hint, accepted baseline profile
//! and stable operation identity. Validation binds the request to a profile
//! from the accepted set and to finite budgets; it never reads paths, the
//! current filesystem or any index payload.

use crate::MaterializationError;
use crate::profile::{SourceEncoding, SourceKind, ValidatedMaterializerProfile};
use core::sync::atomic::{AtomicBool, Ordering};
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

use crate::profile::MaterializerProfileId;

/// Finite per-operation budgets. All dimensions are non-zero after validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializationBudget {
    /// Maximum exact retained input bytes for this operation.
    pub max_input_bytes: u64,
    /// Maximum canonical output bytes for this operation.
    pub max_output_bytes: u64,
    /// Maximum logical lines for this operation.
    pub max_lines: u64,
    /// Maximum coordinate map segments for this operation.
    pub max_map_segments: u64,
    /// Maximum loss map records for this operation.
    pub max_loss_records: u64,
    /// Maximum elementary decode/normalize/map steps for this operation.
    pub max_steps: u64,
}

/// Conservative default operation budget matching the baseline profile limits.
pub const DEFAULT_MATERIALIZATION_BUDGET: MaterializationBudget = MaterializationBudget {
    max_input_bytes: 8 * 1024 * 1024,
    max_output_bytes: 8 * 1024 * 1024,
    max_lines: 1_000_000,
    max_map_segments: 1_000_032,
    max_loss_records: 1_000_032,
    max_steps: 64 * 1024 * 1024,
};

impl MaterializationBudget {
    /// Validates all finite dimensions as non-zero.
    pub const fn validate(self) -> Result<Self, MaterializationError> {
        if self.max_input_bytes == 0
            || self.max_output_bytes == 0
            || self.max_lines == 0
            || self.max_map_segments == 0
            || self.max_loss_records == 0
            || self.max_steps == 0
        {
            return Err(MaterializationError::InvalidLimits);
        }
        Ok(self)
    }

    pub(crate) const fn effective_input(&self, profile_max: u64) -> u64 {
        if self.max_input_bytes < profile_max {
            self.max_input_bytes
        } else {
            profile_max
        }
    }

    pub(crate) const fn effective_output(&self, profile_max: u64) -> u64 {
        if self.max_output_bytes < profile_max {
            self.max_output_bytes
        } else {
            profile_max
        }
    }

    pub(crate) const fn effective_lines(&self, profile_max: u64) -> u64 {
        if self.max_lines < profile_max {
            self.max_lines
        } else {
            profile_max
        }
    }

    pub(crate) const fn effective_segments(&self, profile_max: u64) -> u64 {
        if self.max_map_segments < profile_max {
            self.max_map_segments
        } else {
            profile_max
        }
    }

    pub(crate) const fn effective_loss(&self, profile_max: u64) -> u64 {
        if self.max_loss_records < profile_max {
            self.max_loss_records
        } else {
            profile_max
        }
    }

    pub(crate) const fn effective_steps(&self, profile_max: u64) -> u64 {
        if self.max_steps < profile_max {
            self.max_steps
        } else {
            profile_max
        }
    }
}

/// Cooperative cancellation token over an externally owned atomic flag.
///
/// [`CancellationToken::never`] never cancels and carries no flag; long
/// transforms poll [`CancellationToken::is_cancelled`] at every line and map
/// segment, so cancellation never returns a successful complete product.
#[derive(Clone, Copy, Debug)]
pub struct CancellationToken<'a> {
    flag: Option<&'a AtomicBool>,
}

impl<'a> CancellationToken<'a> {
    /// Token that never cancels.
    #[must_use]
    pub const fn never() -> Self {
        Self { flag: None }
    }

    /// Token observing an externally owned atomic flag.
    #[must_use]
    pub const fn new(flag: &'a AtomicBool) -> Self {
        Self { flag: Some(flag) }
    }

    /// Reports whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.is_some_and(|flag| flag.load(Ordering::SeqCst))
    }
}

/// Unvalidated materialization request. Paths, file handles and index payloads
/// cannot be expressed here by construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializationRequest {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Exact retained revision to reopen.
    pub revision: NonZeroRevision,
    /// Expected residency identity of the retained revision.
    pub residency: OpaqueId,
    /// Exact content digest attested by the revision pipeline.
    pub content_digest: Blake3Digest32,
    /// Caller-recorded exact byte count.
    pub byte_count: u64,
    /// Declared source kind hint, checked against the profile.
    pub declared_kind: SourceKind,
    /// Declared encoding hint, checked against the profile.
    pub declared_encoding: SourceEncoding,
    /// Required baseline profile identity.
    pub profile_id: MaterializerProfileId,
    /// Stable operation identity for retry correlation.
    pub operation_id: OpaqueId,
    /// Whether the bytes originate from an unsaved buffer.
    pub from_unsaved_bytes: bool,
    /// Explicit authenticated durable snapshot-admission receipt for unsaved bytes.
    pub unsaved_snapshot_receipt: Option<ReceiptRef>,
}

/// Accepted baseline profile set for request validation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AcceptedProfiles {
    profiles: Vec<ValidatedMaterializerProfile>,
}

impl AcceptedProfiles {
    /// Builds the accepted set. An empty set accepts nothing.
    #[must_use]
    pub const fn new(profiles: Vec<ValidatedMaterializerProfile>) -> Self {
        Self { profiles }
    }

    /// Finds an accepted profile by canonical identity.
    #[must_use]
    pub fn find(&self, id: &MaterializerProfileId) -> Option<&ValidatedMaterializerProfile> {
        self.profiles.iter().find(|profile| &profile.id() == id)
    }

    /// Number of accepted profiles.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Reports whether the accepted set is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

/// Request bound to one accepted profile and finite budgets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedMaterializationRequest {
    source_id: OpaqueId,
    revision: NonZeroRevision,
    residency: OpaqueId,
    content_digest: Blake3Digest32,
    byte_count: u64,
    declared_kind: SourceKind,
    declared_encoding: SourceEncoding,
    profile: ValidatedMaterializerProfile,
    operation_id: OpaqueId,
    admitted_unsaved: bool,
}

impl ValidatedMaterializationRequest {
    /// Stable source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Exact retained revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Expected residency identity.
    #[must_use]
    pub const fn residency(&self) -> &OpaqueId {
        &self.residency
    }

    /// Exact content digest.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 {
        self.content_digest
    }

    /// Caller-recorded exact byte count.
    #[must_use]
    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    /// Declared source kind hint.
    #[must_use]
    pub const fn declared_kind(&self) -> SourceKind {
        self.declared_kind
    }

    /// Declared encoding hint.
    #[must_use]
    pub const fn declared_encoding(&self) -> SourceEncoding {
        self.declared_encoding
    }

    /// Bound accepted profile.
    #[must_use]
    pub const fn profile(&self) -> &ValidatedMaterializerProfile {
        &self.profile
    }

    /// Stable operation identity.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }

    /// Whether unsaved bytes were admitted through an explicit snapshot receipt.
    #[must_use]
    pub const fn admitted_unsaved(&self) -> bool {
        self.admitted_unsaved
    }

    /// Re-targets a validated request at a different retained revision.
    ///
    /// All other bindings are unchanged. Production callers revalidate and
    /// re-admit the new revision; this helper exists for verification and
    /// determinism flows that must hold every other input stable.
    #[must_use]
    pub const fn with_revision(mut self, revision: NonZeroRevision) -> Self {
        self.revision = revision;
        self
    }
}

/// Validates a materialization request against accepted profiles and budgets.
///
/// Unsaved bytes are rejected unless they carry an explicit authenticated
/// durable snapshot-admission receipt. A path or current file cannot
/// substitute for the revision: the request type cannot express one.
pub fn validate_materialization_request(
    request: &MaterializationRequest,
    accepted: &AcceptedProfiles,
    budget: &MaterializationBudget,
) -> Result<ValidatedMaterializationRequest, MaterializationError> {
    let budget = budget.validate()?;
    let Some(profile) = accepted.find(&request.profile_id) else {
        return Err(MaterializationError::ProfileMismatch);
    };
    if !profile.source_kinds().contains(&request.declared_kind) {
        return Err(MaterializationError::Unsupported);
    }
    if !profile.encodings().contains(&request.declared_encoding) {
        return Err(MaterializationError::EncodingUnsupported);
    }
    if request.byte_count == 0 {
        return Err(MaterializationError::RequestInvalid);
    }
    let max_input = budget.effective_input(profile.limits().max_input_bytes);
    if request.byte_count > max_input {
        return Err(MaterializationError::BudgetExhausted);
    }
    if request.from_unsaved_bytes && request.unsaved_snapshot_receipt.is_none() {
        return Err(MaterializationError::UnsavedSnapshotNotAdmitted);
    }
    Ok(ValidatedMaterializationRequest {
        source_id: request.source_id.clone(),
        revision: request.revision,
        residency: request.residency.clone(),
        content_digest: request.content_digest,
        byte_count: request.byte_count,
        declared_kind: request.declared_kind,
        declared_encoding: request.declared_encoding,
        profile: profile.clone(),
        operation_id: request.operation_id.clone(),
        admitted_unsaved: request.from_unsaved_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{baseline_profile_descriptor, validate_materializer_profile};

    fn profile() -> ValidatedMaterializerProfile {
        validate_materializer_profile(&baseline_profile_descriptor("request-test", 1))
            .expect("profile")
    }

    fn request(profile: &ValidatedMaterializerProfile) -> MaterializationRequest {
        MaterializationRequest {
            source_id: OpaqueId::new("source:test").expect("source"),
            revision: NonZeroRevision::new(1).expect("revision"),
            residency: OpaqueId::new("residency:test").expect("residency"),
            content_digest: Blake3Digest32::from_bytes([7; 32]),
            byte_count: 4,
            declared_kind: SourceKind::Text,
            declared_encoding: SourceEncoding::Utf8,
            profile_id: profile.id(),
            operation_id: OpaqueId::new("operation:test").expect("operation"),
            from_unsaved_bytes: false,
            unsaved_snapshot_receipt: None,
        }
    }

    #[test]
    fn valid_request_binds_profile() {
        let profile = profile();
        let accepted = AcceptedProfiles::new(vec![profile.clone()]);
        let validated = validate_materialization_request(
            &request(&profile),
            &accepted,
            &DEFAULT_MATERIALIZATION_BUDGET,
        )
        .expect("valid");
        assert_eq!(validated.profile().id(), profile.id());
        assert!(!validated.admitted_unsaved());
    }

    #[test]
    fn unknown_profile_is_mismatch() {
        let profile = profile();
        let other =
            validate_materializer_profile(&baseline_profile_descriptor("other", 1)).expect("other");
        let accepted = AcceptedProfiles::new(vec![other]);
        assert_eq!(
            validate_materialization_request(
                &request(&profile),
                &accepted,
                &DEFAULT_MATERIALIZATION_BUDGET
            ),
            Err(MaterializationError::ProfileMismatch)
        );
    }

    #[test]
    fn unsupported_kind_and_encoding_are_typed() {
        // Narrow the accepted profile, then declare outside its support.
        let mut narrow = crate::profile::baseline_profile_descriptor("narrow", 1);
        narrow.source_kinds = vec![SourceKind::Text];
        let narrow = validate_materializer_profile(&narrow).expect("narrow");
        let accepted_narrow = AcceptedProfiles::new(vec![narrow.clone()]);
        let mut kind_input = request(&narrow);
        kind_input.declared_kind = SourceKind::Code;
        assert_eq!(
            validate_materialization_request(
                &kind_input,
                &accepted_narrow,
                &DEFAULT_MATERIALIZATION_BUDGET
            ),
            Err(MaterializationError::Unsupported)
        );
        let mut limited = crate::profile::baseline_profile_descriptor("limited", 1);
        limited.encodings = vec![SourceEncoding::Utf8];
        let limited = validate_materializer_profile(&limited).expect("limited");
        let accepted_limited = AcceptedProfiles::new(vec![limited.clone()]);
        let mut limited_input = request(&limited);
        limited_input.declared_encoding = SourceEncoding::Utf16Be;
        assert_eq!(
            validate_materialization_request(
                &limited_input,
                &accepted_limited,
                &DEFAULT_MATERIALIZATION_BUDGET
            ),
            Err(MaterializationError::EncodingUnsupported)
        );
    }

    #[test]
    fn zero_bytes_are_invalid_and_oversize_is_budget() {
        let profile = profile();
        let accepted = AcceptedProfiles::new(vec![profile.clone()]);
        let mut input = request(&profile);
        input.byte_count = 0;
        assert_eq!(
            validate_materialization_request(&input, &accepted, &DEFAULT_MATERIALIZATION_BUDGET),
            Err(MaterializationError::RequestInvalid)
        );
        input.byte_count = DEFAULT_MATERIALIZATION_BUDGET.max_input_bytes + 1;
        assert_eq!(
            validate_materialization_request(&input, &accepted, &DEFAULT_MATERIALIZATION_BUDGET),
            Err(MaterializationError::BudgetExhausted)
        );
    }

    #[test]
    fn unsaved_bytes_require_snapshot_receipt() {
        let profile = profile();
        let accepted = AcceptedProfiles::new(vec![profile.clone()]);
        let mut input = request(&profile);
        input.from_unsaved_bytes = true;
        assert_eq!(
            validate_materialization_request(&input, &accepted, &DEFAULT_MATERIALIZATION_BUDGET),
            Err(MaterializationError::UnsavedSnapshotNotAdmitted)
        );
        input.unsaved_snapshot_receipt =
            Some(ReceiptRef::new("receipt:snapshot").expect("receipt"));
        let validated =
            validate_materialization_request(&input, &accepted, &DEFAULT_MATERIALIZATION_BUDGET)
                .expect("admitted");
        assert!(validated.admitted_unsaved());
    }

    #[test]
    fn invalid_budgets_fail_closed() {
        let profile = profile();
        let accepted = AcceptedProfiles::new(vec![profile.clone()]);
        let budget = MaterializationBudget {
            max_steps: 0,
            ..DEFAULT_MATERIALIZATION_BUDGET
        };
        assert_eq!(
            validate_materialization_request(&request(&profile), &accepted, &budget),
            Err(MaterializationError::InvalidLimits)
        );
    }
}
