//! Real verified-UnitSet support for focused planner fixtures.
//!
//! Builds one `VerifiedUnitSet` through the public materializer and unitizer
//! entry points over an exact in-memory revision port. No fake product
//! constructor and no package-local digest is used, so completeness authority
//! stays with the owner.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use search_contracts::{
    Blake3Digest32, MaterializationId, NonZeroRevision, OpaqueId, ProfileId, RepresentationId,
    SourceId, SourceNamespaceId, SourceRevisionId, UnitKind,
};
use search_materializer::api::{
    AcceptedProfiles, CancellationToken, DEFAULT_MATERIALIZATION_BUDGET, MaterializationContext,
    MaterializationError, MaterializationProduct, MaterializationRequest, NewlinePolicy,
    RevisionReadPort, SourceEncoding, SourceKind, StoredRevisionBytes,
    ValidatedMaterializationRequest, ValidatedMaterializerProfile, baseline_profile_descriptor,
    materialize_text_or_code, validate_materialization_request, validate_materializer_profile,
};
use search_unitizer::{
    UnitizationBudget, UnitizationLimits, V3AnchorPolicy, V3AttachmentPolicy, V3EmptyPolicy,
    V3OmissionPolicy, V3OverlapPolicy, V3RepresentationKind, V3SourceBinding,
    V3UnitizerProfileDescriptor, VerifiedUnitSet, build_unit_manifest, prepare_unit_set_input,
    validate_v3_unitizer_profile,
};

/// Raw source digest donated only to the revision port attestation.
fn raw_source_digest(bytes: &[u8]) -> Blake3Digest32 {
    search_contracts::blake3_exact_bytes(
        bytes,
        search_contracts::Blake3ExactInputLimit::new(4096).unwrap(),
    )
    .unwrap()
}

/// Deterministic in-memory revision port for one fixed source/revision.
struct ExactRevisionPort {
    source: OpaqueId,
    revision: NonZeroRevision,
    residency: OpaqueId,
    bytes: Vec<u8>,
    reads: AtomicUsize,
}

impl RevisionReadPort for ExactRevisionPort {
    fn read_exact(
        &self,
        source: &OpaqueId,
        revision: NonZeroRevision,
        byte_count: u64,
    ) -> Result<StoredRevisionBytes, MaterializationError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        if &self.source != source
            || self.revision != revision
            || byte_count != self.bytes.len() as u64
        {
            return Err(MaterializationError::RevisionUnavailable);
        }
        Ok(StoredRevisionBytes::new(
            self.bytes.clone(),
            raw_source_digest(&self.bytes),
            self.residency.clone(),
        ))
    }
}

/// Accepted UTF-8 baseline profile through the real entry point.
fn baseline_profile() -> ValidatedMaterializerProfile {
    validate_materializer_profile(&baseline_profile_descriptor("259-baseline-utf8", 1))
        .expect("baseline UTF-8 profile validates")
}

/// One validated request for `bytes` bound to `source`.
fn validated_request(
    bytes: &[u8],
    profile: &ValidatedMaterializerProfile,
    source: &OpaqueId,
    revision: u64,
) -> Result<ValidatedMaterializationRequest, MaterializationError> {
    let revision = NonZeroRevision::new(revision).expect("nonzero revision");
    let request = MaterializationRequest {
        source_id: source.clone(),
        revision,
        residency: OpaqueId::new("residency:259-planner").expect("opaque residency"),
        content_digest: raw_source_digest(bytes),
        byte_count: u64::try_from(bytes.len()).expect("length fits u64"),
        declared_kind: SourceKind::Text,
        declared_encoding: SourceEncoding::Utf8,
        profile_id: profile.id(),
        operation_id: OpaqueId::new("operation:259-materialize").expect("opaque operation"),
        from_unsaved_bytes: false,
        unsaved_snapshot_receipt: None,
    };
    validate_materialization_request(
        &request,
        &AcceptedProfiles::new(vec![profile.clone()]),
        &DEFAULT_MATERIALIZATION_BUDGET,
    )
}

/// Builds the exact materialization product for `bytes`.
fn materialize_product(
    bytes: &[u8],
    source: &OpaqueId,
    revision: u64,
) -> Result<
    (
        MaterializationProduct,
        ValidatedMaterializationRequest,
        ValidatedMaterializerProfile,
    ),
    MaterializationError,
> {
    let profile = baseline_profile();
    assert_eq!(profile.newline_policy(), NewlinePolicy::PreserveExact);
    let request = validated_request(bytes, &profile, source, revision)?;

    let port = ExactRevisionPort {
        source: source.clone(),
        revision: request.revision(),
        residency: request.residency().clone(),
        bytes: bytes.to_vec(),
        reads: AtomicUsize::new(0),
    };
    let context = MaterializationContext {
        profile: profile.clone(),
        budget: DEFAULT_MATERIALIZATION_BUDGET,
        cancel: CancellationToken::never(),
    };
    let product = materialize_text_or_code(&request, &port, &context)?;
    Ok((product, request, profile))
}

/// The accepted baseline v3 unitizer descriptor.
pub fn descriptor() -> V3UnitizerProfileDescriptor {
    V3UnitizerProfileDescriptor {
        profile_name: ProfileId::new("baseline-unitizer-v3").unwrap(),
        profile_revision: 1,
        representation_kind: V3RepresentationKind::Text,
        unit_kind: UnitKind::Section,
        boundary_revision: 1,
        overlap_policy: V3OverlapPolicy::None,
        anchor_policy: V3AnchorPolicy::ExactTextBytes,
        attachment_policy: V3AttachmentPolicy::Absent,
        omission_policy: V3OmissionPolicy::Forbidden,
        empty_policy: V3EmptyPolicy::Reject,
        limits: UnitizationLimits {
            max_input_bytes: 4096,
            preferred_unit_bytes: 8,
            max_unit_bytes: 16,
            max_lines: 512,
            max_units: 512,
        },
        min_unit_bytes: 1,
        max_unit_scalars: 16,
        max_unit_lines: 16,
        max_anchor_depth: 1,
        max_manifest_bytes: 512 * 1024,
        max_steps: 1_000_000,
        fixture_digest: Blake3Digest32::from_bytes([7; 32]),
    }
}

/// Finite bounded budget for the unitizer ingress.
fn budget(cancelled: &AtomicBool) -> UnitizationBudget<'_> {
    UnitizationBudget {
        max_steps: 1_000_000,
        max_encoded_bytes: 512 * 1024,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled,
    }
}

/// The planner unit binding coordinates used by every fixture below.
pub const fn binding() -> V3SourceBinding {
    V3SourceBinding {
        source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
        source_id: SourceId::from_bytes([2; 16]),
        source_revision_id: SourceRevisionId::from_bytes([3; 16]),
        materialization_id: MaterializationId::from_bytes([5; 16]),
        representation_id: RepresentationId::from_bytes([4; 16]),
    }
}

/// The exact opaque source coordinate the materializer attests.
fn opaque_source(binding: &V3SourceBinding) -> OpaqueId {
    OpaqueId::new(binding.source_id.to_string()).expect("opaque source")
}

/// Builds one real verified unit set over `text`.
pub fn verified_unit_set(text: &str, binding: V3SourceBinding) -> VerifiedUnitSet {
    let profile = validate_v3_unitizer_profile(&descriptor()).expect("v3 profile validates");
    let (product, request, materializer) =
        materialize_product(text.as_bytes(), &opaque_source(&binding), 1).expect("materialize");
    let cancel = AtomicBool::new(false);
    let budget = budget(&cancel);
    let input = prepare_unit_set_input(
        binding,
        &product,
        &request,
        &materializer,
        &profile,
        &budget,
    )
    .expect("prepare unit set input");
    build_unit_manifest(&input, &profile, &budget).expect("build unit manifest")
}
