//! Closed canonical CBOR batch record, using the shared canonical owner only.
use super::{DecodedBatch, RegistryStateDelta, RegistryStateKey, RegistryStateRecord};
use crate::error::{JournalEntryKind, RegistryError, RegistryJournalEntry, RegistryLimits};
use crate::membership::{
    MembershipKey, MembershipLifecycle, MembershipRecord, derive_membership_id,
};
use crate::recovery::{RegistryBatch, RegistryChange};
use crate::root::{RootRecord, RootStatus};
use crate::source::{AdmissionBindingProof, RegisteredSource, SourceLifecycle};
use search_contracts::{self as c, CanonicalValue as V, ClosedCanonicalObject as Obj};
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef, SourceIdentity};
use search_source_admission::original_envelope::{
    MAX_ORIGINAL_ADMISSION_BYTES, OriginalAdmissionEnvelope, decode_original_admission,
};
use search_source_admission::{AdmissionBudget, AdmissionOutcome, AdmissionReceipt, CancelFlag};
use search_source_identity::{CanonicalRelativePath, SourceBinding, SourceObservation};

fn bad() -> RegistryError {
    RegistryError::SnapshotInvalid
}
fn object(fields: Vec<(&str, V)>) -> Result<V, RegistryError> {
    c::BoundedMap::from_entries(
        fields
            .into_iter()
            .map(|(k, v)| c::CanonicalKey::new_non_empty(k).map(|k| (k, v)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| bad())?,
    )
    .map(V::Object)
    .map_err(|_| bad())
}
fn text(v: &str) -> Result<V, RegistryError> {
    c::CanonicalText::new(v).map(V::Text).map_err(|_| bad())
}
fn bytes(v: &[u8]) -> Result<V, RegistryError> {
    c::BoundedBytes::new(v.to_vec())
        .map(V::Bytes)
        .map_err(|_| bad())
}
fn list(values: impl Iterator<Item = Result<V, RegistryError>>) -> Result<V, RegistryError> {
    if values
        .size_hint()
        .1
        .is_none_or(|n| n > c::MAX_COLLECTION_ITEMS)
    {
        return Err(RegistryError::CapacityExceeded);
    }
    c::BoundedList::new(values.collect::<Result<Vec<_>, _>>()?)
        .map(V::Array)
        .map_err(|_| bad())
}
fn optional(v: Option<Result<V, RegistryError>>) -> Result<V, RegistryError> {
    v.transpose().map(|v| v.unwrap_or(V::Null))
}
fn open(v: V) -> Result<Obj, RegistryError> {
    Obj::from_value(v, "registry_batch").map_err(|_| bad())
}
fn take(o: &mut Obj, key: &'static str) -> Result<V, RegistryError> {
    o.take_required(key).map_err(|_| bad())
}
fn string(v: V) -> Result<String, RegistryError> {
    if let V::Text(v) = v {
        Ok(v.into_string())
    } else {
        Err(bad())
    }
}
fn number(v: V) -> Result<u64, RegistryError> {
    if let V::U64(v) = v { Ok(v) } else { Err(bad()) }
}
fn boolean(v: V) -> Result<bool, RegistryError> {
    if let V::Bool(v) = v {
        Ok(v)
    } else {
        Err(bad())
    }
}
fn binary(v: V) -> Result<Vec<u8>, RegistryError> {
    if let V::Bytes(v) = v {
        Ok(v.into_vec())
    } else {
        Err(bad())
    }
}
fn array(v: V) -> Result<Vec<V>, RegistryError> {
    if let V::Array(v) = v {
        Ok(v.into_vec())
    } else {
        Err(bad())
    }
}
fn fixed<const N: usize>(v: V) -> Result<[u8; N], RegistryError> {
    binary(v)?.try_into().map_err(|_| bad())
}
fn revision(v: V) -> Result<NonZeroRevision, RegistryError> {
    NonZeroRevision::new(number(v)?).map_err(|_| bad())
}
fn opaque(v: V) -> Result<OpaqueId, RegistryError> {
    OpaqueId::new(string(v)?).map_err(|_| bad())
}
fn receipt(v: V) -> Result<ReceiptRef, RegistryError> {
    ReceiptRef::new(string(v)?).map_err(|_| bad())
}
fn finish(o: Obj) -> Result<(), RegistryError> {
    o.finish().map_err(|_| bad())
}
fn optional_decode<T>(
    v: V,
    f: impl FnOnce(V) -> Result<T, RegistryError>,
) -> Result<Option<T>, RegistryError> {
    if v == V::Null {
        Ok(None)
    } else {
        f(v).map(Some)
    }
}
fn identity(v: &SourceIdentity) -> Result<V, RegistryError> {
    object(vec![
        ("namespace", bytes(v.source_namespace_id.as_bytes())?),
        ("source", bytes(v.source_id.as_bytes())?),
        (
            "kind",
            text(match v.identity_kind {
                c::SourceIdentityKind::NtfsFile => "ntfs",
                c::SourceIdentityKind::GitBlobLineage => "git",
                c::SourceIdentityKind::ImportedObject => "imported",
                c::SourceIdentityKind::AdmittedVirtualSnapshot => "virtual",
            })?,
        ),
        (
            "components",
            bytes(v.stable_identity_components.as_slice())?,
        ),
    ])
}
fn read_identity(v: V) -> Result<SourceIdentity, RegistryError> {
    let mut o = open(v)?;
    let source_namespace_id = c::SourceNamespaceId::from_bytes(fixed(take(&mut o, "namespace")?)?);
    let source_id = c::SourceId::from_bytes(fixed(take(&mut o, "source")?)?);
    let identity_kind = match string(take(&mut o, "kind")?)?.as_str() {
        "ntfs" => c::SourceIdentityKind::NtfsFile,
        "git" => c::SourceIdentityKind::GitBlobLineage,
        "imported" => c::SourceIdentityKind::ImportedObject,
        "virtual" => c::SourceIdentityKind::AdmittedVirtualSnapshot,
        _ => return Err(bad()),
    };
    let stable_identity_components =
        c::OpaqueCanonicalBytes::from_validated(binary(take(&mut o, "components")?)?)
            .map_err(|_| bad())?;
    finish(o)?;
    Ok(SourceIdentity {
        source_namespace_id,
        source_id,
        identity_kind,
        stable_identity_components,
    })
}
fn binding(v: &SourceBinding) -> Result<V, RegistryError> {
    object(vec![
        ("identity", identity(v.identity())?),
        ("root", bytes(v.root_binding_id().as_bytes())?),
        ("relative_path", text(v.relative_path().as_str())?),
        (
            "file_digest",
            optional(v.stable_file_identity_digest().map(|d| bytes(d.as_bytes())))?,
        ),
        ("content_digest", bytes(v.content_digest().as_bytes())?),
        ("content_bytes", V::U64(v.content_bytes())),
        (
            "observation_receipt",
            text(v.observation_receipt().as_str())?,
        ),
        ("binding_revision", V::U64(v.binding_revision().get())),
        (
            "observation_revision",
            V::U64(v.observation_revision().get()),
        ),
    ])
}
fn read_binding(v: V) -> Result<SourceBinding, RegistryError> {
    let mut o = open(v)?;
    let id = read_identity(take(&mut o, "identity")?)?;
    let root_binding_id =
        search_source_identity::RootBindingId::from_bytes(fixed(take(&mut o, "root")?)?);
    let relative_path = CanonicalRelativePath::new(
        string(take(&mut o, "relative_path")?)?,
        search_source_identity::DEFAULT_IDENTITY_LIMITS,
    )
    .map_err(|_| bad())?;
    let stable_file_identity_digest = optional_decode(take(&mut o, "file_digest")?, |v| {
        fixed(v).map(Blake3Digest32::from_stored_bytes)
    })?;
    let content_digest = Blake3Digest32::from_stored_bytes(fixed(take(&mut o, "content_digest")?)?);
    let content_bytes = number(take(&mut o, "content_bytes")?)?;
    let observation_receipt = receipt(take(&mut o, "observation_receipt")?)?;
    let binding_revision = revision(take(&mut o, "binding_revision")?)?;
    let observation_revision = revision(take(&mut o, "observation_revision")?)?;
    finish(o)?;
    Ok(SourceBinding::new(
        id,
        SourceObservation {
            root_binding_id,
            relative_path,
            stable_file_identity_digest,
            content_digest,
            content_bytes,
            observation_receipt,
        },
        binding_revision,
        observation_revision,
    ))
}
fn admission(v: &AdmissionReceipt) -> Result<V, RegistryError> {
    object(vec![
        ("legacy_receipt_digest", bytes(v.receipt_digest())?),
        ("schema", V::U64(u64::from(v.schema_version()))),
        ("policy_revision", V::U64(v.policy_revision().get())),
        (
            "policy_fingerprint",
            bytes(v.policy_fingerprint().as_bytes())?,
        ),
        (
            "observation_digest",
            bytes(v.observation_digest().as_bytes())?,
        ),
        ("outcome", text(v.outcome().as_str())?),
        ("reasons", list(v.reason_codes().map(|r| text(r.as_str())))?),
        ("rules", list(v.matched_rule_ids().map(|r| text(r)))?),
        ("sensitivity", text(v.sensitivity().as_str())?),
    ])
}
fn read_admission(
    v: V,
    original: &OriginalAdmissionEnvelope,
) -> Result<AdmissionReceipt, RegistryError> {
    if admission(original.receipt())? != v {
        return Err(bad());
    }
    Ok(original.receipt().clone())
}
fn assignment(v: &AdmissionBindingProof) -> Result<V, RegistryError> {
    object(vec![
        ("identity", identity(&v.source_identity)?),
        (
            "observation_digest",
            bytes(v.observation_digest.as_bytes())?,
        ),
        ("assignment_digest", bytes(v.assignment_digest.as_bytes())?),
        ("receipt", text(v.assignment_receipt.as_str())?),
        ("readback_verified", V::Bool(v.readback_verified)),
    ])
}
fn read_assignment(v: V) -> Result<AdmissionBindingProof, RegistryError> {
    let mut o = open(v)?;
    let source_identity = read_identity(take(&mut o, "identity")?)?;
    let observation_digest =
        Blake3Digest32::from_stored_bytes(fixed(take(&mut o, "observation_digest")?)?);
    let assignment_digest =
        Blake3Digest32::from_stored_bytes(fixed(take(&mut o, "assignment_digest")?)?);
    let assignment_receipt = receipt(take(&mut o, "receipt")?)?;
    let readback_verified = boolean(take(&mut o, "readback_verified")?)?;
    finish(o)?;
    Ok(AdmissionBindingProof {
        observation_digest,
        source_identity,
        assignment_digest,
        assignment_receipt,
        readback_verified,
    })
}
fn member_key(v: &MembershipKey) -> Result<V, RegistryError> {
    object(vec![
        ("corpus", text(v.corpus_id.as_str())?),
        ("identity", identity(&v.source_identity)?),
    ])
}
fn read_member_key(v: V) -> Result<MembershipKey, RegistryError> {
    let mut o = open(v)?;
    let corpus_id = opaque(take(&mut o, "corpus")?)?;
    let source_identity = read_identity(take(&mut o, "identity")?)?;
    finish(o)?;
    Ok(MembershipKey {
        corpus_id,
        source_identity,
    })
}
pub(super) fn state_value(v: &RegistryStateRecord) -> Result<V, RegistryError> {
    match v {
        RegistryStateRecord::Root(root) => {
            crate::root::validate_metadata(root)?;
            object(vec![
                ("family", text("root")?),
                ("root_binding_id", bytes(root.root_binding_id.as_bytes())?),
                (
                    "canonical_root_digest",
                    bytes(root.canonical_root_digest.as_bytes())?,
                ),
                (
                    "policy_fingerprint",
                    bytes(root.policy_fingerprint.as_bytes())?,
                ),
                ("policy_revision", V::U64(root.policy_revision.get())),
                ("owner_epoch", V::U64(root.owner_epoch.get())),
                ("record_revision", V::U64(root.record_revision.get())),
                ("registry_revision", V::U64(root.registry_revision)),
                (
                    "status",
                    text(match root.status {
                        RootStatus::Active => "active",
                        RootStatus::Unbound => "unbound",
                    })?,
                ),
                ("last_receipt", text(root.last_receipt.as_str())?),
            ])
        }
        RegistryStateRecord::Source(v) => object(vec![
            ("family", text("source")?),
            (
                "original_admission",
                bytes(
                    v.original_admission()
                        .ok_or(RegistryError::AdmissionRestorationUnavailable)?
                        .encoded(),
                )?,
            ),
            ("binding", binding(v.binding())?),
            ("admission", admission(v.admission())?),
            ("assignment", assignment(v.assignment())?),
            (
                "lifecycle",
                text(if v.lifecycle() == SourceLifecycle::Active {
                    "active"
                } else {
                    "retired"
                })?,
            ),
            ("source_revision", V::U64(v.source_revision().get())),
            ("registry_revision", V::U64(v.registry_revision())),
            ("receipt", text(v.last_receipt().as_str())?),
        ]),
        RegistryStateRecord::Membership(v) => object(vec![
            ("family", text("membership")?),
            ("key", member_key(v.key())?),
            ("id", bytes(v.membership_id().as_bytes())?),
            ("generation", V::U64(v.generation().get())),
            ("revision", V::U64(v.membership_revision().get())),
            (
                "lifecycle",
                text(match v.lifecycle() {
                    MembershipLifecycle::Active => "active",
                    MembershipLifecycle::Restricted => "restricted",
                    MembershipLifecycle::Suspended => "suspended",
                    MembershipLifecycle::Retired => "retired",
                    MembershipLifecycle::Removed => "removed",
                })?,
            ),
            ("registry_revision", V::U64(v.registry_revision())),
            ("receipt", text(v.last_receipt().as_str())?),
        ]),
        RegistryStateRecord::Generation { corpus, generation } => object(vec![
            ("family", text("generation")?),
            ("corpus", text(corpus.as_str())?),
            ("generation", V::U64(generation.get())),
        ]),
    }
}
fn read_state(v: V, roots_allowed: bool) -> Result<RegistryStateRecord, RegistryError> {
    let mut o = open(v)?;
    let family = string(take(&mut o, "family")?)?;
    let state = match family.as_str() {
        "root" if roots_allowed => {
            let root = RootRecord {
                root_binding_id: c::RootBindingId::from_bytes(fixed(take(
                    &mut o,
                    "root_binding_id",
                )?)?),
                canonical_root_digest: Blake3Digest32::from_stored_bytes(fixed(take(
                    &mut o,
                    "canonical_root_digest",
                )?)?),
                policy_fingerprint: Blake3Digest32::from_stored_bytes(fixed(take(
                    &mut o,
                    "policy_fingerprint",
                )?)?),
                policy_revision: revision(take(&mut o, "policy_revision")?)?,
                owner_epoch: c::OwnerEpoch::new(number(take(&mut o, "owner_epoch")?)?)
                    .map_err(|_| bad())?,
                record_revision: revision(take(&mut o, "record_revision")?)?,
                registry_revision: number(take(&mut o, "registry_revision")?)?,
                status: match string(take(&mut o, "status")?)?.as_str() {
                    "active" => RootStatus::Active,
                    "unbound" => RootStatus::Unbound,
                    _ => return Err(bad()),
                },
                last_receipt: receipt(take(&mut o, "last_receipt")?)?,
            };
            crate::root::validate_metadata(&root)?;
            RegistryStateRecord::Root(root)
        }
        "source" => {
            let b = read_binding(take(&mut o, "binding")?)?;
            let original = decode_original_admission(
                &binary(take(&mut o, "original_admission")?)?,
                MAX_ORIGINAL_ADMISSION_BYTES,
                AdmissionBudget::default_budget(),
                CancelFlag::live(),
            )
            .map_err(|_| RegistryError::AdmissionBindingMismatch)?;
            let a = read_admission(take(&mut o, "admission")?, &original)?;
            let proof = read_assignment(take(&mut o, "assignment")?)?;
            let lifecycle = match string(take(&mut o, "lifecycle")?)?.as_str() {
                "active" => SourceLifecycle::Active,
                "retired" => SourceLifecycle::Retired,
                _ => return Err(bad()),
            };
            let source_revision = revision(take(&mut o, "source_revision")?)?;
            let registry_revision = number(take(&mut o, "registry_revision")?)?;
            let receipt = receipt(take(&mut o, "receipt")?)?;
            if a.outcome() != AdmissionOutcome::Allow
                || !proof.readback_verified
                || proof.source_identity != *b.identity()
                || proof.observation_digest.as_bytes() != a.observation_digest().as_bytes()
            {
                return Err(bad());
            }
            RegistryStateRecord::Source(
                RegisteredSource::new(
                    b,
                    a,
                    proof,
                    lifecycle,
                    source_revision,
                    registry_revision,
                    receipt,
                )
                .retaining_original(Some(original))?,
            )
        }
        "membership" => {
            let key = read_member_key(take(&mut o, "key")?)?;
            let id = c::SourceMembershipId::from_bytes(fixed(take(&mut o, "id")?)?);
            let generation = revision(take(&mut o, "generation")?)?;
            let rev = revision(take(&mut o, "revision")?)?;
            let lifecycle = match string(take(&mut o, "lifecycle")?)?.as_str() {
                "active" => MembershipLifecycle::Active,
                "restricted" => MembershipLifecycle::Restricted,
                "suspended" => MembershipLifecycle::Suspended,
                "retired" => MembershipLifecycle::Retired,
                "removed" => MembershipLifecycle::Removed,
                _ => return Err(bad()),
            };
            let reg = number(take(&mut o, "registry_revision")?)?;
            let r = receipt(take(&mut o, "receipt")?)?;
            if id != derive_membership_id(&key) {
                return Err(bad());
            }
            RegistryStateRecord::Membership(MembershipRecord::new(
                key, id, generation, rev, lifecycle, reg, r,
            ))
        }
        "generation" => RegistryStateRecord::Generation {
            corpus: opaque(take(&mut o, "corpus")?)?,
            generation: revision(take(&mut o, "generation")?)?,
        },
        _ => return Err(bad()),
    };
    finish(o)?;
    Ok(state)
}
pub(super) fn encode_state(v: &RegistryStateRecord) -> Result<Vec<u8>, RegistryError> {
    encode_state_profile(v, CURRENT_STATE_PROFILE)
}
const STATE_PROFILE_V2: &str = "eliot.registry-state.v2";
const STATE_PROFILE_V3: &str = "eliot.registry-state.v3";
pub(super) const CURRENT_STATE_PROFILE: &str = STATE_PROFILE_V3;

pub(super) fn encode_state_profile(
    v: &RegistryStateRecord,
    profile: &str,
) -> Result<Vec<u8>, RegistryError> {
    c::to_canonical_cbor(&object(vec![
        ("profile", text(profile)?),
        ("state", state_value(v)?),
    ])?)
    .map(|v| v.as_slice().to_vec())
    .map_err(|_| bad())
}
pub(super) fn decode_state(bytes: &[u8]) -> Result<RegistryStateRecord, RegistryError> {
    decode_state_profiled(bytes).map(|(state, _)| state)
}
pub(super) fn decode_state_profiled(
    bytes: &[u8],
) -> Result<(RegistryStateRecord, &'static str), RegistryError> {
    let mut o = open(c::parse_canonical_cbor(bytes).map_err(|_| bad())?)?;
    let profile = string(take(&mut o, "profile")?)?;
    let encoding_profile = match profile.as_str() {
        "eliot.registry-state.v3" => STATE_PROFILE_V3,
        "eliot.registry-state.v2" => STATE_PROFILE_V2,
        "eliot.registry-state.v1" => return Err(RegistryError::AdmissionRestorationUnavailable),
        _ => return Err(bad()),
    };
    let state = read_state(take(&mut o, "state")?, profile == "eliot.registry-state.v3")?;
    finish(o)?;
    if encode_state_profile(&state, &profile)? != bytes {
        return Err(bad());
    }
    Ok((state, encoding_profile))
}
pub(super) fn encode_key(key: &RegistryStateKey) -> Result<Vec<u8>, RegistryError> {
    c::to_canonical_cbor(&object(vec![
        ("profile", text("eliot.registry-state-key.v1")?),
        ("key", key_value(key)?),
    ])?)
    .map(|v| v.as_slice().to_vec())
    .map_err(|_| bad())
}
pub(super) fn decode_key(bytes: &[u8]) -> Result<RegistryStateKey, RegistryError> {
    let mut fields = open(c::parse_canonical_cbor(bytes).map_err(|_| bad())?)?;
    if string(take(&mut fields, "profile")?)? != "eliot.registry-state-key.v1" {
        return Err(bad());
    }
    let key = read_key(take(&mut fields, "key")?)?;
    finish(fields)?;
    if encode_key(&key)? != bytes {
        return Err(bad());
    }
    Ok(key)
}
fn key_value(k: &RegistryStateKey) -> Result<V, RegistryError> {
    match k {
        RegistryStateKey::Root(root) => object(vec![
            ("family", text("root")?),
            ("root", bytes(root.as_bytes())?),
        ]),
        RegistryStateKey::Source(v) => object(vec![
            ("family", text("source")?),
            ("identity", identity(v)?),
        ]),
        RegistryStateKey::Membership(v) => object(vec![
            ("family", text("membership")?),
            ("key", member_key(v)?),
        ]),
        RegistryStateKey::Generation(v) => object(vec![
            ("family", text("generation")?),
            ("corpus", text(v.as_str())?),
        ]),
    }
}
fn read_key(v: V) -> Result<RegistryStateKey, RegistryError> {
    let mut o = open(v)?;
    let result = match string(take(&mut o, "family")?)?.as_str() {
        "root" => {
            RegistryStateKey::Root(c::RootBindingId::from_bytes(fixed(take(&mut o, "root")?)?))
        }
        "source" => RegistryStateKey::Source(read_identity(take(&mut o, "identity")?)?),
        "membership" => RegistryStateKey::Membership(read_member_key(take(&mut o, "key")?)?),
        "generation" => RegistryStateKey::Generation(opaque(take(&mut o, "corpus")?)?),
        _ => return Err(bad()),
    };
    finish(o)?;
    Ok(result)
}
fn record_value(v: &DecodedBatch, profile: &str) -> Result<V, RegistryError> {
    object(vec![
        ("profile", text(profile)?),
        ("operation", text(v.entry.operation_id.as_str())?),
        (
            "declared_digest",
            bytes(v.entry.mutation_digest.as_bytes())?,
        ),
        (
            "request_fingerprint",
            bytes(v.request_fingerprint.as_bytes())?,
        ),
        ("before_revision", V::U64(v.entry.before_revision)),
        ("after_revision", V::U64(v.entry.after_revision)),
        ("receipt", text(v.entry.receipt.as_str())?),
        ("change_count", V::U64(v.change_count as u64)),
        (
            "deltas",
            list(v.deltas.iter().map(|d| {
                object(vec![
                    ("key", key_value(&d.key)?),
                    ("before", optional(d.before.as_ref().map(state_value))?),
                    ("after", state_value(&d.after)?),
                ])
            }))?,
        ),
    ])
}
const BATCH_PROFILE_V2: &str = "eliot.registry-batch.v2";
const BATCH_PROFILE_V3: &str = "eliot.registry-batch.v3";
pub(super) const CURRENT_BATCH_PROFILE: &str = BATCH_PROFILE_V3;

pub(super) fn encode(v: &DecodedBatch) -> Result<Vec<u8>, RegistryError> {
    encode_record_profile(v, v.encoding_profile)
}
fn encode_record_profile(v: &DecodedBatch, profile: &str) -> Result<Vec<u8>, RegistryError> {
    c::to_canonical_cbor(&record_value(v, profile)?)
        .map(|b| b.as_slice().to_vec())
        .map_err(|_| bad())
}
/// Decodes the closed versioned operation/delta record and verifies exact re-encoding.
///
/// Original admission inputs are restored by their sole owner evaluator.
/// Earlier profiles without original provenance explicitly refuse restoration.
pub fn decode_batch_record(
    bytes: &[u8],
    limits: RegistryLimits,
) -> Result<DecodedBatch, RegistryError> {
    limits.validate()?;
    let mut o = open(c::parse_canonical_cbor(bytes).map_err(|_| bad())?)?;
    let profile = string(take(&mut o, "profile")?)?;
    let (encoding_profile, state_encoding_profile) = match profile.as_str() {
        "eliot.registry-batch.v3" => (BATCH_PROFILE_V3, STATE_PROFILE_V3),
        "eliot.registry-batch.v2" => (BATCH_PROFILE_V2, STATE_PROFILE_V2),
        "eliot.registry-batch.v1" => return Err(RegistryError::AdmissionRestorationUnavailable),
        _ => return Err(bad()),
    };
    let operation_id = opaque(take(&mut o, "operation")?)?;
    let mutation_digest =
        Blake3Digest32::from_stored_bytes(fixed(take(&mut o, "declared_digest")?)?);
    let request_fingerprint =
        Blake3Digest32::from_stored_bytes(fixed(take(&mut o, "request_fingerprint")?)?);
    let before_revision = number(take(&mut o, "before_revision")?)?;
    let after_revision = number(take(&mut o, "after_revision")?)?;
    if before_revision.checked_add(1) != Some(after_revision) {
        return Err(bad());
    }
    let r = receipt(take(&mut o, "receipt")?)?;
    let change_count =
        usize::try_from(number(take(&mut o, "change_count")?)?).map_err(|_| bad())?;
    let rows = array(take(&mut o, "deltas")?)?;
    if change_count == 0
        || change_count > limits.max_batch_changes
        || rows.is_empty()
        || rows.len() > limits.max_batch_changes
    {
        return Err(RegistryError::CapacityExceeded);
    }
    let mut deltas = Vec::with_capacity(rows.len());
    let mut previous = None;
    for v in rows {
        let mut d = open(v)?;
        let key = read_key(take(&mut d, "key")?)?;
        let before = optional_decode(take(&mut d, "before")?, |value| {
            read_state(value, profile == "eliot.registry-batch.v3")
        })?;
        let after = read_state(take(&mut d, "after")?, profile == "eliot.registry-batch.v3")?;
        finish(d)?;
        if previous.as_ref().is_some_and(|p| p >= &key)
            || after.key() != key
            || before.as_ref().is_some_and(|v| v.key() != key)
        {
            return Err(bad());
        }
        previous = Some(key.clone());
        deltas.push(RegistryStateDelta { key, before, after });
    }
    finish(o)?;
    let result = DecodedBatch {
        encoding_profile,
        state_encoding_profile,
        entry: RegistryJournalEntry::new(
            operation_id,
            mutation_digest,
            before_revision,
            after_revision,
            JournalEntryKind::BatchApply,
            r,
        ),
        request_fingerprint,
        change_count,
        deltas,
    };
    if encode_record_profile(&result, &profile)? != bytes {
        return Err(bad());
    }
    Ok(result)
}

#[cfg(test)]
mod wire_profile_tests {
    use super::*;

    #[test]
    fn decoded_batch_wire_profile_preserves_exact_v2_and_v3_bytes() {
        for encoding_profile in [BATCH_PROFILE_V2, BATCH_PROFILE_V3] {
            let corpus = OpaqueId::new("profile-retention-fixture").unwrap();
            let record = DecodedBatch {
                encoding_profile,
                state_encoding_profile: if encoding_profile == BATCH_PROFILE_V2 {
                    STATE_PROFILE_V2
                } else {
                    STATE_PROFILE_V3
                },
                entry: RegistryJournalEntry::new(
                    OpaqueId::new("profile-retention-operation").unwrap(),
                    Blake3Digest32::from_stored_bytes([1; 32]),
                    0,
                    1,
                    JournalEntryKind::BatchApply,
                    ReceiptRef::new("profile-retention-receipt").unwrap(),
                ),
                request_fingerprint: Blake3Digest32::from_stored_bytes([2; 32]),
                change_count: 1,
                deltas: vec![RegistryStateDelta {
                    key: RegistryStateKey::Generation(corpus.clone()),
                    before: None,
                    after: RegistryStateRecord::Generation {
                        corpus,
                        generation: NonZeroRevision::new(1).unwrap(),
                    },
                }],
            };
            let bytes = record.encode().unwrap();
            let restored =
                decode_batch_record(&bytes, crate::api::DEFAULT_REGISTRY_LIMITS).unwrap();
            assert_eq!(restored.encoding_profile(), encoding_profile);
            assert_eq!(restored.encode().unwrap(), bytes);
            assert_eq!(restored, record);
            let state = &record.deltas[0].after;
            let key = state.key().encode().unwrap();
            let value = encode_state_profile(state, record.state_encoding_profile).unwrap();
            let (_, actual_profile) =
                super::super::decode_registry_state_entry_profiled(&key, &value).unwrap();
            assert_eq!(actual_profile, record.state_encoding_profile);
            let mut installed =
                super::super::SourceRegistry::new(crate::api::DEFAULT_REGISTRY_LIMITS).unwrap();
            super::super::install(&mut installed, &restored).unwrap();
            assert_eq!(
                super::super::encode_existing_state(&installed, state).unwrap(),
                value,
                "expected-before bytes must retain the actual old format"
            );
            if encoding_profile == BATCH_PROFILE_V2 {
                assert_ne!(
                    value,
                    state.encode().unwrap(),
                    "current encoding would relabel v2"
                );
            }
            let operation_id = &record.entry.operation_id;
            let operations = [crate::recovery::RegistryRestoreOperation {
                operation_id,
                value: &bytes,
            }];
            let states = [crate::recovery::RegistryRestoreState {
                key: &key,
                value: &value,
            }];
            assert_eq!(
                crate::recovery::restore_registry_state(
                    1,
                    &states,
                    &operations,
                    crate::api::DEFAULT_REGISTRY_LIMITS,
                    crate::recovery::MAX_REGISTRY_RESTORE_BYTES
                )
                .unwrap_err(),
                RegistryError::MembershipNotFound,
                "this isolated codec fixture has no corpus members"
            );
            let foreign_profile = if encoding_profile == BATCH_PROFILE_V2 {
                STATE_PROFILE_V3
            } else {
                STATE_PROFILE_V2
            };
            let relabelled = encode_state_profile(state, foreign_profile).unwrap();
            let states = [crate::recovery::RegistryRestoreState {
                key: &key,
                value: &relabelled,
            }];
            assert_eq!(
                crate::recovery::restore_registry_state(
                    1,
                    &states,
                    &operations,
                    crate::api::DEFAULT_REGISTRY_LIMITS,
                    crate::recovery::MAX_REGISTRY_RESTORE_BYTES
                )
                .unwrap_err(),
                RegistryError::SnapshotInvalid,
                "semantic equality cannot relabel stored state without an operation"
            );
        }
    }
}

fn change_value(change: &RegistryChange) -> Result<V, RegistryError> {
    match change {
        RegistryChange::RegisterRoot { request, receipt } => object(vec![
            ("kind", text("register-root")?),
            ("root", bytes(request.root_binding_id.as_bytes())?),
            (
                "canonical_root_digest",
                bytes(request.canonical_root_digest.as_bytes())?,
            ),
            (
                "policy_fingerprint",
                bytes(request.policy_fingerprint.as_bytes())?,
            ),
            ("policy_revision", V::U64(request.policy_revision.get())),
            ("owner_epoch", V::U64(request.owner_epoch.get())),
            (
                "owner_fence_receipt",
                text(request.owner_fence_receipt.as_str())?,
            ),
            ("policy_receipt", text(request.policy_receipt.as_str())?),
            ("receipt", text(receipt.as_str())?),
        ]),
        RegistryChange::UpdateRootPolicy {
            root_binding_id,
            expected_record_revision,
            policy_fingerprint,
            policy_revision,
            receipt,
        } => object(vec![
            ("kind", text("update-root-policy")?),
            ("root", bytes(root_binding_id.as_bytes())?),
            (
                "expected_record_revision",
                V::U64(expected_record_revision.get()),
            ),
            ("policy_fingerprint", bytes(policy_fingerprint.as_bytes())?),
            ("policy_revision", V::U64(policy_revision.get())),
            ("receipt", text(receipt.as_str())?),
        ]),
        RegistryChange::UnbindRoot {
            root_binding_id,
            expected_record_revision,
            receipt,
        } => object(vec![
            ("kind", text("unbind-root")?),
            ("root", bytes(root_binding_id.as_bytes())?),
            (
                "expected_record_revision",
                V::U64(expected_record_revision.get()),
            ),
            ("receipt", text(receipt.as_str())?),
        ]),
        RegistryChange::RegisterSource {
            admission: admitted,
            original_admission,
            binding: bound,
            assignment: proof,
            receipt: reference,
        } => object(vec![
            ("kind", text("register-source")?),
            ("admission", admission(admitted)?),
            (
                "original_admission",
                optional(
                    original_admission
                        .as_ref()
                        .map(|value| bytes(value.encoded())),
                )?,
            ),
            ("binding", binding(bound)?),
            ("assignment", assignment(proof)?),
            ("receipt", text(reference.as_str())?),
        ]),
        RegistryChange::UpdateSourceBinding {
            identity: id,
            expected_source_revision,
            binding: b,
            receipt: r,
        } => object(vec![
            ("kind", text("update-source")?),
            ("identity", identity(id)?),
            ("expected_revision", V::U64(expected_source_revision.get())),
            ("binding", binding(b)?),
            ("receipt", text(r.as_str())?),
        ]),
        RegistryChange::RetireSource {
            identity: id,
            expected_source_revision,
            receipt: r,
        } => object(vec![
            ("kind", text("retire-source")?),
            ("identity", identity(id)?),
            ("expected_revision", V::U64(expected_source_revision.get())),
            ("receipt", text(r.as_str())?),
        ]),
        RegistryChange::AddMembership(v) => object(vec![
            ("kind", text("add-membership")?),
            ("key", member_key(&v.key)?),
            ("generation", V::U64(v.generation.get())),
            ("receipt", text(v.receipt.as_str())?),
        ]),
        RegistryChange::RetireMembership {
            key,
            expected_membership_revision,
            receipt: r,
        } => object(vec![
            ("kind", text("retire-membership")?),
            ("key", member_key(key)?),
            (
                "expected_revision",
                V::U64(expected_membership_revision.get()),
            ),
            ("receipt", text(r.as_str())?),
        ]),
        RegistryChange::CutoverNamespace(v) => object(vec![
            ("kind", text("cutover")?),
            ("corpus", text(v.corpus_id.as_str())?),
            (
                "expected_generation",
                optional(v.expected_generation.map(|n| Ok(V::U64(n.get()))))?,
            ),
            ("next_generation", V::U64(v.next_generation.get())),
            ("inventory", list(v.frozen_inventory.iter().map(identity))?),
            ("inventory_digest", bytes(v.inventory_digest.as_bytes())?),
            ("authorization_verified", V::Bool(v.authorization_verified)),
            ("readback_verified", V::Bool(v.readback_verified)),
            ("receipt", text(v.receipt.as_str())?),
        ]),
    }
}
pub(super) fn request_fingerprint(batch: &RegistryBatch) -> Result<Blake3Digest32, RegistryError> {
    let roots = batch
        .changes
        .iter()
        .any(|change| change.root_target().is_some());
    let value = object(vec![
        (
            "profile",
            text(if roots {
                "registry-batch-input-v3"
            } else {
                "registry-batch-input-v2"
            })?,
        ),
        (
            "expected_revision",
            V::U64(batch.expected_registry_revision),
        ),
        ("operation", text(batch.operation.operation_id().as_str())?),
        (
            "declared_digest",
            bytes(batch.operation.mutation_digest().as_bytes())?,
        ),
        ("changes", list(batch.changes.iter().map(change_value))?),
    ])?;
    c::blake3_canonical(
        &c::CanonicalDigestDomain::parse(if roots {
            "eliot/cbor/registry-batch-input/v3"
        } else {
            "eliot/cbor/registry-batch-input/v2"
        })
        .map_err(|_| bad())?,
        &value,
        c::DigestInputLimit::new(c::MAX_CANONICAL_BYTES).map_err(|_| bad())?,
    )
    .map_err(|_| bad())
}
