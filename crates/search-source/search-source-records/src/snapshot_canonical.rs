//! Canonical digest input for the registry's existing snapshot projection.
use super::{
    REGISTRY_SNAPSHOT_V5_DOMAIN, REGISTRY_SNAPSHOT_V5_HEADER, REGISTRY_SNAPSHOT_V5_SCHEMA,
    REGISTRY_SNAPSHOT_V6_DOMAIN, REGISTRY_SNAPSHOT_V6_HEADER, REGISTRY_SNAPSHOT_V6_SCHEMA,
    RegistrySnapshot, RegistrySnapshotDigest, RegistrySnapshotDigestV5, RegistrySnapshotDigestV6,
};
use crate::error::RegistryError;
use crate::membership::{MembershipKey, MembershipLifecycle, MembershipRecord};
use crate::namespace::{
    CutoverPreparation, NamespaceCutoverState, NamespaceMetadata, SourceOwnerActivationReceipt,
};
use crate::portfolio::ReferencePortfolioRecord;
use crate::provenance::SourceProvenance;
use crate::source::{RegisteredSource, SourceLifecycle};
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, BoundedMap, CanonicalDigestDomain, CanonicalKey,
    CanonicalText, CanonicalValue as V, ContractError, DigestInputLimit, MAX_CANONICAL_BYTES,
    MAX_COLLECTION_ITEMS, NamespaceOwnershipStatus, SourceIdentity, SourceIdentityKind,
    SourceNamespaceOwnership, blake3_canonical_array, blake3_raw,
};
use search_source_identity::{
    CaseBehavior, FilesystemIdentityProfile, LinkBehavior, ReparseBehavior, StableFieldPolicy,
    UnicodeBehavior,
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

fn source_v5(value: &RegisteredSource) -> Result<V, ContractError> {
    let binding = value.binding();
    let provenance = match value.provenance() {
        SourceProvenance::HistoricalV1 {
            admission,
            original_admission,
            assignment,
        } => object(vec![
            ("kind", text("historical-v1")?),
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
                "original_admission",
                original_admission
                    .as_ref()
                    .map(|envelope| bytes(envelope.encoded()))
                    .transpose()?
                    .unwrap_or(V::Null),
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
        ])?,
        SourceProvenance::PermanentV2(permanent) => object(vec![
            ("kind", text("permanent-v2")?),
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
                "policy_inputs_digest",
                metadata_digest(
                    "eliot/raw/registry-retained-policy/v2",
                    permanent.original().policy().encoded_inputs(),
                )?,
            ),
            (
                "admission_inputs_digest",
                metadata_digest(
                    "eliot/raw/registry-original-admission/v2",
                    permanent.original().encoded_source_evidence(),
                )?,
            ),
        ])?,
    };
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
                        .map(|digest| bytes(digest.as_bytes()))
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
        ("provenance", provenance),
        (
            "lifecycle",
            text(match value.lifecycle() {
                SourceLifecycle::Active => "active",
                SourceLifecycle::Retired => "retired",
            })?,
        ),
        ("source_revision", V::U64(value.source_revision().get())),
        ("registry_revision", V::U64(value.registry_revision())),
        ("receipt", text(value.last_receipt().as_str())?),
    ])
}

fn profile_v5(value: FilesystemIdentityProfile) -> Result<V, ContractError> {
    object(vec![
        ("revision", V::U64(value.revision().get())),
        ("schema_digest", bytes(value.schema_digest().as_bytes())?),
        (
            "case_behavior",
            text(match value.case_behavior() {
                CaseBehavior::Sensitive => "sensitive",
                CaseBehavior::InsensitiveAscii => "insensitive_ascii",
                CaseBehavior::PreNormalizedInsensitive => "pre_normalized_insensitive",
                CaseBehavior::Unsupported => "unsupported",
            })?,
        ),
        (
            "unicode_behavior",
            text(match value.unicode_behavior() {
                UnicodeBehavior::PreserveScalarValues => "preserve_scalar_values",
                UnicodeBehavior::PreNormalizedNfc => "pre_normalized_nfc",
                UnicodeBehavior::PreNormalizedNfd => "pre_normalized_nfd",
                UnicodeBehavior::Unsupported => "unsupported",
            })?,
        ),
        (
            "volume_identity",
            text(match value.volume_identity() {
                StableFieldPolicy::Required => "required",
                StableFieldPolicy::OptionalExplicitUnavailable => "optional_explicit_unavailable",
                StableFieldPolicy::Unsupported => "unsupported",
            })?,
        ),
        (
            "file_identity",
            text(match value.file_identity() {
                StableFieldPolicy::Required => "required",
                StableFieldPolicy::OptionalExplicitUnavailable => "optional_explicit_unavailable",
                StableFieldPolicy::Unsupported => "unsupported",
            })?,
        ),
        (
            "link_behavior",
            text(match value.link_behavior() {
                LinkBehavior::StablePhysicalIdentity => "stable_physical_identity",
                LinkBehavior::NoStableLinkIdentity => "no_stable_link_identity",
                LinkBehavior::Unsupported => "unsupported",
            })?,
        ),
        (
            "reparse_behavior",
            text(match value.reparse_behavior() {
                ReparseBehavior::FinalTargetIdentity => "final_target_identity",
                ReparseBehavior::Rejected => "rejected",
                ReparseBehavior::Unsupported => "unsupported",
            })?,
        ),
    ])
}

fn preparation_v5(value: &CutoverPreparation) -> Result<V, ContractError> {
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

fn activation_v5(value: &SourceOwnerActivationReceipt) -> Result<V, ContractError> {
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

fn namespace_metadata_v5(metadata: &NamespaceMetadata) -> Result<V, ContractError> {
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
            ("identity_profile", profile_v5(metadata.identity_profile())?),
            ("policy_reference", bytes(metadata.policy_reference())?),
            (
                "policy_inputs_digest",
                metadata_digest(
                    "eliot/raw/registry-retained-policy/v2",
                    metadata.policy().encoded_inputs(),
                )?,
            ),
            ("registry_revision", V::U64(metadata.registry_revision())),
            ("last_receipt", text(metadata.last_receipt().as_str())?),
        ]),
    }
}

fn namespace_v5(value: &NamespaceCutoverState) -> Result<V, ContractError> {
    object(vec![
        ("ownership", ownership(&value.ownership)?),
        (
            "preparation",
            value
                .preparation
                .as_ref()
                .map(preparation_v5)
                .transpose()?
                .unwrap_or(V::Null),
        ),
        ("fenced", V::Bool(value.fenced)),
        (
            "activation",
            value
                .activation
                .as_ref()
                .map(activation_v5)
                .transpose()?
                .unwrap_or(V::Null),
        ),
        ("metadata", namespace_metadata_v5(value.metadata())?),
    ])
}

pub(super) fn digest_v5(
    snapshot: &RegistrySnapshot,
) -> Result<RegistrySnapshotDigestV5, RegistryError> {
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
        snapshot.namespace_states.len(),
    ];
    let count = counts
        .iter()
        .try_fold(1_usize, |sum, n| sum.checked_add(*n))
        .ok_or(RegistryError::CapacityExceeded)?;
    if count > MAX_COLLECTION_ITEMS {
        return Err(RegistryError::CapacityExceeded);
    }
    let header = object(vec![
        (
            "header",
            text(REGISTRY_SNAPSHOT_V5_HEADER).map_err(|_| RegistryError::SnapshotInvalid)?,
        ),
        ("schema", V::U64(REGISTRY_SNAPSHOT_V5_SCHEMA)),
        ("revision", V::U64(snapshot.revision)),
        (
            "family_counts",
            list(counts.into_iter().map(|n| Ok(V::U64(n as u64))))
                .map_err(|_| RegistryError::SnapshotInvalid)?,
        ),
    ]);
    let roots = snapshot.roots.iter().map(|(key, value)| {
        row(
            "root",
            bytes(key.as_bytes())?,
            object(vec![
                ("root_binding", bytes(value.root_binding_id.as_bytes())?),
                (
                    "canonical_root",
                    bytes(value.canonical_root_digest.as_bytes())?,
                ),
                (
                    "policy_fingerprint",
                    bytes(value.policy_fingerprint.as_bytes())?,
                ),
                ("policy_revision", V::U64(value.policy_revision.get())),
                ("owner_epoch", V::U64(value.owner_epoch.get())),
                ("record_revision", V::U64(value.record_revision.get())),
                ("registry_revision", V::U64(value.registry_revision)),
                (
                    "status",
                    text(match value.status {
                        crate::root::RootStatus::Active => "active",
                        crate::root::RootStatus::Unbound => "unbound",
                    })?,
                ),
                ("receipt", text(value.last_receipt.as_str())?),
            ])?,
        )
    });
    let sources = snapshot
        .sources
        .iter()
        .map(|(key, value)| row("source", identity(key)?, source_v5(value)?));
    let memberships = snapshot
        .memberships
        .iter()
        .map(|(key, value)| row("membership", membership_key(key)?, membership(value)?));
    let portfolios = snapshot.portfolios.iter().map(|(key, value)| {
        row(
            "current-portfolio",
            bytes(key.as_bytes())?,
            portfolio(value)?,
        )
    });
    let revisions = history.revisions().map(|value| {
        row(
            "portfolio-history",
            object(vec![
                ("id", bytes(value.portfolio_id().as_bytes())?),
                ("revision", V::U64(value.portfolio_revision().get())),
            ])?,
            object(vec![
                (
                    "precedence",
                    list(
                        value
                            .membership_precedence()
                            .iter()
                            .map(|id| bytes(id.as_bytes())),
                    )?,
                ),
                ("registry_revision", V::U64(value.registry_revision())),
                ("receipt", text(value.last_receipt().as_str())?),
                (
                    "publication_operation",
                    text(value.operation_id().as_str())?,
                ),
            ])?,
        )
    });
    let pointers = history.current_pointers().map(|(key, value)| {
        row(
            "portfolio-pointer",
            bytes(key.as_bytes())?,
            object(vec![
                ("target_id", bytes(value.target().portfolio_id.as_bytes())?),
                (
                    "target_revision",
                    V::U64(value.target().portfolio_revision.get()),
                ),
                ("pointer_revision", V::U64(value.pointer_revision().get())),
                ("operation_id", text(value.operation_id().as_str())?),
                ("receipt", text(value.last_receipt().as_str())?),
            ])?,
        )
    });
    let generations = snapshot.active_generations.iter().map(|(key, value)| {
        row(
            "active-generation",
            text(key.as_str())?,
            V::U64(value.get()),
        )
    });
    let ownerships = snapshot.ownerships.iter().map(|(key, value)| {
        row(
            "namespace-ownership",
            bytes(key.as_bytes())?,
            ownership(value)?,
        )
    });
    let namespaces = snapshot.namespace_states.iter().map(|(key, value)| {
        row(
            "namespace-state",
            bytes(key.as_bytes())?,
            namespace_v5(value)?,
        )
    });
    let rows = std::iter::once(header)
        .chain(roots)
        .chain(sources)
        .chain(memberships)
        .chain(portfolios)
        .chain(revisions)
        .chain(pointers)
        .chain(generations)
        .chain(ownerships)
        .chain(namespaces);
    let domain = CanonicalDigestDomain::parse(REGISTRY_SNAPSHOT_V5_DOMAIN)
        .map_err(|_| RegistryError::SnapshotInvalid)?;
    let value = blake3_canonical_array(
        &domain,
        count,
        rows,
        limit().map_err(|_| RegistryError::SnapshotInvalid)?,
    )
    .map_err(|_| RegistryError::SnapshotInvalid)?;
    Ok(RegistrySnapshotDigestV5(value))
}

pub(super) fn digest_v6(
    snapshot: &RegistrySnapshot,
) -> Result<RegistrySnapshotDigestV6, RegistryError> {
    let base = digest_v5(snapshot)?.0;
    let permanent_memberships: Vec<_> = snapshot
        .memberships
        .iter()
        .filter_map(|(key, record)| record.permanent_v2().map(|value| (key, value)))
        .collect();
    if permanent_memberships.len() > MAX_COLLECTION_ITEMS {
        return Err(RegistryError::CapacityExceeded);
    }
    let rows = permanent_memberships.iter().map(|(key, value)| {
        let member = value.member();
        let provenance = value.provenance();
        row(
            "permanent-membership-v2",
            membership_key(key)?,
            object(vec![
                (
                    "schema",
                    V::U64(crate::membership::PERMANENT_MEMBERSHIP_RECORD_V2_SCHEMA),
                ),
                (
                    "source_membership_id",
                    bytes(member.source_membership_id.as_bytes())?,
                ),
                ("corpus_id", bytes(member.corpus_id.as_bytes())?),
                ("source_id", bytes(member.source_id.as_bytes())?),
                ("workspace_id", bytes(member.workspace_id.as_bytes())?),
                (
                    "role",
                    text(match member.role {
                        search_contracts::MembershipRole::Source => "source",
                        search_contracts::MembershipRole::Test => "test",
                        search_contracts::MembershipRole::Documentation => "documentation",
                        search_contracts::MembershipRole::Generated => "generated",
                        search_contracts::MembershipRole::Vendor => "vendor",
                        search_contracts::MembershipRole::Reference => "reference",
                    })?,
                ),
                (
                    "preparation_profile_id",
                    text(member.preparation_profile_id.as_str())?,
                ),
                (
                    "access_policy_binding_id",
                    bytes(member.access_policy_binding_id.as_bytes())?,
                ),
                (
                    "retention_policy_id",
                    text(member.retention_policy_id.as_str())?,
                ),
                (
                    "residency_policy_binding_id",
                    bytes(member.residency_policy_binding_id.as_bytes())?,
                ),
                (
                    "membership_revision",
                    V::U64(member.membership_revision.get()),
                ),
                ("source_identity", identity(provenance.source_identity())?),
                (
                    "source_revision",
                    V::U64(provenance.source_revision().get()),
                ),
                (
                    "root_binding_id",
                    bytes(provenance.root_binding_id().as_bytes())?,
                ),
                (
                    "root_record_revision",
                    V::U64(provenance.root_record_revision().get()),
                ),
                (
                    "source_namespace_id",
                    bytes(provenance.source_namespace_id().as_bytes())?,
                ),
                (
                    "namespace_record_revision",
                    V::U64(provenance.namespace_record_revision().get()),
                ),
                (
                    "owner_generation",
                    bytes(provenance.owner_generation().as_bytes())?,
                ),
                ("policy_reference", bytes(provenance.policy_reference())?),
                (
                    "admission_reference",
                    bytes(provenance.admission_reference())?,
                ),
                (
                    "policy_revision",
                    V::U64(provenance.policy_revision().get()),
                ),
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
                ("registry_revision", V::U64(value.registry_revision())),
                ("receipt", text(value.last_receipt().as_str())?),
            ])?,
        )
    });
    let count = permanent_memberships
        .len()
        .checked_add(1)
        .ok_or(RegistryError::CapacityExceeded)?;
    let header = object(vec![
        (
            "header",
            text(REGISTRY_SNAPSHOT_V6_HEADER).map_err(|_| RegistryError::SnapshotInvalid)?,
        ),
        ("schema", V::U64(REGISTRY_SNAPSHOT_V6_SCHEMA)),
        ("revision", V::U64(snapshot.revision)),
        (
            "base_snapshot_digest_v5",
            bytes(base.as_bytes()).map_err(|_| RegistryError::SnapshotInvalid)?,
        ),
        (
            "permanent_membership_count",
            V::U64(permanent_memberships.len() as u64),
        ),
    ])
    .map_err(|_| RegistryError::SnapshotInvalid)?;
    let rows = std::iter::once(Ok(header)).chain(rows);
    let domain = CanonicalDigestDomain::parse(REGISTRY_SNAPSHOT_V6_DOMAIN)
        .map_err(|_| RegistryError::SnapshotInvalid)?;
    let digest = blake3_canonical_array(
        &domain,
        count,
        rows,
        limit().map_err(|_| RegistryError::SnapshotInvalid)?,
    )
    .map_err(|_| RegistryError::SnapshotInvalid)?;
    Ok(RegistrySnapshotDigestV6(digest))
}
