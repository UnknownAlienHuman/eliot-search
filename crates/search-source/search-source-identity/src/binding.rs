//! Revision-fenced path binding, rename, replacement, and hard-link decisions.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, BoundedList, CatalogRevision, PathBindingId, ReceiptRef,
    RootBindingId, SourceId, SourceOwnerGeneration, WorkspaceId,
};

use crate::{
    CanonicalPathKey, IdentityError, LinkBehavior, StableIdentityEvidence,
    StableIdentityKey,
};

/// Maximum bindings returned by one hard-link grouping decision.
pub const MAX_HARDLINK_BINDINGS: usize = 256;
/// Maximum history entries accepted by one validation call.
pub const MAX_BINDING_HISTORY_ENTRIES: usize = 4_096;

/// Why an active path interval was closed.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BindingCloseReason {
    /// Source moved to another path in the same admitted root.
    Renamed,
    /// Source moved to another admitted root.
    MovedToOtherAdmittedRoot,
    /// One hard-link alias disappeared while source identity survived.
    HardlinkRemoved,
    /// Path now resolves to another physical/logical identity.
    PathReplaced,
    /// Source is no longer observed.
    SourceRemoved,
    /// Root binding was retired or fenced.
    RootUnbound,
}

/// Lifecycle of one path binding interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathBindingState {
    /// Binding is currently active.
    Active {
        /// Catalog revision at which the interval opened.
        opened_revision: CatalogRevision,
    },
    /// Binding is closed and cannot be silently resurrected.
    Closed {
        /// Catalog revision at which the interval opened.
        opened_revision: CatalogRevision,
        /// Catalog revision at which the interval closed.
        closed_revision: CatalogRevision,
        /// Explicit close reason.
        reason: BindingCloseReason,
    },
}

impl PathBindingState {
    /// Opening catalog revision.
    #[must_use]
    pub const fn opened_revision(self) -> CatalogRevision {
        match self {
            Self::Active { opened_revision } | Self::Closed { opened_revision, .. } => {
                opened_revision
            }
        }
    }

    /// Closing catalog revision when closed.
    #[must_use]
    pub const fn closed_revision(self) -> Option<CatalogRevision> {
        match self {
            Self::Active { .. } => None,
            Self::Closed {
                closed_revision, ..
            } => Some(closed_revision),
        }
    }

    /// Whether the interval is active.
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active { .. })
    }
}

/// One stable source-to-path interval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathBindingRecord {
    /// Caller-supplied binding identity.
    pub binding_id: PathBindingId,
    /// Stable source identity.
    pub source_id: SourceId,
    /// Workspace boundary containing this locator.
    pub workspace_id: WorkspaceId,
    /// Admitted root binding.
    pub root_binding_id: RootBindingId,
    /// Versioned lookup key.
    pub path_key: CanonicalPathKey,
    /// Exact stable identity proven for the binding.
    pub stable_key: StableIdentityKey,
    /// Source-owner generation under which the interval was written.
    pub owner_generation: SourceOwnerGeneration,
    /// Interval lifecycle.
    pub state: PathBindingState,
}

impl PathBindingRecord {
    /// Returns whether this binding currently occupies its lookup key.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.state.is_active()
    }
}

/// Evidence required to open one active path binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenBindingRequest {
    /// Caller-supplied binding identity.
    pub binding_id: PathBindingId,
    /// Stable source identity produced by accepted resolution.
    pub source_id: SourceId,
    /// Workspace boundary.
    pub workspace_id: WorkspaceId,
    /// Admitted root binding.
    pub root_binding_id: RootBindingId,
    /// Exact path key.
    pub path_key: CanonicalPathKey,
    /// Exact stable identity key.
    pub stable_key: StableIdentityKey,
    /// Current source-owner generation.
    pub owner_generation: SourceOwnerGeneration,
    /// Current catalog revision before opening.
    pub expected_catalog_revision: CatalogRevision,
    /// Exact next catalog revision for the new interval.
    pub opened_revision: CatalogRevision,
}

/// Opens one active binding after collision checks.
///
/// # Errors
///
/// Root mismatch, non-contiguous revision, duplicate binding identity, or an
/// active different source on the same lookup key is rejected.
pub fn open_path_binding(
    request: OpenBindingRequest,
    active_bindings: &[PathBindingRecord],
) -> Result<PathBindingRecord, IdentityError> {
    if request.path_key.root_binding_id() != request.root_binding_id {
        return Err(IdentityError::PathEscapesAdmittedRoot);
    }
    verify_next_catalog_revision(
        request.expected_catalog_revision,
        request.opened_revision,
    )?;
    for existing in active_bindings {
        if existing.binding_id == request.binding_id {
            return Err(IdentityError::PathBindingConflict);
        }
        if existing.is_active()
            && existing.path_key == request.path_key
            && existing.source_id != request.source_id
        {
            return Err(IdentityError::PathBindingConflict);
        }
    }
    Ok(PathBindingRecord {
        binding_id: request.binding_id,
        source_id: request.source_id,
        workspace_id: request.workspace_id,
        root_binding_id: request.root_binding_id,
        path_key: request.path_key,
        stable_key: request.stable_key,
        owner_generation: request.owner_generation,
        state: PathBindingState::Active {
            opened_revision: request.opened_revision,
        },
    })
}

/// Closed path-binding event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PathBindingEvent {
    /// Same source moved to another key in the same root.
    Rename {
        /// Caller-supplied identity for the new interval.
        new_binding_id: PathBindingId,
        /// New lookup key.
        new_path_key: CanonicalPathKey,
        /// Stable evidence at the new path.
        observed_stable_evidence: StableIdentityEvidence,
        /// Exact next catalog revision.
        next_revision: CatalogRevision,
    },
    /// Same source moved to another admitted root/workspace.
    MoveToOtherAdmittedRoot {
        /// Caller-supplied identity for the new interval.
        new_binding_id: PathBindingId,
        /// New workspace boundary.
        new_workspace_id: WorkspaceId,
        /// New admitted root binding.
        new_root_binding_id: RootBindingId,
        /// New lookup key.
        new_path_key: CanonicalPathKey,
        /// Stable evidence at the new path.
        observed_stable_evidence: StableIdentityEvidence,
        /// Exact next catalog revision.
        next_revision: CatalogRevision,
        /// Explicit authorization for the cross-root move.
        authorization_receipt: ReceiptRef,
    },
    /// Add another active hard-link alias without closing the current interval.
    HardlinkAdded {
        /// Caller-supplied identity for the alias interval.
        new_binding_id: PathBindingId,
        /// Alias lookup key.
        new_path_key: CanonicalPathKey,
        /// Stable evidence at the alias.
        observed_stable_evidence: StableIdentityEvidence,
        /// Exact next catalog revision.
        next_revision: CatalogRevision,
    },
    /// Close this alias while retaining source identity elsewhere.
    HardlinkRemoved {
        /// Exact next catalog revision.
        next_revision: CatalogRevision,
    },
    /// Path now resolves to another identity; fresh resolution is mandatory.
    PathReplaced {
        /// Exact next catalog revision.
        next_revision: CatalogRevision,
    },
    /// Source disappeared.
    SourceRemoved {
        /// Exact next catalog revision.
        next_revision: CatalogRevision,
    },
    /// Root binding was retired/fenced.
    RootUnbound {
        /// Exact next catalog revision.
        next_revision: CatalogRevision,
    },
}

/// Pure binding transition result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathBindingTransition {
    /// Prior interval after transition, when it was closed.
    pub closed: Option<Box<PathBindingRecord>>,
    /// New active interval, when one was opened.
    pub opened: Option<Box<PathBindingRecord>>,
    /// Whether the next path occupant requires fresh identity resolution.
    pub fresh_resolution_required: bool,
    /// Authorization receipt used for a cross-root move, when applicable.
    pub authorization_receipt: Option<ReceiptRef>,
}

/// Applies one closed binding event.
///
/// Exact stable evidence is required to preserve source identity across rename,
/// move, or hard-link addition. Path replacement never opens a new source.
///
/// # Errors
///
/// Closed bindings, revision gaps/regressions, root mismatch, and unproved
/// stable identity are rejected.
pub fn transition_path_binding(
    current: &PathBindingRecord,
    current_catalog_revision: CatalogRevision,
    event: PathBindingEvent,
) -> Result<PathBindingTransition, IdentityError> {
    if !current.is_active() {
        return Err(IdentityError::PathBindingHistoryInvalid);
    }
    match event {
        PathBindingEvent::Rename {
            new_binding_id,
            new_path_key,
            observed_stable_evidence,
            next_revision,
        } => {
            verify_next_catalog_revision(current_catalog_revision, next_revision)?;
            if new_path_key.root_binding_id() != current.root_binding_id {
                return Err(IdentityError::PathEscapesAdmittedRoot);
            }
            require_same_stable_identity(current.stable_key, observed_stable_evidence)?;
            Ok(close_and_open(
                current,
                BindingCloseReason::Renamed,
                new_binding_id,
                current.workspace_id,
                current.root_binding_id,
                new_path_key,
                next_revision,
                None,
            ))
        }
        PathBindingEvent::MoveToOtherAdmittedRoot {
            new_binding_id,
            new_workspace_id,
            new_root_binding_id,
            new_path_key,
            observed_stable_evidence,
            next_revision,
            authorization_receipt,
        } => {
            verify_next_catalog_revision(current_catalog_revision, next_revision)?;
            if new_path_key.root_binding_id() != new_root_binding_id {
                return Err(IdentityError::PathEscapesAdmittedRoot);
            }
            require_same_stable_identity(current.stable_key, observed_stable_evidence)?;
            Ok(close_and_open(
                current,
                BindingCloseReason::MovedToOtherAdmittedRoot,
                new_binding_id,
                new_workspace_id,
                new_root_binding_id,
                new_path_key,
                next_revision,
                Some(authorization_receipt),
            ))
        }
        PathBindingEvent::HardlinkAdded {
            new_binding_id,
            new_path_key,
            observed_stable_evidence,
            next_revision,
        } => {
            verify_next_catalog_revision(current_catalog_revision, next_revision)?;
            if new_path_key.root_binding_id() != current.root_binding_id {
                return Err(IdentityError::PathEscapesAdmittedRoot);
            }
            require_same_stable_identity(current.stable_key, observed_stable_evidence)?;
            Ok(PathBindingTransition {
                closed: None,
                opened: Some(Box::new(PathBindingRecord {
                    binding_id: new_binding_id,
                    source_id: current.source_id,
                    workspace_id: current.workspace_id,
                    root_binding_id: current.root_binding_id,
                    path_key: new_path_key,
                    stable_key: current.stable_key,
                    owner_generation: current.owner_generation,
                    state: PathBindingState::Active {
                        opened_revision: next_revision,
                    },
                })),
                fresh_resolution_required: false,
                authorization_receipt: None,
            })
        }
        PathBindingEvent::HardlinkRemoved { next_revision } => Ok(close_only(
            current,
            current_catalog_revision,
            next_revision,
            BindingCloseReason::HardlinkRemoved,
            false,
        )?),
        PathBindingEvent::PathReplaced { next_revision } => Ok(close_only(
            current,
            current_catalog_revision,
            next_revision,
            BindingCloseReason::PathReplaced,
            true,
        )?),
        PathBindingEvent::SourceRemoved { next_revision } => Ok(close_only(
            current,
            current_catalog_revision,
            next_revision,
            BindingCloseReason::SourceRemoved,
            false,
        )?),
        PathBindingEvent::RootUnbound { next_revision } => Ok(close_only(
            current,
            current_catalog_revision,
            next_revision,
            BindingCloseReason::RootUnbound,
            false,
        )?),
    }
}

/// Groups hard-link bindings only through exact accepted physical identity.
///
/// # Errors
///
/// A profile without stable link identity, unavailable evidence, mixed stable
/// keys, or capacity overflow is rejected.
pub fn relate_hardlink_bindings(
    bindings: &[PathBindingRecord],
    link_behavior: LinkBehavior,
) -> Result<BoundedList<PathBindingId, MAX_HARDLINK_BINDINGS>, IdentityError> {
    if link_behavior != LinkBehavior::StablePhysicalIdentity || bindings.is_empty() {
        return Err(IdentityError::HardlinkIdentityUnproved);
    }
    if bindings.len() > MAX_HARDLINK_BINDINGS {
        return Err(IdentityError::IdentityCapacityExceeded);
    }
    let first = &bindings[0];
    if !matches!(first.stable_key, StableIdentityKey::Filesystem { .. }) {
        return Err(IdentityError::HardlinkIdentityUnproved);
    }
    let mut ids = BTreeSet::new();
    for binding in bindings {
        if !binding.is_active()
            || binding.source_id != first.source_id
            || binding.stable_key != first.stable_key
            || !ids.insert(binding.binding_id)
        {
            return Err(IdentityError::HardlinkIdentityUnproved);
        }
    }
    BoundedList::new(ids.into_iter().collect())
        .map_err(|_| IdentityError::IdentityCapacityExceeded)
}

/// Content-free binding-history validation receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PathHistoryReceipt {
    /// Number of validated intervals.
    pub interval_count: usize,
    /// Highest catalog revision represented by history.
    pub highest_revision: CatalogRevision,
    /// Digest of exact canonical history supplied by the caller.
    pub history_digest: Blake3Digest32,
}

/// Validates monotone non-overlapping binding history.
///
/// # Errors
///
/// More than the finite ceiling, invalid intervals, duplicate binding IDs, or
/// overlapping active/closed intervals for one path key is rejected.
pub fn validate_binding_history(
    history: &[PathBindingRecord],
    history_digest: Blake3Digest32,
) -> Result<PathHistoryReceipt, IdentityError> {
    if history.is_empty() || history.len() > MAX_BINDING_HISTORY_ENTRIES {
        return Err(IdentityError::PathBindingHistoryInvalid);
    }
    let mut ids = BTreeSet::new();
    let mut by_path: BTreeMap<&CanonicalPathKey, Vec<(u64, Option<u64>)>> = BTreeMap::new();
    let mut highest_revision = CatalogRevision::new(0);
    for binding in history {
        if !ids.insert(binding.binding_id) {
            return Err(IdentityError::PathBindingHistoryInvalid);
        }
        let opened = binding.state.opened_revision().get();
        let closed = binding.state.closed_revision().map(CatalogRevision::get);
        if closed.is_some_and(|closed| closed <= opened) {
            return Err(IdentityError::PathBindingHistoryInvalid);
        }
        let terminal = closed.unwrap_or(opened);
        if terminal > highest_revision.get() {
            highest_revision = CatalogRevision::new(terminal);
        }
        by_path
            .entry(&binding.path_key)
            .or_default()
            .push((opened, closed));
    }
    for intervals in by_path.values_mut() {
        intervals.sort_unstable_by_key(|interval| interval.0);
        for pair in intervals.windows(2) {
            let previous_end = pair[0].1.unwrap_or(u64::MAX);
            if previous_end >= pair[1].0 {
                return Err(IdentityError::PathBindingHistoryInvalid);
            }
        }
    }
    Ok(PathHistoryReceipt {
        interval_count: history.len(),
        highest_revision,
        history_digest,
    })
}

fn close_and_open(
    current: &PathBindingRecord,
    reason: BindingCloseReason,
    new_binding_id: PathBindingId,
    new_workspace_id: WorkspaceId,
    new_root_binding_id: RootBindingId,
    new_path_key: CanonicalPathKey,
    next_revision: CatalogRevision,
    authorization_receipt: Option<ReceiptRef>,
) -> PathBindingTransition {
    PathBindingTransition {
        closed: Some(Box::new(closed_record(current, next_revision, reason))),
        opened: Some(Box::new(PathBindingRecord {
            binding_id: new_binding_id,
            source_id: current.source_id,
            workspace_id: new_workspace_id,
            root_binding_id: new_root_binding_id,
            path_key: new_path_key,
            stable_key: current.stable_key,
            owner_generation: current.owner_generation,
            state: PathBindingState::Active {
                opened_revision: next_revision,
            },
        })),
        fresh_resolution_required: false,
        authorization_receipt,
    }
}

fn close_only(
    current: &PathBindingRecord,
    current_catalog_revision: CatalogRevision,
    next_revision: CatalogRevision,
    reason: BindingCloseReason,
    fresh_resolution_required: bool,
) -> Result<PathBindingTransition, IdentityError> {
    verify_next_catalog_revision(current_catalog_revision, next_revision)?;
    Ok(PathBindingTransition {
        closed: Some(Box::new(closed_record(current, next_revision, reason))),
        opened: None,
        fresh_resolution_required,
        authorization_receipt: None,
    })
}

fn closed_record(
    current: &PathBindingRecord,
    closed_revision: CatalogRevision,
    reason: BindingCloseReason,
) -> PathBindingRecord {
    let mut closed = current.clone();
    closed.state = PathBindingState::Closed {
        opened_revision: current.state.opened_revision(),
        closed_revision,
        reason,
    };
    closed
}

fn require_same_stable_identity(
    expected: StableIdentityKey,
    observed: StableIdentityEvidence,
) -> Result<(), IdentityError> {
    match observed {
        StableIdentityEvidence::Exact(actual) if actual == expected => Ok(()),
        StableIdentityEvidence::Exact(_) => Err(IdentityError::SourceIdentityConflict),
        StableIdentityEvidence::Unavailable(_) => {
            Err(IdentityError::SourceIdentityInsufficientEvidence)
        }
    }
}

fn verify_next_catalog_revision(
    current: CatalogRevision,
    proposed: CatalogRevision,
) -> Result<(), IdentityError> {
    let expected = current
        .checked_next()
        .map_err(|_| IdentityError::ContractExhausted)?;
    if proposed == expected {
        Ok(())
    } else {
        Err(IdentityError::IdentityRevisionInvalid)
    }
}

#[cfg(test)]
mod tests {
    use search_contracts::{
        Blake3Digest32, CatalogRevision, PathBindingId, RootBindingId, SourceId,
        SourceOwnerGeneration, WorkspaceId,
    };

    use super::{
        BindingCloseReason, OpenBindingRequest, PathBindingEvent, PathBindingState,
        open_path_binding, transition_path_binding,
    };
    use crate::{
        CanonicalPathKey, StableIdentityEvidence, StableIdentityKey,
    };

    fn stable(byte: u8) -> StableIdentityKey {
        StableIdentityKey::Filesystem {
            volume_identity: Blake3Digest32::from_bytes([byte; 32]),
            file_identity: Blake3Digest32::from_bytes([byte.wrapping_add(1); 32]),
            generation: Some(1),
        }
    }

    fn path(name: &str) -> CanonicalPathKey {
        crate::derive_canonical_path_key(
            &crate::PathObservation {
                root_binding_id: RootBindingId::from_bytes([1; 16]),
                root_relative_lookup_path: name.into(),
                profile_revision: search_contracts::NonZeroRevision::new(1).expect("revision"),
                profile_schema_digest: Blake3Digest32::from_bytes([2; 32]),
                normalization_attested: true,
            },
            crate::FilesystemIdentityProfile::new(
                search_contracts::NonZeroRevision::new(1).expect("revision"),
                Blake3Digest32::from_bytes([2; 32]),
                crate::CaseBehavior::Sensitive,
                crate::UnicodeBehavior::PreserveScalarValues,
                crate::StableFieldPolicy::Required,
                crate::StableFieldPolicy::Required,
                crate::LinkBehavior::StablePhysicalIdentity,
                crate::ReparseBehavior::FinalTargetIdentity,
            )
            .expect("profile"),
        )
        .expect("path")
    }

    fn binding() -> super::PathBindingRecord {
        open_path_binding(
            OpenBindingRequest {
                binding_id: PathBindingId::from_bytes([3; 16]),
                source_id: SourceId::from_bytes([4; 16]),
                workspace_id: WorkspaceId::from_bytes([5; 16]),
                root_binding_id: RootBindingId::from_bytes([1; 16]),
                path_key: path("src/a.rs"),
                stable_key: stable(6),
                owner_generation: SourceOwnerGeneration::from_bytes([7; 32]),
                expected_catalog_revision: CatalogRevision::new(0),
                opened_revision: CatalogRevision::new(1),
            },
            &[],
        )
        .expect("binding")
    }

    #[test]
    fn rename_preserves_source_with_exact_stable_evidence() {
        let current = binding();
        let transition = transition_path_binding(
            &current,
            CatalogRevision::new(1),
            PathBindingEvent::Rename {
                new_binding_id: PathBindingId::from_bytes([8; 16]),
                new_path_key: path("src/b.rs"),
                observed_stable_evidence: StableIdentityEvidence::Exact(stable(6)),
                next_revision: CatalogRevision::new(2),
            },
        )
        .expect("rename");
        assert_eq!(
            transition.opened.as_ref().expect("opened").source_id,
            current.source_id
        );
        assert!(matches!(
            transition.closed.as_ref().expect("closed").state,
            PathBindingState::Closed {
                reason: BindingCloseReason::Renamed,
                ..
            }
        ));
    }

    #[test]
    fn path_replacement_requires_new_resolution() {
        let transition = transition_path_binding(
            &binding(),
            CatalogRevision::new(1),
            PathBindingEvent::PathReplaced {
                next_revision: CatalogRevision::new(2),
            },
        )
        .expect("replacement");
        assert!(transition.fresh_resolution_required);
        assert!(transition.opened.is_none());
    }
}
