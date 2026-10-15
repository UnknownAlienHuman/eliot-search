//! Permanent-v2 registry metadata codec, as a child of the closed v1-v3 codec.
//!
//! This module deliberately reuses the parent codec's canonical value helpers,
//! parser and old family encoders. It adds only the v4 dispatch and the
//! source/namespace inverse that must reconstitute original admission through
//! an owner supplied resolver. The resolver observes one coherent pre-read
//! metadata view; this module performs no storage or other I/O.

#[allow(clippy::wildcard_imports)]
use super::*;

use std::sync::Arc;

use crate::namespace::{
    CutoverPreparation, NamespaceCutoverState, NamespaceMetadata, NamespaceMetadataError,
    SourceOwnerActivationReceipt,
};
use crate::provenance::{PermanentSourceAdmissionV2, SourceProvenance};
use search_contracts::{
    InstallationIncarnationId, NamespaceOwnershipStatus, OwnerEpoch, RootBindingId,
    SourceNamespaceId, SourceNamespaceOwnership, SourceOwnerGeneration,
};
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_admission::{AdmissionError, AdmissionOutcome};
use search_source_provenance::{OriginalAdmissionV2, OriginalInputsLayoutV2, RetainedPolicyV2};

/// V4 state-value profile. Older state profiles remain owned by the parent.
pub const STATE_PROFILE_V4: &str = "eliot.registry-state.v4";
/// V4 operation profile. Older operation profiles remain byte-compatible.
pub const BATCH_PROFILE_V4: &str = "eliot.registry-batch.v4";
/// V2 key profile reserved for the new namespace family.
pub const STATE_KEY_PROFILE_V2: &str = "eliot.registry-state-key.v2";

/// Closed v4 codec failure that preserves the original owner error class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BatchCodecV4Error {
    /// The registry metadata or canonical envelope was invalid.
    Registry(RegistryError),
    /// The original policy/admission owner refused reconstruction.
    Admission(AdmissionError),
}

impl From<RegistryError> for BatchCodecV4Error {
    fn from(error: RegistryError) -> Self {
        Self::Registry(error)
    }
}

impl From<AdmissionError> for BatchCodecV4Error {
    fn from(error: AdmissionError) -> Self {
        Self::Admission(error)
    }
}

impl From<NamespaceMetadataError> for BatchCodecV4Error {
    fn from(error: NamespaceMetadataError) -> Self {
        match error {
            NamespaceMetadataError::Registry(error) => Self::Registry(error),
            NamespaceMetadataError::Admission(error) => Self::Admission(error),
        }
    }
}

/// Resolves retained original-input references from one coherent pre-read.
///
/// Implementations belong to the normal control/storage owner. They must use
/// metadata already read coherently by that owner and must not perform I/O in
/// these calls. The supplied probe is the original caller's checkpoint and
/// must be passed unchanged through every reconstruction step.
pub trait OriginalInputResolver {
    /// Rebuilds the shared retained policy from its exact reference.
    fn resolve_policy(
        &mut self,
        policy_reference: &[u8],
        probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Arc<RetainedPolicyV2>, BatchCodecV4Error>;

    /// Rebuilds one complete original admission under the resolved policy.
    fn resolve_admission(
        &mut self,
        admission_reference: &[u8],
        policy: Arc<RetainedPolicyV2>,
        probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<OriginalAdmissionV2, BatchCodecV4Error>;
}

/// Encodes one complete v4 state value and its explicit state profile.
pub fn encode_state_v4(v: &RegistryStateRecord) -> Result<Vec<u8>, RegistryError> {
    c::to_canonical_cbor(&object(vec![
        ("profile", text(STATE_PROFILE_V4)?),
        ("state", state_value_v4(v)?),
    ])?)
    .map(|value| value.as_slice().to_vec())
    .map_err(|_| bad())
}

/// Decodes one complete v4 state value through the actual original owners.
pub fn decode_state_v4<R: OriginalInputResolver + ?Sized>(
    bytes: &[u8],
    resolver: &mut R,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<RegistryStateRecord, BatchCodecV4Error> {
    checkpoint(probe)?;
    let mut outer = open(c::parse_canonical_cbor(bytes).map_err(|_| bad())?)?;
    if string(take(&mut outer, "profile")?)? != STATE_PROFILE_V4 {
        return Err(bad().into());
    }
    let state = read_state_v4(take(&mut outer, "state")?, resolver, probe)?;
    finish(outer)?;
    if encode_state_v4(&state).map_err(BatchCodecV4Error::Registry)? != bytes {
        return Err(bad().into());
    }
    checkpoint(probe)?;
    Ok(state)
}

/// Encodes a family-qualified key, retaining the old key profile for old families.
pub fn encode_key_v4(key: &RegistryStateKey) -> Result<Vec<u8>, RegistryError> {
    if !matches!(key, RegistryStateKey::Namespace(_)) {
        return super::encode_key(key);
    }
    c::to_canonical_cbor(&object(vec![
        ("profile", text(STATE_KEY_PROFILE_V2)?),
        ("key", key_value_v4(key)?),
    ])?)
    .map(|value| value.as_slice().to_vec())
    .map_err(|_| bad())
}

/// Decodes an old key or the v2 key reserved for a namespace.
pub fn decode_key_v4(bytes: &[u8]) -> Result<RegistryStateKey, RegistryError> {
    let mut outer = open(c::parse_canonical_cbor(bytes).map_err(|_| bad())?)?;
    let profile = string(take(&mut outer, "profile")?)?;
    let key = match profile.as_str() {
        "eliot.registry-state-key.v1" => super::read_key(take(&mut outer, "key")?)?,
        STATE_KEY_PROFILE_V2 => {
            let key = read_key_v4(take(&mut outer, "key")?)?;
            if !matches!(key, RegistryStateKey::Namespace(_)) {
                return Err(bad());
            }
            key
        }
        _ => return Err(bad()),
    };
    finish(outer)?;
    if encode_key_v4(&key)? != bytes {
        return Err(bad());
    }
    Ok(key)
}

/// Decodes a v4 operation/delta record and validates exact canonical re-encoding.
pub fn decode_batch_record_v4<R: OriginalInputResolver + ?Sized>(
    bytes: &[u8],
    limits: RegistryLimits,
    resolver: &mut R,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<DecodedBatch, BatchCodecV4Error> {
    limits.validate()?;
    checkpoint(probe)?;
    let mut outer = open(c::parse_canonical_cbor(bytes).map_err(|_| bad())?)?;
    if string(take(&mut outer, "profile")?)? != BATCH_PROFILE_V4 {
        return Err(bad().into());
    }
    let operation_id = opaque(take(&mut outer, "operation")?)?;
    let mutation_digest =
        Blake3Digest32::from_stored_bytes(fixed(take(&mut outer, "declared_digest")?)?);
    let request_fingerprint =
        Blake3Digest32::from_stored_bytes(fixed(take(&mut outer, "request_fingerprint")?)?);
    let before_revision = number(take(&mut outer, "before_revision")?)?;
    let after_revision = number(take(&mut outer, "after_revision")?)?;
    if before_revision.checked_add(1) != Some(after_revision) {
        return Err(bad().into());
    }
    let receipt = receipt(take(&mut outer, "receipt")?)?;
    let change_count =
        usize::try_from(number(take(&mut outer, "change_count")?)?).map_err(|_| bad())?;
    let rows = array(take(&mut outer, "deltas")?)?;
    if change_count == 0
        || change_count > limits.max_batch_changes
        || rows.is_empty()
        || rows.len() > limits.max_batch_changes
    {
        return Err(RegistryError::CapacityExceeded.into());
    }
    let mut deltas = Vec::with_capacity(rows.len());
    let mut previous = None;
    for value in rows {
        checkpoint(probe)?;
        let mut delta = open(value)?;
        let key = read_key_v4(take(&mut delta, "key")?)?;
        let before = optional_decode_v4(take(&mut delta, "before")?, |value| {
            read_state_v4(value, resolver, probe)
        })?;
        let after = read_state_v4(take(&mut delta, "after")?, resolver, probe)?;
        finish(delta)?;
        if previous.as_ref().is_some_and(|candidate| candidate >= &key)
            || after.key() != key
            || before.as_ref().is_some_and(|value| value.key() != key)
        {
            return Err(bad().into());
        }
        previous = Some(key.clone());
        deltas.push(RegistryStateDelta { key, before, after });
    }
    finish(outer)?;
    let result = DecodedBatch {
        encoding_profile: BATCH_PROFILE_V4,
        state_encoding_profile: STATE_PROFILE_V4,
        entry: RegistryJournalEntry::new(
            operation_id,
            mutation_digest,
            before_revision,
            after_revision,
            JournalEntryKind::BatchApply,
            receipt,
        ),
        request_fingerprint,
        change_count,
        deltas,
    };
    if encode_record_v4(&result).map_err(BatchCodecV4Error::Registry)? != bytes {
        return Err(bad().into());
    }
    checkpoint(probe)?;
    Ok(result)
}

fn encode_record_v4(value: &DecodedBatch) -> Result<Vec<u8>, RegistryError> {
    c::to_canonical_cbor(&object(vec![
        ("profile", text(BATCH_PROFILE_V4)?),
        ("operation", text(value.entry.operation_id.as_str())?),
        (
            "declared_digest",
            bytes(value.entry.mutation_digest.as_bytes())?,
        ),
        (
            "request_fingerprint",
            bytes(value.request_fingerprint.as_bytes())?,
        ),
        ("before_revision", V::U64(value.entry.before_revision)),
        ("after_revision", V::U64(value.entry.after_revision)),
        ("receipt", text(value.entry.receipt.as_str())?),
        ("change_count", V::U64(value.change_count as u64)),
        (
            "deltas",
            list(value.deltas.iter().map(|delta| {
                object(vec![
                    ("key", key_value_v4(&delta.key)?),
                    (
                        "before",
                        optional(delta.before.as_ref().map(state_value_v4))?,
                    ),
                    ("after", state_value_v4(&delta.after)?),
                ])
            }))?,
        ),
    ])?)
    .map(|value| value.as_slice().to_vec())
    .map_err(|_| bad())
}

/// Encodes one explicitly v4 operation record without reclassifying it as an
/// older profile.
pub fn encode_batch_record_v4(value: &DecodedBatch) -> Result<Vec<u8>, RegistryError> {
    if value.encoding_profile != BATCH_PROFILE_V4
        || value.state_encoding_profile != STATE_PROFILE_V4
    {
        return Err(bad());
    }
    encode_record_v4(value)
}

fn state_value_v4(value: &RegistryStateRecord) -> Result<V, RegistryError> {
    match value {
        RegistryStateRecord::Root(_)
        | RegistryStateRecord::Membership(_)
        | RegistryStateRecord::Generation { .. } => super::state_value(value),
        RegistryStateRecord::Source(source) => source_value_v4(source),
        RegistryStateRecord::Namespace(namespace) => namespace_value_v4(namespace),
    }
}

fn read_state_v4<R: OriginalInputResolver + ?Sized>(
    value: V,
    resolver: &mut R,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<RegistryStateRecord, BatchCodecV4Error> {
    let mut marker = open(value.clone())?;
    let family = string(take(&mut marker, "family")?)?;
    match family.as_str() {
        "source" => read_source_v4(value, resolver, probe),
        "namespace" => read_namespace_v4(value, resolver, probe),
        "root" | "membership" | "generation" => {
            super::read_state(value, true).map_err(BatchCodecV4Error::Registry)
        }
        _ => Err(bad().into()),
    }
}

fn source_value_v4(source: &RegisteredSource) -> Result<V, RegistryError> {
    match source.provenance() {
        SourceProvenance::HistoricalV1 { .. } => object(vec![
            ("family", text("source")?),
            ("provenance", text("historical-v1")?),
            (
                "legacy_body",
                super::state_value(&RegistryStateRecord::Source(source.clone()))?,
            ),
        ]),
        SourceProvenance::PermanentV2(_) => {
            let permanent = source
                .permanent_admission()
                .ok_or(RegistryError::SnapshotInvalid)?;
            object(vec![
                ("family", text("source")?),
                ("provenance", text("permanent-v2")?),
                ("binding", binding(source.binding())?),
                ("policy_reference", bytes(permanent.policy_reference())?),
                (
                    "admission_reference",
                    bytes(permanent.admission_reference())?,
                ),
                (
                    "root_record_revision",
                    V::U64(permanent.root_record_revision().get()),
                ),
                (
                    "namespace_record_revision",
                    V::U64(permanent.namespace_record_revision().get()),
                ),
                (
                    "owner_generation",
                    bytes(permanent.owner_generation().as_bytes())?,
                ),
                (
                    "lifecycle",
                    text(if source.lifecycle() == SourceLifecycle::Active {
                        "active"
                    } else {
                        "retired"
                    })?,
                ),
                ("source_revision", V::U64(source.source_revision().get())),
                ("registry_revision", V::U64(source.registry_revision())),
                ("receipt", text(source.last_receipt().as_str())?),
            ])
        }
    }
}

fn read_source_v4<R: OriginalInputResolver + ?Sized>(
    value: V,
    resolver: &mut R,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<RegistryStateRecord, BatchCodecV4Error> {
    let mut fields = open(value)?;
    if string(take(&mut fields, "family")?)? != "source" {
        return Err(bad().into());
    }
    let provenance = string(take(&mut fields, "provenance")?)?;
    match provenance.as_str() {
        "historical-v1" => {
            let body = take(&mut fields, "legacy_body")?;
            finish(fields)?;
            let state = super::read_state(body, true)?;
            if !matches!(state, RegistryStateRecord::Source(_)) {
                return Err(bad().into());
            }
            Ok(state)
        }
        "permanent-v2" => {
            let bound = read_binding(take(&mut fields, "binding")?)?;
            let policy_reference = binary(take(&mut fields, "policy_reference")?)?;
            let admission_reference = binary(take(&mut fields, "admission_reference")?)?;
            let root_record_revision = revision(take(&mut fields, "root_record_revision")?)?;
            let namespace_record_revision =
                revision(take(&mut fields, "namespace_record_revision")?)?;
            let owner_generation =
                SourceOwnerGeneration::from_bytes(fixed(take(&mut fields, "owner_generation")?)?);
            let lifecycle = match string(take(&mut fields, "lifecycle")?)?.as_str() {
                "active" => SourceLifecycle::Active,
                "retired" => SourceLifecycle::Retired,
                _ => return Err(bad().into()),
            };
            let source_revision = revision(take(&mut fields, "source_revision")?)?;
            let registry_revision = number(take(&mut fields, "registry_revision")?)?;
            let last_receipt = receipt(take(&mut fields, "receipt")?)?;
            finish(fields)?;

            checkpoint(probe)?;
            let policy = resolver.resolve_policy(&policy_reference, probe)?;
            let actual_policy_reference =
                OriginalInputsLayoutV2::policy_reference(policy.as_ref(), probe)?;
            if actual_policy_reference != policy_reference {
                return Err(AdmissionError::ReceiptMismatch.into());
            }
            let original =
                resolver.resolve_admission(&admission_reference, Arc::clone(&policy), probe)?;
            let actual_admission_reference =
                OriginalInputsLayoutV2::admission_reference(&original, probe)?;
            if actual_admission_reference != admission_reference {
                return Err(AdmissionError::ReceiptMismatch.into());
            }
            if original.receipt().decision().outcome() != AdmissionOutcome::Allow {
                return Err(AdmissionError::AdmissionDenied.into());
            }
            let permanent = PermanentSourceAdmissionV2::new(
                original,
                root_record_revision,
                namespace_record_revision,
                owner_generation,
                probe,
            )?;
            if permanent.policy_reference() != policy_reference
                || permanent.admission_reference() != admission_reference
            {
                return Err(AdmissionError::ReceiptMismatch.into());
            }
            Ok(RegistryStateRecord::Source(
                RegisteredSource::new_permanent(
                    bound,
                    permanent,
                    lifecycle,
                    source_revision,
                    registry_revision,
                    last_receipt,
                )?,
            ))
        }
        _ => Err(bad().into()),
    }
}

fn namespace_value_v4(state: &NamespaceCutoverState) -> Result<V, RegistryError> {
    object(vec![
        ("family", text("namespace")?),
        ("ownership", ownership_value(&state.ownership)?),
        (
            "preparation",
            optional(state.preparation.as_ref().map(preparation_value))?,
        ),
        ("fenced", V::Bool(state.fenced)),
        (
            "activation",
            optional(state.activation.as_ref().map(activation_value))?,
        ),
        ("metadata", namespace_metadata_value(state.metadata())?),
    ])
}

fn namespace_metadata_value(metadata: &NamespaceMetadata) -> Result<V, RegistryError> {
    match metadata {
        NamespaceMetadata::HistoricalV1Unbound => {
            object(vec![("kind", text("historical-v1-unbound")?)])
        }
        NamespaceMetadata::PermanentV2(metadata) => object(vec![
            ("kind", text("permanent-v2")?),
            (
                "root_binding_id",
                bytes(metadata.root_binding_id().as_bytes())?,
            ),
            ("policy_reference", bytes(metadata.policy_reference())?),
            ("registry_revision", V::U64(metadata.registry_revision())),
            ("last_receipt", text(metadata.last_receipt().as_str())?),
        ]),
    }
}

fn read_namespace_v4<R: OriginalInputResolver + ?Sized>(
    value: V,
    resolver: &mut R,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<RegistryStateRecord, BatchCodecV4Error> {
    let mut fields = open(value)?;
    if string(take(&mut fields, "family")?)? != "namespace" {
        return Err(bad().into());
    }
    let ownership = read_ownership(take(&mut fields, "ownership")?)?;
    let preparation = optional_decode(take(&mut fields, "preparation")?, read_preparation)?;
    let fenced = boolean(take(&mut fields, "fenced")?)?;
    let activation = optional_decode(take(&mut fields, "activation")?, read_activation)?;
    let mut metadata = open(take(&mut fields, "metadata")?)?;
    let kind = string(take(&mut metadata, "kind")?)?;
    let state = match kind.as_str() {
        "historical-v1-unbound" => {
            finish(metadata)?;
            let mut state = NamespaceCutoverState::new(ownership);
            state.preparation = preparation;
            state.fenced = fenced;
            state.activation = activation;
            state
        }
        "permanent-v2" => {
            if preparation.is_some() || fenced || activation.is_some() {
                return Err(RegistryError::CutoverRequired.into());
            }
            let root_binding_id =
                RootBindingId::from_bytes(fixed(take(&mut metadata, "root_binding_id")?)?);
            let policy_reference = binary(take(&mut metadata, "policy_reference")?)?;
            let registry_revision = number(take(&mut metadata, "registry_revision")?)?;
            let last_receipt = receipt(take(&mut metadata, "last_receipt")?)?;
            finish(metadata)?;
            checkpoint(probe)?;
            let policy = resolver.resolve_policy(&policy_reference, probe)?;
            NamespaceCutoverState::new_v2_metadata(
                ownership,
                root_binding_id,
                policy,
                policy_reference,
                registry_revision,
                last_receipt,
                probe,
            )?
        }
        _ => return Err(bad().into()),
    };
    finish(fields)?;
    Ok(RegistryStateRecord::Namespace(state))
}

fn ownership_value(ownership: &SourceNamespaceOwnership) -> Result<V, RegistryError> {
    object(vec![
        (
            "source_namespace_id",
            bytes(ownership.source_namespace_id.as_bytes())?,
        ),
        ("owner_system_id", text(ownership.owner_system_id.as_str())?),
        (
            "owner_installation_incarnation_id",
            bytes(ownership.owner_installation_incarnation_id.as_bytes())?,
        ),
        ("owner_epoch", V::U64(ownership.owner_epoch.get())),
        (
            "ownership_record_revision",
            V::U64(ownership.ownership_record_revision.get()),
        ),
        (
            "source_owner_generation",
            bytes(ownership.source_owner_generation.as_bytes())?,
        ),
        (
            "source_admission_policy_revision",
            V::U64(ownership.source_admission_policy_revision.get()),
        ),
        (
            "status",
            text(match ownership.status {
                NamespaceOwnershipStatus::Active => "ACTIVE",
                NamespaceOwnershipStatus::CutoverPrepared => "CUTOVER_PREPARED",
                NamespaceOwnershipStatus::Fenced => "FENCED",
                NamespaceOwnershipStatus::Retired => "RETIRED",
            })?,
        ),
        (
            "cutover_receipt_ref",
            optional(
                ownership
                    .cutover_receipt_ref
                    .as_ref()
                    .map(|value| text(value.as_str())),
            )?,
        ),
    ])
}

fn read_ownership(value: V) -> Result<SourceNamespaceOwnership, RegistryError> {
    let mut fields = open(value)?;
    let source_namespace_id =
        SourceNamespaceId::from_bytes(fixed(take(&mut fields, "source_namespace_id")?)?);
    let owner_system_id = opaque(take(&mut fields, "owner_system_id")?)?;
    let owner_installation_incarnation_id = InstallationIncarnationId::from_bytes(fixed(take(
        &mut fields,
        "owner_installation_incarnation_id",
    )?)?);
    let owner_epoch =
        OwnerEpoch::new(number(take(&mut fields, "owner_epoch")?)?).map_err(|_| bad())?;
    let ownership_record_revision = revision(take(&mut fields, "ownership_record_revision")?)?;
    let source_owner_generation =
        SourceOwnerGeneration::from_bytes(fixed(take(&mut fields, "source_owner_generation")?)?);
    let source_admission_policy_revision = search_contracts::PolicyRevision::new(number(take(
        &mut fields,
        "source_admission_policy_revision",
    )?)?);
    let status = match string(take(&mut fields, "status")?)?.as_str() {
        "ACTIVE" => NamespaceOwnershipStatus::Active,
        "CUTOVER_PREPARED" => NamespaceOwnershipStatus::CutoverPrepared,
        "FENCED" => NamespaceOwnershipStatus::Fenced,
        "RETIRED" => NamespaceOwnershipStatus::Retired,
        _ => return Err(bad()),
    };
    let cutover_receipt_ref = optional_decode(take(&mut fields, "cutover_receipt_ref")?, receipt)?;
    finish(fields)?;
    Ok(SourceNamespaceOwnership {
        source_namespace_id,
        owner_system_id,
        owner_installation_incarnation_id,
        owner_epoch,
        ownership_record_revision,
        source_owner_generation,
        source_admission_policy_revision,
        status,
        cutover_receipt_ref,
    })
}

fn preparation_value(value: &CutoverPreparation) -> Result<V, RegistryError> {
    object(vec![
        ("cutover_id", bytes(value.cutover_id.as_bytes())?),
        ("namespace_id", bytes(value.namespace_id.as_bytes())?),
        (
            "current_generation",
            bytes(value.current_generation.as_bytes())?,
        ),
        ("proposed_owner", text(value.proposed_owner.as_str())?),
        (
            "proposed_incarnation",
            bytes(value.proposed_incarnation.as_bytes())?,
        ),
        (
            "affected_sources",
            list(value.affected_sources.iter().map(identity))?,
        ),
        (
            "affected_memberships",
            list(
                value
                    .affected_memberships
                    .iter()
                    .map(|membership| bytes(membership.as_bytes())),
            )?,
        ),
        ("evidence_digest", bytes(value.evidence_digest.as_bytes())?),
        ("registry_revision", V::U64(value.registry_revision)),
        ("operation_id", text(value.operation_id.as_str())?),
        ("receipt", text(value.receipt.as_str())?),
        ("replayed", V::Bool(value.replayed)),
    ])
}

fn read_preparation(value: V) -> Result<CutoverPreparation, RegistryError> {
    let mut fields = open(value)?;
    let cutover_id =
        search_contracts::CutoverId::from_bytes(fixed(take(&mut fields, "cutover_id")?)?);
    let namespace_id = SourceNamespaceId::from_bytes(fixed(take(&mut fields, "namespace_id")?)?);
    let current_generation =
        SourceOwnerGeneration::from_bytes(fixed(take(&mut fields, "current_generation")?)?);
    let proposed_owner = opaque(take(&mut fields, "proposed_owner")?)?;
    let proposed_incarnation =
        InstallationIncarnationId::from_bytes(fixed(take(&mut fields, "proposed_incarnation")?)?);
    let affected_sources = array(take(&mut fields, "affected_sources")?)?
        .into_iter()
        .map(read_identity)
        .collect::<Result<Vec<_>, _>>()?;
    let affected_memberships = array(take(&mut fields, "affected_memberships")?)?
        .into_iter()
        .map(|value| fixed(value).map(search_contracts::SourceMembershipId::from_bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let evidence_digest =
        Blake3Digest32::from_stored_bytes(fixed(take(&mut fields, "evidence_digest")?)?);
    let registry_revision = number(take(&mut fields, "registry_revision")?)?;
    let operation_id = opaque(take(&mut fields, "operation_id")?)?;
    let receipt = receipt(take(&mut fields, "receipt")?)?;
    let replayed = boolean(take(&mut fields, "replayed")?)?;
    finish(fields)?;
    Ok(CutoverPreparation {
        cutover_id,
        namespace_id,
        current_generation,
        proposed_owner,
        proposed_incarnation,
        affected_sources,
        affected_memberships,
        evidence_digest,
        registry_revision,
        operation_id,
        receipt,
        replayed,
    })
}

fn activation_value(value: &SourceOwnerActivationReceipt) -> Result<V, RegistryError> {
    object(vec![
        ("cutover_id", bytes(value.cutover_id.as_bytes())?),
        ("namespace_id", bytes(value.namespace_id.as_bytes())?),
        ("new_generation", bytes(value.new_generation.as_bytes())?),
        (
            "activation_revision",
            V::U64(value.activation_revision.get()),
        ),
        ("registry_revision", V::U64(value.registry_revision)),
        ("operation_id", text(value.operation_id.as_str())?),
        ("receipt", text(value.receipt.as_str())?),
        ("replayed", V::Bool(value.replayed)),
    ])
}

fn read_activation(value: V) -> Result<SourceOwnerActivationReceipt, RegistryError> {
    let mut fields = open(value)?;
    let cutover_id =
        search_contracts::CutoverId::from_bytes(fixed(take(&mut fields, "cutover_id")?)?);
    let namespace_id = SourceNamespaceId::from_bytes(fixed(take(&mut fields, "namespace_id")?)?);
    let new_generation =
        SourceOwnerGeneration::from_bytes(fixed(take(&mut fields, "new_generation")?)?);
    let activation_revision = revision(take(&mut fields, "activation_revision")?)?;
    let registry_revision = number(take(&mut fields, "registry_revision")?)?;
    let operation_id = opaque(take(&mut fields, "operation_id")?)?;
    let receipt = receipt(take(&mut fields, "receipt")?)?;
    let replayed = boolean(take(&mut fields, "replayed")?)?;
    finish(fields)?;
    Ok(SourceOwnerActivationReceipt {
        cutover_id,
        namespace_id,
        new_generation,
        activation_revision,
        registry_revision,
        operation_id,
        receipt,
        replayed,
    })
}

fn key_value_v4(key: &RegistryStateKey) -> Result<V, RegistryError> {
    match key {
        RegistryStateKey::Namespace(namespace_id) => object(vec![
            ("family", text("namespace")?),
            ("namespace", bytes(namespace_id.as_bytes())?),
        ]),
        _ => super::key_value(key),
    }
}

fn read_key_v4(value: V) -> Result<RegistryStateKey, RegistryError> {
    let mut fields = open(value)?;
    let family = string(take(&mut fields, "family")?)?;
    let key = match family.as_str() {
        "root" => RegistryStateKey::Root(RootBindingId::from_bytes(fixed(take(
            &mut fields,
            "root",
        )?)?)),
        "source" => RegistryStateKey::Source(read_identity(take(&mut fields, "identity")?)?),
        "membership" => RegistryStateKey::Membership(read_member_key(take(&mut fields, "key")?)?),
        "generation" => RegistryStateKey::Generation(opaque(take(&mut fields, "corpus")?)?),
        "namespace" => RegistryStateKey::Namespace(SourceNamespaceId::from_bytes(fixed(take(
            &mut fields,
            "namespace",
        )?)?)),
        _ => return Err(bad()),
    };
    finish(fields)?;
    Ok(key)
}

fn checkpoint(
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<(), BatchCodecV4Error> {
    probe().map_err(|failure| {
        BatchCodecV4Error::Admission(match failure {
            CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
            CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
        })
    })
}

fn optional_decode_v4<T>(
    value: V,
    decode: impl FnOnce(V) -> Result<T, BatchCodecV4Error>,
) -> Result<Option<T>, BatchCodecV4Error> {
    if value == V::Null {
        Ok(None)
    } else {
        decode(value).map(Some)
    }
}
