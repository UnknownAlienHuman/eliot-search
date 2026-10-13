//! Retained no-follow external output objects for the one inactive target.
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use search_control_redb::{MigrationBootstrapIdentity as Identity,
    MigrationBootstrapIdentityProfile as Profile, MigrationBootstrapOutput};
use search_contracts::Blake3Digest32;
use search_revision_store::LegacyRevisionObjectWork as Work;
use super::MigrationSource;

const INVALID: &str = "DIRECT_MIGRATION_BOOTSTRAP_INVALID";
const UNKNOWN: &str = "DIRECT_MIGRATION_BOOTSTRAP_OUTCOME_UNKNOWN";

struct Pin { path: PathBuf, file: File, identity: Identity }
pub(super) struct Output {
    pins: Vec<Pin>,
    target: Option<Pin>,
}
impl Output {
    pub(super) fn admit(source: &MigrationSource<'_, '_, '_>, directory: &Path) -> Result<Self, String> {
        if !directory.is_absolute() { return Err(INVALID.to_owned()); }
        source.charge(Work::Allocate(64 * std::mem::size_of::<Pin>()))?;
        let mut pins = Vec::with_capacity(64);
        let mut path = Some(directory);
        while let Some(current) = path {
            if pins.len() == 64 { return Err(INVALID.to_owned()); }
            source.charge(Work::Open)?;
            let file = directory_file(current, false)?;
            source.charge(Work::Observe)?;
            if !file.metadata().map_err(|_| INVALID.to_owned())?.is_dir() { return Err(INVALID.to_owned()); }
            let identity = observe(source, &file)?;
            source.charge(Work::AllocateLocator(current.as_os_str().len()))?;
            pins.push(Pin { path: current.to_owned(), file, identity });
            path = current.parent();
        }
        let output = Self { pins, target: None };
        // A parent could have moved before its own pin was acquired. Join every
        // original child handle to the now fully pinned path once, before create.
        for pin in &output.pins {
            source.charge(Work::Open)?;
            let current = directory_file(&pin.path, false)?;
            if observe(source, &current)? != pin.identity { return Err(INVALID.to_owned()); }
        }
        output.verify(source)?;
        Ok(output)
    }

    pub(super) fn create(&mut self, source: &MigrationSource<'_, '_, '_>, name: &str)
        -> Result<(File, MigrationBootstrapOutput), String> {
        self.verify(source)?;
        if self.target.is_some() || name.len() > 200 || name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
        { return Err(INVALID.to_owned()); }
        let directory = self.pins.first().ok_or_else(|| INVALID.to_owned())?;
        let locator_bytes = directory.path.as_os_str().len().checked_add(name.len() + 1)
            .ok_or_else(|| INVALID.to_owned())?;
        source.charge(Work::AllocateLocator(locator_bytes))?;
        let path = directory.path.join(name);
        source.charge(Work::Open)?;
        let file = new_file(&path)?;
        // From exclusive create onward, every failure retains the native object.
        let result: Result<MigrationBootstrapOutput, String> = (|| {
            let identity = observe(source, &file)?;
            source.charge(Work::Open)?;
            let retained = file.try_clone().map_err(|_| UNKNOWN.to_owned())?;
            source.charge(Work::AllocateLocator(path.as_os_str().len()))?;
            self.target = Some(Pin { path: path.clone(), file: retained, identity });
            let path_blake3 = path_digest(source, &path)?;
            self.verify(source)?;
            Ok(MigrationBootstrapOutput {
                directory: self.pins[0].identity, target: identity, path_blake3,
            })
        })();
        result.map(|binding| (file, binding)).map_err(|_| UNKNOWN.to_owned())
    }

    pub(super) fn verify(&self, source: &MigrationSource<'_, '_, '_>) -> Result<(), String> {
        source.checkpoint()?;
        for pin in self.pins.iter().chain(self.target.iter()) {
            if observe(source, &pin.file)? != pin.identity { return Err(INVALID.to_owned()); }
        }
        if let Some(target) = &self.target {
            source.charge(Work::Observe)?;
            if eliot_searchd::native_file::hardlink_count(&target.file)
                .map_err(|_| INVALID.to_owned())? != 1 { return Err(INVALID.to_owned()); }
        }
        source.checkpoint()
    }

    pub(super) fn sync_directory(&self, source: &MigrationSource<'_, '_, '_>) -> Result<(), String> {
        self.verify(source)?;
        let pin = self.pins.first().ok_or_else(|| INVALID.to_owned())?;
        source.charge(Work::Open)?;
        let file = directory_file(&pin.path, true)?;
        if observe(source, &file)? != pin.identity { return Err(UNKNOWN.to_owned()); }
        source.charge(Work::Observe)?;
        file.sync_all().map_err(|_| UNKNOWN.to_owned())?;
        self.verify(source).map_err(|_| UNKNOWN.to_owned())
    }
}

fn observe(source: &MigrationSource<'_, '_, '_>, file: &File) -> Result<Identity, String> {
    source.charge(Work::Observe)?;
    let value = eliot_searchd::native_file::observe(file).map_err(|_| INVALID.to_owned())?;
    Ok(Identity { profile: Profile::WindowsFileIndex64,
        volume: u64::from(value.volume_serial), file: value.file_index })
}

#[cfg(windows)]
fn directory_file(path: &Path, write: bool) -> Result<File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    // Read/write sharing permits directory flushes, while denying delete pins
    // every ancestor against rename/replacement for the complete operation.
    OpenOptions::new().read(true).write(write).share_mode(3)
        .custom_flags(0x0200_0000 | 0x0020_0000).open(path).map_err(|_| INVALID.to_owned())
}
#[cfg(not(windows))]
fn directory_file(_: &Path, _: bool) -> Result<File, String> { Err(INVALID.to_owned()) }

#[cfg(windows)]
fn new_file(path: &Path) -> Result<File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    OpenOptions::new().read(true).write(true).create_new(true).share_mode(3)
        .custom_flags(0x0020_0000).open(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                "DIRECT_MIGRATION_BOOTSTRAP_EXISTS".to_owned()
            } else { UNKNOWN.to_owned() }
        })
}
#[cfg(not(windows))]
fn new_file(_: &Path) -> Result<File, String> { Err(INVALID.to_owned()) }

fn path_digest(source: &MigrationSource<'_, '_, '_>, path: &Path) -> Result<Blake3Digest32, String> {
    #[cfg(windows)] {
        use std::os::windows::ffi::OsStrExt;
        let mut hash = blake3::Hasher::new();
        hash.update(b"eliot-search/migration-bootstrap/windows-canonical-target/v1\0");
        let mut pending = 0;
        for (index, unit) in path.as_os_str().encode_wide().enumerate() {
            if index >= 32768 { return Err(INVALID.to_owned()); }
            hash.update(&unit.to_le_bytes());
            pending += 2;
            if pending == 128 { source.charge(Work::Compare(pending))?; pending = 0; }
        }
        if pending != 0 { source.charge(Work::Compare(pending))?; }
        Ok(Blake3Digest32::from_bytes(*hash.finalize().as_bytes()))
    }
    #[cfg(not(windows))] { let _ = (source, path); Err(INVALID.to_owned()) }
}
