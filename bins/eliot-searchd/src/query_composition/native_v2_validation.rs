//! C24 on the actual V2 retained reader; no protocol/V1 request conversion.

use super::{NativeReadback, current_owner_fence, unavailable};
use crate::owner_composition::{NativeProcessRootCustody, RetainedRevisionReadState};
use crate::provider_composition::CanonicalServingError;
use crate::query_composition::native_v2::ReadTurn;
use search_candidate_validator::{
    CandidateAssurance, CandidateNomination, ExactRevisionReadbackRequest, SourceReadback,
    ValidationContext, ValidationOutcome, validate_raw_v3_full_unit_v1,
};
use search_contracts::{
    Blake3Digest32, Blake3ExactInputLimit, CandidateId, SourceRevision, SourceRevisionRef,
    blake3_exact_bytes,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::query_composition) fn validate_evidence_raw_v3(
    custody: &mut NativeProcessRootCustody,
    state: &mut RetainedRevisionReadState,
    registry: &search_source_registry::SourceRegistry,
    turn: &ReadTurn<'_, '_>,
    revision: &SourceRevision,
    expected: &SourceRevisionRef,
    units: &search_unitizer::VerifiedUnitSet,
    nomination: &CandidateNomination,
    validation: &ValidationContext,
    candidate_id: CandidateId,
    live: &search_access::LiveSecurityState,
    config: &crate::config_composition::EffectiveDaemonConfig,
) -> Result<ValidationOutcome, CanonicalServingError> {
    turn.check()?;
    let membership_id = nomination.source_membership_id;
    let (membership, identity) = registry
        .active_source_membership_v2(membership_id)
        .map_err(|_| unavailable())?;
    let owner_fence = current_owner_fence(registry, identity.source_namespace_id)?;
    let read = |request: &ExactRevisionReadbackRequest| {
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
        let start = usize::try_from(unit.source_start()).map_err(|_| unavailable())?;
        let end = usize::try_from(unit.source_end()).map_err(|_| unavailable())?;
        custody.with_evidence_retained_read(
            state,
            registry,
            turn,
            membership_id,
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
                    source_membership_id: membership_id,
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
    let result = validate_raw_v3_full_unit_v1(
        nomination,
        validation,
        candidate_id,
        membership,
        revision,
        expected,
        owner_fence,
        units,
        expected.byte_length,
        &mut port,
        live,
    );
    if let Some(error) = port.failure {
        return Err(error);
    }
    turn.check()?;
    Ok(result)
}
