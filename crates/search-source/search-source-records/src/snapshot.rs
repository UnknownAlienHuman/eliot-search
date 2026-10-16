//! Immutable registry snapshots, validation and canonical digests.
//!
//! Snapshots are the exact inputs to view resolution and cutover
//! verification. Validation fails closed on missing, duplicated or incoherent
//! records; digests cover technical state and immutable refs only.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, OpaqueId, ReferencePortfolioId, RootBindingId, SourceIdentity,
    SourceNamespaceId, SourceNamespaceOwnership,
};

use crate::error::{RegistryError, RegistryLimits};
use crate::membership::{MembershipKey, MembershipLifecycle, MembershipRecord};
use crate::namespace::NamespaceCutoverState;
use crate::portfolio::ReferencePortfolioRecord;
use crate::portfolio_history::PortfolioHistory;
use crate::root::RootRecord;
use crate::source::{RegisteredSource, SourceLifecycle};
#[path = "snapshot_canonical.rs"]
mod canonical;

/// Stable schema header for the full permanent-v2 snapshot digest.
pub const REGISTRY_SNAPSHOT_V5_HEADER: &str = "registry-snapshot-v5";
/// Numeric schema version carried by the full permanent-v2 snapshot header.
pub const REGISTRY_SNAPSHOT_V5_SCHEMA: u64 = 5;
/// Domain-separated digest domain for the full permanent-v2 snapshot.
pub const REGISTRY_SNAPSHOT_V5_DOMAIN: &str = "eliot/cbor/registry-snapshot/v5";
/// Stable schema header for full snapshots containing permanent membership V2.
pub const REGISTRY_SNAPSHOT_V6_HEADER: &str = "registry-snapshot-v6";
/// Numeric schema version for full snapshots containing permanent membership V2.
pub const REGISTRY_SNAPSHOT_V6_SCHEMA: u64 = 6;
/// Domain-separated digest domain for full membership V2 snapshots.
pub const REGISTRY_SNAPSHOT_V6_DOMAIN: &str = "eliot/cbor/registry-snapshot/v6";

/// Immutable registry snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistrySnapshot {
    /// Exact registry revision captured by the snapshot.
    pub revision: u64,
    /// Admitted roots.
    pub roots: BTreeMap<RootBindingId, RootRecord>,
    /// Admitted sources.
    pub sources: BTreeMap<SourceIdentity, RegisteredSource>,
    /// Source/corpus memberships.
    pub memberships: BTreeMap<MembershipKey, MembershipRecord>,
    /// Reference portfolios.
    pub portfolios: BTreeMap<ReferencePortfolioId, ReferencePortfolioRecord>,
    /// Immutable portfolio revisions and the guarded current pointer.
    ///
    /// Every retained revision stays readable after any pointer move, so an
    /// old query or handle keeps its exact interpretation.
    pub portfolio_history: PortfolioHistory,
    /// Active corpus generations.
    pub active_generations: BTreeMap<OpaqueId, NonZeroRevisionAlias>,
    /// Namespace ownership records.
    pub ownerships: BTreeMap<SourceNamespaceId, SourceNamespaceOwnership>,
    /// Read-only full namespace-owner projections from the same registry map.
    ///
    /// This is deliberately a snapshot projection rather than a second mutable
    /// namespace owner. The live registry retains one authoritative namespace
    /// map and this map is copied from it at snapshot capture time.
    pub namespace_states: BTreeMap<SourceNamespaceId, NamespaceCutoverState>,
}

/// Alias preserving the exact [`search_contracts::NonZeroRevision`] shape
/// without leaking a second revision newtype.
pub type NonZeroRevisionAlias = search_contracts::NonZeroRevision;

/// Validated immutable registry snapshot.
///
/// Coherent view metadata is not a full-state restoration or durability proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedRegistrySnapshot {
    inner: RegistrySnapshot,
    original_admission_availability: OriginalAdmissionAvailability,
}

/// Availability of original source-admission inputs within a coherent snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginalAdmissionAvailability {
    /// Every source retains its matching owner-produced original-input envelope.
    RetainedOriginalInputs,
    /// At least one legacy source lacks inputs; receipt/digest metadata cannot restore them.
    LegacyInputsUnavailable,
}

impl ValidatedRegistrySnapshot {
    /// Explicit admission-input availability, independent of snapshot digest equality.
    ///
    /// Retained inputs do not prove namespace full-state durability, live policy,
    /// native root custody or current grants.
    pub const fn original_admission_availability(&self) -> OriginalAdmissionAvailability {
        self.original_admission_availability
    }

    /// Refuses use of a coherent legacy snapshot as admission-restoration evidence.
    pub fn require_original_admission_inputs(&self) -> Result<(), RegistryError> {
        match self.original_admission_availability {
            OriginalAdmissionAvailability::RetainedOriginalInputs => Ok(()),
            OriginalAdmissionAvailability::LegacyInputsUnavailable => {
                Err(RegistryError::AdmissionRestorationUnavailable)
            }
        }
    }
    /// Exact validated snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &RegistrySnapshot {
        &self.inner
    }

    /// Exact registry revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.inner.revision
    }
}

/// Domain-separated canonical digest over technical registry state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegistrySnapshotDigest(pub Blake3Digest32);

/// Domain-separated canonical digest over the full permanent-v2 snapshot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegistrySnapshotDigestV5(pub Blake3Digest32);

/// Domain-separated canonical digest over complete permanent-membership-v2 state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegistrySnapshotDigestV6(pub Blake3Digest32);

/// Validates registry/root/source/membership/portfolio/owner generations,
/// uniqueness, referential integrity, closed states and bounded counts.
pub fn validate_registry_snapshot(
    snapshot: &RegistrySnapshot,
    limits: RegistryLimits,
) -> Result<ValidatedRegistrySnapshot, RegistryError> {
    limits.validate()?;
    if snapshot.roots.len() > limits.max_roots
        || snapshot.sources.len() > limits.max_sources
        || snapshot.memberships.len() > limits.max_memberships
        || snapshot.portfolios.len() > limits.max_portfolios
        || snapshot.ownerships.len() > limits.max_namespaces
    {
        return Err(RegistryError::CapacityExceeded);
    }
    let mut root_identities = BTreeSet::new();
    for (id, root) in &snapshot.roots {
        crate::root::validate_metadata(root)?;
        if root.registry_revision > snapshot.revision
            || !root_identities.insert(root.canonical_root_digest)
        {
            return Err(RegistryError::SnapshotInvalid);
        }
        if &root.root_binding_id != id {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    for (identity, source) in &snapshot.sources {
        if source
            .original_admission()
            .is_some_and(|original| Some(original.receipt()) != source.legacy_admission())
        {
            return Err(RegistryError::SnapshotInvalid);
        }
        if let Some(permanent) = source.permanent_admission() {
            permanent.validate_binding(source.binding())?;
        }
        if source.identity() != identity {
            return Err(RegistryError::SnapshotInvalid);
        }
        let root_id = source.binding().root_binding_id();
        if !snapshot.roots.contains_key(&root_id) {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    let mut reverse_ids = BTreeSet::new();
    for (key, membership) in &snapshot.memberships {
        if membership.key() != key {
            return Err(RegistryError::SnapshotInvalid);
        }
        if !reverse_ids.insert(membership.membership_id()) {
            return Err(RegistryError::SnapshotInvalid);
        }
        let source = snapshot
            .sources
            .get(&key.source_identity)
            .ok_or(RegistryError::SnapshotInvalid)?;
        if let Some(permanent) = membership.permanent_v2() {
            let member = permanent.member();
            let provenance = permanent.provenance();
            let canonical_corpus_id = member.corpus_id.to_string();
            let admission = source
                .permanent_admission()
                .ok_or(RegistryError::SnapshotInvalid)?;
            if member.source_membership_id != membership.membership_id()
                || member.source_id != key.source_identity.source_id
                || member.membership_revision.get() != membership.membership_revision().get()
                || provenance.source_revision() > source.source_revision()
                || permanent.lifecycle() != membership.lifecycle()
                || permanent.registry_revision() != membership.registry_revision()
                || permanent.last_receipt() != membership.last_receipt()
                || canonical_corpus_id != key.corpus_id.as_str()
                || provenance.source_identity() != &key.source_identity
                || provenance.root_binding_id() != source.binding().root_binding_id()
                || provenance.policy_reference() != admission.policy_reference()
                || provenance.admission_reference() != admission.admission_reference()
                || provenance.root_record_revision() != admission.root_record_revision()
                || provenance.namespace_record_revision() != admission.namespace_record_revision()
                || provenance.owner_generation() != admission.owner_generation()
            {
                return Err(RegistryError::SnapshotInvalid);
            }
        }
        if source.lifecycle() == SourceLifecycle::Retired
            && membership.lifecycle() == MembershipLifecycle::Active
        {
            return Err(RegistryError::SnapshotInvalid);
        }
        if membership.is_permanent_v2() {
            continue;
        }
        if let Some(active) = snapshot.active_generations.get(&key.corpus_id) {
            if membership.lifecycle() == MembershipLifecycle::Active
                && &membership.generation() != active
            {
                return Err(RegistryError::SnapshotInvalid);
            }
        } else if membership.lifecycle() == MembershipLifecycle::Active {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    for record in snapshot.portfolios.values() {
        if record.membership_precedence.len() > limits.max_portfolio_items {
            return Err(RegistryError::SnapshotInvalid);
        }
        let mut seen = BTreeSet::new();
        for membership_id in &record.membership_precedence {
            if !seen.insert(*membership_id) {
                return Err(RegistryError::SnapshotInvalid);
            }
        }
    }
    validate_portfolio_history(&snapshot.portfolio_history, limits)?;
    validate_namespace_projection(snapshot)?;
    for (namespace, ownership) in &snapshot.ownerships {
        if ownership.source_namespace_id != *namespace {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    for state in snapshot.namespace_states.values() {
        if let Some(metadata) = state.permanent_metadata()
            && (metadata.registry_revision() == 0
                || metadata.registry_revision() > snapshot.revision
                || !snapshot.roots.contains_key(&metadata.root_binding_id()))
        {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    let current = snapshot.portfolio_history.current_view();
    if current.len() != snapshot.portfolios.len()
        || current.iter().any(|(id, projection)| {
            snapshot.portfolios.get(id).is_none_or(|record| {
                record.portfolio_id != *id
                    || record.portfolio_revision != projection.portfolio_revision
                    || record.membership_precedence != projection.membership_precedence
                    || record.registry_revision != projection.registry_revision
                    || record.last_receipt != projection.last_receipt
            })
        })
    {
        return Err(RegistryError::SnapshotInvalid);
    }
    Ok(ValidatedRegistrySnapshot {
        inner: snapshot.clone(),
        original_admission_availability: if snapshot
            .sources
            .values()
            .any(|source| !source.has_original_admission())
        {
            OriginalAdmissionAvailability::LegacyInputsUnavailable
        } else {
            OriginalAdmissionAvailability::RetainedOriginalInputs
        },
    })
}

/// Validates the immutable portfolio history and its guarded current pointer.
///
/// Every retained revision is keyed by its own content and every current
/// pointer must name a retained revision. A pointer to an absent revision is
/// refused rather than treated as an empty scope, so a history gap stays
/// explicit instead of silently degrading to nothing.
fn validate_portfolio_history(
    history: &PortfolioHistory,
    limits: RegistryLimits,
) -> Result<(), RegistryError> {
    if history.len() > limits.max_portfolio_revisions {
        return Err(RegistryError::CapacityExceeded);
    }
    for record in history.revisions() {
        if record.membership_precedence().len() > limits.max_portfolio_items {
            return Err(RegistryError::SnapshotInvalid);
        }
        let mut seen = BTreeSet::new();
        for membership_id in record.membership_precedence() {
            if !seen.insert(*membership_id) {
                return Err(RegistryError::SnapshotInvalid);
            }
        }
    }
    for (portfolio_id, pointer) in history.current_pointers() {
        if pointer.target().portfolio_id != *portfolio_id
            || history.revision(&pointer.target()).is_none()
        {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    Ok(())
}

/// Computes the version-3 canonical digest of every snapshot field.
///
/// The flattened ownership projection does not persist cutover preparation,
/// fence or activation state. Those remain in NamespaceCutoverState.
///
/// # Errors
/// Returns no digest when the shared canonical item/byte ceilings are exceeded.
pub fn snapshot_digest(
    snapshot: &RegistrySnapshot,
) -> Result<RegistrySnapshotDigest, RegistryError> {
    if snapshot
        .sources
        .values()
        .any(|source| source.permanent_admission().is_some())
        || snapshot
            .namespace_states
            .values()
            .any(NamespaceCutoverState::is_permanent_v2)
        || snapshot
            .memberships
            .values()
            .any(MembershipRecord::is_permanent_v2)
    {
        return Err(RegistryError::SnapshotInvalid);
    }
    canonical::digest(snapshot)
}

/// Computes the version-5 canonical digest over complete source provenance and
/// full namespace metadata.
///
/// Historical-v1 values remain historical evidence. Permanent-v2 values bind
/// the exact retained policy/admission references and admission fences, while
/// namespace metadata binds the qualified filesystem profile and root/policy
/// references. No field in the digest grants current access or ownership.
///
/// # Errors
/// Returns `SnapshotInvalid` for a missing or contradictory full namespace
/// projection, a permanent-v2 model that cannot be represented by the bounded
/// canonical shape, or a canonical input over the shared limits.
pub fn snapshot_digest_v5(
    snapshot: &RegistrySnapshot,
) -> Result<RegistrySnapshotDigestV5, RegistryError> {
    if snapshot
        .memberships
        .values()
        .any(MembershipRecord::is_permanent_v2)
    {
        return Err(RegistryError::SnapshotInvalid);
    }
    validate_namespace_projection(snapshot)?;
    canonical::digest_v5(snapshot)
}

/// Computes the version-6 canonical digest over every snapshot field,
/// including the accepted typed permanent membership and exact provenance.
pub fn snapshot_digest_v6(
    snapshot: &RegistrySnapshot,
) -> Result<RegistrySnapshotDigestV6, RegistryError> {
    validate_namespace_projection(snapshot)?;
    canonical::digest_v6(snapshot)
}

fn validate_namespace_projection(snapshot: &RegistrySnapshot) -> Result<(), RegistryError> {
    if snapshot.namespace_states.len() != snapshot.ownerships.len() {
        return Err(RegistryError::SnapshotInvalid);
    }
    for (namespace, ownership) in &snapshot.ownerships {
        let state = snapshot
            .namespace_states
            .get(namespace)
            .ok_or(RegistryError::SnapshotInvalid)?;
        if state.ownership != *ownership || state.ownership.source_namespace_id != *namespace {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    for (namespace, state) in &snapshot.namespace_states {
        if state.ownership.source_namespace_id != *namespace
            || snapshot.ownerships.get(namespace) != Some(&state.ownership)
        {
            return Err(RegistryError::SnapshotInvalid);
        }
    }
    Ok(())
}
