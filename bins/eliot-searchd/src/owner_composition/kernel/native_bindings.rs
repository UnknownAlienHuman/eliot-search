//! Fixed native object identity profile for the explicit DIRECT root layout.

use std::fs::{self, File, OpenOptions};
use std::path::Path;

use search_contracts::{
    BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalValue, DigestInputLimit,
    blake3_canonical,
};
use search_runtime_owner::OwnerError;

use super::codec::is_reparse;
use super::read_existing::open_bound_directory;
use super::spec::{INSTALLATION_FILE, OWNER_SLOT_A, OWNER_SLOT_B};

/// The ordered CBOR array contains only native identities, never paths/content.
/// Windows identity is volume serial, file index and creation FILETIME; Unix
/// identity is device/inode. Mutable length, mtime and file bytes are excluded.
pub(super) fn native_objects_digest(root: &Path) -> Result<[u8; 32], OwnerError> {
    digest_objects(&open_objects(root)?)
}

/// Retained native layout objects. Windows sharing prevents locator replacement
/// until every child closes; these handles carry no independent owner authority.
pub(crate) struct NativeLayoutPins {
    files: Vec<File>,
}

impl NativeLayoutPins {
    pub(super) fn acquire(root: &Path) -> Result<Self, OwnerError> {
        Ok(Self {
            files: open_objects(root)?,
        })
    }

    pub(crate) fn verify(&self, root: &Path) -> Result<(), OwnerError> {
        if digest_objects(&self.files)? != native_objects_digest(root)? {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        Ok(())
    }
}

fn open_objects(root: &Path) -> Result<Vec<File>, OwnerError> {
    let objects = [
        ("", true),
        (".eliot-search-owner.lock", false),
        (".eliot-search-sealed-owner.lock", false),
        ("control", true),
        ("revisions", true),
        (INSTALLATION_FILE, false),
        (OWNER_SLOT_A, false),
        (OWNER_SLOT_B, false),
        ("control/namespace.id", false),
        ("control/source-events.log", false),
    ];
    let mut files = Vec::with_capacity(objects.len());
    for (name, directory) in objects {
        let path = root.join(name);
        let metadata = fs::symlink_metadata(&path).map_err(|_| OwnerError::OwnerGuardMismatch)?;
        if metadata.file_type().is_symlink()
            || is_reparse(&metadata)
            || (directory && !metadata.is_dir())
            || (!directory && !metadata.is_file())
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let file = if directory {
            open_bound_directory(&path)?
        } else {
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.custom_flags(0x0020_0000).share_mode(0x3);
            }
            options
                .open(&path)
                .map_err(|_| OwnerError::OwnerGuardMismatch)?
        };
        let opened = file
            .metadata()
            .map_err(|_| OwnerError::OwnerGuardMismatch)?;
        if is_reparse(&opened)
            || (directory && !opened.is_dir())
            || (!directory && !opened.is_file())
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        files.push(file);
    }
    Ok(files)
}

fn digest_objects(files: &[File]) -> Result<[u8; 32], OwnerError> {
    let identities = files
        .iter()
        .map(|file| {
            BoundedBytes::new(native_identity(file)?)
                .map(CanonicalValue::Bytes)
                .map_err(|_| OwnerError::DataRootInvalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let domain = CanonicalDigestDomain::parse("eliot/cbor/data-root-native-objects/v1")
        .map_err(|_| OwnerError::DataRootInvalid)?;
    let limit = DigestInputLimit::new(4096).map_err(|_| OwnerError::DataRootInvalid)?;
    let value = CanonicalValue::Array(
        BoundedList::new(identities).map_err(|_| OwnerError::DataRootInvalid)?,
    );
    let digest =
        blake3_canonical(&domain, &value, limit).map_err(|_| OwnerError::DataRootInvalid)?;
    Ok(*digest.as_bytes())
}

fn native_identity(file: &File) -> Result<Vec<u8>, OwnerError> {
    #[cfg(windows)]
    {
        let observed = eliot_searchd::native_file::observe(file)
            .map_err(|_| OwnerError::OwnerGuardMismatch)?;
        let mut bytes = Vec::with_capacity(20);
        bytes.extend_from_slice(&observed.volume_serial.to_be_bytes());
        bytes.extend_from_slice(&observed.file_index.to_be_bytes());
        bytes.extend_from_slice(&observed.creation_time.to_be_bytes());
        Ok(bytes)
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file
            .metadata()
            .map_err(|_| OwnerError::OwnerGuardMismatch)?;
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&metadata.dev().to_be_bytes());
        bytes.extend_from_slice(&metadata.ino().to_be_bytes());
        Ok(bytes)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = file;
        Err(OwnerError::DataRootInvalid)
    }
}
