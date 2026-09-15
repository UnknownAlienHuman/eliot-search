//! All-before-write bounded batch planning and publication.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::spec::MAX_BATCH_INPUT_BYTES;
use super::super::super::{
    CONTROL_DIRECTORY, DirectStore, FileSnapshot, IndexedSource,
    MAX_DIRECTORY_FILES, SOURCE_LOG_FILE, load_registry, read_file_snapshot,
};

impl DirectStore {
    pub(super) fn index_paths_bounded(
        &mut self,
        paths: &[PathBuf],
        max_batch_bytes: usize,
        writer: &mut impl FnMut(
            &Self,
            &IndexedSource,
            &[u8],
        ) -> Result<(), String>,
    ) -> Result<Vec<IndexedSource>, String> {
        if max_batch_bytes == 0 || max_batch_bytes > MAX_BATCH_INPUT_BYTES {
            return Err("DIRECT_BATCH_LIMIT_INVALID".to_owned());
        }
        if paths.len() > MAX_DIRECTORY_FILES {
            return Err("DIRECT_DIRECTORY_FILE_LIMIT_EXCEEDED".to_owned());
        }
        let current = load_registry(
            &self.root.join(CONTROL_DIRECTORY).join(SOURCE_LOG_FILE),
        )?;
        if current.last_sequence != self.registry.last_sequence
            || current.last_digest != self.registry.last_digest
            || current.latest != self.registry.latest
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }

        // One coherent admission fence and replayed view for the entire batch.
        let policy = Self::admission_policy();
        let view = self.registry_view(&policy)?;

        // Every source is read under the remaining aggregate budget before any
        // storage writer can run.
        let mut snapshots: Vec<(PathBuf, FileSnapshot)> =
            Vec::with_capacity(paths.len());
        let mut retained_bytes = 0_usize;
        for path in paths {
            let remaining = max_batch_bytes
                .checked_sub(retained_bytes)
                .ok_or_else(|| "DIRECT_BATCH_BYTES_EXCEEDED".to_owned())?;
            let snapshot = read_file_snapshot(path, &self.root, remaining)?;
            retained_bytes = retained_bytes
                .checked_add(snapshot.bytes.len())
                .filter(|length| *length <= max_batch_bytes)
                .ok_or_else(|| "DIRECT_BATCH_BYTES_EXCEEDED".to_owned())?;
            snapshots.push((path.clone(), snapshot));
        }
        snapshots.sort_by(|left, right| {
            left.1.path_digest.cmp(&right.1.path_digest)
        });

        // Canonical admission and collision checks complete for every member
        // before the first possible CAS/write call.
        let mut seen = BTreeSet::new();
        let mut planned = Vec::with_capacity(snapshots.len());
        for (original_path, snapshot) in snapshots {
            planned.push(self.plan_snapshot(
                &original_path,
                snapshot,
                &mut seen,
                &policy,
                &view,
            )?);
        }

        let mut results = Vec::with_capacity(planned.len());
        let mut drafts = Vec::new();
        for (snapshot, source, draft) in planned {
            // A failed writer may leave an orphan immutable object, but no
            // source-catalog event is appended for the failed batch.
            writer(self, &source, &snapshot.bytes)?;
            if let Some(draft) = draft {
                drafts.push(draft);
            }
            results.push(source);
        }
        if !drafts.is_empty() {
            self.append_drafts(drafts)?;
        }
        Ok(results)
    }
}
