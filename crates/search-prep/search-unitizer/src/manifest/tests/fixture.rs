//! Real materializer fixture for the unitizer's exact UTF-8 ingress.
//!
//! Builds a real `MaterializationProduct` through the public materializer API
//! with the accepted UTF-8 baseline profile. No fake product constructor: the
//! product is produced by the real pipeline over an injected `RevisionReadPort`,
//! and `open_exact_revision` re-verifies the port's BLAKE3 attestation against
//! the bytes it reads. The baseline profile pins `NewlinePolicy::PreserveExact`,
//! `BomPolicy::StripAndRecord` and `UnicodeNormalization::None`.

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, SourceId};
use search_materializer::api::{
    AcceptedProfiles, CancellationToken, DEFAULT_MATERIALIZATION_BUDGET, MaterializationContext,
    MaterializationError, MaterializationProduct, MaterializationRequest, NewlinePolicy,
    RevisionReadPort, SourceEncoding, SourceKind, StoredRevisionBytes,
    ValidatedMaterializationRequest, ValidatedMaterializerProfile, baseline_profile_descriptor,
    materialize_text_or_code, validate_materialization_request, validate_materializer_profile,
};

/// `blake3` (already `=1.8.2` in this crate's Cargo.toml) is the donor for the
/// raw source digest only; `search-materializer` produces every product digest.
fn raw_source_digest(bytes: &[u8]) -> Blake3Digest32 {
    Blake3Digest32::from_bytes(*blake3::hash(bytes).as_bytes())
}

/// Deterministic in-memory `RevisionReadPort` for one fixed source/revision.
///
/// It attests exactly the source/revision/byte-count triple it was built with,
/// so a request for anything else fails typed instead of fabricating bytes.
struct ExactRevisionPort {
    source: OpaqueId,
    revision: NonZeroRevision,
    residency: OpaqueId,
    bytes: Vec<u8>,
}

impl RevisionReadPort for ExactRevisionPort {
    fn read_exact(
        &self,
        source: &OpaqueId,
        revision: NonZeroRevision,
        byte_count: u64,
    ) -> Result<StoredRevisionBytes, MaterializationError> {
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

/// Accepted UTF-8 baseline profile, validated through the real entry point.
fn baseline_profile() -> ValidatedMaterializerProfile {
    validate_materializer_profile(&baseline_profile_descriptor("257-baseline-utf8", 1))
        .expect("baseline UTF-8 profile validates")
}

/// One validated request for `bytes` under `profile`, bound to `source`.
fn validated_request(
    bytes: &[u8],
    profile: &ValidatedMaterializerProfile,
    source: &SourceId,
    revision: u64,
    kind: SourceKind,
) -> Result<ValidatedMaterializationRequest, MaterializationError> {
    let revision = NonZeroRevision::new(revision).expect("nonzero revision");
    let request = MaterializationRequest {
        source_id: OpaqueId::new(source.to_string()).expect("opaque source"),
        revision,
        residency: OpaqueId::new("residency:257-watchdog").expect("opaque residency"),
        content_digest: raw_source_digest(bytes),
        byte_count: u64::try_from(bytes.len()).expect("length fits u64"),
        declared_kind: kind,
        declared_encoding: SourceEncoding::Utf8,
        profile_id: profile.id(),
        operation_id: OpaqueId::new("operation:257-materialize").expect("opaque operation"),
        from_unsaved_bytes: false,
        unsaved_snapshot_receipt: None,
    };
    validate_materialization_request(
        &request,
        &AcceptedProfiles::new(vec![profile.clone()]),
        &DEFAULT_MATERIALIZATION_BUDGET,
    )
}

/// Builds the product plus the owning request and profile for
/// `prepare_unit_set_input`. `source` renders once through
/// `SourceId::to_string` and is carried as `OpaqueId`.
pub(super) fn materialize_product(
    bytes: &[u8],
    source: &SourceId,
    revision: u64,
) -> Result<
    (
        MaterializationProduct,
        ValidatedMaterializationRequest,
        ValidatedMaterializerProfile,
    ),
    MaterializationError,
> {
    materialize_product_with_kind(bytes, source, revision, SourceKind::Text)
}

pub(super) fn materialize_product_with_kind(
    bytes: &[u8],
    source: &SourceId,
    revision: u64,
    kind: SourceKind,
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
    let request = validated_request(bytes, &profile, source, revision, kind)?;
    let port = ExactRevisionPort {
        source: OpaqueId::new(source.to_string()).expect("opaque source"),
        revision: request.revision(),
        residency: request.residency().clone(),
        bytes: bytes.to_vec(),
    };
    let context = MaterializationContext {
        profile: profile.clone(),
        budget: DEFAULT_MATERIALIZATION_BUDGET,
        cancel: CancellationToken::never(),
    };
    let product = materialize_text_or_code(&request, &port, &context)?;
    Ok((product, request, profile))
}
