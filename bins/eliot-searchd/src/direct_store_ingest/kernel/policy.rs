//! One coherent admission policy and replayed registry projection per batch.

use super::super::super::{DirectStore, SourceState};
use crate::source_composition as canonical;

impl DirectStore {
    /// Current primary-ingestion admission fence.
    pub(super) fn admission_policy() -> canonical::AdmissionPolicy {
        canonical::AdmissionPolicy::baseline()
    }

    /// Builds one transient view over replayed DIRECT state.
    ///
    /// This is a projection, never a second durable catalog.
    pub(super) fn registry_view(
        &self,
        policy: &canonical::AdmissionPolicy,
    ) -> Result<canonical::RegistryView, String> {
        let mut prior = Vec::with_capacity(self.registry.latest.len());
        for record in self.registry.latest.values() {
            prior.push(canonical::PriorSourceView {
                source_id: record.source_id.clone(),
                file_identity_digest: record.file_identity_digest.clone(),
                path_digest: record.path_digest.clone(),
                revision_id: record.revision_id.clone(),
                record_digest: record.record_digest.clone(),
                is_active: record.state == SourceState::Active,
            });
        }
        canonical::RegistryView::build(prior, policy)
    }
}
