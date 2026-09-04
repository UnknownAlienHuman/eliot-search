//! Evidence-based revision-storage security status.
//!
//! The status is computed from the verified control-log reference set and a
//! complete bounded revision-directory inventory. A platform capability alone
//! never raises `encrypted_at_rest`: every referenced revision must have one
//! protected object, and no plaintext, temporary, unexpected, or malformed
//! protected object may remain anywhere below the revision root.

use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::Path;

use crate::maintenance::collect_orphan_revisions;
use crate::revision_protection::{
    PROTECTED_OBJECT_EXTENSION, RevisionProtector,
};
use crate::sha256;

const REVISION_DIRECTORY: &str = "revisions";
const PROTECTED_MAGIC_BYTES: usize = 8;
const MAX_REVISION_OBJECTS: usize = 2_000_000;

/// Exact current at-rest layout classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StorageSecurityStatus {
    pub(crate) backend: &'static str,
    pub(crate) protects_new_objects: bool,
    pub(crate) referenced_revisions: usize,
    pub(crate) referenced_protected_revisions: usize,
    pub(crate) missing_protected_revisions: usize,
    pub(crate) protected_objects: usize,
    pub(crate) plaintext_objects: usize,
    pub(crate) temporary_objects: usize,
    pub(crate) unexpected_objects: usize,
    pub(crate) malformed_protected_objects: usize,
    pub(crate) encrypted_at_rest: bool,
}

impl StorageSecurityStatus {
    /// Inspects one owner-fenced root without changing its contents.
    pub(crate) fn inspect(root: &Path) -> Result<Self, String> {
        let inventory = collect_orphan_revisions(root, false)?;
        let malformed_protected_objects = inspect_protected_headers(root)?;
        let missing_protected_revisions = inventory
            .referenced_revisions
            .saturating_sub(inventory.referenced_protected_objects);
        let protects_new_objects = cfg!(windows);
        let encrypted_at_rest = protects_new_objects
            && missing_protected_revisions == 0
            && inventory.plaintext_objects == 0
            && inventory.temporary_objects == 0
            && inventory.unexpected_objects == 0
            && malformed_protected_objects == 0;
        Ok(Self {
            backend: if cfg!(windows) {
                "windows-dpapi-credential-manager-v1"
            } else {
                "plaintext-development-v1"
            },
            protects_new_objects,
            referenced_revisions: inventory.referenced_revisions,
            referenced_protected_revisions: inventory
                .referenced_protected_objects,
            missing_protected_revisions,
            protected_objects: inventory.protected_objects,
            plaintext_objects: inventory.plaintext_objects,
            temporary_objects: inventory.temporary_objects,
            unexpected_objects: inventory.unexpected_objects,
            malformed_protected_objects,
            encrypted_at_rest,
        })
    }

    /// Complete JSON object suitable for embedding in health output.
    pub(crate) fn json(&self) -> String {
        format!(
            concat!(
                "{{\"backend\":\"{}\",",
                "\"protects_new_objects\":{},",
                "\"referenced_revisions\":{},",
                "\"referenced_protected_revisions\":{},",
                "\"missing_protected_revisions\":{},",
                "\"protected_objects\":{},",
                "\"plaintext_objects\":{},",
                "\"temporary_objects\":{},",
                "\"unexpected_objects\":{},",
                "\"malformed_protected_objects\":{},",
                "\"encrypted_at_rest\":{}}}"
            ),
            self.backend,
            self.protects_new_objects,
            self.referenced_revisions,
            self.referenced_protected_revisions,
            self.missing_protected_revisions,
            self.protected_objects,
            self.plaintext_objects,
            self.temporary_objects,
            self.unexpected_objects,
            self.malformed_protected_objects,
            self.encrypted_at_rest,
        )
    }
}

fn inspect_protected_headers(root: &Path) -> Result<usize, String> {
    let revisions = root.join(REVISION_DIRECTORY);
    ensure_directory(&revisions)?;
    let mut malformed = 0_usize;
    let mut observed = 0_usize;
    for shard in fs::read_dir(&revisions)
        .map_err(|error| format!("DIRECT_STORAGE_STATUS_READ_ERROR:{error}"))?
    {
        let shard = shard
            .map_err(|error| format!("DIRECT_STORAGE_STATUS_READ_ERROR:{error}"))?;
        let shard_path = shard.path();
        let metadata = fs::symlink_metadata(&shard_path)
            .map_err(|error| format!("DIRECT_STORAGE_STATUS_METADATA_ERROR:{error}"))?;
        let shard_name = shard.file_name();
        let shard_name = shard_name.to_string_lossy();
        if metadata.file_type().is_symlink()
            || is_reparse(&metadata)
            || !metadata.is_dir()
            || !valid_shard_name(&shard_name)
        {
            continue;
        }
        for entry in fs::read_dir(&shard_path)
            .map_err(|error| format!("DIRECT_STORAGE_STATUS_READ_ERROR:{error}"))?
        {
            let entry = entry
                .map_err(|error| format!("DIRECT_STORAGE_STATUS_READ_ERROR:{error}"))?;
            observed = observed.saturating_add(1);
            if observed > MAX_REVISION_OBJECTS {
                return Err("DIRECT_STORAGE_STATUS_OBJECT_LIMIT_EXCEEDED".to_owned());
            }
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let suffix = format!(".{PROTECTED_OBJECT_EXTENSION}");
            let Some(revision_id) = name.strip_suffix(&suffix) else {
                continue;
            };
            if sha256::decode_digest(revision_id).is_none()
                || !revision_id.starts_with(shard_name.as_ref())
            {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("DIRECT_STORAGE_STATUS_METADATA_ERROR:{error}"))?;
            if metadata.file_type().is_symlink()
                || is_reparse(&metadata)
                || !metadata.is_file()
            {
                malformed = malformed.saturating_add(1);
                continue;
            }
            let mut prefix = [0_u8; PROTECTED_MAGIC_BYTES];
            let read = File::open(&path)
                .and_then(|mut file| file.read(&mut prefix))
                .map_err(|error| format!("DIRECT_STORAGE_STATUS_READ_ERROR:{error}"))?;
            if read != PROTECTED_MAGIC_BYTES
                || !RevisionProtector::is_protected_object(&prefix)
            {
                malformed = malformed.saturating_add(1);
            }
        }
    }
    Ok(malformed)
}

fn valid_shard_name(value: &str) -> bool {
    value.len() == 2 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_STORAGE_STATUS_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_STORAGE_STATUS_DIRECTORY_INVALID".to_owned());
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &Metadata) -> bool {
    false
}
