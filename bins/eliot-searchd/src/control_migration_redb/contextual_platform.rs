//! One physical native observer for inert and original-context record calls.
use search_control_redb::migration::SourceImportRecordArtifactError as Error;
use search_control_redb::{ControlError, MigrationBootstrapWork as Work};
#[cfg(windows)]
use std::cell::RefCell;
use std::fs::{self, File};
use std::path::Path;
use std::time::Instant;

type Check<'a> = &'a dyn Fn(Work) -> Result<(), Error<String>>;

pub(super) fn reserve(
    deadline: Instant,
    work: &dyn Fn(Work) -> Result<(), ControlError>,
    unit: Work,
) -> Result<(), Error<String>> {
    if Instant::now() >= deadline {
        return Err(Error::DeadlineExceeded);
    }
    work(unit).map_err(Error::Context)?;
    if Instant::now() >= deadline {
        return Err(Error::DeadlineExceeded);
    }
    Ok(())
}

fn call<T>(
    check: Check<'_>,
    unit: Work,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, Error<String>> {
    check(unit)?;
    let result = action().map_err(Error::Platform);
    let checkpoint = check(Work::Checkpoint);
    result.and_then(|value| checkpoint.map(|()| value))
}

pub(super) fn directory(path: &Path, check: Check<'_>) -> Result<(), Error<String>> {
    // ensure_directory performs exactly one symlink_metadata observation.
    call(check, Work::Read(0), || super::ensure_directory(path))
}

pub(super) fn identity(file: &File, check: Check<'_>) -> Result<(u64, u64), Error<String>> {
    let invalid = || "DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned();
    let metadata = call(check, Work::Read(0), || {
        file.metadata().map_err(|_| invalid())
    })?;
    if !super::regular(&metadata) {
        return Err(Error::Platform(invalid()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok((metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        // The existing native observer has three Windows dispatches. Preserve
        // its engine and carry refusal between GetFileType, volume and identity.
        let first = RefCell::new(None);
        let observed = eliot_searchd::native_file::observe_with_check(file, &|before| {
            let unit = if before {
                Work::Read(0)
            } else {
                Work::Checkpoint
            };
            check(unit).map_err(|error| {
                first.borrow_mut().get_or_insert(error);
                eliot_searchd::native_file::ObservationError::ObservationFailed
            })
        });
        if let Some(error) = first.into_inner() {
            return Err(error);
        }
        let observed = observed.map_err(|_| Error::Platform(invalid()))?;
        Ok((u64::from(observed.volume_serial), observed.file_index))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(Error::Platform(
            "DIRECT_MIGRATION_LOCK_PLATFORM_UNSUPPORTED".to_owned(),
        ))
    }
}

pub(super) fn locator(expected: &File, path: &Path, check: Check<'_>) -> Result<(), Error<String>> {
    let invalid = || "DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned();
    directory(
        path.parent().ok_or_else(|| Error::Platform(invalid()))?,
        check,
    )?;
    let metadata = call(check, Work::Read(0), || {
        fs::symlink_metadata(path).map_err(|_| invalid())
    })?;
    if !super::regular(&metadata) {
        return Err(Error::Platform(invalid()));
    }
    let current = call(check, Work::Open, || {
        File::open(path).map_err(|_| invalid())
    })?;
    if identity(&current, check)? != identity(expected, check)? {
        return Err(Error::Platform(invalid()));
    }
    Ok(())
}

pub(super) fn sync_directory(path: &Path, check: Check<'_>) -> Result<(), Error<String>> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let failed = || "DIRECT_MIGRATION_RECORD_DIRECTORY_SYNC_FAILED".to_owned();
        let file = call(check, Work::Open, || {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(0x0200_0000)
                .open(path)
                .map_err(|_| failed())
        })?;
        call(check, Work::Write(0), || {
            file.sync_all().map_err(|_| failed())
        })
    }
    #[cfg(unix)]
    {
        let file = call(check, Work::Open, || {
            File::open(path).map_err(|error| format!("DIRECT_DIRECTORY_SYNC_ERROR:{error}"))
        })?;
        call(check, Work::Write(0), || {
            file.sync_all()
                .map_err(|error| format!("DIRECT_DIRECTORY_SYNC_ERROR:{error}"))
        })
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (path, check);
        Err(Error::Platform(
            "DIRECT_MIGRATION_RECORD_DIRECTORY_SYNC_UNSUPPORTED".to_owned(),
        ))
    }
}

pub(super) fn inert_reason(error: Error<String>) -> String {
    match error {
        Error::Platform(reason) => reason,
        _ => "DIRECT_MIGRATION_UNAVAILABLE".to_owned(),
    }
}
