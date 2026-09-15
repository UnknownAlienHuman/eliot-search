//! Single-file and directory ingestion entrypoints.

use std::fs;
use std::path::Path;

use super::spec::MAX_BATCH_INPUT_BYTES;
use super::super::super::{
    DirectStore, IndexedSource, collect_regular_files, ensure_directory,
    path_identity_bytes,
};

impl DirectStore {
    /// Publishes metadata only after the composition-owned writer verifies
    /// immutable revision and preparation bytes.
    pub(crate) fn index_file_with_writer(
        &mut self,
        path: &Path,
        writer: &mut impl FnMut(
            &Self,
            &IndexedSource,
            &[u8],
        ) -> Result<(), String>,
    ) -> Result<IndexedSource, String> {
        self.index_paths_bounded(
            &[path.to_path_buf()],
            MAX_BATCH_INPUT_BYTES,
            writer,
        )?
        .pop()
        .ok_or_else(|| "DIRECT_INDEX_EMPTY_RESULT".to_owned())
    }

    /// Uses the same prepublication barrier for every directory member.
    pub(crate) fn index_directory_with_writer(
        &mut self,
        directory: &Path,
        writer: &mut impl FnMut(
            &Self,
            &IndexedSource,
            &[u8],
        ) -> Result<(), String>,
    ) -> Result<Vec<IndexedSource>, String> {
        ensure_directory(directory)?;
        let canonical_dir = fs::canonicalize(directory)
            .map_err(|error| {
                format!("DIRECT_DIRECTORY_CANONICALIZE_ERROR:{error}")
            })?;
        if canonical_dir == self.root {
            return Err("DIRECT_SOURCE_DIRECTORY_IS_DATA_ROOT".to_owned());
        }
        ensure_directory(&canonical_dir)?;
        let mut paths = Vec::new();
        collect_regular_files(&canonical_dir, &self.root, 0, &mut paths)?;
        paths.sort_by_key(|path| path_identity_bytes(path));
        self.index_paths_bounded(&paths, MAX_BATCH_INPUT_BYTES, writer)
    }
}
