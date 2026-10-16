//! Explicit v5 batch/state codec for accepted permanent SourceMembership values.
//!
//! Legacy v2-v4 encoders remain byte-frozen. This profile stores the full
//! typed contract member and exact permanent admission references; it never
//! synthesizes a v1 admission receipt.

#[allow(clippy::wildcard_imports)]
use super::*;

use crate::membership::{
    MembershipCommand, MembershipLifecycle, MembershipRecord, PermanentMembershipProvenanceV2,
};
use search_contracts::{
    AccessPolicyBindingId, CorpusId, InstallationIncarnationId, MembershipRevision, OwnerEpoch,
    ProfileId, ResidencyPolicyBindingId, RootBindingId, SourceId, SourceMembership,
    SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration, WorkspaceId,
};

/// V5 state-value profile for typed permanent memberships.
pub const STATE_PROFILE_V5: &str = "eliot.registry-state.v5";
/// V5 operation profile for typed permanent memberships.
pub const BATCH_PROFILE_V5: &str = "eliot.registry-batch.v5";

fn lifecycle_text(value: MembershipLifecycle) -> &'static str {
    match value {
        MembershipLifecycle::Active => "active",
        MembershipLifecycle::Restricted => "restricted",
        MembershipLifecycle::Suspended => "suspended",
        MembershipLifecycle::Retired => "retired",
        MembershipLifecycle::Removed => "removed",
    }
}

fn read_lifecycle(value: V) -> Result<MembershipLifecycle, RegistryError> {
    match string(value)?.as_str() {
        "active" => Ok(MembershipLifecycle::Active),
        "restricted" => Ok(MembershipLifecycle::Restricted),
        "suspended" => Ok(MembershipLifecycle::Suspended),
        "retired" => Ok(MembershipLifecycle::Retired),
        "removed" => Ok(MembershipLifecycle::Removed),
        _ => Err(bad()),
    }
}

fn membership_command_text(value: MembershipCommand) -> &'static str {
    match value {
        MembershipCommand::Activate => "activate",
        MembershipCommand::Restrict => "restrict",
        MembershipCommand::Suspend => "suspend",
        MembershipCommand::Retire => "retire",
        MembershipCommand::Remove => "remove",
    }
}

fn read_membership_command(value: V) -> Result<MembershipCommand, RegistryError> {
    match string(value)?.as_str() {
        "activate" => Ok(MembershipCommand::Activate),
        "restrict" => Ok(MembershipCommand::Restrict),
        "suspend" => Ok(MembershipCommand::Suspend),
        "retire" => Ok(MembershipCommand::Retire),
        "remove" => Ok(MembershipCommand::Remove),
        _ => Err(bad()),
    }
}

fn member_value(value: &SourceMembership) -> Result<V, RegistryError> {
    object(vec![
        (
            "source_membership_id",
            bytes(value.source_membership_id.as_bytes())?,
        ),
        ("corpus_id", bytes(value.corpus_id.as_bytes())?),
        ("source_id", bytes(value.source_id.as_bytes())?),
        ("workspace_id", bytes(value.workspace_id.as_bytes())?),
        (
            "role",
            text(match value.role {
                c::MembershipRole::Source => "source",
                c::MembershipRole::Test => "test",
                c::MembershipRole::Documentation => "documentation",
                c::MembershipRole::Generated => "generated",
                c::MembershipRole::Vendor => "vendor",
                c::MembershipRole::Reference => "reference",
            })?,
        ),
        (
            "preparation_profile_id",
            text(value.preparation_profile_id.as_str())?,
        ),
        (
            "access_policy_binding_id",
            bytes(value.access_policy_binding_id.as_bytes())?,
        ),
        (
            "retention_policy_id",
            text(value.retention_policy_id.as_str())?,
        ),
        (
            "residency_policy_binding_id",
            bytes(value.residency_policy_binding_id.as_bytes())?,
        ),
        (
            "membership_revision",
            V::U64(value.membership_revision.get()),
        ),
    ])
}

fn read_member(value: V) -> Result<SourceMembership, RegistryError> {
    let mut fields = open(value)?;
    let source_membership_id =
        SourceMembershipId::from_bytes(fixed(take(&mut fields, "source_membership_id")?)?);
    let corpus_id = CorpusId::from_bytes(fixed(take(&mut fields, "corpus_id")?)?);
    let source_id = SourceId::from_bytes(fixed(take(&mut fields, "source_id")?)?);
    let workspace_id = WorkspaceId::from_bytes(fixed(take(&mut fields, "workspace_id")?)?);
    let role = match string(take(&mut fields, "role")?)?.as_str() {
        "source" => c::MembershipRole::Source,
        "test" => c::MembershipRole::Test,
        "documentation" => c::MembershipRole::Documentation,
        "generated" => c::MembershipRole::Generated,
        "vendor" => c::MembershipRole::Vendor,
        "reference" => c::MembershipRole::Reference,
        _ => return Err(bad()),
    };
    let preparation_profile_id =
        ProfileId::new(string(take(&mut fields, "preparation_profile_id")?)?).map_err(|_| bad())?;
    let access_policy_binding_id =
        AccessPolicyBindingId::from_bytes(fixed(take(&mut fields, "access_policy_binding_id")?)?);
    let retention_policy_id =
        ProfileId::new(string(take(&mut fields, "retention_policy_id")?)?).map_err(|_| bad())?;
    let residency_policy_binding_id = ResidencyPolicyBindingId::from_bytes(fixed(take(
        &mut fields,
        "residency_policy_binding_id",
    )?)?);
    let membership_revision =
        MembershipRevision::new(number(take(&mut fields, "membership_revision")?)?);
    finish(fields)?;
    Ok(SourceMembership {
        source_membership_id,
        corpus_id,
        source_id,
        workspace_id,
        role,
        preparation_profile_id,
        access_policy_binding_id,
        retention_policy_id,
        residency_policy_binding_id,
        membership_revision,
    })
}

fn provenance_value(value: &PermanentMembershipProvenanceV2) -> Result<V, RegistryError> {
    object(vec![
        ("source_identity", identity(value.source_identity())?),
        ("source_revision", V::U64(value.source_revision().get())),
        (
            "root_binding_id",
            bytes(value.root_binding_id().as_bytes())?,
        ),
        (
            "root_record_revision",
            V::U64(value.root_record_revision().get()),
        ),
        (
            "source_namespace_id",
            bytes(value.source_namespace_id().as_bytes())?,
        ),
        (
            "namespace_record_revision",
            V::U64(value.namespace_record_revision().get()),
        ),
        (
            "owner_generation",
            bytes(value.owner_generation().as_bytes())?,
        ),
        ("policy_reference", bytes(value.policy_reference())?),
        ("admission_reference", bytes(value.admission_reference())?),
        ("policy_revision", V::U64(value.policy_revision().get())),
    ])
}

fn read_provenance(value: V) -> Result<PermanentMembershipProvenanceV2, RegistryError> {
    let mut fields = open(value)?;
    let source_identity = read_identity(take(&mut fields, "source_identity")?)?;
    let source_revision = revision(take(&mut fields, "source_revision")?)?;
    let root_binding_id = RootBindingId::from_bytes(fixed(take(&mut fields, "root_binding_id")?)?);
    let root_record_revision = revision(take(&mut fields, "root_record_revision")?)?;
    let source_namespace_id =
        SourceNamespaceId::from_bytes(fixed(take(&mut fields, "source_namespace_id")?)?);
    let namespace_record_revision = revision(take(&mut fields, "namespace_record_revision")?)?;
    let owner_generation =
        SourceOwnerGeneration::from_bytes(fixed(take(&mut fields, "owner_generation")?)?);
    let policy_reference = binary(take(&mut fields, "policy_reference")?)?;
    let admission_reference = binary(take(&mut fields, "admission_reference")?)?;
    let policy_revision = revision(take(&mut fields, "policy_revision")?)?;
    finish(fields)?;
    if source_namespace_id != source_identity.source_namespace_id {
        return Err(bad());
    }
    PermanentMembershipProvenanceV2::from_stored_parts(
        source_identity,
        source_revision,
        root_binding_id,
        root_record_revision,
        source_namespace_id,
        namespace_record_revision,
        owner_generation,
        policy_reference,
        admission_reference,
        policy_revision,
    )
}

fn membership_state_value(value: &MembershipRecord) -> Result<V, RegistryError> {
    let member = value.permanent_v2().ok_or(RegistryError::SnapshotInvalid)?;
    object(vec![
        ("family", text("membership")?),
        (
            "schema",
            V::U64(crate::membership::PERMANENT_MEMBERSHIP_RECORD_V2_SCHEMA),
        ),
        ("key", member_key(value.key())?),
        ("member", member_value(member.member())?),
        ("provenance", provenance_value(member.provenance())?),
        ("lifecycle", text(lifecycle_text(value.lifecycle()))?),
        ("registry_revision", V::U64(value.registry_revision())),
        ("receipt", text(value.last_receipt().as_str())?),
    ])
}

fn read_membership_state(value: V) -> Result<RegistryStateRecord, RegistryError> {
    let mut fields = open(value)?;
    if string(take(&mut fields, "family")?)? != "membership"
        || number(take(&mut fields, "schema")?)?
            != crate::membership::PERMANENT_MEMBERSHIP_RECORD_V2_SCHEMA
    {
        return Err(bad());
    }
    let key = read_member_key(take(&mut fields, "key")?)?;
    let member = read_member(take(&mut fields, "member")?)?;
    let provenance = read_provenance(take(&mut fields, "provenance")?)?;
    let lifecycle = read_lifecycle(take(&mut fields, "lifecycle")?)?;
    let registry_revision = number(take(&mut fields, "registry_revision")?)?;
    let receipt = receipt(take(&mut fields, "receipt")?)?;
    finish(fields)?;
    let canonical_corpus_id = member.corpus_id.to_string();
    if key.corpus_id.as_str() != canonical_corpus_id.as_str()
        || member.source_id != provenance.source_identity().source_id
        || key.source_identity != *provenance.source_identity()
    {
        return Err(bad());
    }
    let generation = NonZeroRevision::new(member.membership_revision.get()).map_err(|_| bad())?;
    MembershipRecord::new_permanent_v2(
        key,
        generation,
        member,
        provenance,
        lifecycle,
        registry_revision,
        receipt,
    )
    .map(RegistryStateRecord::Membership)
}

fn state_value_v5(value: &RegistryStateRecord) -> Result<V, RegistryError> {
    match value {
        RegistryStateRecord::Membership(record) if record.is_permanent_v2() => {
            membership_state_value(record)
        }
        _ => Err(RegistryError::SnapshotInvalid),
    }
}

fn read_state_v5(value: V) -> Result<RegistryStateRecord, RegistryError> {
    read_membership_state(value)
}

/// Encodes one explicit V5 membership state envelope.
pub fn encode_state_v5(value: &RegistryStateRecord) -> Result<Vec<u8>, RegistryError> {
    c::to_canonical_cbor(&object(vec![
        ("profile", text(STATE_PROFILE_V5)?),
        ("state", state_value_v5(value)?),
    ])?)
    .map(|encoded| encoded.as_slice().to_vec())
    .map_err(|_| bad())
}

/// Decodes one explicit V5 membership state envelope and verifies its canonical bytes.
pub fn decode_state_v5(
    bytes: &[u8],
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<RegistryStateRecord, BatchCodecV4Error> {
    probe().map_err(checkpoint_error)?;
    let mut outer = open(c::parse_canonical_cbor(bytes).map_err(|_| bad())?)?;
    if string(take(&mut outer, "profile")?)? != STATE_PROFILE_V5 {
        return Err(bad().into());
    }
    let state = read_state_v5(take(&mut outer, "state")?)?;
    finish(outer)?;
    if encode_state_v5(&state)? != bytes {
        return Err(bad().into());
    }
    probe().map_err(checkpoint_error)?;
    Ok(state)
}

fn change_value_v5(value: &RegistryChange) -> Result<V, RegistryError> {
    match value {
        RegistryChange::BindPermanentMembershipV2 {
            member,
            source_namespace_id,
            root_binding_id,
            owner,
            source_identity,
            expected_source_revision,
            expected_root_record_revision,
            expected_namespace_record_revision,
            expected_owner_generation,
            receipt: durability_receipt,
        } => object(vec![
            ("kind", text("bind-permanent-membership-v2")?),
            ("member", member_value(member)?),
            (
                "source_namespace_id",
                bytes(source_namespace_id.as_bytes())?,
            ),
            ("root_binding_id", bytes(root_binding_id.as_bytes())?),
            ("owner_system_id", text(owner.system_id.as_str())?),
            (
                "owner_installation_incarnation_id",
                bytes(owner.installation_incarnation.as_bytes())?,
            ),
            ("owner_epoch", V::U64(owner.epoch.get())),
            ("source_identity", identity(source_identity)?),
            (
                "expected_source_revision",
                V::U64(expected_source_revision.get()),
            ),
            (
                "expected_root_record_revision",
                V::U64(expected_root_record_revision.get()),
            ),
            (
                "expected_namespace_record_revision",
                V::U64(expected_namespace_record_revision.get()),
            ),
            (
                "expected_owner_generation",
                bytes(expected_owner_generation.as_bytes())?,
            ),
            ("receipt", text(durability_receipt.as_str())?),
        ]),
        RegistryChange::TransitionPermanentMembershipV2 {
            membership_id,
            expected_membership_revision,
            command,
            receipt: durability_receipt,
        } => object(vec![
            ("kind", text("transition-permanent-membership-v2")?),
            ("membership_id", bytes(membership_id.as_bytes())?),
            (
                "expected_membership_revision",
                V::U64(expected_membership_revision.get()),
            ),
            ("command", text(membership_command_text(*command))?),
            ("receipt", text(durability_receipt.as_str())?),
        ]),
        _ => super::change_value(value),
    }
}

fn read_change_v5(value: V) -> Result<RegistryChange, RegistryError> {
    let mut fields = open(value)?;
    let kind = string(take(&mut fields, "kind")?)?;
    let change = match kind.as_str() {
        "bind-permanent-membership-v2" => {
            let member = read_member(take(&mut fields, "member")?)?;
            let source_namespace_id =
                SourceNamespaceId::from_bytes(fixed(take(&mut fields, "source_namespace_id")?)?);
            let root_binding_id =
                RootBindingId::from_bytes(fixed(take(&mut fields, "root_binding_id")?)?);
            let owner = crate::source_request_v2::SourceOwnerScopeV2 {
                system_id: opaque(take(&mut fields, "owner_system_id")?)?,
                installation_incarnation: InstallationIncarnationId::from_bytes(fixed(take(
                    &mut fields,
                    "owner_installation_incarnation_id",
                )?)?),
                epoch: OwnerEpoch::new(number(take(&mut fields, "owner_epoch")?)?)
                    .map_err(|_| bad())?,
            };
            let source_identity = read_identity(take(&mut fields, "source_identity")?)?;
            if source_identity.source_namespace_id != source_namespace_id
                || source_identity.source_id != member.source_id
            {
                return Err(bad());
            }
            RegistryChange::BindPermanentMembershipV2 {
                member,
                source_namespace_id,
                root_binding_id,
                owner,
                source_identity,
                expected_source_revision: revision(take(&mut fields, "expected_source_revision")?)?,
                expected_root_record_revision: revision(take(
                    &mut fields,
                    "expected_root_record_revision",
                )?)?,
                expected_namespace_record_revision: revision(take(
                    &mut fields,
                    "expected_namespace_record_revision",
                )?)?,
                expected_owner_generation: SourceOwnerGeneration::from_bytes(fixed(take(
                    &mut fields,
                    "expected_owner_generation",
                )?)?),
                receipt: receipt(take(&mut fields, "receipt")?)?,
            }
        }
        "transition-permanent-membership-v2" => RegistryChange::TransitionPermanentMembershipV2 {
            membership_id: SourceMembershipId::from_bytes(fixed(take(
                &mut fields,
                "membership_id",
            )?)?),
            expected_membership_revision: MembershipRevision::new(number(take(
                &mut fields,
                "expected_membership_revision",
            )?)?),
            command: read_membership_command(take(&mut fields, "command")?)?,
            receipt: receipt(take(&mut fields, "receipt")?)?,
        },
        _ => return Err(bad()),
    };
    finish(fields)?;
    Ok(change)
}

/// Canonical full-input fingerprint for V5 membership operations.
pub fn request_fingerprint_v5(batch: &RegistryBatch) -> Result<Blake3Digest32, RegistryError> {
    let value = object(vec![
        ("profile", text("registry-batch-input-v4")?),
        (
            "expected_revision",
            V::U64(batch.expected_registry_revision),
        ),
        ("operation", text(batch.operation.operation_id().as_str())?),
        (
            "declared_digest",
            bytes(batch.operation.mutation_digest().as_bytes())?,
        ),
        ("changes", list(batch.changes.iter().map(change_value_v5))?),
    ])?;
    c::blake3_canonical(
        &c::CanonicalDigestDomain::parse("eliot/cbor/registry-batch-input/v4")
            .map_err(|_| bad())?,
        &value,
        c::DigestInputLimit::new(c::MAX_CANONICAL_BYTES).map_err(|_| bad())?,
    )
    .map_err(|_| bad())
}

/// Encodes one V5 operation/delta record.
pub fn encode_batch_record_v5(value: &DecodedBatch) -> Result<Vec<u8>, RegistryError> {
    if value.encoding_profile() != BATCH_PROFILE_V5
        || value.state_encoding_profile() != STATE_PROFILE_V5
    {
        return Err(bad());
    }
    let original_changes = value
        .original_changes
        .as_ref()
        .filter(|changes| changes.len() == value.change_count)
        .ok_or_else(bad)?;
    let encoded = object(vec![
        ("profile", text(BATCH_PROFILE_V5)?),
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
        ("input", list(original_changes.iter().map(change_value_v5))?),
        (
            "deltas",
            list(value.deltas.iter().map(|delta| {
                object(vec![
                    ("key", super::v4::key_value_v4(&delta.key)?),
                    (
                        "before",
                        optional(delta.before.as_ref().map(state_value_v5))?,
                    ),
                    ("after", state_value_v5(&delta.after)?),
                ])
            }))?,
        ),
    ])?;
    c::to_canonical_cbor(&encoded)
        .map(|bytes| bytes.as_slice().to_vec())
        .map_err(|_| bad())
}

/// Decodes one V5 operation/delta record, retaining all full typed inputs.
pub fn decode_batch_record_v5(
    bytes: &[u8],
    limits: RegistryLimits,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<DecodedBatch, BatchCodecV4Error> {
    probe().map_err(checkpoint_error)?;
    limits.validate()?;
    let mut outer = open(c::parse_canonical_cbor(bytes).map_err(|_| bad())?)?;
    if string(take(&mut outer, "profile")?)? != BATCH_PROFILE_V5 {
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
    let input_rows = array(take(&mut outer, "input")?)?;
    if input_rows.len() != change_count || change_count != 1 {
        return Err(RegistryError::BatchSizeInvalid.into());
    }
    let original_changes = input_rows
        .into_iter()
        .map(read_change_v5)
        .collect::<Result<Vec<_>, _>>()?;
    let rows = array(take(&mut outer, "deltas")?)?;
    if change_count != 1 || rows.len() != 1 || rows.len() > limits.max_batch_changes {
        return Err(RegistryError::BatchSizeInvalid.into());
    }
    let mut row = open(rows.into_iter().next().ok_or_else(bad)?)?;
    let key = super::read_key(take(&mut row, "key")?)?;
    let before = match take(&mut row, "before")? {
        V::Null => None,
        value => Some(read_state_v5(value)?),
    };
    let after = read_state_v5(take(&mut row, "after")?)?;
    finish(row)?;
    finish(outer)?;
    if !matches!(key, RegistryStateKey::Membership(_))
        || after.key() != key
        || before.as_ref().is_some_and(|value| value.key() != key)
    {
        return Err(bad().into());
    }
    let record = DecodedBatch {
        encoding_profile: BATCH_PROFILE_V5,
        state_encoding_profile: STATE_PROFILE_V5,
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
        deltas: vec![RegistryStateDelta { key, before, after }],
        original_changes: Some(original_changes),
    };
    let original_request = record
        .replay_batch()
        .ok_or_else(|| BatchCodecV4Error::Registry(bad()))?;
    original_request.validate(limits)?;
    if request_fingerprint_v5(&original_request)? != request_fingerprint {
        return Err(bad().into());
    }
    if encode_batch_record_v5(&record)? != bytes {
        return Err(bad().into());
    }
    probe().map_err(checkpoint_error)?;
    Ok(record)
}
