//! Closed first-bind profile. All earlier profiles keep their exact encoders.

#[allow(clippy::wildcard_imports)]
use super::*;
use crate::projection_membership::{ProjectionMembershipBindRequest, ProjectionMembershipRecord};
use search_contracts::{
    AccessPartitionId, MembershipRevision, ProfileId, ProjectionMembership, ProjectionMembershipId,
    RepresentationId, ScoringPartitionId, SourceMembershipId,
};

pub const STATE_PROFILE_V6: &str = "eliot.registry-state.v6";
pub const BATCH_PROFILE_V6: &str = "eliot.registry-batch.v6";
const KEY_PROFILE_V3: &str = "eliot.registry-state-key.v3";

fn key_value_v6(key: &RegistryStateKey) -> Result<V, RegistryError> {
    let RegistryStateKey::ProjectionMembership(id) = key else {
        return Err(bad());
    };
    object(vec![
        ("family", text("projection-membership")?),
        ("source_membership_id", bytes(id.as_bytes())?),
    ])
}

pub(in crate::batch) fn encode_key_v6(key: &RegistryStateKey) -> Result<Vec<u8>, RegistryError> {
    c::to_canonical_cbor(&object(vec![
        ("profile", text(KEY_PROFILE_V3)?),
        ("key", key_value_v6(key)?),
    ])?)
    .map(|b| b.as_slice().to_vec())
    .map_err(|_| bad())
}

pub(in crate::batch) fn decode_key_v6(encoded: &[u8]) -> Result<RegistryStateKey, RegistryError> {
    let mut outer = open(c::parse_canonical_cbor(encoded).map_err(|_| bad())?)?;
    if string(take(&mut outer, "profile")?)? != KEY_PROFILE_V3 {
        return super::decode_key(encoded);
    }
    let mut key = open(take(&mut outer, "key")?)?;
    if string(take(&mut key, "family")?)? != "projection-membership" {
        return Err(bad());
    }
    let result = RegistryStateKey::ProjectionMembership(SourceMembershipId::from_bytes(fixed(
        take(&mut key, "source_membership_id")?,
    )?));
    finish(key)?;
    finish(outer)?;
    if encode_key_v6(&result)? != encoded {
        return Err(bad());
    }
    Ok(result)
}

fn request_value(request: &ProjectionMembershipBindRequest) -> Result<V, RegistryError> {
    request.validate_shape()?;
    let b = &request.binding;
    object(vec![
        (
            "projection_membership_id",
            bytes(b.projection_membership_id.as_bytes())?,
        ),
        (
            "source_membership_id",
            bytes(b.source_membership_id.as_bytes())?,
        ),
        ("representation_id", bytes(b.representation_id.as_bytes())?),
        (
            "access_partition_id",
            bytes(b.access_partition_id.as_bytes())?,
        ),
        (
            "scoring_partition_id",
            bytes(b.scoring_partition_id.as_bytes())?,
        ),
        (
            "projection_schema_id",
            text(b.projection_schema_id.as_str())?,
        ),
        (
            "membership_revision",
            V::U64(request.expected_membership_revision.get()),
        ),
        (
            "provenance",
            super::v5::provenance_value(&request.expected_provenance)?,
        ),
    ])
}

fn read_request(value: V) -> Result<ProjectionMembershipBindRequest, RegistryError> {
    let mut fields = open(value)?;
    let request = ProjectionMembershipBindRequest {
        binding: ProjectionMembership {
            projection_membership_id: ProjectionMembershipId::from_bytes(fixed(take(
                &mut fields,
                "projection_membership_id",
            )?)?),
            source_membership_id: SourceMembershipId::from_bytes(fixed(take(
                &mut fields,
                "source_membership_id",
            )?)?),
            representation_id: RepresentationId::from_bytes(fixed(take(
                &mut fields,
                "representation_id",
            )?)?),
            access_partition_id: AccessPartitionId::from_bytes(fixed(take(
                &mut fields,
                "access_partition_id",
            )?)?),
            scoring_partition_id: ScoringPartitionId::from_bytes(fixed(take(
                &mut fields,
                "scoring_partition_id",
            )?)?),
            projection_schema_id: ProfileId::new(string(take(
                &mut fields,
                "projection_schema_id",
            )?)?)
            .map_err(|_| bad())?,
        },
        expected_membership_revision: MembershipRevision::new(number(take(
            &mut fields,
            "membership_revision",
        )?)?),
        expected_provenance: super::v5::read_provenance(take(&mut fields, "provenance")?)?,
    };
    finish(fields)?;
    request.validate_shape()?;
    Ok(request)
}

fn state_value_v6(value: &RegistryStateRecord) -> Result<V, RegistryError> {
    let RegistryStateRecord::ProjectionMembership(record) = value else {
        return Err(bad());
    };
    object(vec![
        ("family", text("projection-membership")?),
        ("request", request_value(record.request())?),
        ("registry_revision", V::U64(record.registry_revision())),
        ("receipt", text(record.receipt().as_str())?),
    ])
}

fn read_state_v6(value: V) -> Result<RegistryStateRecord, RegistryError> {
    let mut fields = open(value)?;
    if string(take(&mut fields, "family")?)? != "projection-membership" {
        return Err(bad());
    }
    let request = read_request(take(&mut fields, "request")?)?;
    let revision = number(take(&mut fields, "registry_revision")?)?;
    let receipt = receipt(take(&mut fields, "receipt")?)?;
    finish(fields)?;
    ProjectionMembershipRecord::from_observed_parts(request, revision, receipt)
        .map(RegistryStateRecord::ProjectionMembership)
}

pub(in crate::batch) fn encode_state_v6(
    value: &RegistryStateRecord,
) -> Result<Vec<u8>, RegistryError> {
    c::to_canonical_cbor(&object(vec![
        ("profile", text(STATE_PROFILE_V6)?),
        ("state", state_value_v6(value)?),
    ])?)
    .map(|b| b.as_slice().to_vec())
    .map_err(|_| bad())
}

pub(super) fn decode_state_v6(encoded: &[u8]) -> Result<RegistryStateRecord, RegistryError> {
    let mut outer = open(c::parse_canonical_cbor(encoded).map_err(|_| bad())?)?;
    if string(take(&mut outer, "profile")?)? != STATE_PROFILE_V6 {
        return Err(bad());
    }
    let state = read_state_v6(take(&mut outer, "state")?)?;
    finish(outer)?;
    if encode_state_v6(&state)? != encoded {
        return Err(bad());
    }
    Ok(state)
}

fn change_value_v6(value: &RegistryChange) -> Result<V, RegistryError> {
    let RegistryChange::BindProjectionMembership { request, receipt } = value else {
        return Err(bad());
    };
    object(vec![
        ("kind", text("bind-projection-membership")?),
        ("request", request_value(request)?),
        ("receipt", text(receipt.as_str())?),
    ])
}

pub(super) fn request_fingerprint_v6(
    batch: &RegistryBatch,
) -> Result<Blake3Digest32, RegistryError> {
    let value = object(vec![
        ("profile", text("registry-projection-bind-input-v1")?),
        (
            "expected_revision",
            V::U64(batch.expected_registry_revision),
        ),
        ("operation", text(batch.operation.operation_id().as_str())?),
        (
            "declared_digest",
            bytes(batch.operation.mutation_digest().as_bytes())?,
        ),
        ("changes", list(batch.changes.iter().map(change_value_v6))?),
    ])?;
    c::blake3_canonical(
        &c::CanonicalDigestDomain::parse("eliot/cbor/registry-projection-bind-input/v1")
            .map_err(|_| bad())?,
        &value,
        c::DigestInputLimit::new(c::MAX_CANONICAL_BYTES).map_err(|_| bad())?,
    )
    .map_err(|_| bad())
}

fn validate_batch(value: &DecodedBatch) -> Result<(), RegistryError> {
    let [RegistryChange::BindProjectionMembership { request, receipt }] =
        value.original_changes.as_deref().ok_or_else(bad)?
    else {
        return Err(bad());
    };
    let [delta] = value.deltas.as_slice() else {
        return Err(bad());
    };
    let RegistryStateRecord::ProjectionMembership(record) = delta.after() else {
        return Err(bad());
    };
    if value.encoding_profile != BATCH_PROFILE_V6
        || value.state_encoding_profile != STATE_PROFILE_V6
        || value.change_count != 1
        || value.entry.kind != JournalEntryKind::BatchApply
        || value.entry.before_revision.checked_add(1) != Some(value.entry.after_revision)
        || delta.before().is_some()
        || delta.key()
            != &RegistryStateKey::ProjectionMembership(record.binding().source_membership_id)
        || record.request() != request
        || record.receipt() != receipt
        || receipt != &value.entry.receipt
        || record.registry_revision() != value.entry.after_revision
    {
        return Err(bad());
    }
    Ok(())
}

pub(in crate::batch) fn encode_batch_record_v6(
    value: &DecodedBatch,
) -> Result<Vec<u8>, RegistryError> {
    validate_batch(value)?;
    let delta = &value.deltas[0];
    let changes = value.original_changes.as_ref().ok_or_else(bad)?;
    c::to_canonical_cbor(&object(vec![
        ("profile", text(BATCH_PROFILE_V6)?),
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
        ("input", change_value_v6(&changes[0])?),
        ("after", state_value_v6(delta.after())?),
    ])?)
    .map(|b| b.as_slice().to_vec())
    .map_err(|_| bad())
}

pub(super) fn decode_batch_record_v6(
    encoded: &[u8],
    limits: RegistryLimits,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<DecodedBatch, BatchCodecV4Error> {
    probe().map_err(checkpoint_error)?;
    limits.validate()?;
    let mut outer = open(c::parse_canonical_cbor(encoded).map_err(|_| bad())?)?;
    if string(take(&mut outer, "profile")?)? != BATCH_PROFILE_V6 {
        return Err(bad().into());
    }
    let operation_id = opaque(take(&mut outer, "operation")?)?;
    let digest = Blake3Digest32::from_stored_bytes(fixed(take(&mut outer, "declared_digest")?)?);
    let fingerprint =
        Blake3Digest32::from_stored_bytes(fixed(take(&mut outer, "request_fingerprint")?)?);
    let before = number(take(&mut outer, "before_revision")?)?;
    let after = number(take(&mut outer, "after_revision")?)?;
    let receipt = receipt(take(&mut outer, "receipt")?)?;
    let mut input = open(take(&mut outer, "input")?)?;
    if string(take(&mut input, "kind")?)? != "bind-projection-membership" {
        return Err(bad().into());
    }
    let request = read_request(take(&mut input, "request")?)?;
    let input_receipt = super::receipt(take(&mut input, "receipt")?)?;
    finish(input)?;
    let state = read_state_v6(take(&mut outer, "after")?)?;
    finish(outer)?;
    let value = DecodedBatch {
        encoding_profile: BATCH_PROFILE_V6,
        state_encoding_profile: STATE_PROFILE_V6,
        entry: RegistryJournalEntry::new(
            operation_id,
            digest,
            before,
            after,
            JournalEntryKind::BatchApply,
            receipt,
        ),
        request_fingerprint: fingerprint,
        change_count: 1,
        deltas: vec![RegistryStateDelta::new(state.key(), None, state)?],
        original_changes: Some(vec![RegistryChange::BindProjectionMembership {
            request,
            receipt: input_receipt,
        }]),
    };
    let batch = value.replay_batch().ok_or_else(bad)?;
    batch.validate(limits)?;
    if request_fingerprint_v6(&batch)? != fingerprint || encode_batch_record_v6(&value)? != encoded
    {
        return Err(bad().into());
    }
    probe().map_err(checkpoint_error)?;
    Ok(value)
}
