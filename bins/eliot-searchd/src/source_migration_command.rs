//! Offline source-map staging without normal daemon startup or source-state writes.
//! Uses retained existing-only native custody; no competing ownership mechanism,
//! credential creation, root recovery, revision conversion or query service is started.

use std::ffi::OsString;
#[cfg(test)]
use std::fs::{self, File, OpenOptions, TryLockError};
use std::fs::Metadata;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use search_contracts::SourceNamespaceId;
use search_runtime_owner::OwnerError;

#[cfg(test)]
use crate::development::{DataRootGuard, OwnerLockProfile, live_owner_denial};
use crate::owner_composition::{
    NativeReadOperation, NativeRootAdmission, NativeRootLimits, NativeRootRequest, admit_existing,
};
use crate::source_roots;

pub fn maybe_run() -> Option<ExitCode> {
    let started = Instant::now();
    let arguments = std::env::args_os().skip(1).take(257).collect::<Vec<_>>();
    let (selected, _) = crate::config_composition::strip_config_args_os(&arguments).ok()?;
    if selected.first().and_then(|arg| arg.to_str()) != Some("--plan-control-migration") {
        return None;
    }
    Some(run(&arguments, started))
}

fn run(arguments: &[OsString], started: Instant) -> ExitCode {
    let failed = ExitCode::from(2);
    let Some(deadline) = started.checked_add(Duration::from_secs(120)) else {
        return failed;
    };
    let Ok(limits) = NativeRootLimits::new([
        256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256,
    ]) else {
        return failed;
    };
    let cancellation = Arc::new(AtomicBool::new(false));
    let (request, selected) = match NativeRootRequest::for_control_migration_plan(
        arguments, deadline, Arc::clone(&cancellation), limits,
    ) {
        Ok(value) => value,
        Err(error) => {
            let _ = NativeRootRequest::emit_validation_refusal(
                deadline, &cancellation, limits, &mut std::io::stderr().lock(), error,
            );
            return failed;
        }
    };
    let Some(target) = selected[2].to_str().and_then(|value| SourceNamespaceId::parse(value).ok()) else {
        return failed;
    };
    // Admission consumes the original request and its work. There is no fresh
    // validation/output request after this point, including admission failure.
    let Ok(admission) = admit_existing(request) else {
        return failed;
    };
    match admission {
        NativeRootAdmission::Inspection(owner) => owner
            .run_registration_one_shot(|operation| {
                let result = stage(operation, target, Path::new(&selected[3]));
                let (line, exit) = match result {
                    Ok(receipt) if receipt.len() <= 60 * 1024 => (receipt, ExitCode::SUCCESS),
                    Ok(_) => (error_frame("DIRECT_MIGRATION_OUTPUT_LIMIT"), failed),
                    Err(error) => (error_frame(public_reason(&error)), failed),
                };
                emit_frame(operation, &mut std::io::stdout().lock(), &line)?;
                // A successfully emitted refusal is a completed callback, not
                // an Err after output. Later teardown failures stay unknown.
                Ok(exit)
            })
            .unwrap_or(failed),
        NativeRootAdmission::Unavailable(_, request) => {
            let _ = request.emit(
                &mut std::io::stderr().lock(),
                &error_frame("DIRECT_MIGRATION_ROOT_UNAVAILABLE"),
            );
            failed
        }
        NativeRootAdmission::Mutation(_) => failed,
    }
}

fn stage(
    operation: &mut NativeReadOperation<'_>,
    target: SourceNamespaceId,
    output: &Path,
) -> Result<String, String> {
    let registration = operation.registration_bytes()?;
    let roots = source_roots::migration_input_from_bytes(operation.root(), registration.as_deref())
        .map_err(|error| error.code().to_owned())?;
    let directory = output_directory(operation, output)?;
    if overlaps(&directory, operation.root())
        || roots.paths.iter().any(|root| overlaps(&directory, root))
    {
        return Err("DIRECT_MIGRATION_OUTPUT_OVERLAP".to_owned());
    }
    let receipt = {
        let store: crate::direct_store::ExistingMigrationStore = crate::direct_store::open_existing_migration(operation)
            .map_err(|error| error.code().to_owned())?;
        store.stage_mapping(target, &directory, operation, registration.as_deref())
    }?; // Store/protector closes before registration/native custody.
    if output_directory(operation, &directory)? != directory {
        return Err("DIRECT_MIGRATION_DIRECTORY_CHANGED".to_owned());
    }
    operation.verify().map_err(|error| error.code().to_owned())?;
    Ok(receipt)
}

fn output_directory(
    operation: &mut NativeReadOperation<'_>,
    path: &Path,
) -> Result<PathBuf, String> {
    let invalid = || "DIRECT_MIGRATION_DIRECTORY_INVALID".to_owned();
    let before = operation.source_root_metadata(path)
        .map_err(|error| error.code().to_owned())?.map_err(|_| invalid())?;
    if !before.is_dir() || is_link(&before) { return Err(invalid()); }
    let canonical = operation.source_root_canonicalize(path)
        .map_err(|error| error.code().to_owned())?.map_err(|_| invalid())?;
    let after = operation.source_root_metadata(&canonical)
        .map_err(|error| error.code().to_owned())?.map_err(|_| invalid())?;
    if !after.is_dir() || is_link(&after) { return Err(invalid()); }
    Ok(canonical)
}

fn emit_frame(
    operation: &mut NativeReadOperation<'_>,
    writer: &mut impl Write,
    line: &str,
) -> Result<(), String> {
    if line.is_empty() || line.len() > 60 * 1024 || line.bytes().any(|byte| matches!(byte, b'\n' | b'\r')) {
        return Err("DIRECT_MIGRATION_OUTPUT_INVALID".to_owned());
    }
    operation.verify().map_err(|error| error.code().to_owned())?;
    operation.migration_charge(search_revision_store::LegacyRevisionObjectWork::Allocate(line.len() + 1))
        .map_err(|error| error.code().to_owned())?;
    let frame = format!("{line}\n");
    let count = operation.write_output(writer, frame.as_bytes())
        .map_err(|error| error.code().to_owned())?;
    if count != frame.len() {
        operation.cancel_output();
        return Err(OwnerError::OwnerAcquireOutcomeUnknown.code().to_owned());
    }
    operation.flush_output(writer).map_err(|error| error.code().to_owned())?;
    operation.verify().map_err(|error| error.code().to_owned())
}

fn error_frame(reason: &str) -> String {
    format!("{{\"error\":\"{reason}\"}}")
}

// Lower legacy layers can carry private OS details. Only these literal reasons
// can reach the public command; every other private failure is collapsed.
fn public_reason(reason: &str) -> &'static str {
    match reason {
        "DIRECT_MIGRATION_DIRECTORY_INVALID" => "DIRECT_MIGRATION_DIRECTORY_INVALID",
        "DIRECT_MIGRATION_DIRECTORY_CHANGED" => "DIRECT_MIGRATION_DIRECTORY_CHANGED",
        "DIRECT_MIGRATION_OUTPUT_OVERLAP" => "DIRECT_MIGRATION_OUTPUT_OVERLAP",
        "DIRECT_MIGRATION_CUTOVER_CORRUPT" => "DIRECT_MIGRATION_CUTOVER_CORRUPT",
        "DIRECT_MIGRATION_CUTOVER_CONFLICT" => "DIRECT_MIGRATION_CUTOVER_CONFLICT",
        "DIRECT_MIGRATION_CUTOVER_SUPERSEDED" => "DIRECT_MIGRATION_CUTOVER_SUPERSEDED",
        "DIRECT_MIGRATION_PLAN_READBACK_MISMATCH" => "DIRECT_MIGRATION_PLAN_READBACK_MISMATCH",
        "DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH" => "DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH",
        "DIRECT_MIGRATION_PLAN_PUBLISH_OUTCOME_UNKNOWN" => "DIRECT_MIGRATION_PLAN_PUBLISH_OUTCOME_UNKNOWN",
        "DIRECT_MIGRATION_INVENTORY_PUBLISH_OUTCOME_UNKNOWN" => "DIRECT_MIGRATION_INVENTORY_PUBLISH_OUTCOME_UNKNOWN",
        "DIRECT_MIGRATION_INVENTORY_CHANGED" => "DIRECT_MIGRATION_INVENTORY_CHANGED",
        "DIRECT_MIGRATION_INVENTORY_LIMIT" => "DIRECT_MIGRATION_INVENTORY_LIMIT",
        "DIRECT_MIGRATION_CONTENT_PUBLISH_OUTCOME_UNKNOWN" => "DIRECT_MIGRATION_CONTENT_PUBLISH_OUTCOME_UNKNOWN",
        "DIRECT_MIGRATION_IMPORT_PUBLISH_OUTCOME_UNKNOWN" => "DIRECT_MIGRATION_IMPORT_PUBLISH_OUTCOME_UNKNOWN",
        "DPAPI_LEGACY_REVISION_CONTEXT_REFUSED" => "DPAPI_LEGACY_REVISION_CONTEXT_REFUSED",
        "DPAPI_LEGACY_REVISION_CONTEXT_AND_CLEAR_ACCOUNTING_FAILED" => "DPAPI_LEGACY_REVISION_CONTEXT_AND_CLEAR_ACCOUNTING_FAILED",
        "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT" => "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT",
        "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT_CLEAR_ACCOUNTING_FAILED" => "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT_CLEAR_ACCOUNTING_FAILED",
        "DPAPI_LEGACY_REVISION_ALLOCATION_FAILED" => "DPAPI_LEGACY_REVISION_ALLOCATION_FAILED",
        "DPAPI_LEGACY_REVISION_ALLOCATION_AND_CONTEXT" => "DPAPI_LEGACY_REVISION_ALLOCATION_AND_CONTEXT",
        "DPAPI_LEGACY_REVISION_ALLOCATION_AND_CONTEXT_CLEAR_ACCOUNTING_FAILED" => "DPAPI_LEGACY_REVISION_ALLOCATION_AND_CONTEXT_CLEAR_ACCOUNTING_FAILED",
        "DIRECT_CONTROL_READBACK_MISMATCH" => "DIRECT_CONTROL_READBACK_MISMATCH",
        "SOURCE_ROOT_UPDATE_OUTCOME_UNKNOWN" => "SOURCE_ROOT_UPDATE_OUTCOME_UNKNOWN",
        "SOURCE_ROOT_CATALOG_CORRUPT" => "SOURCE_ROOT_CATALOG_CORRUPT",
        _ => "DIRECT_MIGRATION_UNAVAILABLE",
    }
}

#[cfg(test)]
impl DataRootGuard {
    /// Borrow an existing canonical data root under its normal exclusive OS lock.
    /// No `DataRootGuard` instance is constructed: its ordinary initializer and
    /// marker-cleaning destructor must not run during source-preserving migration.
    /// Missing lock files require explicit recovery, never an invented new lease.
    pub(crate) fn with_existing_lock<T>(
        path: &Path, inspect: impl FnOnce(&Path) -> Result<T, String>,
    ) -> Result<T, String> {
        // Sole qualified borrower: the offline migration command borrows the
        // primary lock as the migration-output instance (same file, named
        // borrower, no marker cleanup). Behavior is byte-identical to the
        // direct borrow.
        Self::acquire_borrowed(path, OwnerLockProfile::MigrationOutput, inspect)
    }

    /// Named borrowed-owner adapter: inspects a canonical root under an
    /// already-existing exclusive OS lock without constructing a guard.
    ///
    /// `profile` selects only the lock file name (`Direct` and
    /// `MigrationOutput` share the primary file; see [`OwnerLockProfile`]).
    /// The file is never created, truncated, written, synced, removed or
    /// replaced, and no owner-record cleanup runs on any return or unwind:
    /// the held `File` owns the OS lock across both locator verifications.
    /// Missing lock files require explicit recovery, never a new lease.
    pub(crate) fn acquire_borrowed<T>(
        path: &Path,
        profile: OwnerLockProfile,
        inspect: impl FnOnce(&Path) -> Result<T, String>,
    ) -> Result<T, String> {
        let root = canonical_directory(path)?;
        let lock_path = profile.lock_path(&root);
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
            Err(TryLockError::WouldBlock) => return Err(live_owner_denial(None).to_owned()),
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

#[cfg(test)]
fn canonical_directory(path: &Path) -> Result<std::path::PathBuf, String> {
    let invalid = || "DIRECT_MIGRATION_DIRECTORY_INVALID".to_owned();
    let metadata = fs::symlink_metadata(path).map_err(|_| invalid())?;
    if !metadata.is_dir() || is_link(&metadata) { return Err(invalid()); }
    let canonical = fs::canonicalize(path).map_err(|_| invalid())?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|_| invalid())?;
    if !metadata.is_dir() || is_link(&metadata) { return Err(invalid()); }
    Ok(canonical)
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
mod borrowed_lock_tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::DataRootGuard;
    use crate::development::OwnerLockProfile;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Scratch {
        base: std::path::PathBuf,
        data: std::path::PathBuf,
    }

    impl Scratch {
        fn fresh() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let base = std::env::temp_dir().join(format!(
                "eliot-borrowed-lock-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let data = base.join("data");
            fs::create_dir_all(&data).unwrap();
            Self { base, data }
        }

        /// Establishes a primary owner once so the lock file exists, then
        /// releases exclusion; the durable lock marker remains.
        fn with_released_owner(&self) {
            drop(DataRootGuard::acquire(&self.data).unwrap());
        }

        fn lock_bytes(&self) -> Vec<u8> {
            fs::read(self.data.join(OwnerLockProfile::DIRECT_LOCK_FILE)).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn named_borrow_uses_the_primary_lock_file() {
        assert_eq!(
            OwnerLockProfile::MigrationOutput.file_name(),
            OwnerLockProfile::Direct.file_name()
        );
        assert_eq!(
            OwnerLockProfile::Direct.file_name(),
            ".eliot-search-owner.lock"
        );
    }

    #[test]
    fn missing_lock_file_requires_recovery_without_inventing_a_lease() {
        let scratch = Scratch::fresh();
        for profile in [OwnerLockProfile::Direct, OwnerLockProfile::MigrationOutput] {
            assert_eq!(
                DataRootGuard::acquire_borrowed(&scratch.data, profile, |_| Ok::<(), String>(())),
                Err("DATA_ROOT_EXISTING_LOCK_REQUIRED".to_owned())
            );
        }
        assert_eq!(
            DataRootGuard::with_existing_lock(&scratch.data, |_| Ok::<(), String>(())),
            Err("DATA_ROOT_EXISTING_LOCK_REQUIRED".to_owned())
        );
        // Nothing was invented: no lock file appeared.
        assert!(
            !scratch
                .data
                .join(OwnerLockProfile::DIRECT_LOCK_FILE)
                .exists()
        );
    }

    #[test]
    fn held_primary_lock_denies_the_borrow_like_a_second_owner() {
        let scratch = Scratch::fresh();
        scratch.with_released_owner();
        let _live = DataRootGuard::acquire(&scratch.data).unwrap();
        for profile in [OwnerLockProfile::Direct, OwnerLockProfile::MigrationOutput] {
            assert_eq!(
                DataRootGuard::acquire_borrowed(&scratch.data, profile, |_| Ok::<(), String>(())),
                Err("DATA_ROOT_ALREADY_OWNED".to_owned())
            );
        }
        assert_eq!(
            DataRootGuard::with_existing_lock(&scratch.data, |_| Ok::<(), String>(())),
            Err("DATA_ROOT_ALREADY_OWNED".to_owned())
        );
    }

    #[test]
    fn borrow_inspects_without_marker_cleanup_and_reports_values() {
        let scratch = Scratch::fresh();
        scratch.with_released_owner();
        let before = scratch.lock_bytes();
        let canonical = fs::canonicalize(&scratch.data).unwrap();
        for profile in [OwnerLockProfile::Direct, OwnerLockProfile::MigrationOutput] {
            let seen = DataRootGuard::acquire_borrowed(&scratch.data, profile, |root| {
                assert_eq!(root, canonical);
                Ok::<_, String>(40 + profile.file_name().len())
            })
            .unwrap();
            assert_eq!(seen, 40 + ".eliot-search-owner.lock".len());
        }
        // The compat wrapper observes the same root with the same semantics.
        let seen = DataRootGuard::with_existing_lock(&scratch.data, |root| {
            assert_eq!(root, canonical);
            Ok::<_, String>(7)
        })
        .unwrap();
        assert_eq!(seen, 7);
        // No marker cleanup: the lock file still exists with identical bytes.
        assert_eq!(scratch.lock_bytes(), before);
        // Inspection errors propagate without cleanup either.
        assert_eq!(
            DataRootGuard::acquire_borrowed(
                &scratch.data,
                OwnerLockProfile::MigrationOutput,
                |_| Err::<(), _>("INSPECT_DENIED".to_owned())
            ),
            Err("INSPECT_DENIED".to_owned())
        );
        assert_eq!(scratch.lock_bytes(), before);
    }

    #[test]
    fn borrow_rejects_non_directories_before_touching_any_lock() {
        let scratch = Scratch::fresh();
        let file = scratch.base.join("file.txt");
        fs::write(&file, b"bytes").unwrap();
        assert_eq!(
            DataRootGuard::acquire_borrowed(&file, OwnerLockProfile::Direct, |_| Ok::<(), String>(
                ()
            )),
            Err("DIRECT_MIGRATION_DIRECTORY_INVALID".to_owned())
        );
        assert_eq!(
            DataRootGuard::with_existing_lock(&file, |_| Ok::<(), String>(())),
            Err("DIRECT_MIGRATION_DIRECTORY_INVALID".to_owned())
        );
    }
}
