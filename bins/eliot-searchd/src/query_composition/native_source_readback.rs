//! C24 validation through the retained native process owner and actual C03 state.
//! Source bytes and residency metadata come from the same guarded C02/C07 reopen.
//! The caller retains one read state across the entire admitted request.

use search_candidate_validator::{
    CandidateAssurance, CandidateNomination, ExactRevisionReadbackRequest, RevisionReadbackPort,
    SourceReadback, ValidationContext, ValidationOutcome, validate_raw_v3_full_unit_v1,
};
use search_contracts::{
    Blake3Digest32, Blake3ExactInputLimit, CandidateId, NamespaceOwnershipStatus, SourceOwnerFence,
    SourceRevision, SourceRevisionRef, blake3_exact_bytes,
};
use search_unitizer::VerifiedUnitSet;

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

/// Validate through C24, then the original native read and actual C07 metadata.
///
/// The ID is allocated once by the admitted task's ID source. C24 owns source,
/// unit, profile and range validation, actual-byte hashing, final permit and
/// receipt construction. Root supplies current C03 ownership and the concrete
/// native port. No query-time store write or fresh source-read budget is created.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_retained_raw_v3(
    custody: &mut NativeProcessRootCustody,
    state: &mut RetainedRevisionReadState,
    registry: &search_source_registry::SourceRegistry,
    output: &mut CanonicalWorkOutput<'_, '_, '_>,
    admission: &StandalonePreRetrievalAdmission,
    source_revision: &SourceRevision,
    expected: &SourceRevisionRef,
    units: &VerifiedUnitSet,
    nomination: &CandidateNomination,
    validation: &ValidationContext,
    candidate_id: CandidateId,
    final_live_security: &search_access::LiveSecurityState,
    config: &EffectiveDaemonConfig,
) -> Result<ValidationOutcome, CanonicalServingError> {
    if registry.revision() != admission.scope.source_catalog_generation {
        return Err(CanonicalServingError::Access(
            search_access::AccessError::SnapshotStale,
        ));
    }
    let membership_id = nomination.source_membership_id;
    let (membership, identity) = registry
        .active_source_membership_v2(membership_id)
        .map_err(|_| unavailable())?;
    let namespace = registry
        .namespaces()
        .get(&identity.source_namespace_id)
        .ok_or_else(unavailable)?;
    if namespace.ownership.status != NamespaceOwnershipStatus::Active
        || namespace.fenced
        || namespace.preparation.is_some()
        || namespace.activation.is_some()
    {
        return Err(unavailable());
    }
    let owner_fence = SourceOwnerFence {
        source_namespace_id: identity.source_namespace_id,
        source_owner_generation: namespace.ownership.source_owner_generation,
    };
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
        custody.with_serving_retained_read(
            state,
            registry,
            output,
            admission,
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
    let validated = validate_raw_v3_full_unit_v1(
        nomination,
        validation,
        candidate_id,
        membership,
        source_revision,
        expected,
        owner_fence,
        units,
        expected.byte_length,
        &mut port,
        final_live_security,
    );
    if let Some(error) = port.failure {
        return Err(error);
    }
    Ok(validated)
}
