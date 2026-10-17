//! Nominated-only C07 reconstruction and actual C24 source readback.

use super::ReadTurn;
use crate::owner_composition::{
    NativeProcessRootCustody, QualifiedNativeProfiles, RetainedRevisionReadState,
};
use crate::preparation_composition::artifact_payloads::PreparedRawCodeArtifacts;
use crate::provider_composition::CanonicalServingError as Error;
use crate::query_composition::{
    canonical_retrieval::{
        CanonicalPointIdentityResolver, UntrustedProjectionMembershipHint, VerifiedCanonicalPoint,
    },
    native_source_readback::validate_evidence_raw_v3,
};
use search_candidate_validator::{
    CandidateAssurance, CandidateNomination, ValidatedSearchCandidate, ValidationContext,
    ValidationOutcome,
};
use search_contracts::{self as c, indexed::PointPayload};
use search_point_identity::s11::{PointId128, PointIdentity};
use search_retrieval_executor::{
    LegTicket,
    canonical::{CanonicalIndexedNomination, CanonicalIndexedPortError},
};
use std::collections::{BTreeMap, BTreeSet};

const fn unavailable() -> Error {
    Error::Backend("NATIVE_V2_NOMINATION_UNAVAILABLE")
}

pub(super) struct ReopenedProjection {
    pub(super) prepared: PreparedRawCodeArtifacts,
    revision: c::SourceRevision,
    expected: c::SourceRevisionRef,
}

pub(super) struct RetainedProjectionResolver<'a, 'request, 'grant> {
    pub(super) custody: &'a mut NativeProcessRootCustody,
    pub(super) state: &'a mut RetainedRevisionReadState,
    pub(super) registry: &'a search_source_registry::SourceRegistry,
    pub(super) turn: &'a ReadTurn<'request, 'grant>,
    pub(super) profiles: &'a QualifiedNativeProfiles,
    pub(super) config: &'a crate::config_composition::EffectiveDaemonConfig,
    pub(super) cache: BTreeMap<c::ProjectionMembershipId, ReopenedProjection>,
    pub(super) memory_remaining: u64,
}

impl CanonicalPointIdentityResolver for RetainedProjectionResolver<'_, '_, '_> {
    fn resolve(
        &mut self,
        address: PointId128,
        hint: UntrustedProjectionMembershipHint,
        observation: &PointPayload,
    ) -> Result<PointIdentity, CanonicalIndexedPortError> {
        self.resolve_current(address, hint.projection_membership_id(), observation)
            .map_err(|_| CanonicalIndexedPortError::Failure)
    }
}

impl RetainedProjectionResolver<'_, '_, '_> {
    fn resolve_current(
        &mut self,
        address: PointId128,
        projection: c::ProjectionMembershipId,
        observation: &PointPayload,
    ) -> Result<PointIdentity, Error> {
        self.turn.check()?;
        let row = self
            .turn
            .population
            .projection_memberships()
            .values()
            .find(|row| row.projection_membership_id == projection)
            .ok_or_else(unavailable)?;
        if observation.projection_membership_id != projection {
            return Err(unavailable());
        }
        if !self.cache.contains_key(&projection) {
            let (prepared, revision) = self.custody.reopen_evidence_projection(
                self.state,
                self.registry,
                self.turn,
                projection,
                observation,
                self.profiles,
                self.memory_remaining,
                self.config,
            )?;
            self.memory_remaining = revision
                .byte_length
                .checked_mul(32)
                .and_then(|bytes| self.memory_remaining.checked_sub(bytes))
                .ok_or(Error::Backend("RESOURCE_EXHAUSTED"))?;
            let binding = prepared.units().manifest().provenance().binding();
            let expected = c::SourceRevisionRef {
                source_namespace_id: binding.source_namespace_id,
                source_id: binding.source_id,
                revision_id: binding.source_revision_id,
                content_digest: revision.content_digest,
                byte_length: revision.byte_length,
            };
            if prepared.scope().membership.source_membership_id != row.source_membership_id
                || prepared.scope().membership.projection_membership_id != projection
                || prepared.scope().membership.access_partition_id != row.access_partition_id
                || prepared.scope().membership.scoring_partition_id != row.scoring_partition_id
                || prepared.scope().membership.projection_profile_set_id
                    != row.projection_profile_set_id
            {
                return Err(unavailable());
            }
            self.cache.insert(
                projection,
                ReopenedProjection {
                    prepared,
                    revision,
                    expected,
                },
            );
        }
        let record = self.cache.get(&projection).ok_or_else(unavailable)?;
        let point = record
            .prepared
            .plan()
            .points
            .iter()
            .find(|point| point.point_id == address)
            .ok_or_else(unavailable)?;
        // The query observation is never expected identity. Both the complete
        // identity and expected payload come from the C07-verified producer.
        if observation.to_canonical_cbor().map_err(|_| unavailable())?
            != point.expected_readback.payload_cbor
        {
            return Err(unavailable());
        }
        self.turn.check()?;
        Ok(point.identity.clone())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Keep nominated identity, exact owner metadata and C24 readback validation together"
    )]
    pub(super) fn validate(
        &mut self,
        nomination: &CanonicalIndexedNomination<VerifiedCanonicalPoint>,
        ticket: &LegTicket,
        candidate_id: c::CandidateId,
        ordinal: usize,
        live: &search_access::LiveSecurityState,
    ) -> Result<ValidatedSearchCandidate, Error> {
        self.turn.check()?;
        let row = self
            .turn
            .population
            .projection_memberships()
            .get(&nomination.source_membership_id)
            .ok_or_else(unavailable)?;
        if row.projection_membership_id != nomination.projection_membership_id
            || !ticket
                .leg
                .memberships()
                .contains(&nomination.source_membership_id)
            || !nomination.raw_score.is_finite()
        {
            return Err(unavailable());
        }
        let record = self
            .cache
            .get(&nomination.projection_membership_id)
            .ok_or_else(unavailable)?;
        let prepared = &record.prepared;
        let point = prepared
            .plan()
            .points
            .iter()
            .find(|point| point.point_id.as_bytes() == nomination.point_address.as_bytes())
            .ok_or_else(unavailable)?;
        let payload = &nomination.candidate.payload;
        if point.identity != nomination.candidate.identity
            || payload.to_canonical_cbor().map_err(|_| unavailable())?
                != point.expected_readback.payload_cbor
        {
            return Err(unavailable());
        }
        let unit = prepared
            .units()
            .units()
            .iter()
            .find(|unit| unit.unit_id() == payload.unit_id)
            .ok_or_else(unavailable)?;
        let profile_id = prepared.units().manifest().profile_id();
        self.memory_remaining = unit
            .source_end()
            .checked_sub(unit.source_start())
            .and_then(|bytes| self.memory_remaining.checked_sub(bytes))
            .ok_or(Error::Backend("RESOURCE_EXHAUSTED"))?;
        let profile_digest = profile_id
            .as_str()
            .strip_prefix("unitizer-v3-")
            .and_then(|digest| c::Blake3Digest32::parse_hex(digest).ok())
            .ok_or_else(unavailable)?;
        let candidate = CandidateNomination {
            request_id: ticket.request_id,
            plan_digest: *ticket.plan_digest.as_bytes(),
            leg_id: ticket.leg.leg_id(),
            point_id: *nomination.point_address.as_bytes(),
            source_membership_id: nomination.source_membership_id,
            collection_generation_id: payload.collection_generation_id,
            valid_from_epoch: payload.valid_from_epoch,
            valid_until_epoch_exclusive: payload.valid_until_epoch_exclusive,
            profile_id: profile_id.clone(),
            profile_digest,
            source_revision_id: record.expected.revision_id,
            representation_id: prepared.units().representation().representation_id,
            unit_id: unit.unit_id(),
            unit_byte_start: unit.source_start(),
            unit_byte_end: unit.source_end(),
            expected_content_digest: record.expected.content_digest,
            expected_unit_digest: unit.unit_content_digest(),
            expected_excerpt_digest: None,
            expected_assurance: CandidateAssurance::Exact,
            expected_residency_digest: c::Blake3Digest32::from_bytes(
                *record.revision.object_residency_key_digest.as_bytes(),
            ),
            raw_score: nomination.raw_score,
        };
        let search_provider_protocol::frame::EvidenceRecipeRequestV2::RetrieveEvidence(request) =
            &self.turn.request.envelope().recipe_request
        else {
            return Err(unavailable());
        };
        let validation = ValidationContext {
            request_id: *self.turn.request.guard().request_id(),
            plan_digest: *ticket.plan_digest.as_bytes(),
            collection_generation_id: self.turn.population.population().collection_generation_id(),
            visible_epoch: self.turn.population.population().visible_epoch(),
            allowed_memberships: self
                .turn
                .population
                .projection_memberships()
                .keys()
                .copied()
                .collect(),
            shadowed_units: BTreeSet::new(),
            max_candidates: usize::try_from(request.budgets.max_validated_candidates)
                .map_err(|_| unavailable())?,
            candidate_ordinal: ordinal,
            request_security: self.turn.fence.clone(),
            live_security: live.clone(),
            contaminated_legs: BTreeSet::new(),
        };
        match validate_evidence_raw_v3(
            self.custody,
            self.state,
            self.registry,
            self.turn,
            &record.revision,
            &record.expected,
            prepared.units(),
            &candidate,
            &validation,
            candidate_id,
            live,
            self.config,
        )? {
            ValidationOutcome::Validated(candidate) => Ok(candidate),
            // Never silently drop an actual readback gap and return complete success.
            ValidationOutcome::Gap(_) => Err(Error::Backend("SOURCE_REVISION_UNAVAILABLE")),
            ValidationOutcome::ContaminatedLeg(_) => Err(Error::Backend("V2_CONTAMINATED_LEG")),
        }
    }
}
