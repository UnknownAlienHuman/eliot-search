//! C28 V2 composition using the same C07 read gate, pin registry and handle store.

use super::{
    PreparedCrypto, PreparedProtection, RootHandleCryptoPort, SourceProtection, retained_target,
    unavailable,
};
use crate::owner_composition::{NativeProcessRootCustody, RetainedRevisionReadState};
use crate::provider_composition::CanonicalServingError;
use crate::query_composition::native_v2::ReadTurn;
use search_candidate_validator::ValidatedSearchCandidate;
use search_contracts::SearchSourceHandle;
use search_handles::{HandleCryptoPort, PendingHandleMints};
use search_retention::source_pins::SourcePinTarget;
use std::time::Instant;

#[allow(clippy::too_many_arguments)]
pub(in crate::query_composition) fn mint_evidence_candidate_handle(
    custody: &mut NativeProcessRootCustody,
    state: &mut RetainedRevisionReadState,
    registry: &search_source_registry::SourceRegistry,
    turn: &ReadTurn<'_, '_>,
    pending: &mut PendingHandleMints<'_>,
    candidate: &ValidatedSearchCandidate,
    prepared: &crate::preparation_composition::artifact_payloads::PreparedRawCodeArtifacts,
    clock: &mut crate::access_composition::SystemGrantClock,
    deadline: Instant,
    config: &crate::config_composition::EffectiveDaemonConfig,
) -> Result<SearchSourceHandle, CanonicalServingError> {
    turn.check()?;
    let source = candidate.source();
    let expected = candidate.source_revision_ref();
    let target = retained_target(candidate, prepared, *turn.request.guard().request_id())?;
    let namespace = registry
        .namespaces()
        .get(&expected.source_namespace_id)
        .ok_or_else(unavailable)?;
    if namespace.ownership.source_owner_generation != target.source_owner_generation {
        return Err(unavailable());
    }
    let name = search_config::ConfigSectionName::new("handles", 64).map_err(|_| unavailable())?;
    let handles = config
        .snapshot()
        .section(&name)
        .ok_or_else(unavailable)?
        .validated();
    let cancellation = turn.request.guard().cancellation();
    let mut crypto = RootHandleCryptoPort::new(clock, handles, deadline, &cancellation)
        .map_err(|_| unavailable())?;
    let material = crypto.mint_material().map_err(|_| unavailable())?;
    let pins = custody.source_pins().clone();
    let protection = custody.with_evidence_retained_metadata(
        state,
        registry,
        turn,
        source.source_membership_id(),
        &expected,
        |metadata| {
            if metadata.source().namespace_id != expected.source_namespace_id
                || metadata.source().source_id != expected.source_id
                || metadata.source().revision_id != expected.revision_id
                || metadata.object_residency_key_digest() != target.object_residency_key_digest
            {
                return Err(unavailable());
            }
            let pin = pins
                .pin(
                    SourcePinTarget::new(
                        metadata.storage_object_id().clone(),
                        expected,
                        metadata.object_residency_key_digest(),
                    ),
                    material.monotonic_deadline,
                )
                .map_err(|_| unavailable())?;
            Ok(SourceProtection {
                pin,
                target: target.clone(),
                expires_at: material.expires_at.clone(),
            })
        },
    )?;
    let mut protection = PreparedProtection(Some(protection));
    let mut crypto = PreparedCrypto {
        crypto: &mut crypto,
        material: Some(material),
    };
    let claims = &turn.request.envelope().grant.claims;
    turn.check()?;
    pending
        .mint_ephemeral_retained_source(
            target,
            source.source_membership_id(),
            source.representation_id(),
            source.unit_id(),
            claims.binding_id,
            claims.grant_id,
            custody.journal_identity().owner_epoch,
            claims.disclosure_ceiling,
            &mut protection,
            &mut crypto,
            turn.request.guard().is_cancelled(),
        )
        .map_err(|_| unavailable())
}
