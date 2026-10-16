//! C24 exact raw-unit readback through the retained native process owner.
//!
//! Source bytes and residency metadata come from the same guarded C02/C07
//! reopen. No metadata copied from a nomination can authorize residency. The
//! caller retains one read state for the entire admitted request, including
//! yields; reopening a small unit still charges the full encrypted revision.

use search_candidate_validator::{
    CandidateAssurance, ExactRevisionReadbackRequest, RevisionReadbackPort, SourceReadback,
    VerifiedSourceSlice, reopen_and_verify_raw_v3_full_unit_v1,
};
use search_contracts::{
    Blake3Digest32, Blake3ExactInputLimit, SourceMembershipId, SourceRevisionRef,
    blake3_exact_bytes,
};
use search_unitizer::{
    V3AnchorPolicy, V3AttachmentPolicy, V3OmissionPolicy, V3OverlapPolicy, VerifiedUnitSet,
};

use crate::{
    access_composition::StandalonePreRetrievalAdmission,
    config_composition::EffectiveDaemonConfig,
    owner_composition::{NativeProcessRootCustody, RetainedRevisionReadState},
    provider_composition::{CanonicalServingError, CanonicalWorkOutput},
};

const fn unavailable() -> CanonicalServingError {
    CanonicalServingError::Backend("SOURCE_REVISION_UNAVAILABLE")
}

struct NativeReadback<F> {
    read: F,
    failure: Option<CanonicalServingError>,
}

impl<F> RevisionReadbackPort for NativeReadback<F>
where
    F: FnMut(&ExactRevisionReadbackRequest) -> Result<SourceReadback, CanonicalServingError>,
{
    type Error = ();

    fn read_exact(
        &mut self,
        request: &ExactRevisionReadbackRequest,
    ) -> Result<SourceReadback, Self::Error> {
        match (self.read)(request) {
            Ok(readback) => Ok(readback),
            Err(error) => {
                self.failure = Some(error);
                Err(())
            }
        }
    }
}

/// Reopen and verify exactly one complete raw-v3 unit in the current live turn.
///
/// `expected` is the immutable source reference used by the source owner, not a
/// path or Qdrant record. The native owner independently resolves its exact
/// committed object and checks access, purge and original request context.
#[allow(clippy::too_many_arguments)]
pub(super) fn reopen_retained_raw_v3(
    custody: &mut NativeProcessRootCustody,
    state: &mut RetainedRevisionReadState,
    registry: &search_source_registry::SourceRegistry,
    output: &mut CanonicalWorkOutput<'_, '_, '_>,
    admission: &StandalonePreRetrievalAdmission,
    membership: SourceMembershipId,
    expected: &SourceRevisionRef,
    units: &VerifiedUnitSet,
    request: &ExactRevisionReadbackRequest,
    config: &EffectiveDaemonConfig,
) -> Result<VerifiedSourceSlice, CanonicalServingError> {
    let profile = units.manifest().profile();
    let provenance = units.manifest().provenance();
    let binding = provenance.binding();
    let unit = units
        .units()
        .iter()
        .find(|unit| unit.unit_id() == request.unit_id)
        .ok_or_else(unavailable)?;
    let profile_id = units.manifest().profile_id();
    let profile_digest = profile_id
        .as_str()
        .strip_prefix("unitizer-v3-")
        .and_then(|digest| Blake3Digest32::parse_hex(digest).ok())
        .ok_or_else(unavailable)?;
    if profile.anchor_policy != V3AnchorPolicy::ExactTextBytes
        || profile.attachment_policy != V3AttachmentPolicy::Absent
        || profile.omission_policy != V3OmissionPolicy::Forbidden
        || profile.overlap_policy != V3OverlapPolicy::None
        || binding.source_id != expected.source_id
        || binding.source_revision_id != expected.revision_id
        || provenance.input_digest() != expected.content_digest
        || provenance.canonical_digest() != expected.content_digest
        || provenance.native_bytes() != expected.byte_length
        || provenance.canonical_bytes() != expected.byte_length
        || request.source_membership_id != membership
        || request.source_revision_id != expected.revision_id
        || request.representation_id != units.representation().representation_id
        || request.byte_start != unit.source_start()
        || request.byte_end != unit.source_end()
        || request.expected_content_digest != expected.content_digest
        || request.expected_unit_digest != unit.unit_content_digest()
        || request.expected_profile_id != *profile_id
        || request.expected_profile_digest != profile_digest
        || request.expected_assurance != CandidateAssurance::Exact
        || request.byte_start >= request.byte_end
        || request.byte_end - request.byte_start > request.max_read_bytes
    {
        return Err(unavailable());
    }
    let start = usize::try_from(unit.source_start()).map_err(|_| unavailable())?;
    let end = usize::try_from(unit.source_end()).map_err(|_| unavailable())?;
    let read = |_: &ExactRevisionReadbackRequest| {
        custody.with_serving_retained_read(
            state,
            registry,
            output,
            admission,
            membership,
            expected,
            config,
            |body, metadata| {
                let residency =
                    Blake3Digest32::from_bytes(*metadata.object_residency_key_digest().as_bytes());
                if residency != request.expected_residency_digest {
                    return Err(unavailable());
                }
                let bytes = body.get(start..end).ok_or_else(unavailable)?;
                let limit = Blake3ExactInputLimit::new(bytes.len()).map_err(|_| unavailable())?;
                let excerpt_digest = blake3_exact_bytes(bytes, limit).map_err(|_| unavailable())?;
                let mut copied = Vec::new();
                copied
                    .try_reserve_exact(bytes.len())
                    .map_err(|_| unavailable())?;
                copied.extend_from_slice(bytes);
                Ok(SourceReadback {
                    source_membership_id: membership,
                    source_revision_id: expected.revision_id,
                    representation_id: units.representation().representation_id,
                    unit_id: unit.unit_id(),
                    content_digest: expected.content_digest,
                    unit_digest: unit.unit_content_digest(),
                    excerpt_digest,
                    profile_id: profile_id.clone(),
                    profile_digest,
                    residency_digest: residency,
                    assurance: CandidateAssurance::Exact,
                    source_byte_start: unit.source_start(),
                    source_byte_end: unit.source_end(),
                    coordinate_map_exact: true,
                    residency_authorized: true,
                    bytes: copied,
                })
            },
        )
    };
    let mut port = NativeReadback {
        read,
        failure: None,
    };
    let verified = reopen_and_verify_raw_v3_full_unit_v1(request, &mut port);
    if let Some(error) = port.failure {
        return Err(error);
    }
    verified.map_err(|_| unavailable())
}
