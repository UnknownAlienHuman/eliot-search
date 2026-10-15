//! Immutable Source admission request metadata. It allocates no identity or authority.

use crate::batch::{DecodedBatch, RegistryStateRecord};
use crate::error::RegistryError;
use crate::identity_lookup_v2::IdentityLookupV2Error;
use crate::source::AdmittedSourceReceipt;
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText,
    CanonicalValue as V, ContractError, DigestInputLimit, InstallationIncarnationId,
    NonZeroRevision, OpaqueCanonicalBytes, OpaqueId, OwnerEpoch, ReceiptRef, RootBindingId,
    SourceNamespaceId, SourceOwnerGeneration, blake3_canonical,
};
use search_source_admission::{AdmissionError, AdmissionOutcome, policy_v2::CheckpointFailureV2};
use search_source_identity::{
    FilesystemIdentityProfile, IdentityError, SourceObservation, ValidatedIdentityObservation,
};
use search_source_provenance::{OriginalAdmissionV2, OriginalInputsLayoutV2};
use std::sync::Arc;

/// Source-namespace owner scope supplied by the qualified normal adapter.
/// Current Native custody is proved separately, not by historical Root metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceOwnerScopeV2 {
    /// Stable provider system identity.
    pub system_id: OpaqueId,
    /// Actual installation incarnation, never inferred from a root or locator.
    pub installation_incarnation: InstallationIncarnationId,
    /// Admitted namespace ownership epoch, independent of Root registration history.
    pub epoch: OwnerEpoch,
}

/// Explicit namespace choice; fresh IDs come only from the allocation port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceNamespaceSelectionV2 {
    /// First source admission creates a fresh namespace in the same transaction.
    CreateNew,
    /// Use this exact previously qualified namespace and expected owner fence.
    Existing {
        /// Exact namespace identity.
        id: SourceNamespaceId,
        /// Exact namespace ownership record revision.
        revision: NonZeroRevision,
        /// Exact owner generation.
        generation: SourceOwnerGeneration,
    },
}

/// Complete immutable caller input, without an assigned new Source ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAdmissionRequestV2 {
    /// Semantic registry revision observed by the caller.
    pub expected_registry_revision: u64,
    /// Actual admitted root.
    pub root_binding_id: RootBindingId,
    /// Exact root-record predecessor fence.
    pub expected_root_revision: NonZeroRevision,
    /// Explicit namespace selection.
    pub namespace: SourceNamespaceSelectionV2,
    /// Actual owner observations; these are metadata, not a native grant.
    pub owner: SourceOwnerScopeV2,
    /// Exact C04 profile used to capture the identity observation.
    pub identity_profile: FilesystemIdentityProfile,
    /// Actual owner-validated identity observation.
    pub identity_observation: ValidatedIdentityObservation,
    /// Source binding facts captured without executing source code.
    pub binding_observation: SourceObservation,
    /// Complete original admission from the sole v2 owners.
    pub original: OriginalAdmissionV2,
    /// Required Source predecessor when an existing physical identity matches.
    pub expected_source_revision: Option<NonZeroRevision>,
    /// Stable immutable operation identity.
    pub operation_id: OpaqueId,
    /// Content-free original operation receipt reference.
    pub receipt: ReceiptRef,
}

impl SourceAdmissionRequestV2 {
    /// Fingerprints the complete actual request, excluding fresh output IDs.
    ///
    /// Stable components must come from the sole C04 resolver's draft. No caller
    /// fingerprint is accepted. A replay compares this before allocating IDs.
    pub fn fingerprint(
        &self,
        stable_components: &OpaqueCanonicalBytes,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Blake3Digest32, AdmissionError> {
        let mut check = || {
            probe().map_err(|failure| match failure {
                CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
                CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
            })
        };
        check()?;
        let policy_reference =
            OriginalInputsLayoutV2::policy_reference(self.original.policy(), &mut || {
                check().map_err(|error| {
                    if error == AdmissionError::Cancelled {
                        CheckpointFailureV2::Cancelled
                    } else {
                        CheckpointFailureV2::DeadlineExceeded
                    }
                })
            })?;
        let observed = self.identity_observation.as_inner();
        let profile = self.identity_profile;
        let source = &self.binding_observation;
        let namespace = match self.namespace {
            SourceNamespaceSelectionV2::CreateNew => list(vec![text("create")?])?,
            SourceNamespaceSelectionV2::Existing {
                id,
                revision,
                generation,
            } => list(vec![
                text("existing")?,
                bytes(id.as_bytes())?,
                V::U64(revision.get()),
                bytes(generation.as_bytes())?,
            ])?,
        };
        // The numeric unit-enum values are part of this closed request@2 profile.
        let profile = list(vec![
            V::U64(profile.revision().get()),
            bytes(profile.schema_digest().as_bytes())?,
            V::U64(profile.case_behavior() as u64),
            V::U64(profile.unicode_behavior() as u64),
            V::U64(profile.volume_identity() as u64),
            V::U64(profile.file_identity() as u64),
            V::U64(profile.link_behavior() as u64),
            V::U64(profile.reparse_behavior() as u64),
        ])?;
        let value = list(vec![
            text("eliot.source-admission-request@2")?,
            V::U64(self.expected_registry_revision),
            bytes(self.root_binding_id.as_bytes())?,
            V::U64(self.expected_root_revision.get()),
            namespace,
            text(self.owner.system_id.as_str())?,
            bytes(self.owner.installation_incarnation.as_bytes())?,
            V::U64(self.owner.epoch.get()),
            profile,
            bytes(stable_components.as_slice())?,
            bytes(observed.path_key.root_binding_id().as_bytes())?,
            V::U64(observed.path_key.profile_revision().get()),
            bytes(observed.path_key.profile_schema_digest().as_bytes())?,
            text(observed.path_key.as_str())?,
            observed
                .content_digest_hint
                .map(|v| bytes(v.as_bytes()))
                .transpose()?
                .unwrap_or(V::Null),
            observed.metadata_generation.map_or(V::Null, V::U64),
            observed.byte_length_hint.map_or(V::Null, V::U64),
            observed
                .claimed_source_id
                .map(|id| bytes(id.as_bytes()))
                .transpose()?
                .unwrap_or(V::Null),
            V::U64(observed.candidate_catalog_revision.get()),
            V::U64(observed.profile_revision.get()),
            bytes(observed.profile_schema_digest.as_bytes())?,
            V::U64(observed.confidence as u64),
            bytes(observed.evidence_digest.as_bytes())?,
            bytes(source.root_binding_id.as_bytes())?,
            text(source.relative_path.as_str())?,
            source
                .stable_file_identity_digest
                .map(|v| bytes(v.as_bytes()))
                .transpose()?
                .unwrap_or(V::Null),
            bytes(source.content_digest.as_bytes())?,
            V::U64(source.content_bytes),
            text(source.observation_receipt.as_str())?,
            bytes(&policy_reference)?,
            bytes(self.original.encoded_source_evidence())?,
            self.expected_source_revision
                .map_or(V::Null, |v| V::U64(v.get())),
            text(self.operation_id.as_str())?,
            text(self.receipt.as_str())?,
        ])?;
        check()?;
        let digest = blake3_canonical(
            &CanonicalDigestDomain::parse("eliot/cbor/source-admission-request/v2")
                .map_err(error)?,
            &value,
            DigestInputLimit::new(search_contracts::MAX_CANONICAL_BYTES).map_err(error)?,
        )
        .map_err(error)?;
        check()?;
        Ok(digest)
    }
}

fn error(_: ContractError) -> AdmissionError {
    AdmissionError::BudgetExhausted
}
fn text(value: &str) -> Result<V, AdmissionError> {
    CanonicalText::new(value).map(V::Text).map_err(error)
}
fn bytes(value: &[u8]) -> Result<V, AdmissionError> {
    BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(error)
}
fn list(value: Vec<V>) -> Result<V, AdmissionError> {
    BoundedList::new(value).map(V::Array).map_err(error)
}

/// Exact refusal before mutation, or unresolved external outcome.
#[derive(Debug)]
pub enum SourceAdmissionV2Error<E> {
    /// Semantic registry refusal.
    Registry(RegistryError),
    /// Original admission owner refusal.
    Admission(AdmissionError),
    /// Identity owner refusal.
    Identity(IdentityError),
    /// Technical identity cache refusal.
    Lookup(IdentityLookupV2Error),
    /// Original allocation-port failure.
    Allocation(E),
    /// Original control-port failure; no replacement context is synthesized.
    Control(E),
    /// Exact non-Allow decision; no IDs or state are created.
    Decision(AdmissionOutcome),
}
impl<E> From<crate::batch::BatchCodecV4Error> for SourceAdmissionV2Error<E> {
    fn from(v: crate::batch::BatchCodecV4Error) -> Self {
        match v {
            crate::batch::BatchCodecV4Error::Registry(v) => Self::Registry(v),
            crate::batch::BatchCodecV4Error::Admission(v) => Self::Admission(v),
        }
    }
}
impl<E> From<RegistryError> for SourceAdmissionV2Error<E> {
    fn from(v: RegistryError) -> Self {
        Self::Registry(v)
    }
}
impl<E> From<AdmissionError> for SourceAdmissionV2Error<E> {
    fn from(v: AdmissionError) -> Self {
        Self::Admission(v)
    }
}
impl<E> From<IdentityError> for SourceAdmissionV2Error<E> {
    fn from(v: IdentityError) -> Self {
        Self::Identity(v)
    }
}
impl<E> From<IdentityLookupV2Error> for SourceAdmissionV2Error<E> {
    fn from(v: IdentityLookupV2Error) -> Self {
        Self::Lookup(v)
    }
}

/// Prepared immutable original models; this resolver performs no I/O or current admission.
pub struct MemoryOriginals {
    values: Vec<search_source_provenance::OriginalAdmissionV2>,
}
impl MemoryOriginals {
    /// Retain complete models already produced by their sole original-input owners.
    pub fn new(values: Vec<search_source_provenance::OriginalAdmissionV2>) -> Self {
        Self { values }
    }
}

impl crate::batch::OriginalInputResolver for MemoryOriginals {
    fn resolve_policy(
        &mut self,
        reference: &[u8],
        probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Arc<search_source_provenance::RetainedPolicyV2>, crate::batch::BatchCodecV4Error>
    {
        for value in &self.values {
            if search_source_provenance::OriginalInputsLayoutV2::policy_reference(
                value.policy(),
                probe,
            )? == reference
            {
                return Ok(Arc::clone(value.policy()));
            }
        }
        Err(AdmissionError::ReceiptMismatch.into())
    }
    fn resolve_admission(
        &mut self,
        reference: &[u8],
        policy: Arc<search_source_provenance::RetainedPolicyV2>,
        probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<search_source_provenance::OriginalAdmissionV2, crate::batch::BatchCodecV4Error>
    {
        for value in &self.values {
            if search_source_provenance::OriginalInputsLayoutV2::admission_reference(value, probe)?
                == reference
            {
                let mut original_probe = || probe();
                return search_source_provenance::OriginalAdmissionV2::restore(
                    policy,
                    value.encoded_source_evidence(),
                    &mut original_probe,
                )
                .map_err(Into::into);
            }
        }
        Err(AdmissionError::ReceiptMismatch.into())
    }
}

/// Projects the exact Source result from immutable operation metadata.
pub fn source_receipt(
    record: &DecodedBatch,
    replayed: bool,
) -> Result<AdmittedSourceReceipt, RegistryError> {
    let mut sources = record.deltas().iter().filter_map(|delta| {
        if let RegistryStateRecord::Source(source) = delta.after() {
            Some(source)
        } else {
            None
        }
    });
    let source = sources.next().ok_or(RegistryError::SnapshotInvalid)?;
    if sources.next().is_some() || source.permanent_admission().is_none() {
        return Err(RegistryError::SnapshotInvalid);
    }
    Ok(AdmittedSourceReceipt {
        source_identity: source.identity().clone(),
        source_revision: source.source_revision(),
        registry_revision: record.registry_revision_after(),
        operation_id: record.entry().operation_id.clone(),
        receipt: record.entry().receipt.clone(),
        replayed,
    })
}

/// Exact recovery classification; retry retains the same prepared IDs and bytes.
pub enum SourceAdmissionRecoveryV2 {
    /// Exact committed state and original receipt were read back.
    Committed(AdmittedSourceReceipt),
    /// Atomic absence was proven; only the identical prepared transaction may retry.
    RetrySamePrepared,
}

/// Propagates the original caller checkpoint without renewing its allowance.
pub fn source_admission_checkpoint<E>(
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<(), SourceAdmissionV2Error<E>> {
    probe().map_err(|v| {
        SourceAdmissionV2Error::Admission(match v {
            CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
            CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
        })
    })
}

