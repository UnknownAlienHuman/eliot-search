//! Existing native handles and bounded exact same-object readback.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use search_contracts::{
    BoundedBytes, CanonicalDigestDomain, CanonicalValue, DigestInputLimit, blake3_canonical_array,
};
use search_runtime_owner::OwnerError;

use super::codec::is_reparse;
use super::native_request::{NativeRootRequest, NativeWork};
use super::spec::{INSTALLATION_FILE, OWNER_SLOT_A, OWNER_SLOT_B};

pub(super) const OBJECTS: [(&str, bool); 10] = [
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

pub(super) fn metadata(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    path: &Path,
) -> Result<Option<Metadata>, OwnerError> {
    work.call(request, || match fs::symlink_metadata(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(OwnerError::OwnerRecoveryQuarantined),
    })
}

pub(super) fn require_kind(metadata: &Metadata, directory: bool) -> Result<(), OwnerError> {
    if metadata.file_type().is_symlink()
        || is_reparse(metadata)
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    Ok(())
}

pub(super) fn open_existing(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    path: &Path,
    directory: bool,
    writable: bool,
) -> Result<File, OwnerError> {
    open_existing_with_metadata(request, work, path, directory, writable).map(|(file, _)| file)
}

/// Returns the metadata already observed on this opened handle, without another dispatch.
pub(super) fn open_existing_with_metadata(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    path: &Path,
    directory: bool,
    writable: bool,
) -> Result<(File, Metadata), OwnerError> {
    open_with_sharing(request, work, path, directory, writable, 3)
}

/// Initialization retains read handles that exclude writes and deletion of parents.
pub(super) fn open_initialization(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    path: &Path,
    directory: bool,
) -> Result<File, OwnerError> {
    open_with_sharing(request, work, path, directory, false, 1).map(|(file, _)| file)
}

fn open_with_sharing(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    path: &Path,
    directory: bool,
    writable: bool,
    sharing: u32,
) -> Result<(File, Metadata), OwnerError> {
    let before = metadata(request, work, path)?.ok_or(OwnerError::OwnerRecoveryEvidenceMissing)?;
    require_kind(&before, directory)?;
    work.open(request)?;
    let file = work.call(request, || {
        let mut options = OpenOptions::new();
        options.read(true).write(writable);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options
                .custom_flags(if directory { 0x0220_0000 } else { 0x0020_0000 })
                .share_mode(sharing);
        }
        #[cfg(not(windows))]
        let _ = sharing;
        options
            .open(path)
            .map_err(|_| OwnerError::OwnerGuardMismatch)
    })?;
    let opened = work.call(request, || {
        file.metadata().map_err(|_| OwnerError::OwnerGuardMismatch)
    })?;
    require_kind(&opened, directory)?;
    Ok((file, opened))
}

/// Fixed ordered native profile. It does not attest file contents or a store.
pub(super) struct NativePins {
    pub(super) direct: Option<DirectPin>,
    pub(super) files: Vec<File>,
}

/// Auxiliary custody constructed only after the real Direct journal has verified.
pub(super) struct DirectPin { file: File, observed: Vec<u8> }
impl DirectPin {
    pub(super) const fn borrow_file(&self) -> &File { &self.file }
    pub(super) const fn verified(file: File, observed: Vec<u8>) -> Self { Self { file, observed } }
    pub(super) fn check(&self, request: &NativeRootRequest, work: &mut NativeWork, root: &Path) -> Result<(), OwnerError> {
        let current = open_existing(request, work, &root.join("control/control.redb"), false, false)?;
        for file in [&self.file, &current] {
            if work.call(request, || native_identity(file))? != self.observed
                || work.call(request, || eliot_searchd::native_file::hardlink_count(file).map_err(|_| OwnerError::OwnerGuardMismatch))? != 1 {
                return Err(OwnerError::OwnerGuardMismatch);
            }
        }
        request.check()
    }
}

impl NativePins {
    pub(super) fn acquire_profile(
        request: &NativeRootRequest,
        work: &mut NativeWork,
        root: &Path,
        mutation: bool,
        journal: bool,
    ) -> Result<Self, OwnerError> {
        if journal {
            Self::acquire_with_journal(request, work, root, mutation)
        } else {
            Self::acquire(request, work, root, mutation)
        }
    }
    pub(super) fn acquire_initialization(
        request: &NativeRootRequest,
        work: &mut NativeWork,
        root: &Path,
    ) -> Result<Self, OwnerError> {
        let mut files = Vec::with_capacity(11);
        for (index, (name, directory)) in OBJECTS.iter().enumerate() {
            let path = root.join(name);
            // Actual writable exclusion handles were already acquired and retained.
            files.push(if matches!(index, 1 | 2) {
                open_existing(request, work, &path, *directory, false)?
            } else {
                open_initialization(request, work, &path, *directory)?
            });
        }
        files.push(open_initialization(
            request,
            work,
            &root.join(super::initialization_journal::JOURNAL_FILE),
            false,
        )?);
        Ok(Self { files, direct: None })
    }
    pub(super) fn acquire_with_journal(
        request: &NativeRootRequest,
        work: &mut NativeWork,
        root: &Path,
        mutation: bool,
    ) -> Result<Self, OwnerError> {
        let mut pins = Self::acquire(request, work, root, mutation)?;
        pins.files.push(open_existing(
            request,
            work,
            &root.join(super::initialization_journal::JOURNAL_FILE),
            false,
            false,
        )?);
        Ok(pins)
    }
    pub(super) fn acquire(
        request: &NativeRootRequest,
        work: &mut NativeWork,
        root: &Path,
        mutation: bool,
    ) -> Result<Self, OwnerError> {
        let mut files = Vec::with_capacity(OBJECTS.len());
        for (index, (name, directory)) in OBJECTS.iter().enumerate() {
            files.push(open_existing(
                request,
                work,
                &root.join(name),
                *directory,
                mutation && matches!(index, 6 | 7),
            )?);
        }
        Ok(Self { files, direct: None })
    }

    pub(super) fn digest(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
    ) -> Result<[u8; 32], OwnerError> {
        if self.files.len() == 11 {
            let mut identities = [[0; 20]; 11];
            for (file, identity) in self.files.iter().zip(&mut identities) {
                *identity = work
                    .call(request, || native_identity(file))?
                    .try_into()
                    .map_err(|_| OwnerError::OwnerGuardMismatch)?;
            }
            return super::initialization_journal::native_objects_digest(
                &identities,
                &mut |bytes| work.canonical(request, bytes),
            );
        }
        if self.files.len() != 10 {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let mut identities = Vec::with_capacity(OBJECTS.len());
        for file in &self.files {
            let bytes = work.call(request, || native_identity(file))?;
            identities.push(
                BoundedBytes::new(bytes)
                    .map(CanonicalValue::Bytes)
                    .map_err(|_| OwnerError::DataRootInvalid)?,
            );
        }
        let domain = CanonicalDigestDomain::parse("eliot/cbor/data-root-native-objects/v1")
            .map_err(|_| OwnerError::DataRootInvalid)?;
        let digest = blake3_canonical_array(
            &domain,
            identities.len(),
            identities.into_iter().map(Ok),
            DigestInputLimit::new(4096).map_err(|_| OwnerError::DataRootInvalid)?,
        )
        .map_err(|_| OwnerError::DataRootInvalid)?;
        request.check()?;
        Ok(*digest.as_bytes())
    }

    pub(super) fn verify_locators(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
        root: &Path,
    ) -> Result<(), OwnerError> {
        if !matches!(self.files.len(), 10 | 11) {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let journal = (super::initialization_journal::JOURNAL_FILE, false);
        for ((name, directory), held) in OBJECTS
            .iter()
            .chain(std::iter::once(&journal))
            .zip(&self.files)
        {
            let current = open_existing(request, work, &root.join(name), *directory, false)?;
            let original = work.call(request, || native_identity(held))?;
            if work.call(request, || native_identity(&current))? != original {
                return Err(OwnerError::OwnerGuardMismatch);
            }
        }
        request.check()
    }
}

pub(super) fn native_identity(file: &File) -> Result<Vec<u8>, OwnerError> {
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

pub(super) fn read_bytes(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    file: &mut File,
    ceiling: usize,
) -> Result<Vec<u8>, OwnerError> {
    let before = work.call(request, || {
        file.metadata().map_err(|_| OwnerError::OwnerGuardMismatch)
    })?;
    require_kind(&before, false)?;
    let length = usize::try_from(before.len()).map_err(|_| OwnerError::ContractExhausted)?;
    if length > ceiling {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    // Reserve the complete finite read and EOF probe before allocation or I/O.
    work.read_capacity(
        request,
        length.checked_add(1).ok_or(OwnerError::ContractExhausted)?,
    )?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| OwnerError::ContractExhausted)?;
    bytes.resize(length, 0);
    work.call(request, || {
        file.seek(SeekFrom::Start(0))
            .map(|_| ())
            .map_err(|_| OwnerError::OwnerGuardMismatch)
    })?;
    let mut offset = 0;
    while offset < bytes.len() {
        work.read(request, bytes.len() - offset)?;
        let count = work.call(request, || {
            file.read(&mut bytes[offset..])
                .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
        })?;
        if count == 0 {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        offset += count;
    }
    let mut extra = [0];
    work.read(request, 1)?;
    if work.call(request, || {
        file.read(&mut extra)
            .map_err(|_| OwnerError::OwnerGuardMismatch)
    })? != 0
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    let after = work.call(request, || {
        file.metadata().map_err(|_| OwnerError::OwnerGuardMismatch)
    })?;
    if after.len() != before.len() || after.modified().ok() != before.modified().ok() {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    request.check()?;
    Ok(bytes)
}
