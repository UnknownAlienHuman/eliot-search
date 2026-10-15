//! Pure bounded C04 identity lookup metadata.
//!
//! This is a derived technical cache: it allocates no identity, grants no
//! access, evaluates no policy, reads no native state, and owns no query.
use std::collections::BTreeMap;
use search_contracts::{OpaqueCanonicalBytes, SourceIdentity, SourceIdentityKind, SourceNamespaceId};
use search_source_identity::{CanonicalPathKey, CaseBehavior, FilesystemIdentityProfile, IdentityError, PathObservation, SourceBinding, UnicodeBehavior, ValidatedIdentityObservation, derive_canonical_path_key};
use crate::error::RegistryError;
use crate::namespace::NamespaceCutoverState;
use crate::provenance::SourceProvenance;
use crate::source::{RegisteredSource, SourceLifecycle};

/// Closed failure from lookup preparation or a missing C04 replay contract.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IdentityLookupV2Error {
    /// Preserve the C04 identity owner's exact failure.
    Identity(IdentityError),
    /// Preserve the records owner's exact failure.
    Registry(RegistryError),
    /// The source namespace was absent or keyed inconsistently.
    NamespaceUnavailable,
    /// A permanent source had no permanent namespace metadata.
    NamespaceMetadataUnavailable,
    /// Source binding and namespace root metadata disagree.
    RootBindingMismatch,
    /// Stored binding metadata cannot prove a pre-normalized C04 path.
    C04ObservationRequired,
    /// The parent producer did not persist the validated canonical path.
    RecordedPathMismatch,
    /// One stable material key maps to different source identities.
    StableIdentityCollision,
    /// One active canonical path maps to different source identities.
    ActivePathCollision,
    /// Permanent material may leave only through retirement.
    PermanentRemovalRequiresRetirement,
}
impl From<IdentityError> for IdentityLookupV2Error { fn from(error: IdentityError) -> Self { Self::Identity(error) } }
impl From<RegistryError> for IdentityLookupV2Error { fn from(error: RegistryError) -> Self { Self::Registry(error) } }

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StableLookupKey { namespace_id: SourceNamespaceId, identity_kind: SourceIdentityKind, stable_components: OpaqueCanonicalBytes }
impl From<&SourceIdentity> for StableLookupKey {
    fn from(identity: &SourceIdentity) -> Self { Self { namespace_id: identity.source_namespace_id, identity_kind: identity.identity_kind, stable_components: identity.stable_identity_components.clone() } }
}

/// At most two identities returned by one material/path candidate lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityLookupCandidates { first: Option<SourceIdentity>, second: Option<SourceIdentity> }
impl IdentityLookupCandidates {
    fn new(first: Option<&SourceIdentity>, second: Option<&SourceIdentity>) -> Self { if first == second { Self { first: first.cloned(), second: None } } else { Self { first: first.cloned(), second: second.cloned() } } }
    /// Number of distinct candidates, bounded to two.
    #[must_use]
    pub fn len(&self) -> usize { usize::from(self.first.is_some()) + usize::from(self.second.is_some()) }
    /// Whether no material or path candidate was found.
    #[must_use]
    pub const fn is_empty(&self) -> bool { self.first.is_none() && self.second.is_none() }
    /// Iterates the distinct candidates in material-then-path order.
    pub fn iter(&self) -> impl Iterator<Item = &SourceIdentity> { [self.first.as_ref(), self.second.as_ref()].into_iter().flatten() }
}

/// Opaque post-commit cache delta; it contains at most two removals and inserts.
#[derive(Clone)]
pub struct IdentityLookupV2Delta { stable_remove: Option<StableLookupKey>, stable_insert: Option<(StableLookupKey, SourceIdentity)>, path_remove: Option<CanonicalPathKey>, path_insert: Option<(CanonicalPathKey, SourceIdentity)> }
#[derive(Clone)]
struct PreparedSource { stable: (StableLookupKey, SourceIdentity), active_path: Option<(CanonicalPathKey, SourceIdentity)> }

/// Derived C04 material/path metadata, rebuilt at startup and installed after commit.
#[derive(Clone, Default)]
pub struct SourceIdentityLookupV2 { stable: BTreeMap<StableLookupKey, SourceIdentity>, active_paths: BTreeMap<CanonicalPathKey, SourceIdentity> }
impl SourceIdentityLookupV2 {
    /// Rebuilds the cache from one coherent source and namespace view. Historical sources are explicitly skipped; unproven pre-normalization fails closed.
    pub fn rebuild(sources: &BTreeMap<SourceIdentity, RegisteredSource>, namespaces: &BTreeMap<SourceNamespaceId, NamespaceCutoverState>) -> Result<Self, IdentityLookupV2Error> {
        let mut lookup = Self::default();
        for (key, source) in sources {
            if key != source.identity() { return Err(RegistryError::SnapshotInvalid.into()); }
            let Some(prepared) = prepare_source(source, namespaces)? else { continue };
            insert(&mut lookup.stable, prepared.stable, IdentityLookupV2Error::StableIdentityCollision)?;
            if let Some(path) = prepared.active_path { insert(&mut lookup.active_paths, path, IdentityLookupV2Error::ActivePathCollision)?; }
        }
        Ok(lookup)
    }
    /// Validates one replacement without cloning either cache map.
    pub fn prepare_change(&self, before: Option<&RegisteredSource>, after: Option<&RegisteredSource>, namespaces: &BTreeMap<SourceNamespaceId, NamespaceCutoverState>) -> Result<IdentityLookupV2Delta, IdentityLookupV2Error> {
        if before.is_some_and(is_permanent) && !after.is_some_and(is_permanent) { return Err(IdentityLookupV2Error::PermanentRemovalRequiresRetirement); }
        let before = before.map(|source| prepare_source(source, namespaces)).transpose()?.flatten();
        let after = after.map(|source| prepare_source(source, namespaces)).transpose()?.flatten();
        check_slot(&self.stable, before.as_ref().map(|v| (&v.stable.0, &v.stable.1)), after.as_ref().map(|v| (&v.stable.0, &v.stable.1)), IdentityLookupV2Error::StableIdentityCollision)?;
        check_slot(&self.active_paths, before.as_ref().and_then(|v| v.active_path.as_ref()).map(|v| (&v.0, &v.1)), after.as_ref().and_then(|v| v.active_path.as_ref()).map(|v| (&v.0, &v.1)), IdentityLookupV2Error::ActivePathCollision)?;
        Ok(IdentityLookupV2Delta { stable_remove: before.as_ref().map(|v| v.stable.0.clone()), stable_insert: after.as_ref().map(|v| v.stable.clone()), path_remove: before.as_ref().and_then(|v| v.active_path.as_ref().map(|p| p.0.clone())), path_insert: after.as_ref().and_then(|v| v.active_path.clone()) })
    }
    /// Installs a validated delta after the owning commit succeeds.
    pub fn install(&mut self, delta: IdentityLookupV2Delta) {
        if let Some(key) = delta.stable_remove { self.stable.remove(&key); }
        if let Some((key, identity)) = delta.stable_insert { self.stable.insert(key, identity); }
        if let Some(key) = delta.path_remove { self.active_paths.remove(&key); }
        if let Some((key, identity)) = delta.path_insert { self.active_paths.insert(key, identity); }
    }
    /// Looks up one full stable-material key in O(log N) time.
    pub fn lookup_material(&self, namespace_id: SourceNamespaceId, identity_kind: SourceIdentityKind, stable_components: &OpaqueCanonicalBytes) -> Option<&SourceIdentity> { self.stable.get(&StableLookupKey { namespace_id, identity_kind, stable_components: stable_components.clone() }) }
    /// Looks up one active canonical path key in O(log N) time.
    pub fn lookup_path(&self, path: &CanonicalPathKey) -> Option<&SourceIdentity> { self.active_paths.get(path) }
    /// Returns distinct material/path candidates, bounded to two IDs.
    pub fn lookup_candidates(&self, material: Option<(SourceNamespaceId, SourceIdentityKind, &OpaqueCanonicalBytes)>, path: Option<&CanonicalPathKey>) -> IdentityLookupCandidates { IdentityLookupCandidates::new(material.and_then(|(namespace, kind, components)| self.lookup_material(namespace, kind, components)), path.and_then(|key| self.lookup_path(key))) }
}

/// Requires the parent producer's binding to equal its validated C04 path metadata.
pub fn require_recorded_path(binding: &SourceBinding, observation: &ValidatedIdentityObservation) -> Result<(), IdentityLookupV2Error> {
    let path = &observation.as_inner().path_key;
    if binding.root_binding_id() != path.root_binding_id() || binding.relative_path().as_str() != path.as_str() { return Err(IdentityLookupV2Error::RecordedPathMismatch); }
    Ok(())
}
fn is_permanent(source: &RegisteredSource) -> bool { matches!(source.provenance(), SourceProvenance::PermanentV2(_)) }
fn prepare_source(source: &RegisteredSource, namespaces: &BTreeMap<SourceNamespaceId, NamespaceCutoverState>) -> Result<Option<PreparedSource>, IdentityLookupV2Error> {
    if !is_permanent(source) { return Ok(None); }
    let namespace_id = source.identity().source_namespace_id;
    let namespace = namespaces.get(&namespace_id).ok_or(IdentityLookupV2Error::NamespaceUnavailable)?;
    if namespace.ownership.source_namespace_id != namespace_id { return Err(IdentityLookupV2Error::NamespaceUnavailable); }
    let metadata = namespace.permanent_metadata().ok_or(IdentityLookupV2Error::NamespaceMetadataUnavailable)?;
    if metadata.root_binding_id() != source.binding().root_binding_id() { return Err(IdentityLookupV2Error::RootBindingMismatch); }
    let stable = (StableLookupKey::from(source.identity()), source.identity().clone());
    let active_path = (source.lifecycle() == SourceLifecycle::Active).then(|| derive_path_key(source.binding(), metadata.identity_profile()).map(|path| (path, source.identity().clone()))).transpose()?;
    Ok(Some(PreparedSource { stable, active_path }))
}
fn derive_path_key(binding: &SourceBinding, profile: FilesystemIdentityProfile) -> Result<CanonicalPathKey, IdentityLookupV2Error> {
    if profile.case_behavior() == CaseBehavior::PreNormalizedInsensitive || matches!(profile.unicode_behavior(), UnicodeBehavior::PreNormalizedNfc | UnicodeBehavior::PreNormalizedNfd) { return Err(IdentityLookupV2Error::C04ObservationRequired); }
    derive_canonical_path_key(&PathObservation { root_binding_id: binding.root_binding_id(), root_relative_lookup_path: binding.relative_path().as_str().to_owned(), profile_revision: profile.revision(), profile_schema_digest: profile.schema_digest(), normalization_attested: false }, profile).map_err(IdentityLookupV2Error::Identity)
}
fn insert<K: Ord>(map: &mut BTreeMap<K, SourceIdentity>, (key, identity): (K, SourceIdentity), collision: IdentityLookupV2Error) -> Result<(), IdentityLookupV2Error> {
    if map.get(&key).is_some_and(|old| old != &identity) { return Err(collision); }
    map.insert(key, identity);
    Ok(())
}
fn check_slot<K: Ord>(map: &BTreeMap<K, SourceIdentity>, before: Option<(&K, &SourceIdentity)>, after: Option<(&K, &SourceIdentity)>, collision: IdentityLookupV2Error) -> Result<(), IdentityLookupV2Error> {
    if let (Some((old_key, old_id)), Some((new_key, new_id))) = (before, after) { if old_key == new_key && old_id != new_id { return Err(collision); } }
    if let Some((key, identity)) = before { if map.get(key).is_some_and(|old| old != identity) { return Err(collision); } }
    if let Some((key, identity)) = after { if let Some(old) = map.get(key) { let replaced = before.is_some_and(|(old_key, old_id)| old_key == key && old_id == old); if old != identity && !replaced { return Err(collision); } } }
    Ok(())
}
