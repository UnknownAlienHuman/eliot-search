//! One catalog mutation under the original native owner and retained child handles.

use std::fs::{self, File, Metadata};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use search_runtime_owner::{DrainReason, OwnerError};

use super::super::native_objects::{native_identity, read_bytes, require_kind};
use super::super::registration_native::{self, CatalogName, CatalogOpen};
use super::super::spec::LifecycleState;
use super::{
    NativeRootCapability, NativeRootRequest, NativeWork, acquire_existing, finish_in_place,
};
use crate::source_roots::{MAX_SOURCE_ROOT_FILE_BYTES, PreparedSourceRootChange, SourceRootAction};

struct CatalogFile {
    file: File,
    identity: Vec<u8>,
    length: u64,
    modified: Option<SystemTime>,
}

impl CatalogFile {
    fn acquire(
        file: File,
        request: &NativeRootRequest,
        work: &mut NativeWork,
    ) -> Result<Self, OwnerError> {
        let metadata = work.call(request, || {
            file.metadata().map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        require_kind(&metadata, false)?;
        require_single_link(&file, request, work)?;
        let identity = work.call(request, || native_identity(&file))?;
        Ok(Self {
            file,
            identity,
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    fn check(&self, request: &NativeRootRequest, work: &mut NativeWork) -> Result<(), OwnerError> {
        let metadata = work.call(request, || {
            self.file
                .metadata()
                .map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        require_kind(&metadata, false)?;
        require_single_link(&self.file, request, work)?;
        if metadata.len() != self.length
            || metadata.modified().ok() != self.modified
            || work.call(request, || native_identity(&self.file))? != self.identity
        {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        request.check()
    }
}

fn require_single_link(
    file: &File,
    request: &NativeRootRequest,
    work: &mut NativeWork,
) -> Result<(), OwnerError> {
    if work.call(request, || {
        eliot_searchd::native_file::hardlink_count(file).map_err(|_| OwnerError::OwnerGuardMismatch)
    })? != 1
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    request.check()
}

#[derive(Clone, Copy)]
enum CatalogPhase {
    Original,
    Staged,
    BackedUp,
    Published,
    Clean,
}

// Held original and staged catalog objects drop before native root custody.
struct CatalogCustody {
    original: Option<CatalogFile>,
    staged: Option<CatalogFile>,
    bytes: Option<Vec<u8>>,
    consumed: bool,
    phase: CatalogPhase,
}

fn open_catalog(
    owner: &mut NativeRootCapability,
    name: CatalogName,
    mode: CatalogOpen,
) -> Result<Option<File>, OwnerError> {
    owner.work.open(&owner.request)?;
    owner.work.call(&owner.request, || {
        registration_native::open(&owner.pins.files[3], name, mode)
    })
}

impl CatalogCustody {
    fn acquire(owner: &mut NativeRootCapability) -> Result<Self, OwnerError> {
        let original = open_catalog(owner, CatalogName::Primary, CatalogOpen::ExistingMutation)?
            .map(|file| CatalogFile::acquire(file, &owner.request, &mut owner.work))
            .transpose()?;
        let mut custody = Self {
            original,
            staged: None,
            bytes: None,
            consumed: false,
            phase: CatalogPhase::Original,
        };
        custody.verify_names(owner)?;
        if let Some(original) = &mut custody.original {
            if original.length > MAX_SOURCE_ROOT_FILE_BYTES as u64 {
                // The source owner supplies the compatible semantic refusal.
                return Ok(custody);
            }
            custody.bytes = Some(read_bytes(
                &owner.request,
                &mut owner.work,
                &mut original.file,
                MAX_SOURCE_ROOT_FILE_BYTES,
            )?);
        }
        custody.verify_names(owner)?;
        Ok(custody)
    }

    fn check_held(&self, owner: &mut NativeRootCapability) -> Result<(), OwnerError> {
        for file in self.original.iter().chain(self.staged.iter()) {
            file.check(&owner.request, &mut owner.work)?;
        }
        owner.check_context()
    }

    fn verify_names(&self, owner: &mut NativeRootCapability) -> Result<(), OwnerError> {
        self.check_held(owner)?;
        let (primary, temporary, backup) = match self.phase {
            CatalogPhase::Original => (self.original.as_ref(), None, None),
            CatalogPhase::Staged => (self.original.as_ref(), self.staged.as_ref(), None),
            CatalogPhase::BackedUp => (None, self.staged.as_ref(), self.original.as_ref()),
            CatalogPhase::Published => (self.staged.as_ref(), None, self.original.as_ref()),
            CatalogPhase::Clean => (self.staged.as_ref(), None, None),
        };
        for (name, expected) in [
            (CatalogName::Primary, primary),
            (CatalogName::Temporary, temporary),
            (CatalogName::Backup, backup),
        ] {
            let current = open_catalog(owner, name, CatalogOpen::ExistingRead)?;
            match (current, expected) {
                (None, None) => {}
                (Some(file), Some(expected)) => {
                    let actual = CatalogFile::acquire(file, &owner.request, &mut owner.work)?;
                    if actual.identity != expected.identity
                        || actual.length != expected.length
                        || actual.modified != expected.modified
                    {
                        return Err(OwnerError::OwnerGuardMismatch);
                    }
                }
                _ => return Err(OwnerError::OwnerRecoveryQuarantined),
            }
        }
        owner.check_context()
    }

    fn stage(&mut self, owner: &mut NativeRootCapability, bytes: &[u8]) -> Result<(), OwnerError> {
        self.verify_names(owner)?;
        owner.verify()?;
        owner
            .work
            .arrive(&owner.request, "registration-before-create")?;
        let file = open_catalog(owner, CatalogName::Temporary, CatalogOpen::CreateNew)?
            .ok_or(OwnerError::OwnerGuardMismatch)?;
        // Retain the new child immediately; no error path unlinks uncertain files.
        self.staged = Some(CatalogFile::acquire(file, &owner.request, &mut owner.work)?);
        let staged = self.staged.as_mut().ok_or(OwnerError::OwnerGuardMismatch)?;
        let mut remaining = bytes;
        while !remaining.is_empty() {
            owner.work.write(&owner.request, remaining.len())?;
            let count = owner.work.call(&owner.request, || {
                staged
                    .file
                    .write(remaining)
                    .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })?;
            if count == 0 || count > remaining.len() {
                return Err(OwnerError::OwnerAcquireOutcomeUnknown);
            }
            remaining = &remaining[count..];
        }
        owner.work.call(&owner.request, || {
            staged
                .file
                .sync_all()
                .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
        })?;
        if read_bytes(
            &owner.request,
            &mut owner.work,
            &mut staged.file,
            MAX_SOURCE_ROOT_FILE_BYTES,
        )? != bytes
        {
            return Err(OwnerError::OwnerAcquireOutcomeUnknown);
        }
        let metadata = owner.work.call(&owner.request, || {
            staged
                .file
                .metadata()
                .map_err(|_| OwnerError::OwnerGuardMismatch)
        })?;
        staged.length = metadata.len();
        staged.modified = metadata.modified().ok();
        self.phase = CatalogPhase::Staged;
        owner
            .work
            .arrive(&owner.request, "registration-stage-verified")?;
        self.verify_names(owner)
    }

    fn publish(
        &mut self,
        owner: &mut NativeRootCapability,
        bytes: &[u8],
    ) -> Result<(), OwnerError> {
        self.stage(owner, bytes)?;
        owner.verify()?;
        if let Some(original) = &self.original {
            owner.work.call(&owner.request, || {
                registration_native::rename(
                    &original.file,
                    &owner.pins.files[3],
                    CatalogName::Backup,
                )
            })?;
            self.phase = CatalogPhase::BackedUp;
            owner
                .work
                .arrive(&owner.request, "registration-original-backed-up")?;
            self.verify_names(owner)?;
        }
        let staged = self.staged.as_ref().ok_or(OwnerError::OwnerGuardMismatch)?;
        owner.work.call(&owner.request, || {
            registration_native::rename(&staged.file, &owner.pins.files[3], CatalogName::Primary)
        })?;
        self.phase = CatalogPhase::Published;
        owner
            .work
            .arrive(&owner.request, "registration-primary-published")?;
        self.verify_names(owner)?;
        // Exact primary name, identity and bytes are proven before deleting backup.
        let staged = self.staged.as_mut().ok_or(OwnerError::OwnerGuardMismatch)?;
        if read_bytes(
            &owner.request,
            &mut owner.work,
            &mut staged.file,
            MAX_SOURCE_ROOT_FILE_BYTES,
        )? != bytes
        {
            return Err(OwnerError::OwnerAcquireOutcomeUnknown);
        }
        owner.verify()?;
        if let Some(original) = &self.original {
            owner.work.call(&owner.request, || {
                registration_native::mark_delete(&original.file)
            })?;
            drop(self.original.take()); // Close the exact marked original immediately.
        }
        self.phase = CatalogPhase::Clean;
        owner
            .work
            .arrive(&owner.request, "registration-backup-closed")?;
        self.verify_names(owner)?;
        owner.verify()
    }
}

/// Source policy can inspect and prepare; only this owner publishes or releases.
pub struct NativeRegistrationOperation<'owner> {
    owner: &'owner mut NativeRootCapability,
    catalog: &'owner mut CatalogCustody,
    output_attempted: &'owner mut bool,
}

impl NativeRegistrationOperation<'_> {
    pub(crate) fn root(&self) -> &Path {
        &self.owner.root
    }

    fn call<T>(
        &mut self,
        action: impl FnOnce(&NativeRootRequest, &mut NativeWork) -> Result<T, OwnerError>,
    ) -> Result<T, OwnerError> {
        if let Some(error) = self.owner.poisoned {
            return Err(error);
        }
        let result = self
            .catalog
            .check_held(self.owner)
            .and_then(|()| action(&self.owner.request, &mut self.owner.work));
        result.map_err(|error| self.owner.failure(error))
    }

    pub(crate) fn checkpoint(&mut self) -> Result<(), OwnerError> {
        // RELEASED acquisition already validated the native bindings. Retained
        // parent/catalog custody spans effect-free planning. ACTIVE publication
        // revalidates the whole owner immediately before and after its effect.
        let binding = if self.owner.effect_observed {
            self.owner.verify()
        } else {
            self.owner.check_context()
        };
        let result = binding.and_then(|()| self.catalog.verify_names(self.owner));
        result.map_err(|error| self.owner.failure(error))
    }

    pub(crate) fn registration_bytes(&mut self) -> Result<Option<Vec<u8>>, String> {
        if self.catalog.consumed {
            return Err(OwnerError::OwnerOperationConflict.code().to_owned());
        }
        self.catalog.consumed = true;
        if self
            .catalog
            .original
            .as_ref()
            .is_some_and(|file| file.length > MAX_SOURCE_ROOT_FILE_BYTES as u64)
        {
            return Err("SOURCE_ROOT_CONFIG_TOO_LARGE".to_owned());
        }
        self.checkpoint().map_err(|error| error.code().to_owned())?;
        Ok(self.catalog.bytes.take())
    }

    pub(crate) fn source_root_metadata(
        &mut self,
        path: &Path,
    ) -> Result<io::Result<Metadata>, OwnerError> {
        self.call(|request, work| work.call(request, || Ok(fs::symlink_metadata(path))))
    }

    pub(crate) fn source_root_canonicalize(
        &mut self,
        path: &Path,
    ) -> Result<io::Result<PathBuf>, OwnerError> {
        self.call(|request, work| work.call(request, || Ok(fs::canonicalize(path))))
    }

    fn emit(&mut self, writer: &mut impl Write, bytes: &[u8]) -> Result<(), OwnerError> {
        self.checkpoint()?;
        let mut remaining = bytes;
        while !remaining.is_empty() {
            let count = self.output_call(Some(remaining.len()), || writer.write(remaining))?;
            if count == 0 || count > remaining.len() {
                self.owner.poisoned = Some(OwnerError::OwnerAcquireOutcomeUnknown);
                return Err(OwnerError::OwnerAcquireOutcomeUnknown);
            }
            remaining = &remaining[count..];
        }
        self.output_call(None, || writer.flush())?;
        self.checkpoint().map_err(|error| {
            if *self.output_attempted {
                self.owner.poisoned = Some(OwnerError::OwnerAcquireOutcomeUnknown);
                OwnerError::OwnerAcquireOutcomeUnknown
            } else {
                error
            }
        })
    }

    fn output_call<T>(
        &mut self,
        bytes: Option<usize>,
        output: impl FnOnce() -> io::Result<T>,
    ) -> Result<T, OwnerError> {
        let result = (|| {
            if let Some(error) = self.owner.poisoned {
                return Err(error);
            }
            self.catalog.check_held(self.owner)?;
            if let Some(bytes) = bytes {
                self.owner.work.write(&self.owner.request, bytes)?;
            }
            self.owner.work.call(&self.owner.request, || {
                *self.output_attempted = true;
                output().map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })
        })();
        result.map_err(|error| {
            if *self.output_attempted {
                self.owner.poisoned = Some(OwnerError::OwnerAcquireOutcomeUnknown);
                OwnerError::OwnerAcquireOutcomeUnknown
            } else {
                self.owner.failure(error)
            }
        })
    }
}

struct RegistrationHolder {
    catalog: CatalogCustody,
    output_attempted: bool,
    owner: Box<NativeRootCapability>,
}

impl RegistrationHolder {
    fn operation(&mut self) -> NativeRegistrationOperation<'_> {
        NativeRegistrationOperation {
            owner: &mut self.owner,
            catalog: &mut self.catalog,
            output_attempted: &mut self.output_attempted,
        }
    }

    fn close(self, release_owner: bool) -> Result<(), OwnerError> {
        let Self {
            catalog,
            output_attempted,
            owner,
        } = self;
        close_parts(Some(catalog), output_attempted, owner, release_owner)
    }
}

fn close_parts(
    catalog: Option<CatalogCustody>,
    output_attempted: bool,
    mut owner: Box<NativeRootCapability>,
    release_owner: bool,
) -> Result<(), OwnerError> {
    drop(catalog);
    let mut result = owner
        .work
        .arrive(&owner.request, "registration-children-closed")
        .map_err(|error| owner.failure(error));
    // Observed failure must never attempt further clean lifecycle writes.
    if release_owner && result.is_ok() && owner.poisoned.is_none() {
        result = owner
            .begin_drain(DrainReason::Shutdown)
            .and_then(|()| owner.release_cleanly().map(|_| ()));
    }
    let late_error = owner.poisoned.unwrap_or_else(|| {
        if owner.effect_observed && owner.record.lifecycle == LifecycleState::Released {
            OwnerError::OwnerReleaseOutcomeUnknown
        } else {
            OwnerError::OwnerAcquireOutcomeUnknown
        }
    });
    let NativeRootCapability {
        pins,
        root,
        installation,
        record,
        request,
        mut work,
        bound_root,
        exclusions,
        effect_observed,
        ..
    } = *owner;
    drop((pins, root, installation, record, bound_root));
    let before = work.arrive(&request, "registration-pins-closed");
    drop(exclusions);
    let after = work.arrive(&request, "registration-exclusions-released");
    result.and(before).and(after).map_err(|error| {
        if output_attempted || effect_observed {
            late_error
        } else {
            error
        }
    })
}
fn error_bytes(error: &str) -> Vec<u8> {
    format!(
        "{{\"error\":{}}}\n",
        crate::service_output::json_string(&eliot_searchd::diagnostics::sanitize_code(error))
    )
    .into_bytes()
}

/// No DIRECT/credential/recovery path; full source preflight precedes ACTIVE.
pub fn run_source_registration(
    request: NativeRootRequest,
    action: SourceRootAction,
    requested: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), String> {
    run_with_work(
        request,
        NativeWork::new(),
        action,
        requested,
        stdout,
        stderr,
    )
}

fn run_with_work(
    request: NativeRootRequest,
    mut work: NativeWork,
    action: SourceRootAction,
    requested: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), String> {
    let acquired = match acquire_existing(&request, &mut work) {
        Ok(Ok(acquired)) => acquired,
        result => {
            let error = match result {
                Err(error) => error,
                _ => OwnerError::OwnerRecoveryQuarantined,
            };
            let request = request.retain_refusal_work(work);
            let line = String::from_utf8(error_bytes(error.code()))
                .map_err(|_| error.code().to_owned())?;
            request
                .emit(stderr, line.trim_end())
                .map_err(|error| error.code().to_owned())?;
            return Err(error.code().to_owned());
        }
    };
    let (owner, successor) = acquired.bind(request, work);
    let mut owner = Box::new(owner);
    let catalog = match CatalogCustody::acquire(&mut owner) {
        Ok(catalog) => catalog,
        Err(error) => {
            let line =
                String::from_utf8(error_bytes(error.code())).map_err(|_| error.code().to_owned());
            let emitted = line.and_then(|line| {
                owner
                    .request
                    .emit_using_work(&mut owner.work, stderr, line.trim_end())
                    .map_err(|error| error.code().to_owned())
            });
            let closed = close_parts(None, emitted.is_ok(), owner, false)
                .map_err(|error| error.code().to_owned());
            emitted?;
            closed?;
            return Err(error.code().to_owned());
        }
    };
    let mut holder = RegistrationHolder {
        catalog,
        output_attempted: false,
        owner,
    };
    let result = (|| {
        let plan =
            match PreparedSourceRootChange::prepare(&mut holder.operation(), action, requested) {
                Ok(plan) => plan,
                Err(error) => {
                    holder
                        .operation()
                        .emit(stderr, &error_bytes(&error))
                        .map_err(|error| error.code().to_owned())?;
                    return Ok(Some(error));
                }
            };
        holder
            .operation()
            .checkpoint()
            .map_err(|error| error.code().to_owned())?;
        finish_in_place(&mut holder.owner, successor).map_err(|error| error.code().to_owned())?;
        if let Some(bytes) = plan.replacement() {
            let result = holder.catalog.publish(&mut holder.owner, bytes);
            result.map_err(|error| holder.owner.failure(error).code().to_owned())?;
        }
        // Compatible registration observation; it does not claim completed release.
        holder
            .operation()
            .emit(stdout, plan.output())
            .map_err(|error| error.code().to_owned())?;
        Ok(None)
    })();
    let release_owner = result.as_ref().is_ok_and(Option::is_none);
    let closed = holder
        .close(release_owner)
        .map_err(|error| error.code().to_owned());
    match result {
        Err(error) => {
            let _ = closed;
            Err(error)
        }
        Ok(Some(error)) => {
            closed?;
            Err(error)
        }
        Ok(None) => closed,
    }
}

#[cfg(test)]
#[path = "registration_mutation_tests.rs"]
mod tests;
