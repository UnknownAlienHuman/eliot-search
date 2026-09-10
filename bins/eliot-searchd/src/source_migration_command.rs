//! Offline source-map staging without normal daemon startup or source-state writes.
//! Uses the normal owner's existing lock file; no competing ownership mechanism,
//! credential creation, root recovery, revision conversion or query service is started.

use std::fs::{self, File, Metadata, OpenOptions, TryLockError};
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use search_contracts::SourceNamespaceId;

use crate::development::DataRootGuard;
use crate::direct_store::DirectStore;
use crate::plaintext_direct_store;
use crate::service_output::{json_string, write_line};
use crate::source_roots;

pub fn maybe_run() -> Option<ExitCode> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().and_then(|arg| arg.to_str()) != Some("--plan-control-migration") {
        return None;
    }
    let result = (|| -> Result<(), String> {
        let [_, root, target, output] = args.as_slice() else {
            return Err("USAGE_ERROR".to_owned());
        };
        let target = target.to_str().and_then(|value| SourceNamespaceId::parse(value).ok())
            .filter(|value| value.as_bytes() != &[0; 16])
            .ok_or_else(|| "DIRECT_MIGRATION_TARGET_NAMESPACE_INVALID".to_owned())?;
        let deadline = Instant::now().checked_add(Duration::from_secs(120))
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        let response = DataRootGuard::with_existing_lock(Path::new(root), |root| {
            let registration = source_roots::migration_input(root)
                .map_err(|error| error.code().to_owned())?;
            // Output must already exist and cannot overlap either original state
            // or a registered observation root. Do not probe current source paths.
            let output = canonical_directory(Path::new(output))?;
            if overlaps(&output, root)
                || registration.paths.iter().any(|source| overlaps(&output, source))
            {
                return Err("DIRECT_MIGRATION_OUTPUT_OVERLAPS_SOURCE".to_owned());
            }
            let response = plaintext_direct_store::DirectStore::with_existing_mapping_source(
                root, deadline, |source| {
                    DirectStore::stage_mapping_artifact(source, root, target, &output, "", deadline)
                },
            )?;
            if source_roots::migration_input(root).map_err(|error| error.code().to_owned())? != registration {
                return Err("DIRECT_MIGRATION_ROOT_STATE_CHANGED".to_owned());
            }
            Ok(response)
        })?;
        write_line(&mut std::io::stdout().lock(), &response)
            .map_err(|_| "DIRECT_MIGRATION_PLAN_ACK_OUTCOME_UNKNOWN".to_owned())
    })();
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            ExitCode::from(2)
        }
    })
}

impl DataRootGuard {
    /// Borrow an existing canonical data root under its normal exclusive OS lock.
    /// No `DataRootGuard` instance is constructed: its ordinary initializer and
    /// marker-cleaning destructor must not run during source-preserving migration.
    /// Missing lock files require explicit recovery, never an invented new lease.
    pub(crate) fn with_existing_lock<T>(
        path: &Path, inspect: impl FnOnce(&Path) -> Result<T, String>,
    ) -> Result<T, String> {
        let root = canonical_directory(path)?;
        let lock_path = root.join(".eliot-search-owner.lock");
        let before = fs::symlink_metadata(&lock_path)
            .map_err(|_| "DATA_ROOT_EXISTING_LOCK_REQUIRED".to_owned())?;
        if !regular(&before) { return Err("DATA_ROOT_LOCK_OBJECT_INVALID".to_owned()); }
        // Opening for locking needs write access on supported targets, but does
        // not create, truncate, write, sync, remove or replace the marker.
        let file = OpenOptions::new().read(true).write(true).open(&lock_path)
            .map_err(|_| "DATA_ROOT_LOCK_OPEN_ERROR".to_owned())?;
        if !regular(&file.metadata().map_err(|_| "DATA_ROOT_LOCK_OPEN_ERROR".to_owned())?) {
            return Err("DATA_ROOT_LOCK_OBJECT_INVALID".to_owned());
        }
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err("DATA_ROOT_ALREADY_OWNED".to_owned()),
            Err(TryLockError::Error(_)) => return Err("DATA_ROOT_LOCK_ERROR".to_owned()),
        }
        verify_locked_locator(&file, &lock_path)?;
        let result = inspect(&root);
        verify_locked_locator(&file, &lock_path)?;
        // File owns the OS lock throughout both checks and releases it on every
        // return/unwind. No normal owner-record cleanup is called here.
        drop(file);
        result
    }
}

fn overlaps(a: &Path, b: &Path) -> bool { a.starts_with(b) || b.starts_with(a) }

fn canonical_directory(path: &Path) -> Result<std::path::PathBuf, String> {
    let invalid = || "DIRECT_MIGRATION_DIRECTORY_INVALID".to_owned();
    let metadata = fs::symlink_metadata(path).map_err(|_| invalid())?;
    if !metadata.is_dir() || is_link(&metadata) { return Err(invalid()); }
    let canonical = fs::canonicalize(path).map_err(|_| invalid())?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|_| invalid())?;
    if !metadata.is_dir() || is_link(&metadata) { return Err(invalid()); }
    Ok(canonical)
}

fn regular(metadata: &Metadata) -> bool { metadata.is_file() && !is_link(metadata) }
fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    { metadata.file_type().is_symlink() }
}

fn verify_locked_locator(locked: &File, path: &Path) -> Result<(), String> {
    let invalid = || "DATA_ROOT_LOCK_IDENTITY_CHANGED".to_owned();
    if !regular(&fs::symlink_metadata(path).map_err(|_| invalid())?) { return Err(invalid()); }
    let current = File::open(path).map_err(|_| invalid())?;
    let original_metadata = locked.metadata().map_err(|_| invalid())?;
    let current_metadata = current.metadata().map_err(|_| invalid())?;
    if !regular(&original_metadata) || !regular(&current_metadata) { return Err(invalid()); }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if original_metadata.dev() != current_metadata.dev() || original_metadata.ino() != current_metadata.ino() {
            return Err(invalid());
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        let original = eliot_searchd::native_file::observe(locked).map_err(|_| invalid())?;
        let observed = eliot_searchd::native_file::observe(&current).map_err(|_| invalid())?;
        if original.volume_serial != observed.volume_serial || original.file_index != observed.file_index {
            return Err(invalid());
        }
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    { Err("DIRECT_MIGRATION_LOCK_PLATFORM_UNSUPPORTED".to_owned()) }
}
