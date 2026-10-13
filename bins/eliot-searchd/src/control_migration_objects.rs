//! Read-only object evidence for the legacy migration, under the existing owner.
//! Normal revision reads and migration inspect the same bytes with the same
//! decoder. Neither path creates credentials, repairs objects or reads sources.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Instant;
#[cfg(test)]
use std::time::Duration;
use zeroize::Zeroizing;

#[path = "control_migration_directories.rs"]
#[cfg(test)]
mod directories;
#[path = "control_migration_orphans.rs"]
#[cfg(test)]
mod orphans;
#[path = "control_migration_plan.rs"]
mod source_plan;

use super::{DirectStore, RevisionMetadata, MAX_REVISION_OBJECT_BYTES, REVISION_DIRECTORY,
    legacy_path, protected_path, read_regular_file, verify_plaintext, verify_revision_identity};
use super::storage_io::{ensure_directory, join_revision_path, read_revision_object_with_context, revision_object_path_with_context};
use super::migration_context::MigrationContext;
use search_revision_store::LegacyRevisionObjectWork as Work;
use crate::revision_protection::{RevisionProtector, PROTECTED_OBJECT_EXTENSION};
use crate::{development::MAX_SCAN_INPUT_BYTES, sha256};
#[cfg(test)]
use crate::service_output::json_string;

#[cfg(test)]
const MAX_PAGE_REVISIONS: usize = 16;
#[cfg(test)]
const MAX_PAGE_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
#[cfg(test)]
const MAX_PAGE_STORED_BYTES: u64 = 512 * 1024 * 1024;
#[cfg(test)]
const MAX_PAGE_BYTES: usize = 60 * 1024;
#[cfg(test)]
const DEADLINE: Duration = Duration::from_secs(30);

#[cfg(test)]
struct ObjectEvidence {
    encoded_bytes: u64,
    encoded_sha256: [u8; 32],
}

#[cfg(test)]
impl ObjectEvidence {
    fn from_bytes(bytes: &[u8]) -> Self {
        Self { encoded_bytes: bytes.len() as u64, encoded_sha256: sha256::digest(bytes) }
    }

    fn json(value: Option<&Self>) -> String {
        value.map_or_else(|| "null".to_owned(), |value| format!(
            "{{\"encoded_bytes\":{},\"encoded_sha256\":\"{}\"}}",
            value.encoded_bytes, sha256::hex(&value.encoded_sha256),
        ))
    }
}

struct RevisionReadback {
    bytes: Zeroizing<Vec<u8>>,
    #[cfg(test)]
    plaintext: Option<ObjectEvidence>,
    #[cfg(test)]
    protected: Option<ObjectEvidence>,
}

#[cfg(test)]
impl RevisionReadback {
    fn stored_bytes(&self) -> u64 {
        self.plaintext.as_ref().map_or(0, |value| value.encoded_bytes)
            + self.protected.as_ref().map_or(0, |value| value.encoded_bytes)
    }

    fn json(&self, revision: &RevisionMetadata, preparation: &str) -> String {
        format!(
            concat!(
                "{{\"source_id\":{},\"legacy_revision_id\":{},\"content_sha256\":{},",
                "\"byte_length\":{},\"plaintext_object\":{},\"protected_object\":{},",
                "\"content_verified\":true,\"preparation\":{}}}"
            ),
            json_string(&revision.source_id), json_string(&revision.revision_id),
            json_string(&revision.content_digest), revision.byte_length,
            ObjectEvidence::json(self.plaintext.as_ref()), ObjectEvidence::json(self.protected.as_ref()), preparation,
        )
    }
}

/// Administrative bookmark; a caller can choose a suffix, not assert prior work.
#[cfg(test)]
struct Cursor {
    checkpoint: [u8; 32],
    after: String,
}

#[cfg(test)]
impl Cursor {
    fn parse(value: &str) -> Result<Self, String> {
        // r2.<checkpoint hex>.<last revision hex>; reject before byte slicing.
        if value.len() != 132 || !value.is_ascii() || !value.starts_with("r2.")
            || value.as_bytes()[67] != b'.'
        {
            return Err("DIRECT_MIGRATION_REVISION_CURSOR_INVALID".to_owned());
        }
        let checkpoint = sha256::decode_digest(&value[3..67])
            .ok_or_else(|| "DIRECT_MIGRATION_REVISION_CURSOR_INVALID".to_owned())?;
        let after = sha256::decode_digest(&value[68..])
            .ok_or_else(|| "DIRECT_MIGRATION_REVISION_CURSOR_INVALID".to_owned())?;
        if sha256::hex(&checkpoint) != value[3..67] || sha256::hex(&after) != value[68..] {
            return Err("DIRECT_MIGRATION_REVISION_CURSOR_INVALID".to_owned());
        }
        Ok(Self { checkpoint, after: value[68..].to_owned() })
    }
}

impl DirectStore {
    /// Normal reads retain the same storage policy, but also verify a second
    /// representation when present. A bad protected object never falls back to
    /// plaintext, and dangling links cannot be mistaken for an absent object.
    pub(super) fn read_revision_detailed(&self, metadata: &RevisionMetadata) -> Result<Vec<u8>, String> {
        let mut observed = self.read_revision_objects(metadata, None)?;
        // Transfer the successful bytes to the existing caller; rejected buffers
        // and temporary plaintext copies stay inside their zeroizing owners.
        Ok(std::mem::take(&mut *observed.bytes))
    }

    /// Page all retained object identities, not just the latest active sources.
    /// Each result binds the same source-chain snapshot as control-migration-page.
    /// This is not an atomic filesystem snapshot or a complete canonical import:
    /// an importer must revalidate these object fingerprints before cutover.
    #[cfg(test)]
    pub(crate) fn inspect_migration_revisions(&self, cursor: Option<&str>) -> Result<String, String> {
        // The same administrative command has an explicit physical-orphan mode.
        // Subsequent o1 bookmarks keep that mode; an ordinary r2 page never mixes it.
        if cursor == Some("orphans") || cursor.is_some_and(|value| value.starts_with("o1.")) {
            return self.inspect_migration_orphans(cursor.filter(|value| *value != "orphans"));
        }
        // All physical preparation files, including unresolved old-profile residue.
        if cursor == Some("preparation-files") || cursor.is_some_and(|value| value.starts_with("p1.")) {
            return self.inspect_migration_preparation_files(cursor.filter(|value| *value != "preparation-files"));
        }
        let deadline = Instant::now().checked_add(DEADLINE)
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        let cursor = cursor.map(Cursor::parse).transpose()?;
        let snapshot = self.inner.verify_migration_snapshot(deadline)?;
        let checkpoint = sha256::digest_parts(b"eliot-search/control-migration-revision-cursor/v2", &[
            &snapshot, self.protector.backend_name().as_bytes(), &crate::direct_preparation::profile_digest(),
        ]);
        if cursor.as_ref().is_some_and(|value| value.checkpoint != checkpoint
            || self.inner.retained_revision(&value.after).is_none())
        {
            return Err("DIRECT_MIGRATION_REVISION_CURSOR_STALE".to_owned());
        }
        let after = cursor.as_ref().map(|value| value.after.as_str());
        let mut pending = self.inner.retained_revisions_after(after).peekable();
        let mut entries = Vec::with_capacity(MAX_PAGE_REVISIONS);
        let mut last = after.unwrap_or("").to_owned();
        let (mut source_bytes, mut stored_bytes) = (0_u64, 0_u64);
        let (mut plaintext_objects, mut protected_objects) = (0_usize, 0_usize);
        let (mut preparation_records, mut preparation_missing) = (0_usize, 0_usize);
        while let Some(metadata) = pending.peek() {
            check_deadline(Some(deadline))?;
            // Reserve room for both possible encodings before the next read.
            // Actual encoded bytes come from readback, never source-log lengths.
            let object_ceiling = metadata.byte_length.checked_add(2 * MAX_REVISION_OBJECT_BYTES as u64 + 81)
                .ok_or_else(|| "DIRECT_MIGRATION_BYTES_EXCEEDED".to_owned())?;
            if entries.len() == MAX_PAGE_REVISIONS
                || source_bytes.checked_add(metadata.byte_length)
                    .is_none_or(|value| value > MAX_PAGE_SOURCE_BYTES)
                || stored_bytes.checked_add(object_ceiling)
                    .is_none_or(|value| value > MAX_PAGE_STORED_BYTES)
            {
                if entries.is_empty() { return Err("DIRECT_MIGRATION_BYTES_EXCEEDED".to_owned()); }
                break;
            }
            let metadata = pending.next().ok_or_else(|| "DIRECT_MIGRATION_NO_PROGRESS".to_owned())?;
            let observed = self.read_revision_objects(&metadata, Some(deadline))?;
            let preparation = super::preparation_store::inspect(
                &self.root, &self.protector, &self.namespace_id(), &metadata, &observed.bytes, deadline,
            )?;
            source_bytes += metadata.byte_length;
            stored_bytes += observed.stored_bytes() + preparation.stored_bytes;
            preparation_records += usize::from(preparation.present);
            preparation_missing += usize::from(!preparation.present);
            plaintext_objects += usize::from(observed.plaintext.is_some());
            protected_objects += usize::from(observed.protected.is_some());
            entries.push(observed.json(&metadata, &preparation.json));
            last = metadata.revision_id;
            // No revision body survives into the next iteration or output frame.
        }
        let exhausted = pending.peek().is_none();
        if self.inner.verify_migration_snapshot(deadline)? != snapshot {
            return Err("DIRECT_MIGRATION_REVISION_CURSOR_STALE".to_owned());
        }
        let body = entries.join(",");
        let page_digest = sha256::digest_parts(b"eliot-search/control-migration-revision-page/v2", &[
            &checkpoint, after.unwrap_or("").as_bytes(), last.as_bytes(), body.as_bytes(),
        ]);
        let next_cursor = if exhausted { "null".to_owned() }
            else { json_string(&format!("r2.{}.{last}", sha256::hex(&checkpoint))) };
        let output = format!(
            concat!(
                "{{\"event\":\"control_migration_revisions\",\"schema\":\"legacy-revision-readback-v2\",",
                "\"scope\":\"referenced_revisions_and_current_preparation\",\"namespace_id\":{},",
                "\"catalog_snapshot_sha256\":\"{}\",\"backend\":{},",
                "\"referenced_revision_ids\":{},\"after_revision\":{},\"last_revision\":{},",
                "\"page_revisions\":{},\"source_bytes\":{},\"stored_bytes\":{},",
                "\"plaintext_objects\":{},\"protected_objects\":{},\"entries\":[{}],",
                "\"page_sha256\":\"{}\",\"next_cursor\":{},\"exhausted\":{},",
                "\"read_only\":true,\"page_payloads_verified\":true,",
                "\"orphans_enumerated\":false,\"derived_preparation_verified\":{},",
                "\"preparation_records_verified\":{},\"preparation_missing\":{},\"preparation_profile_sha256\":\"{}\",",
                "\"cutover_revalidation_required\":true,",
                "\"canonical_mapping_complete\":false}}"
            ),
            json_string(&self.namespace_id()), sha256::hex(&snapshot), json_string(self.protector.backend_name()),
            self.inner.retained_revisions().len(), after.map_or_else(|| "null".to_owned(), json_string),
            if last.is_empty() { "null".to_owned() } else { json_string(&last) },
            entries.len(), source_bytes, stored_bytes, plaintext_objects, protected_objects, body,
            sha256::hex(&page_digest), next_cursor, exhausted, preparation_missing == 0,
            preparation_records, preparation_missing, sha256::hex(&crate::direct_preparation::profile_digest()),
        );
        if output.len() > MAX_PAGE_BYTES { return Err("DIRECT_MIGRATION_PAGE_TOO_LARGE".to_owned()); }
        check_deadline(Some(deadline))?;
        Ok(output)
    }

    fn read_revision_objects(
        &self, metadata: &RevisionMetadata, deadline: Option<Instant>,
    ) -> Result<RevisionReadback, String> {
        read_revision_objects_at(&self.root, Some(&self.protector), metadata, cfg!(windows), deadline)
    }
}

/// Explicit legacy import can read a verified plaintext object on Windows without
/// creating a credential or converting the source. Normal serving retains its
/// protected-only Windows floor. A present protected copy is never ignored.
fn read_import_revision(
    root: &Path, protector: Option<&RevisionProtector>, metadata: &RevisionMetadata,
    deadline: Instant,
) -> Result<Zeroizing<Vec<u8>>, String> {
    read_revision_objects_at(root, protector, metadata, false, Some(deadline)).map(|readback| readback.bytes)
}

fn read_import_revision_with_context(
    root: &Path, protector: Option<&RevisionProtector>, metadata: &RevisionMetadata,
    context: &MigrationContext<'_, '_>,
) -> Result<Zeroizing<Vec<u8>>, String> {
    read_revision_objects_at_with_context(
        root, protector, metadata, false, Some(context.deadline()), Some(context),
    ).map(|readback| readback.bytes)
}

fn read_revision_objects_at(
    root: &Path, protector: Option<&RevisionProtector>, metadata: &RevisionMetadata,
    require_protected: bool, deadline: Option<Instant>,
) -> Result<RevisionReadback, String> {
    read_revision_objects_at_with_context(root, protector, metadata, require_protected, deadline, None)
}

fn read_revision_objects_at_with_context(
    root: &Path, protector: Option<&RevisionProtector>, metadata: &RevisionMetadata,
    require_protected: bool, deadline: Option<Instant>, context: Option<&MigrationContext<'_, '_>>,
) -> Result<RevisionReadback, String> {
    revision_checkpoint(deadline, context)?;
    revision_charge(context, Work::Compare(metadata.revision_id.len()))?;
    verify_revision_identity(metadata)?;
    if metadata.byte_length > MAX_SCAN_INPUT_BYTES as u64 {
        return Err("DIRECT_REVISION_LENGTH_MISMATCH".to_owned());
    }
    revision_directory(root, deadline, context)?;
    let revisions = join_revision_path(root, Path::new(REVISION_DIRECTORY), context)?;
    revision_directory(&revisions, deadline, context)?;
    let shard = join_revision_path(&revisions, Path::new(&metadata.revision_id[..2]), context)?;
    revision_directory(&shard, deadline, context)?;
    let protected_path = context.map_or_else(
        || protected_path(root, &metadata.revision_id),
        |context| revision_object_path_with_context(root, &metadata.revision_id, PROTECTED_OBJECT_EXTENSION, Some(context)),
    )?;
    let plaintext_path = context.map_or_else(
        || legacy_path(root, &metadata.revision_id),
        |context| revision_object_path_with_context(root, &metadata.revision_id, "bin", Some(context)),
    )?;
    let encoded = read_optional(&protected_path, MAX_REVISION_OBJECT_BYTES, deadline, context)?;
    #[cfg(test)]
    let protected = encoded.as_deref().map(|bytes| ObjectEvidence::from_bytes(bytes));
    let mut protected_bytes = if let Some(encoded) = encoded.as_ref() {
        revision_checkpoint(deadline, context)?;
        let protector = protector.ok_or_else(|| "DIRECT_REVISION_KEY_UNAVAILABLE".to_owned())?;
        revision_charge(context, Work::Allocate(usize::try_from(metadata.byte_length)
            .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH".to_owned())?))?;
        revision_charge(context, Work::Observe)?;
        let decoded = protector.unprotect(
            encoded, &metadata.revision_id, &metadata.content_digest, metadata.byte_length,
        );
        revision_checkpoint(deadline, context)?;
        Some(Zeroizing::new(decoded?))
    } else { None };
    drop(encoded);
    revision_checkpoint(deadline, context)?;
    let plaintext_bytes = read_optional(&plaintext_path, MAX_SCAN_INPUT_BYTES, deadline, context)?;
    if let Some(bytes) = plaintext_bytes.as_ref() {
        revision_charge(context, Work::Compare(bytes.len()))?;
        verify_plaintext(metadata, bytes)?;
        revision_checkpoint(deadline, context)?;
    }
    #[cfg(test)]
    let plaintext = plaintext_bytes.as_deref().map(|bytes| ObjectEvidence::from_bytes(bytes));
    if let Some(bytes) = protected_bytes.as_ref() {
        revision_charge(context, Work::Compare(bytes.len()))?;
        verify_plaintext(metadata, bytes)?;
        revision_checkpoint(deadline, context)?;
    }
    if let (Some(protected), Some(_)) = (protected_bytes.as_ref(), plaintext_bytes.as_ref()) {
        revision_charge(context, Work::Compare(protected.len()))?;
    }
    if let (Some(protected), Some(plaintext)) = (protected_bytes.as_ref(), plaintext_bytes.as_ref())
        && protected.as_slice() != plaintext.as_slice()
    {
        return Err("DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned());
    }
    let bytes = match (protected_bytes.take(), plaintext_bytes) {
        (Some(bytes), _) => bytes,
        (None, Some(bytes)) if !require_protected => bytes,
        (None, Some(_)) => return Err("DIRECT_REVISION_PROTECTION_INCOMPLETE".to_owned()),
        (None, None) => return Err("DIRECT_REVISION_MISSING".to_owned()),
    };
    revision_checkpoint(deadline, context)?;
    Ok(RevisionReadback {
        bytes,
        #[cfg(test)]
        plaintext,
        #[cfg(test)]
        protected,
    })
}

fn read_optional(
    path: &Path, maximum: usize, deadline: Option<Instant>, context: Option<&MigrationContext<'_, '_>>,
) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
    revision_checkpoint(deadline, context)?;
    revision_charge(context, Work::Observe)?;
    let inspected = fs::symlink_metadata(path);
    revision_checkpoint(deadline, context)?;
    match inspected {
        Ok(_) => {
            let read = context.map_or_else(
                || read_regular_file(path, maximum, "DIRECT_REVISION_OBJECT_READ_FAILED"),
                |context| read_revision_object_with_context(path, maximum, context),
            );
            revision_checkpoint(deadline, context)?;
            read.map(Zeroizing::new).map(Some)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err("DIRECT_REVISION_OBJECT_INSPECTION_FAILED".to_owned()),
    }
}

fn revision_checkpoint(deadline: Option<Instant>, context: Option<&MigrationContext<'_, '_>>) -> Result<(), String> {
    context.map_or_else(|| check_deadline(deadline), MigrationContext::checkpoint)
}

fn revision_charge(context: Option<&MigrationContext<'_, '_>>, work: Work) -> Result<(), String> {
    context.map_or(Ok(()), |context| context.charge(work))
}

fn revision_directory(path: &Path, deadline: Option<Instant>, context: Option<&MigrationContext<'_, '_>>) -> Result<(), String> {
    revision_checkpoint(deadline, context)?;
    revision_charge(context, Work::Observe)?;
    let result = ensure_directory(path);
    revision_checkpoint(deadline, context)?;
    result
}

fn check_deadline(deadline: Option<Instant>) -> Result<(), String> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())
    } else { Ok(()) }
}
