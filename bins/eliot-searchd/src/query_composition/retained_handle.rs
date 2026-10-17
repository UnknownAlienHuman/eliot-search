//! Retained-source handle composition over the actual C07 link and C28 pin owner.
//! Neither the pin nor the bearer token grants source-read permission.

#[path = "native_v2_handle.rs"]
mod evidence;
pub(super) use evidence::mint_evidence_candidate_handle;

use std::fmt;
use std::time::Instant;

use search_candidate_validator::ValidatedSearchCandidate;
use search_contracts::{
    AssuranceClass, HandleTokenDigest, NativeAnchor, OpaqueHandleToken, SearchSourceHandle,
    SourceView, UtcTimestamp,
};
use search_handles::{
    HandleCryptoPort, HandleError, MintMaterial, PendingHandleMints, RetainedSourceProtectionGuard,
    RetainedSourceProtectionPort, RetainedSourceTarget,
};
use search_ports::PackageOpaque;
use search_retention::source_pins::{OwnedSourcePinGuard, SourcePinTarget};

use crate::access_composition::StandalonePreRetrievalAdmission;
use crate::access_composition::SystemGrantClock;
use crate::config_composition::EffectiveDaemonConfig;
use crate::owner_composition::{NativeProcessRootCustody, RetainedRevisionReadState};
use crate::preparation_composition::artifact_payloads::PreparedRawCodeArtifacts;
use crate::provider_composition::{CanonicalServingError, CanonicalWorkOutput};

use super::handle_crypto::RootHandleCryptoPort;

struct SourceProtection {
    pin: OwnedSourcePinGuard,
    target: RetainedSourceTarget,
    expires_at: UtcTimestamp,
}

impl fmt::Debug for SourceProtection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceProtection")
            .finish_non_exhaustive()
    }
}

impl PackageOpaque for SourceProtection {
    fn owner_package(&self) -> &'static str {
        "eliot-searchd"
    }
}

impl RetainedSourceProtectionGuard for SourceProtection {
    fn target(&self) -> &RetainedSourceTarget {
        &self.target
    }
    fn expires_at(&self) -> &UtcTimestamp {
        &self.expires_at
    }
    fn monotonic_deadline(&self) -> Instant {
        self.pin.monotonic_deadline()
    }
    fn revalidate_current(&self) -> Result<(), HandleError> {
        self.pin
            .validate_active()
            .map_err(|_| HandleError::SourceProtectionUnavailable)
    }
}

/// The only protection this port can return was already acquired inside the
/// original issuer/native operation. It owns no borrowed Native custody.
#[derive(Debug)]
struct PreparedProtection(Option<SourceProtection>);

impl PackageOpaque for PreparedProtection {
    fn owner_package(&self) -> &'static str {
        "eliot-searchd"
    }
}

impl RetainedSourceProtectionPort for PreparedProtection {
    type Guard = SourceProtection;

    fn protect(
        &mut self,
        target: &RetainedSourceTarget,
        expires_at: &UtcTimestamp,
        deadline: Instant,
    ) -> Result<Self::Guard, HandleError> {
        let guard = self
            .0
            .take()
            .ok_or(HandleError::SourceProtectionUnavailable)?;
        if guard.target() != target
            || guard.expires_at() != expires_at
            || guard.monotonic_deadline() != deadline
        {
            return Err(HandleError::SourceProtectionMismatch);
        }
        guard.revalidate_current()?;
        Ok(guard)
    }
}

/// C28 asks for material after protection preparation. Return the original
/// single OS draw/window, rather than drawing a second token or extending TTL.
struct PreparedCrypto<'a, C> {
    crypto: &'a mut C,
    material: Option<MintMaterial>,
}

impl<C: HandleCryptoPort> HandleCryptoPort for PreparedCrypto<'_, C> {
    type Error = HandleError;

    fn mint_material(&mut self) -> Result<MintMaterial, Self::Error> {
        let material = self.material.take().ok_or(HandleError::TokenInvalid)?;
        // This reaches the original Root cancellation/deadline before insertion.
        self.token_digest(&material.opaque_token)?;
        Ok(material)
    }

    fn token_digest(&self, token: &OpaqueHandleToken) -> Result<HandleTokenDigest, Self::Error> {
        self.crypto
            .token_digest(token)
            .map_err(|_| HandleError::TokenInvalid)
    }
}

const fn unavailable() -> CanonicalServingError {
    CanonicalServingError::Backend("RETAINED_HANDLE_UNAVAILABLE")
}

fn retained_target(
    candidate: &ValidatedSearchCandidate,
    prepared: &PreparedRawCodeArtifacts,
    request_id: search_contracts::RequestId,
) -> Result<RetainedSourceTarget, CanonicalServingError> {
    let source = candidate.source();
    let expected = candidate.source_revision_ref();
    let units = prepared.units();
    let unit = units
        .units()
        .iter()
        .find(|unit| unit.unit_id() == source.unit_id())
        .ok_or_else(unavailable)?;
    let binding = units.manifest().provenance().binding();
    if units.representation().representation_id != source.representation_id()
        || binding.source_namespace_id != expected.source_namespace_id
        || binding.source_id != expected.source_id
        || binding.source_revision_id != expected.revision_id
        || unit.unit_content_digest() != source.unit_digest()
        || units.manifest().profile_id() != source.profile_id()
    {
        return Err(unavailable());
    }
    let target = RetainedSourceTarget {
        source_namespace_id: expected.source_namespace_id,
        source_owner_generation: candidate.source_owner_generation(),
        source_revision_ref: expected,
        source_view: SourceView::RetainedRevision(expected.revision_id),
        workspace_view_revision_ref: None,
        native_anchor: unit.occurrence().native_anchor.clone(),
        excerpt_digest: source.excerpt_digest(),
        materialization_profile_id: search_contracts::ProfileId::new(
            prepared.product().profile_id().to_string(),
        )
        .map_err(|_| unavailable())?,
        assurance_ceiling: AssuranceClass::ExactBytes,
        object_residency_key_digest: candidate.object_residency_key_digest(),
    };
    let NativeAnchor::TextBytes(anchor) = &target.native_anchor else {
        return Err(unavailable());
    };
    // This producer serves only the qualified raw-text exact-byte baseline.
    // Other materializers require their own coordinate/assurance composition.
    if source.assurance() != search_candidate_validator::CandidateAssurance::Exact {
        return Err(unavailable());
    }
    if candidate.request_id() != request_id
        || (anchor.byte_start_0, anchor.byte_end_exclusive_0) != source.byte_range()
        || anchor.content_digest != expected.content_digest
    {
        return Err(unavailable());
    }
    Ok(target)
}

/// Mint for a C24-validated retained revision. The target and materializer
/// profile come from that same opaque owner preparation, never a caller target
/// or Qdrant payload. Current-worktree/overlay handles need their own view producer.
/// Final C18 output validation is still mandatory before returning the token.
#[allow(dead_code, clippy::too_many_arguments)] // Recipe factory wiring follows this adapter.
pub(super) fn mint_retained_candidate_handle(
    custody: &mut NativeProcessRootCustody,
    state: &mut RetainedRevisionReadState,
    registry: &search_source_registry::SourceRegistry,
    output: &mut CanonicalWorkOutput<'_, '_, '_>,
    admission: &StandalonePreRetrievalAdmission,
    pending_mints: &mut PendingHandleMints<'_>,
    candidate: &ValidatedSearchCandidate,
    prepared: &PreparedRawCodeArtifacts,
    clock: &mut SystemGrantClock,
    config: &EffectiveDaemonConfig,
) -> Result<SearchSourceHandle, CanonicalServingError> {
    let source = candidate.source();
    let expected = candidate.source_revision_ref();
    let target = retained_target(candidate, prepared, *output.guard().request_id())?;
    let namespace = registry
        .namespaces()
        .get(&expected.source_namespace_id)
        .ok_or_else(unavailable)?;
    if namespace.ownership.source_owner_generation != target.source_owner_generation {
        return Err(unavailable());
    }
    let section_name =
        search_config::ConfigSectionName::new("handles", 64).map_err(|_| unavailable())?;
    let handles = config
        .snapshot()
        .section(&section_name)
        .ok_or_else(unavailable)?
        .validated();
    let cancellation = output.guard().cancellation();
    let mut crypto = RootHandleCryptoPort::new(
        clock,
        handles,
        output.work_deadline_instant()?,
        &cancellation,
    )
    .map_err(|_| unavailable())?;
    let material = crypto.mint_material().map_err(|_| unavailable())?;
    let pins = custody.source_pins().clone();
    let protection = custody.with_serving_retained_metadata(
        state,
        registry,
        output,
        admission,
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
            let pin_target = SourcePinTarget::new(
                metadata.storage_object_id().clone(),
                expected,
                metadata.object_residency_key_digest(),
            );
            let pin = pins
                .pin(pin_target, material.monotonic_deadline)
                .map_err(|_| unavailable())?;
            Ok(SourceProtection {
                pin,
                target: target.clone(),
                expires_at: material.expires_at.clone(),
            })
        },
    )?;
    // Do not insert before the metadata owner's final readback/checkpoint. If it
    // fails, the owned guard drops here without leaving a stored handle behind.
    let mut protection = PreparedProtection(Some(protection));
    let mut crypto = PreparedCrypto {
        crypto: &mut crypto,
        material: Some(material),
    };
    let claims = &output.request().grant;
    pending_mints
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
            output.guard().is_cancelled(),
        )
        .map_err(|_| unavailable())
}
