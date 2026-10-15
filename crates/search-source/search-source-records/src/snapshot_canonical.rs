//! Canonical digest input for the registry's existing snapshot projection.
use super::{RegistrySnapshot, RegistrySnapshotDigest};
use crate::error::RegistryError;
use crate::membership::{MembershipKey, MembershipLifecycle, MembershipRecord};
use crate::portfolio::ReferencePortfolioRecord;
use crate::source::{RegisteredSource, SourceLifecycle};
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, BoundedMap, CanonicalDigestDomain, CanonicalKey,
    CanonicalText, CanonicalValue as V, ContractError, DigestInputLimit, MAX_CANONICAL_BYTES,
    MAX_COLLECTION_ITEMS, NamespaceOwnershipStatus, SourceIdentity, SourceIdentityKind,
    SourceNamespaceOwnership, blake3_canonical_array, blake3_raw,
};

fn object(entries: Vec<(&str, V)>) -> Result<V, ContractError> {
    BoundedMap::from_entries(
        entries
            .into_iter()
            .map(|(key, value)| CanonicalKey::new_non_empty(key).map(|key| (key, value)))
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map(V::Object)
}
fn text(value: &str) -> Result<V, ContractError> {
    CanonicalText::new(value).map(V::Text)
}
fn bytes(value: &[u8]) -> Result<V, ContractError> {
    BoundedBytes::new(value.to_vec()).map(V::Bytes)
}
fn list(rows: impl Iterator<Item = Result<V, ContractError>>) -> Result<V, ContractError> {
    BoundedList::new(rows.collect::<Result<Vec<_>, _>>()?).map(V::Array)
}
fn limit() -> Result<DigestInputLimit, ContractError> {
    DigestInputLimit::new(MAX_CANONICAL_BYTES)
}
fn metadata_digest(domain: &str, value: &[u8]) -> Result<V, ContractError> {
    let digest = blake3_raw(&CanonicalDigestDomain::parse(domain)?, value, limit()?)?;
    bytes(digest.as_bytes())
}
fn identity(value: &SourceIdentity) -> Result<V, ContractError> {
    object(vec![
        ("namespace", bytes(value.source_namespace_id.as_bytes())?),
        ("source", bytes(value.source_id.as_bytes())?),
        (
            "kind",
            text(match value.identity_kind {
                SourceIdentityKind::NtfsFile => "ntfs-file",
                SourceIdentityKind::GitBlobLineage => "git-blob-lineage",
                SourceIdentityKind::ImportedObject => "imported-object",
                SourceIdentityKind::AdmittedVirtualSnapshot => "admitted-virtual-snapshot",
            })?,
        ),
        (
            "components",
            metadata_digest(
                "eliot/raw/registry-source-identity/v1",
                value.stable_identity_components.as_slice(),
            )?,
        ),
    ])
}
fn source(value: &RegisteredSource) -> Result<V, ContractError> {
    let binding = value.binding();
    let admission = value
        .legacy_admission()
        .ok_or_else(|| ContractError::unsupported_version("source_provenance"))?;
    let assignment = value
        .legacy_assignment()
        .ok_or_else(|| ContractError::unsupported_version("source_provenance"))?;
    object(vec![
        ("identity", identity(value.identity())?),
        (
            "binding",
            object(vec![
                ("root", bytes(binding.root_binding_id().as_bytes())?),
                (
                    "relative_path_digest",
                    metadata_digest(
                        "eliot/raw/registry-path-binding/v1",
                        binding.relative_path().as_str().as_bytes(),
                    )?,
                ),
                (
                    "file_identity",
                    binding
                        .stable_file_identity_digest()
                        .map(|d| bytes(d.as_bytes()))
                        .transpose()?
                        .unwrap_or(V::Null),
                ),
                (
                    "content_digest",
                    bytes(binding.content_digest().as_bytes())?,
                ),
                ("content_bytes", V::U64(binding.content_bytes())),
                ("binding_revision", V::U64(binding.binding_revision().get())),
                (
                    "observation_revision",
                    V::U64(binding.observation_revision().get()),
                ),
                (
                    "observation_receipt",
                    text(binding.observation_receipt().as_str())?,
                ),
            ])?,
        ),
        (
            "admission",
            object(vec![
                ("schema", V::U64(u64::from(admission.schema_version()))),
                ("policy_revision", V::U64(admission.policy_revision().get())),
                (
                    "policy_fingerprint",
                    bytes(admission.policy_fingerprint().as_bytes())?,
                ),
                (
                    "observation_digest",
                    bytes(admission.observation_digest().as_bytes())?,
                ),
                ("outcome", text(admission.outcome().as_str())?),
                (
                    "reasons",
                    list(admission.reason_codes().map(|v| text(v.as_str())))?,
                ),
                (
                    "matched_rules",
                    list(admission.matched_rule_ids().map(|v| text(v.as_str())))?,
                ),
                ("sensitivity", text(admission.sensitivity().as_str())?),
                ("receipt_digest", bytes(admission.receipt_digest())?),
            ])?,
        ),
        (
            "assignment",
            object(vec![
                ("identity", identity(&assignment.source_identity)?),
                (
                    "observation_digest",
                    bytes(assignment.observation_digest.as_bytes())?,
                ),
                (
                    "assignment_digest",
                    bytes(assignment.assignment_digest.as_bytes())?,
                ),
                ("receipt", text(assignment.assignment_receipt.as_str())?),
                ("readback_verified", V::Bool(assignment.readback_verified)),
            ])?,
        ),
        (
            "lifecycle",
            text(match value.lifecycle() {
                SourceLifecycle::Active => "active",
                SourceLifecycle::Retired => "retired",
            })?,
        ),
        ("source_revision", V::U64(value.source_revision().get())),
        (
            "original_admission",
            value
                .original_admission()
                .map(|envelope| bytes(envelope.encoded()))
                .transpose()?
                .unwrap_or(V::Null),
        ),
        ("registry_revision", V::U64(value.registry_revision())),
        ("receipt", text(value.last_receipt().as_str())?),
    ])
}
fn membership_key(value: &MembershipKey) -> Result<V, ContractError> {
    object(vec![
        ("corpus", text(value.corpus_id.as_str())?),
        ("source", identity(&value.source_identity)?),
    ])
}
fn membership(value: &MembershipRecord) -> Result<V, ContractError> {
    object(vec![
        ("key", membership_key(value.key())?),
        ("membership_id", bytes(value.membership_id().as_bytes())?),
        ("generation", V::U64(value.generation().get())),
        ("revision", V::U64(value.membership_revision().get())),
        ("registry_revision", V::U64(value.registry_revision())),
        (
            "lifecycle",
            text(match value.lifecycle() {
                MembershipLifecycle::Active => "active",
                MembershipLifecycle::Restricted => "restricted",
                MembershipLifecycle::Suspended => "suspended",
                MembershipLifecycle::Retired => "retired",
                MembershipLifecycle::Removed => "removed",
            })?,
        ),
        ("receipt", text(value.last_receipt().as_str())?),
    ])
}
fn portfolio(value: &ReferencePortfolioRecord) -> Result<V, ContractError> {
    object(vec![
        ("portfolio_id", bytes(value.portfolio_id.as_bytes())?),
        ("portfolio_revision", V::U64(value.portfolio_revision.get())),
        (
            "precedence",
            list(
                value
                    .membership_precedence
                    .iter()
                    .map(|v| bytes(v.as_bytes())),
            )?,
        ),
        ("registry_revision", V::U64(value.registry_revision)),
        ("receipt", text(value.last_receipt.as_str())?),
    ])
}
fn ownership(value: &SourceNamespaceOwnership) -> Result<V, ContractError> {
    object(vec![
        ("namespace", bytes(value.source_namespace_id.as_bytes())?),
        ("owner_system", text(value.owner_system_id.as_str())?),
        (
            "owner_incarnation",
            bytes(value.owner_installation_incarnation_id.as_bytes())?,
        ),
        ("owner_epoch", V::U64(value.owner_epoch.get())),
        (
            "ownership_revision",
            V::U64(value.ownership_record_revision.get()),
        ),
        (
            "owner_generation",
            bytes(value.source_owner_generation.as_bytes())?,
        ),
        (
            "policy_revision",
            V::U64(value.source_admission_policy_revision.get()),
        ),
        (
            "status",
            text(match value.status {
                NamespaceOwnershipStatus::Active => "active",
                NamespaceOwnershipStatus::CutoverPrepared => "cutover-prepared",
                NamespaceOwnershipStatus::Fenced => "fenced",
                NamespaceOwnershipStatus::Retired => "retired",
            })?,
        ),
        (
            "cutover_receipt",
            value
                .cutover_receipt_ref
                .as_ref()
                .map(|v| text(v.as_str()))
                .transpose()?
                .unwrap_or(V::Null),
        ),
    ])
}
fn row(kind: &str, key: V, value: V) -> Result<V, ContractError> {
    object(vec![("kind", text(kind)?), ("key", key), ("value", value)])
}

pub(super) fn digest(snapshot: &RegistrySnapshot) -> Result<RegistrySnapshotDigest, RegistryError> {
    let history = &snapshot.portfolio_history;
    let counts = [
        snapshot.roots.len(),
        snapshot.sources.len(),
        snapshot.memberships.len(),
        snapshot.portfolios.len(),
        history.len(),
        history.current_pointers().count(),
        snapshot.active_generations.len(),
        snapshot.ownerships.len(),
    ];
    let count = counts
        .iter()
        .try_fold(1_usize, |sum, n| sum.checked_add(*n))
        .ok_or(RegistryError::CapacityExceeded)?;
    // The shared encoder refuses before producing a partial digest.
    if count > MAX_COLLECTION_ITEMS {
        return Err(RegistryError::CapacityExceeded);
    }
    let header = object(vec![
        (
            "kind",
            text("registry-snapshot-v4").map_err(|_| RegistryError::SnapshotInvalid)?,
        ),
        ("revision", V::U64(snapshot.revision)),
        (
            "family_counts",
            list(counts.into_iter().map(|n| Ok(V::U64(n as u64))))
                .map_err(|_| RegistryError::SnapshotInvalid)?,
        ),
    ]);
    let roots = snapshot.roots.iter().map(|(key, v)| {
        row(
            "root",
            bytes(key.as_bytes())?,
            object(vec![
                ("root_binding", bytes(v.root_binding_id.as_bytes())?),
                ("canonical_root", bytes(v.canonical_root_digest.as_bytes())?),
                (
                    "policy_fingerprint",
                    bytes(v.policy_fingerprint.as_bytes())?,
                ),
                ("policy_revision", V::U64(v.policy_revision.get())),
                ("owner_epoch", V::U64(v.owner_epoch.get())),
                ("record_revision", V::U64(v.record_revision.get())),
                ("registry_revision", V::U64(v.registry_revision)),
                (
                    "status",
                    text(match v.status {
                        crate::root::RootStatus::Active => "active",
                        crate::root::RootStatus::Unbound => "unbound",
                    })?,
                ),
                ("receipt", text(v.last_receipt.as_str())?),
            ])?,
        )
    });
    let sources = snapshot
        .sources
        .iter()
        .map(|(key, v)| row("source", identity(key)?, source(v)?));
    let memberships = snapshot
        .memberships
        .iter()
        .map(|(key, v)| row("membership", membership_key(key)?, membership(v)?));
    let portfolios = snapshot
        .portfolios
        .iter()
        .map(|(key, v)| row("current-portfolio", bytes(key.as_bytes())?, portfolio(v)?));
    let revisions = history.revisions().map(|v| {
        row(
            "portfolio-history",
            object(vec![
                ("id", bytes(v.portfolio_id().as_bytes())?),
                ("revision", V::U64(v.portfolio_revision().get())),
            ])?,
            object(vec![
                (
                    "precedence",
                    list(
                        v.membership_precedence()
                            .iter()
                            .map(|id| bytes(id.as_bytes())),
                    )?,
                ),
                ("registry_revision", V::U64(v.registry_revision())),
                ("receipt", text(v.last_receipt().as_str())?),
                ("publication_operation", text(v.operation_id().as_str())?),
            ])?,
        )
    });
    let pointers = history.current_pointers().map(|(key, v)| {
        row(
            "portfolio-pointer",
            bytes(key.as_bytes())?,
            object(vec![
                ("target_id", bytes(v.target().portfolio_id.as_bytes())?),
                (
                    "target_revision",
                    V::U64(v.target().portfolio_revision.get()),
                ),
                ("pointer_revision", V::U64(v.pointer_revision().get())),
                ("operation_id", text(v.operation_id().as_str())?),
                ("receipt", text(v.last_receipt().as_str())?),
            ])?,
        )
    });
    let generations = snapshot
        .active_generations
        .iter()
        .map(|(key, v)| row("active-generation", text(key.as_str())?, V::U64(v.get())));
    let ownerships = snapshot
        .ownerships
        .iter()
        .map(|(key, v)| row("namespace-ownership", bytes(key.as_bytes())?, ownership(v)?));
    let rows = std::iter::once(header)
        .chain(roots)
        .chain(sources)
        .chain(memberships)
        .chain(portfolios)
        .chain(revisions)
        .chain(pointers)
        .chain(generations)
        .chain(ownerships);
    let domain = CanonicalDigestDomain::parse("eliot/cbor/registry-snapshot/v4")
        .map_err(|_| RegistryError::SnapshotInvalid)?;
    let value: Blake3Digest32 = blake3_canonical_array(
        &domain,
        count,
        rows,
        limit().map_err(|_| RegistryError::SnapshotInvalid)?,
    )
    .map_err(|_| RegistryError::SnapshotInvalid)?;
    Ok(RegistrySnapshotDigest(value))
}
