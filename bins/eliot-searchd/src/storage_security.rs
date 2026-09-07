//! Evidence-based revision-storage security status.
//!
//! The status is computed from the control-log reference set and bounded storage
//! inventories. Preparation bodies are included in the at-rest layout check;
//! ciphertext presence is not proof of successful decryption or source admission.

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
            && malformed_protected_objects == 0
            && preparation_is_protected(root)?;
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
            let Some(revision_id) = name.strip_suffix(suffix.as_str()) else {
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

/// Inspect both current and orphan preparation entries. A plaintext development
/// artifact carried to Windows must not disappear from the encryption claim.
/// Reference bytes are technical hashes/lengths only; body authenticity is checked
/// by the owning preparation reader, not inferred from this inventory header.
fn preparation_is_protected(root: &Path) -> Result<bool, String> {
    let base = root.join("preparation");
    match fs::symlink_metadata(&base) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(_) => return Err("DIRECT_PREPARATION_STATUS_READ_FAILED".to_owned()),
        Ok(_) => ensure_directory(&base)?,
    }
    for entry in fs::read_dir(&base).map_err(preparation_status_error)? {
        let name = entry.map_err(preparation_status_error)?.file_name();
        if !matches!(name.to_str(), Some("refs" | "objects")) { return Ok(false); }
    }
    let mut observed = 0_usize;
    for (directory, suffix, magic) in [("refs", ".ref", *b"ELSPRF01"), ("objects", ".dpapi", *b"ELSRV2\0\0")] {
        let directory_path = base.join(directory);
        ensure_directory(&directory_path)?;
        let mut shards = 0;
        for shard in fs::read_dir(&directory_path).map_err(preparation_status_error)? {
            let shard = shard.map_err(preparation_status_error)?;
            shards += 1;
            let name = shard.file_name();
            let Some(shard_name) = name.to_str() else { return Ok(false); };
            if shards > 256 || !valid_shard_name(shard_name) { return Ok(false); }
            ensure_directory(&shard.path())?;
            for entry in fs::read_dir(shard.path()).map_err(preparation_status_error)? {
                let entry = entry.map_err(preparation_status_error)?;
                observed += 1;
                if observed > MAX_REVISION_OBJECTS {
                    return Err("DIRECT_PREPARATION_STATUS_LIMIT_EXCEEDED".to_owned());
                }
                let name = entry.file_name();
                let Some(id) = name.to_str().and_then(|name| name.strip_suffix(suffix)) else { return Ok(false); };
                if sha256::decode_digest(id).is_none() || !id.starts_with(shard_name) { return Ok(false); }
                let metadata = fs::symlink_metadata(entry.path()).map_err(preparation_status_error)?;
                if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse(&metadata)
                    || (directory == "refs" && metadata.len() != 81)
                    || metadata.len() > 65 * 1024 * 1024
                { return Ok(false); }
                let mut prefix = [0; 8];
                File::open(entry.path()).and_then(|mut file| file.read_exact(&mut prefix))
                    .map_err(preparation_status_error)?;
                if prefix != magic { return Ok(false); }
            }
        }
    }
    Ok(true)
}
fn preparation_status_error(_: std::io::Error) -> String {
    "DIRECT_PREPARATION_STATUS_READ_FAILED".to_owned()
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
