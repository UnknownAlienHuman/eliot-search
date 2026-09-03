//! Content-minimized identity and binding views.

use search_contracts::{
    BoundedList, CatalogRevision, PathBindingId, RootBindingId, SourceId, SourceIdentityKind,
};

use crate::{
    IdentityError, IdentityResolution, MAX_IDENTITY_CANDIDATES, PathBindingRecord, PathBindingState,
};

/// Redacted identity/binding lifecycle class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RedactedIdentityState {
    /// Exact existing source matched.
    Existing,
    /// Exact unseen stable evidence produced a new-identity draft.
    NewDraft,
    /// Stable evidence or candidate evidence remains ambiguous.
    Ambiguous,
    /// One stable key maps to multiple source IDs.
    Collision,
    /// Claimed identity or active binding conflicts.
    Conflict,
    /// Source kind/profile is unsupported.
    Unsupported,
    /// Path binding is active.
    BindingActive,
    /// Path binding is closed.
    BindingClosed,
}

/// Redacted content-free identity view.
///
/// Unrestricted path text, source content, remote URLs, foreign workspace
/// details, and stable identity component bytes are structurally absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactedIdentityView {
    /// Redacted lifecycle/result class.
    pub state: RedactedIdentityState,
    /// Opaque source IDs when disclosure permits identity correlation.
    pub source_ids: BoundedList<SourceId, MAX_IDENTITY_CANDIDATES>,
    /// Stable source identity kind when established.
    pub identity_kind: Option<SourceIdentityKind>,
    /// Admitted root binding for a path binding, when applicable.
    pub root_binding_id: Option<RootBindingId>,
    /// Opaque path-binding ID, when applicable.
    pub path_binding_id: Option<PathBindingId>,
    /// Opening catalog revision for a binding, when applicable.
    pub opened_revision: Option<CatalogRevision>,
    /// Closing catalog revision for a binding, when applicable.
    pub closed_revision: Option<CatalogRevision>,
    /// Content-free reason when the view is not a successful existing/new result.
    pub reason: Option<IdentityError>,
}

/// Builds a redacted view of one resolution result.
///
/// # Errors
///
/// Candidate copying remains bounded by the resolution contract.
pub fn redacted_resolution_view(
    resolution: &IdentityResolution,
) -> Result<RedactedIdentityView, IdentityError> {
    let (state, ids, identity_kind, reason) = match resolution {
        IdentityResolution::MatchExisting {
            source_id,
            evidence,
        } => (
            RedactedIdentityState::Existing,
            vec![*source_id],
            Some(evidence.stable_key.identity_kind()),
            None,
        ),
        IdentityResolution::CreateNew(draft) => (
            RedactedIdentityState::NewDraft,
            Vec::new(),
            Some(draft.stable_key.identity_kind()),
            None,
        ),
        IdentityResolution::Ambiguous { candidates, .. } => (
            RedactedIdentityState::Ambiguous,
            candidates.as_slice().to_vec(),
            None,
            Some(IdentityError::SourceIdentityAmbiguous),
        ),
        IdentityResolution::Collision { source_ids } => (
            RedactedIdentityState::Collision,
            source_ids.as_slice().to_vec(),
            None,
            Some(IdentityError::SourceIdentityCollision),
        ),
        IdentityResolution::Conflict {
            claimed_source_id,
            existing_source_ids,
        } => {
            let mut ids = existing_source_ids.as_slice().to_vec();
            if let Some(claimed) = claimed_source_id
                && !ids.contains(claimed)
            {
                ids.push(*claimed);
                ids.sort_unstable();
            }
            (
                RedactedIdentityState::Conflict,
                ids,
                None,
                Some(IdentityError::SourceIdentityConflict),
            )
        }
        IdentityResolution::Unsupported(_) => (
            RedactedIdentityState::Unsupported,
            Vec::new(),
            None,
            Some(IdentityError::FilesystemProfileUnsupported),
        ),
    };
    Ok(RedactedIdentityView {
        state,
        source_ids: BoundedList::new(ids).map_err(|_| IdentityError::IdentityCapacityExceeded)?,
        identity_kind,
        root_binding_id: None,
        path_binding_id: None,
        opened_revision: None,
        closed_revision: None,
        reason,
    })
}

/// Builds a redacted view of one path-binding interval.
pub fn redacted_binding_view(
    binding: &PathBindingRecord,
) -> Result<RedactedIdentityView, IdentityError> {
    let (state, opened_revision, closed_revision) = match binding.state {
        PathBindingState::Active { opened_revision } => (
            RedactedIdentityState::BindingActive,
            Some(opened_revision),
            None,
        ),
        PathBindingState::Closed {
            opened_revision,
            closed_revision,
            ..
        } => (
            RedactedIdentityState::BindingClosed,
            Some(opened_revision),
            Some(closed_revision),
        ),
    };
    Ok(RedactedIdentityView {
        state,
        source_ids: BoundedList::new(vec![binding.source_id])
            .map_err(|_| IdentityError::IdentityCapacityExceeded)?,
        identity_kind: Some(binding.stable_key.identity_kind()),
        root_binding_id: Some(binding.root_binding_id),
        path_binding_id: Some(binding.binding_id),
        opened_revision,
        closed_revision,
        reason: None,
    })
}

#[cfg(test)]
mod tests {
    use search_contracts::{
        Blake3Digest32, CatalogRevision, PathBindingId, RootBindingId, SourceId,
        SourceOwnerGeneration, WorkspaceId,
    };

    use super::{RedactedIdentityState, redacted_binding_view};
    use crate::{PathBindingRecord, PathBindingState, StableIdentityKey};

    #[test]
    fn binding_view_contains_no_path_text() {
        let binding = PathBindingRecord {
            binding_id: PathBindingId::from_bytes([1; 16]),
            source_id: SourceId::from_bytes([2; 16]),
            workspace_id: WorkspaceId::from_bytes([3; 16]),
            root_binding_id: RootBindingId::from_bytes([4; 16]),
            path_key: crate::derive_canonical_path_key(
                &crate::PathObservation {
                    root_binding_id: RootBindingId::from_bytes([4; 16]),
                    root_relative_lookup_path: "private/name.rs".into(),
                    profile_revision: search_contracts::NonZeroRevision::new(1).expect("revision"),
                    profile_schema_digest: Blake3Digest32::from_bytes([5; 32]),
                    normalization_attested: true,
                },
                crate::FilesystemIdentityProfile::new(
                    search_contracts::NonZeroRevision::new(1).expect("revision"),
                    Blake3Digest32::from_bytes([5; 32]),
                    crate::CaseBehavior::Sensitive,
                    crate::UnicodeBehavior::PreserveScalarValues,
                    crate::StableFieldPolicy::Required,
                    crate::StableFieldPolicy::Required,
                    crate::LinkBehavior::StablePhysicalIdentity,
                    crate::ReparseBehavior::FinalTargetIdentity,
                )
                .expect("profile"),
            )
            .expect("path"),
            stable_key: StableIdentityKey::Filesystem {
                volume_identity: Blake3Digest32::from_bytes([6; 32]),
                file_identity: Blake3Digest32::from_bytes([7; 32]),
                generation: Some(1),
            },
            owner_generation: SourceOwnerGeneration::from_bytes([8; 32]),
            state: PathBindingState::Active {
                opened_revision: CatalogRevision::new(1),
            },
        };
        let view = redacted_binding_view(&binding).expect("redacted view");
        assert_eq!(view.state, RedactedIdentityState::BindingActive);
        assert!(!format!("{view:?}").contains("private/name.rs"));
    }
}
