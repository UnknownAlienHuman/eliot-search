//! Test-only plaintext revision fixtures and exact verification.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::development::MAX_SCAN_INPUT_BYTES;
use crate::sha256;

use super::super::catalog::{RevisionMetadata, verify_revision_identity};
use super::super::model::{
    DirectStore, IndexedSource, REVISION_DIRECTORY, SourceRecord, SourceState,
    StoreVerification,
};
use super::filesystem::{
    ensure_child_directory, ensure_regular_file, is_reparse, sync_directory,
};
use super::validation::validate_digest_text;

impl DirectStore {
    /// Indexes one exact same-handle snapshot using the development writer.
    pub(crate) fn index_file(&mut self, path: &Path) -> Result<IndexedSource, String> {
        self.index_file_with_writer(path, &mut |store, source, bytes| {
            store.persist_revision(&source.revision_id, &source.content_digest, bytes)
        })
    }

    /// Verifies the log chain and every unique referenced immutable revision.
    pub(crate) fn verify(&self) -> Result<StoreVerification, String> {
        self.verify_control()?;
        let reloaded = &self.registry;
        let mut verified_revisions = 0_usize;
        let mut total_revision_bytes = 0_u64;
        for record in reloaded.revisions.values() {
            let bytes = self
                .read_verified_revision(record)
                .map_err(str::to_owned)?;
            total_revision_bytes = total_revision_bytes
                .checked_add(
                    u64::try_from(bytes.len())
                        .map_err(|_| "DIRECT_TOTAL_BYTES_OVERFLOW".to_owned())?,
                )
                .ok_or_else(|| "DIRECT_TOTAL_BYTES_OVERFLOW".to_owned())?;
            verified_revisions = verified_revisions.saturating_add(1);
        }
        Ok(StoreVerification {
            source_events: reloaded.event_count,
            registered_sources: reloaded.latest.len(),
            active_sources: reloaded
                .latest
                .values()
                .filter(|record| record.state == SourceState::Active)
                .count(),
            referenced_revisions: reloaded.revisions.len(),
            verified_revisions,
            total_revision_bytes,
        })
    }

    pub(in super::super) fn persist_revision(
        &self,
        revision_id: &str,
        expected_content_digest: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        validate_digest_text(revision_id, "DIRECT_REVISION_ID_INVALID")?;
        validate_digest_text(
            expected_content_digest,
            "DIRECT_CONTENT_DIGEST_INVALID",
        )?;
        let shard = self
            .root
            .join(REVISION_DIRECTORY)
            .join(&revision_id[..2]);
        ensure_child_directory(&shard)?;
        let path = shard.join(format!("{revision_id}.bin"));
        if path.exists() {
            verify_revision_path(&path, expected_content_digest, bytes.len())?;
            return Ok(());
        }

        let temporary = shard.join(format!(".{revision_id}.{}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| format!("DIRECT_REVISION_CREATE_ERROR:{error}"))?;
        let write_result = file
            .write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("DIRECT_REVISION_WRITE_ERROR:{error}"));
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        drop(file);
        if let Err(error) = fs::rename(&temporary, &path) {
            let _ = fs::remove_file(&temporary);
            if path.exists() {
                verify_revision_path(&path, expected_content_digest, bytes.len())?;
            } else {
                return Err(format!("DIRECT_REVISION_RENAME_ERROR:{error}"));
            }
        }
        #[cfg(unix)]
        sync_directory(&shard)?;
        #[cfg(not(unix))]
        sync_directory(&shard);
        verify_revision_path(&path, expected_content_digest, bytes.len())
    }

    fn read_verified_revision(&self, record: &SourceRecord) -> Result<Vec<u8>, &'static str> {
        let path = revision_path(&self.root, &record.revision_id)
            .map_err(|_| "DIRECT_REVISION_ID_INVALID")?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| "DIRECT_REVISION_MISSING")?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
            return Err("DIRECT_REVISION_OBJECT_INVALID");
        }
        if metadata.len() != record.byte_length
            || metadata.len() > u64::try_from(MAX_SCAN_INPUT_BYTES).unwrap_or(u64::MAX)
        {
            return Err("DIRECT_REVISION_LENGTH_MISMATCH");
        }
        let mut file = File::open(&path).map_err(|_| "DIRECT_REVISION_OPEN_FAILED")?;
        let mut bytes = Vec::with_capacity(
            usize::try_from(record.byte_length)
                .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH")?,
        );
        (&mut file)
            .take(u64::try_from(MAX_SCAN_INPUT_BYTES + 1).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes)
            .map_err(|_| "DIRECT_REVISION_READ_FAILED")?;
        if bytes.len() != usize::try_from(record.byte_length).unwrap_or(usize::MAX) {
            return Err("DIRECT_REVISION_LENGTH_MISMATCH");
        }
        let content_digest = sha256::hex(&sha256::digest(&bytes));
        if content_digest != record.content_digest {
            return Err("DIRECT_REVISION_CONTENT_MISMATCH");
        }
        verify_revision_identity(&RevisionMetadata::from(record))
            .map_err(|_| "DIRECT_REVISION_ID_MISMATCH")?;
        Ok(bytes)
    }
}

fn revision_path(root: &Path, revision_id: &str) -> Result<PathBuf, String> {
    validate_digest_text(revision_id, "DIRECT_REVISION_ID_INVALID")?;
    Ok(root
        .join(REVISION_DIRECTORY)
        .join(&revision_id[..2])
        .join(format!("{revision_id}.bin")))
}

fn verify_revision_path(
    path: &Path,
    expected_content_digest: &str,
    expected_length: usize,
) -> Result<(), String> {
    ensure_regular_file(path)?;
    let metadata = fs::metadata(path)
        .map_err(|error| format!("DIRECT_REVISION_METADATA_ERROR:{error}"))?;
    if metadata.len() != u64::try_from(expected_length).unwrap_or(u64::MAX) {
        return Err("DIRECT_REVISION_LENGTH_MISMATCH".to_owned());
    }
    let mut bytes = Vec::with_capacity(expected_length);
    File::open(path)
        .and_then(|file| {
            file.take(u64::try_from(MAX_SCAN_INPUT_BYTES + 1).unwrap_or(u64::MAX))
                .read_to_end(&mut bytes)
        })
        .map_err(|error| format!("DIRECT_REVISION_READ_ERROR:{error}"))?;
    if bytes.len() != expected_length
        || sha256::hex(&sha256::digest(&bytes)) != expected_content_digest
    {
        return Err("DIRECT_REVISION_CONTENT_MISMATCH".to_owned());
    }
    Ok(())
}
