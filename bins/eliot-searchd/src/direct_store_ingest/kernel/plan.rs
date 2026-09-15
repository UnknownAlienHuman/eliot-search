//! Canonical snapshot admission and durable identifier planning.

use std::collections::BTreeSet;
use std::path::Path;

use super::super::super::{
    DirectStore, FileSnapshot, IndexedSource, RecordDraft, SourceState,
    ZERO_DIGEST,
};
use crate::sha256;
use crate::source_composition as canonical;

impl DirectStore {
    /// Plans one retained snapshot without invoking any storage adapter.
    ///
    /// Stable identity comes from the kernel-verified snapshot plus namespace;
    /// the caller path is classification input only. Exact prior stable matches
    /// retain their existing source ID, including legacy-domain IDs.
    pub(super) fn plan_snapshot(
        &self,
        original_path: &Path,
        snapshot: FileSnapshot,
        seen: &mut BTreeSet<String>,
        policy: &canonical::AdmissionPolicy,
        view: &canonical::RegistryView,
    ) -> Result<(FileSnapshot, IndexedSource, Option<RecordDraft>), String> {
        if sha256::decode_digest(&snapshot.file_identity_digest).is_none() {
            return Err("DIRECT_FILE_IDENTITY_INVALID".to_owned());
        }
        if sha256::decode_digest(&snapshot.content_digest).is_none() {
            return Err("DIRECT_CONTENT_DIGEST_INVALID".to_owned());
        }
        let namespace_hex = sha256::hex(&self.namespace_id);
        let planned = canonical::plan_snapshot(
            original_path,
            &snapshot.file_identity_digest,
            snapshot.identity_strength.tag(),
            &snapshot.content_digest,
            &snapshot.bytes,
            &namespace_hex,
            policy,
            view,
        )?;
        let source_id = planned.source_id;
        let revision_id = planned.revision_id;
        if !seen.insert(source_id.clone()) {
            return Err("DIRECT_DUPLICATE_SOURCE_IN_BATCH".to_owned());
        }
        let byte_length = u64::try_from(snapshot.bytes.len())
            .map_err(|_| "DIRECT_SOURCE_TOO_LARGE".to_owned())?;
        let previous = view.get(&source_id);
        if let Some(ref prior) = previous
            && prior.file_identity_digest != snapshot.file_identity_digest
        {
            return Err("DIRECT_SOURCE_ID_COLLISION".to_owned());
        }
        let changed = !previous.as_ref().is_some_and(|prior| {
            prior.is_active
                && prior.revision_id == revision_id
                && prior.path_digest == snapshot.path_digest
        });
        let source = IndexedSource {
            source_id,
            revision_id,
            content_digest: snapshot.content_digest.clone(),
            path_digest: snapshot.path_digest.clone(),
            byte_length,
            identity_strength: snapshot.identity_strength.tag(),
            changed,
        };
        let draft = if changed {
            let predecessor = previous
                .as_ref()
                .map_or(ZERO_DIGEST, |prior| prior.record_digest.as_str());
            let operation_id = sha256::hex(&sha256::digest_parts(
                b"eliot-search/direct-index-operation/v2",
                &[
                    source.source_id.as_bytes(),
                    source.revision_id.as_bytes(),
                    source.path_digest.as_bytes(),
                    predecessor.as_bytes(),
                ],
            ));
            Some(RecordDraft {
                operation_id,
                state: SourceState::Active,
                source_id: source.source_id.clone(),
                revision_id: source.revision_id.clone(),
                content_digest: source.content_digest.clone(),
                byte_length,
                file_identity_digest: snapshot.file_identity_digest.clone(),
                path_digest: source.path_digest.clone(),
                identity_strength: snapshot.identity_strength,
            })
        } else {
            None
        };
        Ok((snapshot, source, draft))
    }
}
