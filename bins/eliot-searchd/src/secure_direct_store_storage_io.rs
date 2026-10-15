//! Qualified daemon adapters for package-owned immutable object I/O.
//!
//! `search-revision-store` owns revision-object reads/publication and
//! `search-materializer` owns preparation-object/reference reads/publication.
//! This module retains legacy path derivation, native platform observations,
//! temporary-name entropy and stable DIRECT reason mapping.

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines
)]

use std::fs::{self, File, Metadata};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

use search_materializer::{
    LegacyPreparationArtifactError, LegacyPreparationArtifactPlatform,
    publish_legacy_preparation_artifact, read_legacy_preparation_artifact,
};
use search_revision_store::{
    ContextualRevisionObjectRead,
    ContextualLegacyRevisionObjectPlatform, LegacyRevisionObjectContext,
    LegacyRevisionObjectError, LegacyRevisionObjectPlatform,
    LegacyRevisionObjectPlatformFailure as PlatformFailure, LegacyRevisionObjectWork as Work,
    publish_legacy_revision_object, read_legacy_revision_object,
    read_legacy_revision_object_with_context,
};

use crate::development::MAX_SCAN_INPUT_BYTES;
use crate::revision_protection::PROTECTED_OBJECT_EXTENSION;
use crate::sha256;
use super::migration_context::MigrationContext;

use super::{
    MAX_REVISION_OBJECT_BYTES, REVISION_DIRECTORY, RevisionMetadata,
    verify_plaintext,
};

pub(super) fn read_plaintext_path(
    path: &Path,
    metadata: &RevisionMetadata,
) -> Result<Vec<u8>, String> {
    let mut bytes = Zeroizing::new(read_revision_object(
        path,
        MAX_SCAN_INPUT_BYTES,
        "DIRECT_REVISION_READ_ERROR",
    )?);
    verify_plaintext(metadata, &bytes)?;
    Ok(std::mem::take(&mut *bytes))
}

pub(super) fn read_regular_file(
    path: &Path,
    max_bytes: usize,
    error_prefix: &'static str,
) -> Result<Vec<u8>, String> {
    read_legacy_preparation_artifact(&DaemonImmutableObjectPlatform, path, max_bytes)
        .map(|observed| observed.into_bytes())
        .map_err(|error| preparation_read_reason(error, error_prefix))
}

pub(super) fn read_revision_object(
    path: &Path,
    max_bytes: usize,
    error_prefix: &'static str,
) -> Result<Vec<u8>, String> {
    read_legacy_revision_object(&DaemonImmutableObjectPlatform, path, max_bytes)
        .map(|observed| observed.into_bytes())
        .map_err(|error| revision_read_reason(error, error_prefix))
}

pub(super) fn read_revision_object_with_context(
    path: &Path,
    max_bytes: usize,
    context: &MigrationContext<'_, '_>,
) -> Result<Zeroizing<Vec<u8>>, String> {
    read_revision_object_detailed_with_context(path, max_bytes, context)
        .map(|observed| Zeroizing::new(observed.into_bytes()))
}

// Preserve the exact owner's native identity before the bytes-only adapter
// consumes it. No path is reopened and no second work allowance is created.
pub(super) fn read_revision_object_detailed_with_context(
    path: &Path,
    max_bytes: usize,
    context: &MigrationContext<'_, '_>,
) -> Result<ContextualRevisionObjectRead<(u64, u64)>, String> {
    let mut object_context = context;
    read_legacy_revision_object_with_context(
        &DaemonImmutableObjectPlatform, &mut object_context, path, max_bytes,
    )
    .map_err(|error| {
        error.context_cause().cloned().unwrap_or_else(|| match error.object_code() {
            Some("REVISION_OBJECT_PARENT_MISSING") => "DIRECT_REVISION_PARENT_MISSING",
            Some("REVISION_OBJECT_NAME_INVALID") => "DIRECT_REVISION_FILENAME_INVALID",
            Some("REVISION_OBJECT_SIZE_INVALID") => "DIRECT_REVISION_OBJECT_READ_FAILED:TOO_LARGE",
            Some("REVISION_OBJECT_INVALID") => "DIRECT_FILE_INVALID",
            Some("REVISION_OBJECT_READBACK_MISMATCH") => "DIRECT_REVISION_OBJECT_READ_FAILED:LENGTH_MISMATCH",
            Some("REVISION_OBJECT_IDENTITY_CHANGED") => "DIRECT_REVISION_OBJECT_CHANGED",
            _ => "DIRECT_REVISION_OBJECT_READ_FAILED",
        }.to_owned())
    })
}

pub(super) fn persist_immutable_object(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_REVISION_OBJECT_BYTES {
        return Err("DIRECT_REVISION_PROTECTED_SIZE_INVALID".to_owned());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "DIRECT_REVISION_PARENT_MISSING".to_owned())?;
    let final_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "DIRECT_REVISION_FILENAME_INVALID".to_owned())?;
    let file_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "DIRECT_REVISION_FILENAME_INVALID".to_owned())?;
    let temporary_name = temporary_name(file_stem)?;
    let receipt = publish_legacy_preparation_artifact(
        &DaemonImmutableObjectPlatform,
        parent,
        final_name,
        &temporary_name,
        bytes,
        MAX_REVISION_OBJECT_BYTES,
    )
    .map_err(preparation_publish_reason)?;
    if receipt.encoded_bytes()
        != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err("DIRECT_REVISION_PROTECTED_READ_ERROR:LENGTH_MISMATCH".to_owned());
    }
    Ok(())
}

pub(super) fn persist_revision_object(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_REVISION_OBJECT_BYTES {
        return Err("DIRECT_REVISION_PROTECTED_SIZE_INVALID".to_owned());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "DIRECT_REVISION_PARENT_MISSING".to_owned())?;
    let final_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "DIRECT_REVISION_FILENAME_INVALID".to_owned())?;
    let file_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "DIRECT_REVISION_FILENAME_INVALID".to_owned())?;
    let temporary_name = temporary_name(file_stem)?;
    let receipt = publish_legacy_revision_object(
        &DaemonImmutableObjectPlatform,
        parent,
        final_name,
        &temporary_name,
        bytes,
        MAX_REVISION_OBJECT_BYTES,
    )
    .map_err(revision_publish_reason)?;
    if receipt.encoded_bytes()
        != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err("DIRECT_REVISION_PROTECTED_READ_ERROR:LENGTH_MISMATCH".to_owned());
    }
    Ok(())
}

fn temporary_name(file_stem: &str) -> Result<String, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "DIRECT_REVISION_CLOCK_INVALID".to_owned())?
        .as_nanos();
    Ok(format!(
        ".{file_stem}.{}.{timestamp}.dpapi.tmp",
        std::process::id()
    ))
}

pub(super) fn remove_plaintext_after_readback(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    ensure_regular_file(path)?;
    fs::remove_file(path)
        .map_err(|error| format!("DIRECT_REVISION_PLAINTEXT_DELETE_ERROR:{error}"))?;
    let parent = path
        .parent()
        .ok_or_else(|| "DIRECT_REVISION_PARENT_MISSING".to_owned())?;
    #[cfg(unix)]
    {
        sync_directory(parent)
    }
    #[cfg(not(unix))]
    {
        sync_directory(parent);
        Ok(())
    }
}

pub(super) fn legacy_path(root: &Path, revision_id: &str) -> Result<PathBuf, String> {
    revision_object_path(root, revision_id, "bin")
}

pub(super) fn protected_path(root: &Path, revision_id: &str) -> Result<PathBuf, String> {
    revision_object_path(root, revision_id, PROTECTED_OBJECT_EXTENSION)
}

fn revision_object_path(
    root: &Path,
    revision_id: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    revision_object_path_with_context(root, revision_id, extension, None)
}

pub(super) fn revision_object_path_with_context(
    root: &Path,
    revision_id: &str,
    extension: &str,
    context: Option<&MigrationContext<'_, '_>>,
) -> Result<PathBuf, String> {
    if let Some(context) = context { context.charge(Work::Compare(revision_id.len()))?; }
    if sha256::decode_digest(revision_id).is_none() {
        return Err("DIRECT_REVISION_ID_INVALID".to_owned());
    }
    let directory = join_revision_path(root, Path::new(REVISION_DIRECTORY), context)?;
    let shard = join_revision_path(&directory, Path::new(&revision_id[..2]), context)?;
    if let Some(context) = context {
        context.charge(Work::AllocateLocator(revision_id.len() + extension.len() + 1))?;
    }
    let name = format!("{revision_id}.{extension}");
    join_revision_path(&shard, Path::new(&name), context)
}

pub(super) fn join_revision_path(
    parent: &Path, child: &Path, context: Option<&MigrationContext<'_, '_>>,
) -> Result<PathBuf, String> {
    if let Some(context) = context {
        let capacity = parent.as_os_str().as_encoded_bytes().len()
            .checked_add(child.as_os_str().as_encoded_bytes().len())
            .and_then(|length| length.checked_add(1))
            .ok_or_else(|| "DIRECT_REVISION_FILENAME_INVALID".to_owned())?;
        context.charge(Work::AllocateLocator(capacity))?;
    }
    let path = parent.join(child);
    if let Some(context) = context { context.checkpoint()?; }
    Ok(path)
}

pub(super) fn ensure_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_DIRECTORY_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_DIRECTORY_INVALID".to_owned());
    }
    Ok(())
}

pub(super) fn ensure_child_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir(path)
            .map_err(|error| format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}"))?;
    }
    ensure_directory(path)
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_FILE_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err("DIRECT_FILE_INVALID".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct DaemonImmutableObjectPlatform;

impl LegacyRevisionObjectPlatform for DaemonImmutableObjectPlatform {
    type Identity = (u64, u64);
    type Error = String;

    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
        ensure_directory(path)
    }

    fn verify_locator(
        &self,
        expected: &File,
        path: &Path,
    ) -> Result<(), Self::Error> {
        verify_locator(expected, path)
    }

    fn identity(&self, file: &File) -> Result<Self::Identity, Self::Error> {
        native_identity(file)
    }

    fn sync_directory(&self, path: &Path) -> Result<(), Self::Error> {
        sync_directory_result(path)
    }
}

// Every nested observation/open is reserved against the original caller before
// dispatch. These bounded synchronous platform calls remain non-preemptible.
fn platform_step<C: LegacyRevisionObjectContext, T>(
    context: &mut C, work: Work, execute: impl FnOnce() -> Result<T, String>,
) -> Result<T, PlatformFailure<String, C::Error>> {
    context.checkpoint().map_err(PlatformFailure::Context)?;
    context.charge(work).map_err(PlatformFailure::Context)?;
    let result = execute();
    let checkpoint = context.checkpoint();
    match (result, checkpoint) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(platform), Ok(())) => Err(PlatformFailure::Platform(platform)),
        (Ok(_), Err(context)) => Err(PlatformFailure::Context(context)),
        (Err(platform), Err(context)) => Err(PlatformFailure::AfterPlatform { platform, context }),
    }
}

impl<C: LegacyRevisionObjectContext> ContextualLegacyRevisionObjectPlatform<C>
    for DaemonImmutableObjectPlatform
{
    fn validate_directory_contextual(
        &self, context: &mut C, path: &Path,
    ) -> Result<(), PlatformFailure<String, C::Error>> {
        platform_step(context, Work::Observe, || ensure_directory(path))
    }

    fn verify_locator_contextual(
        &self, context: &mut C, expected: &File, path: &Path,
    ) -> Result<(), PlatformFailure<String, C::Error>> {
        let parent = path.parent().ok_or_else(|| PlatformFailure::Platform(
            "DIRECT_REVISION_PARENT_MISSING".to_owned(),
        ))?;
        self.validate_directory_contextual(context, parent)?;
        platform_step(context, Work::Observe, || ensure_regular_file(path))?;
        let current = platform_step(context, Work::Open, || File::open(path)
            .map_err(|_| "DIRECT_REVISION_OBJECT_READ_FAILED".to_owned()))?;
        if self.identity_contextual(context, &current)? != self.identity_contextual(context, expected)? {
            return Err(PlatformFailure::Platform("DIRECT_REVISION_OBJECT_CHANGED".to_owned()));
        }
        Ok(())
    }

    fn identity_contextual(
        &self, context: &mut C, file: &File,
    ) -> Result<Self::Identity, PlatformFailure<String, C::Error>> {
        #[cfg(windows)]
        { held_native_identity(context, file) }
        #[cfg(not(windows))]
        {
            let metadata = platform_step(context, Work::Observe, || file.metadata()
                .map_err(|_| "DIRECT_REVISION_OBJECT_CHANGED".to_owned()))?;
            context.checkpoint().map_err(PlatformFailure::Context)?;
            let result = native_identity_from_metadata(file, &metadata);
            let checkpoint = context.checkpoint();
            match (result, checkpoint) {
                (Ok(value), Ok(())) => Ok(value),
                (Err(platform), Ok(())) => Err(PlatformFailure::Platform(platform)),
                (Ok(_), Err(context)) => Err(PlatformFailure::Context(context)),
                (Err(platform), Err(context)) => Err(PlatformFailure::AfterPlatform { platform, context }),
            }
        }
    }

    fn sync_directory_contextual(
        &self, context: &mut C, _path: &Path,
    ) -> Result<(), PlatformFailure<String, C::Error>> {
        context.checkpoint().map_err(PlatformFailure::Context)?;
        // This adapter is a read consumer. Durability is never dispatched.
        Err(PlatformFailure::Platform("DIRECT_MIGRATION_SOURCE_WRITE_FORBIDDEN".to_owned()))
    }
}

#[cfg(windows)]
fn held_native_identity<C: LegacyRevisionObjectContext>(
    context: &mut C, file: &File,
) -> Result<(u64, u64), PlatformFailure<String, C::Error>> {
    use std::cell::{Cell, RefCell};
    use eliot_searchd::native_file::{ObservationError, observe_with_check};

    context.checkpoint().map_err(PlatformFailure::Context)?;
    let original = RefCell::new(context);
    let first = RefCell::new(None);
    let refused_dispatch = Cell::new(false);
    let observed = observe_with_check(file, &|before| {
        if before && first.borrow().is_some() {
            refused_dispatch.set(true);
            return Err(ObservationError::ObservationFailed);
        }
        let mut context = original.borrow_mut();
        let result = if before { context.held_observer_dispatch() } else { context.progress() };
        if let Err(error) = result {
            first.borrow_mut().get_or_insert(error);
            if before {
                refused_dispatch.set(true);
                return Err(ObservationError::ObservationFailed);
            }
            // Let the completed ABI's result be classified, but admit no next
            // ABI. This preserves a platform failure paired with its post-poll
            // refusal; the wrapper below never releases a snapshot on refusal.
        }
        Ok(())
    });
    let checkpoint = original.borrow_mut().checkpoint();
    if let Some(context) = first.into_inner() {
        return Err(if observed.is_err() && !refused_dispatch.get() {
            PlatformFailure::AfterPlatform { platform: "DIRECT_REVISION_OBJECT_CHANGED".to_owned(), context }
        } else { PlatformFailure::Context(context) });
    }
    // The same native snapshot proves disk/NTFS, no reparse and non-directory,
    // exactly the regular-file eligibility previously checked by file.metadata.
    let result = observed.ok().filter(|value| value.attributes & 0x10 == 0)
        .map(|value| (u64::from(value.volume_serial), value.file_index))
        .ok_or_else(|| "DIRECT_REVISION_OBJECT_CHANGED".to_owned());
    match (result, checkpoint) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(platform), Ok(())) => Err(PlatformFailure::Platform(platform)),
        (Ok(_), Err(context)) => Err(PlatformFailure::Context(context)),
        (Err(platform), Err(context)) => Err(PlatformFailure::AfterPlatform { platform, context }),
    }
}

#[cfg(all(test, windows))]
mod held_identity_tests {
    use super::*;

    #[derive(Default)]
    struct DefaultContext { checkpoints: usize, observations: usize, refuse_at: Option<usize> }
    impl LegacyRevisionObjectContext for DefaultContext {
        type Error = &'static str;
        fn checkpoint(&mut self) -> Result<(), Self::Error> {
            self.checkpoints += 1;
            if self.refuse_at.is_some_and(|at| self.checkpoints >= at) {
                Err("original-post-observer-refusal")
            } else { Ok(()) }
        }
        fn charge(&mut self, work: Work) -> Result<(), Self::Error> {
            assert_eq!(work, Work::Observe);
            self.observations += 1;
            Ok(())
        }
        fn effect_attempted(&mut self) { panic!("read-only observer"); }
    }

    #[test]
    fn held_identity_default_retains_guards_and_exact_regular_file_identity() {
        use std::os::windows::fs::OpenOptionsExt;
        let file = File::open(std::env::current_exe().unwrap()).unwrap();
        let mut context = DefaultContext::default();
        let actual = held_native_identity(&mut context, &file).unwrap_or_else(|_| panic!("NTFS file"));
        assert_eq!(actual, native_identity(&file).unwrap());
        assert_eq!((context.observations, context.checkpoints), (3, 8));
        let directory = fs::OpenOptions::new().read(true).custom_flags(0x0220_0000)
            .open(std::env::temp_dir()).unwrap();
        let mut context = DefaultContext::default();
        assert!(matches!(held_native_identity(&mut context, &directory), Err(PlatformFailure::Platform(_))));
        assert_eq!(context.observations, 3, "native attributes refuse a directory without metadata duplication");
    }

    #[test]
    fn held_identity_preserves_platform_failure_paired_with_original_post_refusal() {
        let file = File::open("NUL").unwrap();
        let mut context = DefaultContext { refuse_at: Some(3), ..DefaultContext::default() };
        assert!(matches!(held_native_identity(&mut context, &file),
            Err(PlatformFailure::AfterPlatform { platform, context })
                if platform == "DIRECT_REVISION_OBJECT_CHANGED" && context == "original-post-observer-refusal"));
        assert_eq!(context.observations, 1, "no later ABI after the first post-poll refusal");
    }
}

impl LegacyPreparationArtifactPlatform for DaemonImmutableObjectPlatform {
    type Identity = (u64, u64);
    type Error = String;

    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
        ensure_directory(path)
    }

    fn verify_locator(
        &self,
        expected: &File,
        path: &Path,
    ) -> Result<(), Self::Error> {
        verify_locator(expected, path)
    }

    fn identity(&self, file: &File) -> Result<Self::Identity, Self::Error> {
        native_identity(file)
    }

    fn sync_directory(&self, path: &Path) -> Result<(), Self::Error> {
        sync_directory_result(path)
    }
}

fn verify_locator(expected: &File, path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "DIRECT_REVISION_PARENT_MISSING".to_owned())?;
    ensure_directory(parent)?;
    ensure_regular_file(path)?;
    let current = File::open(path)
        .map_err(|error| format!("DIRECT_REVISION_OBJECT_READ_FAILED:{error}"))?;
    if native_identity(&current)? != native_identity(expected)? {
        return Err("DIRECT_REVISION_OBJECT_CHANGED".to_owned());
    }
    Ok(())
}

fn native_identity(file: &File) -> Result<(u64, u64), String> {
    let metadata = file
        .metadata()
        .map_err(|_| "DIRECT_REVISION_OBJECT_CHANGED".to_owned())?;
    native_identity_from_metadata(file, &metadata)
}

fn native_identity_from_metadata(file: &File, metadata: &Metadata) -> Result<(u64, u64), String> {
    if !metadata.is_file() || is_reparse(metadata) {
        return Err("DIRECT_REVISION_OBJECT_CHANGED".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok((metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        let observed = eliot_searchd::native_file::observe(file)
            .map_err(|_| "DIRECT_REVISION_OBJECT_CHANGED".to_owned())?;
        Ok((u64::from(observed.volume_serial), observed.file_index))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err("DIRECT_REVISION_OBJECT_PLATFORM_UNSUPPORTED".to_owned())
    }
}

fn revision_read_reason(
    error: LegacyRevisionObjectError<String>,
    prefix: &str,
) -> String {
    match error {
        LegacyRevisionObjectError::Platform(reason) => reason,
        LegacyRevisionObjectError::ParentMissing => {
            "DIRECT_REVISION_PARENT_MISSING".to_owned()
        }
        LegacyRevisionObjectError::LocalNameInvalid => {
            "DIRECT_REVISION_FILENAME_INVALID".to_owned()
        }
        LegacyRevisionObjectError::SizeInvalid => format!("{prefix}:TOO_LARGE"),
        LegacyRevisionObjectError::DirectoryCreate(error) => {
            format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}")
        }
        LegacyRevisionObjectError::ObjectInspect(error) => {
            format!("DIRECT_FILE_METADATA_ERROR:{error}")
        }
        LegacyRevisionObjectError::ObjectInvalid => "DIRECT_FILE_INVALID".to_owned(),
        LegacyRevisionObjectError::ObjectRead(error) => format!("{prefix}:{error}"),
        LegacyRevisionObjectError::ReadbackMismatch => {
            format!("{prefix}:LENGTH_MISMATCH")
        }
        LegacyRevisionObjectError::IdentityChanged => {
            "DIRECT_REVISION_OBJECT_CHANGED".to_owned()
        }
        LegacyRevisionObjectError::Create(error)
        | LegacyRevisionObjectError::Write(error)
        | LegacyRevisionObjectError::PublishOutcomeUnknown(error)
        | LegacyRevisionObjectError::Cleanup(error) => format!("{prefix}:{error}"),
        LegacyRevisionObjectError::PublishPlatformOutcomeUnknown(reason) => reason,
        LegacyRevisionObjectError::ImmutableConflict => {
            "DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned()
        }
    }
}

fn revision_publish_reason(error: LegacyRevisionObjectError<String>) -> String {
    match error {
        LegacyRevisionObjectError::Platform(reason) => reason,
        LegacyRevisionObjectError::ParentMissing => {
            "DIRECT_REVISION_PARENT_MISSING".to_owned()
        }
        LegacyRevisionObjectError::LocalNameInvalid => {
            "DIRECT_REVISION_FILENAME_INVALID".to_owned()
        }
        LegacyRevisionObjectError::SizeInvalid => {
            "DIRECT_REVISION_PROTECTED_SIZE_INVALID".to_owned()
        }
        LegacyRevisionObjectError::DirectoryCreate(error) => {
            format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}")
        }
        LegacyRevisionObjectError::ObjectInspect(_) => {
            "DIRECT_REVISION_OBJECT_INSPECTION_FAILED".to_owned()
        }
        LegacyRevisionObjectError::ObjectInvalid => "DIRECT_FILE_INVALID".to_owned(),
        LegacyRevisionObjectError::ObjectRead(error) => {
            format!("DIRECT_REVISION_PROTECTED_READ_ERROR:{error}")
        }
        LegacyRevisionObjectError::ReadbackMismatch => {
            "DIRECT_REVISION_PROTECTED_READ_ERROR:LENGTH_MISMATCH".to_owned()
        }
        LegacyRevisionObjectError::Create(error) => {
            format!("DIRECT_REVISION_PROTECTED_CREATE_ERROR:{error}")
        }
        LegacyRevisionObjectError::Write(error) => {
            format!("DIRECT_REVISION_PROTECTED_WRITE_ERROR:{error}")
        }
        LegacyRevisionObjectError::PublishOutcomeUnknown(error) => {
            format!("DIRECT_REVISION_PROTECTED_PUBLISH_ERROR:{error}")
        }
        LegacyRevisionObjectError::PublishPlatformOutcomeUnknown(reason) => {
            format!("DIRECT_REVISION_PROTECTED_PUBLISH_ERROR:{reason}")
        }
        LegacyRevisionObjectError::ImmutableConflict
        | LegacyRevisionObjectError::IdentityChanged => {
            "DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned()
        }
        LegacyRevisionObjectError::Cleanup(error) => {
            format!("DIRECT_REVISION_TEMP_CLEANUP_ERROR:{error}")
        }
    }
}

fn preparation_read_reason(
    error: LegacyPreparationArtifactError<String>,
    prefix: &str,
) -> String {
    match error {
        LegacyPreparationArtifactError::Platform(reason) => reason,
        LegacyPreparationArtifactError::ParentMissing => {
            "DIRECT_REVISION_PARENT_MISSING".to_owned()
        }
        LegacyPreparationArtifactError::LocalNameInvalid => {
            "DIRECT_REVISION_FILENAME_INVALID".to_owned()
        }
        LegacyPreparationArtifactError::SizeInvalid => format!("{prefix}:TOO_LARGE"),
        LegacyPreparationArtifactError::DirectoryCreate(error) => {
            format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}")
        }
        LegacyPreparationArtifactError::ObjectInspect(error) => {
            format!("DIRECT_FILE_METADATA_ERROR:{error}")
        }
        LegacyPreparationArtifactError::ObjectInvalid => "DIRECT_FILE_INVALID".to_owned(),
        LegacyPreparationArtifactError::ObjectRead(error) => format!("{prefix}:{error}"),
        LegacyPreparationArtifactError::ReadbackMismatch => {
            format!("{prefix}:LENGTH_MISMATCH")
        }
        LegacyPreparationArtifactError::IdentityChanged => {
            "DIRECT_REVISION_OBJECT_CHANGED".to_owned()
        }
        LegacyPreparationArtifactError::Create(error)
        | LegacyPreparationArtifactError::Write(error)
        | LegacyPreparationArtifactError::PublishOutcomeUnknown(error)
        | LegacyPreparationArtifactError::Cleanup(error) => format!("{prefix}:{error}"),
        LegacyPreparationArtifactError::PublishPlatformOutcomeUnknown(reason) => reason,
        LegacyPreparationArtifactError::ImmutableConflict => {
            "DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned()
        }
    }
}

fn preparation_publish_reason(
    error: LegacyPreparationArtifactError<String>,
) -> String {
    match error {
        LegacyPreparationArtifactError::Platform(reason) => reason,
        LegacyPreparationArtifactError::ParentMissing => {
            "DIRECT_REVISION_PARENT_MISSING".to_owned()
        }
        LegacyPreparationArtifactError::LocalNameInvalid => {
            "DIRECT_REVISION_FILENAME_INVALID".to_owned()
        }
        LegacyPreparationArtifactError::SizeInvalid => {
            "DIRECT_REVISION_PROTECTED_SIZE_INVALID".to_owned()
        }
        LegacyPreparationArtifactError::DirectoryCreate(error) => {
            format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}")
        }
        LegacyPreparationArtifactError::ObjectInspect(_) => {
            "DIRECT_REVISION_OBJECT_INSPECTION_FAILED".to_owned()
        }
        LegacyPreparationArtifactError::ObjectInvalid => "DIRECT_FILE_INVALID".to_owned(),
        LegacyPreparationArtifactError::ObjectRead(error) => {
            format!("DIRECT_REVISION_PROTECTED_READ_ERROR:{error}")
        }
        LegacyPreparationArtifactError::ReadbackMismatch => {
            "DIRECT_REVISION_PROTECTED_READ_ERROR:LENGTH_MISMATCH".to_owned()
        }
        LegacyPreparationArtifactError::Create(error) => {
            format!("DIRECT_REVISION_PROTECTED_CREATE_ERROR:{error}")
        }
        LegacyPreparationArtifactError::Write(error) => {
            format!("DIRECT_REVISION_PROTECTED_WRITE_ERROR:{error}")
        }
        LegacyPreparationArtifactError::PublishOutcomeUnknown(error) => {
            format!("DIRECT_REVISION_PROTECTED_PUBLISH_ERROR:{error}")
        }
        LegacyPreparationArtifactError::PublishPlatformOutcomeUnknown(reason) => {
            format!("DIRECT_REVISION_PROTECTED_PUBLISH_ERROR:{reason}")
        }
        LegacyPreparationArtifactError::ImmutableConflict
        | LegacyPreparationArtifactError::IdentityChanged => {
            "DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned()
        }
        LegacyPreparationArtifactError::Cleanup(error) => {
            format!("DIRECT_REVISION_TEMP_CLEANUP_ERROR:{error}")
        }
    }
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

fn sync_directory_result(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        sync_directory(path)
    }
    #[cfg(not(unix))]
    {
        sync_directory(path);
        Ok(())
    }
}

#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("DIRECT_DIRECTORY_SYNC_ERROR:{error}"))
}

#[cfg(not(unix))]
pub(super) const fn sync_directory(_path: &Path) {}

#[cfg(test)]
#[path = "immutable_object_tests.rs"]
mod tests;
