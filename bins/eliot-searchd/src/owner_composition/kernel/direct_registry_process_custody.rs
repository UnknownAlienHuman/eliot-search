#![allow(dead_code)] // The native route stays dormant until provider qualification.

use std::path::Path;

use search_control_redb::{ControlSnapshotPublisher, JournalIdentity, PersistentControlJournal};

use super::DirectRegistryOwner;

#[must_use = "retain for the full native provider process lifetime"]
pub(crate) struct NativeProcessRootCustody {
    pub(super) owner: DirectRegistryOwner,
    pub(super) journal_identity: JournalIdentity,
}

impl NativeProcessRootCustody {
    pub(crate) const fn journal_identity(&self) -> JournalIdentity {
        self.journal_identity
    }
    pub(crate) fn canonical_root(&self) -> &Path {
        &self.owner.owner.root
    }
    pub(crate) const fn control(
        &self,
    ) -> Option<(&PersistentControlJournal, &ControlSnapshotPublisher)> {
        match self.owner.control.journal.as_ref() {
            Some(journal) => Some((journal, &self.owner.control.publisher)),
            None => None,
        }
    }

    #[cfg(feature = "wave4-query")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_serving_retained_read<T>(
        &mut self,
        state: &mut super::retained_reader::RetainedRevisionReadState,
        registry: &search_source_registry::SourceRegistry,
        output: &mut crate::provider_composition::CanonicalWorkOutput<'_, '_, '_>,
        admission: &crate::access_composition::StandalonePreRetrievalAdmission,
        membership: search_contracts::SourceMembershipId,
        expected: &search_contracts::SourceRevisionRef,
        config: &crate::config_composition::EffectiveDaemonConfig,
        consume: impl FnOnce(&[u8], &search_revision_store::PersistedSelectedObjectMetadata) -> Result<T, crate::provider_composition::CanonicalServingError>,
    ) -> Result<T, crate::provider_composition::CanonicalServingError> {
        self.owner.with_serving_retained_read(
            state, registry, output, admission, membership, expected, config, consume,
        )
    }
}
