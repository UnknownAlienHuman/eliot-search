//! Original-context Native observations for C07's sole immutable-object lifecycle.

use std::{cell::RefCell, fs::File, path::{Path, PathBuf}};
use search_contracts::ObjectResidencyKeyDigest;
use search_control_redb::MutationId;
use search_ports::CancellationProbe;
use search_revision_store::{
    ContextualLegacyRevisionObjectPlatform, ContextualRevisionObjectError, ContextualRevisionObjectRead,
    ContextualRevisionObjectReceipt, LegacyRevisionObjectContext,
    LegacyRevisionObjectPlatform, LegacyRevisionObjectPlatformFailure as Failure,
    LegacyRevisionObjectWork as Work, publish_legacy_revision_object_with_context,
    read_legacy_revision_object_with_context,
};
use search_runtime_owner::OwnerError;
use super::{FreshRootContext, FreshRootError};

/// Original refusal and possible effects remain distinct from the object reason.
pub(crate) enum NativeSelectedObjectFailure {
    Native { cause: OwnerError, outcome_unknown: bool },
    Object(ContextualRevisionObjectError<OwnerError, OwnerError>),
}
impl NativeSelectedObjectFailure {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Native { cause, .. } => cause.code(),
            Self::Object(error) => error.object_code().unwrap_or(error.code()),
        }
    }
    pub(crate) const fn outcome_unknown(&self) -> bool {
        match self {
            Self::Native { outcome_unknown, .. } => *outcome_unknown,
            Self::Object(error) => error.outcome_unknown(),
        }
    }
    pub(crate) const fn context_cause(&self) -> Option<&OwnerError> {
        match self {
            Self::Native { cause, .. } => Some(cause),
            Self::Object(error) => error.context_cause(),
        }
    }
}
impl std::fmt::Debug for NativeSelectedObjectFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeSelectedObjectFailure").field("code", &self.code())
            .field("outcome_unknown", &self.outcome_unknown()).finish()
    }
}
impl std::fmt::Display for NativeSelectedObjectFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.code()) }
}

struct OriginalObjectContext<'a, 'operation, 'owner> {
    native: &'a FreshRootContext<'operation, 'owner>,
    first: Option<OwnerError>,
}

const fn owner_error(error: &FreshRootError) -> OwnerError {
    match error {
        FreshRootError::Native(error) => *error,
        _ => OwnerError::OwnerGuardMismatch,
    }
}

impl LegacyRevisionObjectContext for OriginalObjectContext<'_, '_, '_> {
    type Error = OwnerError;
    fn checkpoint(&mut self) -> Result<(), OwnerError> {
        // Keep the original cancellation/deadline cause even when the parent
        // must additionally classify an earlier filesystem effect as unknown.
        if let Some(error) = self.native.operation.borrow().first_refusal { self.first.get_or_insert(error); }
        let cancellation = self.native.context().cancellation();
        let _cancelled = cancellation.is_cancelled();
        if let Some(error) = cancellation.refusal() { self.first.get_or_insert(error); }
        if let Some(error) = self.first { return Err(error); }
        self.native.checkpoint().map_err(|error| *self.first.get_or_insert_with(|| owner_error(&error)))
    }
    fn charge(&mut self, work: Work) -> Result<(), OwnerError> {
        self.checkpoint()?;
        let result = if work == Work::Effect {
            self.native.guard()?;
            self.native.operation.borrow_mut().with_binding(|binding, request, meter, _| {
                if binding.pins.direct.is_none() || binding.storage_acl.is_none() {
                    return Err(OwnerError::OwnerInvalidTransition);
                }
                meter.call(request, || Ok(()))
            })
        } else { self.native.charge(work).map_err(|error| owner_error(&error)) };
        result.map_err(|error| {
            let original = self.native.operation.borrow().first_refusal.unwrap_or(error);
            *self.first.get_or_insert(original)
        })
    }
    fn effect_attempted(&mut self) {
        // C07 invokes this immediately before the actual effect, after every
        // fallible reservation. Retain the parent's unknown-outcome fence too.
        self.native.operation.borrow_mut().output_attempted = true;
    }
}

struct NativeObjectPlatform<'a, 'operation, 'owner> {
    native: &'a FreshRootContext<'operation, 'owner>,
    root: PathBuf,
    directory: PathBuf,
    // One actual shard custody pin, not a path catalog or alternate store.
    shard: RefCell<Option<File>>,
}

// This platform cannot be used through an unmetered legacy entry point.
impl LegacyRevisionObjectPlatform for NativeObjectPlatform<'_, '_, '_> {
    type Identity = Vec<u8>;
    type Error = OwnerError;
    fn validate_directory(&self, _: &Path) -> Result<(), OwnerError> { Err(OwnerError::OwnerInvalidTransition) }
    fn verify_locator(&self, _: &File, _: &Path) -> Result<(), OwnerError> { Err(OwnerError::OwnerInvalidTransition) }
    fn identity(&self, _: &File) -> Result<Self::Identity, OwnerError> { Err(OwnerError::OwnerInvalidTransition) }
    fn sync_directory(&self, _: &Path) -> Result<(), OwnerError> { Err(OwnerError::OwnerInvalidTransition) }
}

fn checked<T>(context: &mut OriginalObjectContext<'_, '_, '_>,
    execute: impl FnOnce() -> Result<T, OwnerError>,
) -> Result<T, Failure<OwnerError, OwnerError>> {
    context.checkpoint().map_err(Failure::Context)?;
    let result = execute();
    match (result, context.checkpoint()) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(Failure::Platform(error)),
        (Ok(_), Err(error)) => Err(Failure::Context(error)),
        (Err(platform), Err(context)) => Err(Failure::AfterPlatform { platform, context }),
    }
}

impl ContextualLegacyRevisionObjectPlatform<OriginalObjectContext<'_, '_, '_>>
    for NativeObjectPlatform<'_, '_, '_>
{
    fn validate_directory_contextual(&self, context: &mut OriginalObjectContext<'_, '_, '_>, path: &Path)
        -> Result<(), Failure<OwnerError, OwnerError>> {
        checked(context, || {
            if path != self.root && path != self.directory { return Err(OwnerError::OwnerGuardMismatch); }
            self.native.operation.borrow_mut().with_binding(|binding, request, meter, _| {
                let file = crate::owner_composition::kernel::native_objects::open_existing(request, meter, path, true, false)?;
                binding.storage_acl.ok_or(OwnerError::OwnerStorageAclUnqualified)?
                    .qualify_file(request, meter, &file)?;
                if path == self.root {
                    // OBJECTS[4] is the original held revisions directory. A
                    // new path lookup may never adopt a different parent.
                    let original = binding.pins.files.get(4).ok_or(OwnerError::OwnerGuardMismatch)?;
                    meter.read(request, 120)?;
                    let before = meter.call(request, || crate::owner_composition::kernel::native_objects::native_identity(original))?;
                    let after = meter.call(request, || crate::owner_composition::kernel::native_objects::native_identity(&file))?;
                    if before != after { return Err(OwnerError::OwnerGuardMismatch); }
                } else {
                    let mut pin = self.shard.borrow_mut();
                    if let Some(previous) = &*pin {
                        // Two 20-byte identity allocations/constructions and
                        // their complete comparison share the original meter.
                        meter.read(request, 120)?;
                        let before = meter.call(request, || crate::owner_composition::kernel::native_objects::native_identity(previous))?;
                        let after = meter.call(request, || crate::owner_composition::kernel::native_objects::native_identity(&file))?;
                        if before != after { return Err(OwnerError::OwnerGuardMismatch); }
                    } else { *pin = Some(file); }
                }
                Ok(())
            })
        })
    }

    fn verify_locator_contextual(&self, context: &mut OriginalObjectContext<'_, '_, '_>,
        expected: &File, path: &Path,
    ) -> Result<(), Failure<OwnerError, OwnerError>> {
        self.validate_directory_contextual(context, &self.directory)?;
        checked(context, || {
            if path.parent() != Some(self.directory.as_path()) { return Err(OwnerError::OwnerGuardMismatch); }
            self.native.operation.borrow_mut().with_binding(|binding, request, meter, _| {
                let current = crate::owner_composition::kernel::native_objects::open_existing(request, meter, path, false, false)?;
                binding.storage_acl.ok_or(OwnerError::OwnerStorageAclUnqualified)?
                    .qualify_file(request, meter, &current)?;
                meter.read(request, 120)?;
                let before = meter.call(request, || crate::owner_composition::kernel::native_objects::native_identity(expected))?;
                let after = meter.call(request, || crate::owner_composition::kernel::native_objects::native_identity(&current))?;
                if before != after { return Err(OwnerError::OwnerGuardMismatch); }
                Ok(())
            })
        })
    }

    fn identity_contextual(&self, context: &mut OriginalObjectContext<'_, '_, '_>, file: &File)
        -> Result<Vec<u8>, Failure<OwnerError, OwnerError>> {
        checked(context, || self.native.operation.borrow_mut().with_binding(|binding, request, meter, _| {
            // The sole C07 kernel calls this before both read_data and write_data.
            // Qualify the actual held file, including a new empty stage, first.
            binding.storage_acl.ok_or(OwnerError::OwnerStorageAclUnqualified)?
                .qualify_file(request, meter, file)?;
            meter.read(request, 40)?;
            meter.call(request, || crate::owner_composition::kernel::native_objects::native_identity(file))
        }))
    }

    fn sync_directory_contextual(&self, context: &mut OriginalObjectContext<'_, '_, '_>, path: &Path)
        -> Result<(), Failure<OwnerError, OwnerError>> {
        self.validate_directory_contextual(context, path)?;
        // Preserve the existing qualified platform contract: C07 flushes actual
        // file data; directory flush is additionally supported on Unix. Windows
        // has no claimed directory-flush operation in the existing adapter.
        #[cfg(unix)]
        return checked(context, || self.native.operation.borrow_mut().with_binding(|_, request, meter, _| {
            let directory = crate::owner_composition::kernel::native_objects::open_existing(request, meter, path, true, false)?;
            meter.call(request, || directory.sync_all().map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown))
        }));
        #[cfg(not(unix))]
        context.checkpoint().map_err(Failure::Context)
    }
}

impl<'operation, 'owner> FreshRootContext<'operation, 'owner> {
    fn selected_object_platform(&self, physical: ObjectResidencyKeyDigest)
        -> Result<(NativeObjectPlatform<'_, 'operation, 'owner>, String), OwnerError> {
        self.verify_owner().map_err(|error| owner_error(&error))?;
        self.charge(Work::AllocateLocator(512)).map_err(|error| owner_error(&error))?;
        let stem = hex32(physical.as_bytes());
        let shard = stem.get(..2).filter(|value| value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(OwnerError::OwnerGuardMismatch)?;
        let root = self.operation.borrow_mut().with_binding(|binding, request, meter, _| {
            if binding.pins.direct.is_none() || binding.storage_acl.is_none() {
                return Err(OwnerError::OwnerInvalidTransition);
            }
            meter.read(request, binding.root.as_os_str().len().saturating_mul(4).saturating_add(64))?;
            Ok(binding.root.join("revisions"))
        })?;
        let directory = root.join(shard);
        Ok((NativeObjectPlatform { native: self, root, directory, shard: RefCell::new(None) }, stem))
    }

    pub(crate) fn publish_selected_object(&self, physical: ObjectResidencyKeyDigest,
        operation: MutationId, bytes: &[u8], maximum_bytes: usize,
    ) -> Result<ContextualRevisionObjectReceipt<Vec<u8>>, NativeSelectedObjectFailure> {
        let (platform, stem) = self.selected_object_platform(physical).map_err(|error| self.object_native_failure(error))?;
        self.charge(Work::AllocateLocator(320)).map_err(|error| self.object_native_failure(owner_error(&error)))?;
        let final_name = format!("{stem}.bin");
        let operation_hex = hex32(&operation.0);
        let temporary = format!(".{stem}.{operation_hex}.tmp");
        let mut context = OriginalObjectContext { native: self, first: None };
        publish_legacy_revision_object_with_context(&platform, &mut context, &platform.directory,
            &final_name, &temporary, bytes, maximum_bytes).map_err(NativeSelectedObjectFailure::Object)
    }

    pub(crate) fn read_selected_object(&self, physical: ObjectResidencyKeyDigest, maximum_bytes: usize)
        -> Result<ContextualRevisionObjectRead<Vec<u8>>, NativeSelectedObjectFailure> {
        let (platform, stem) = self.selected_object_platform(physical).map_err(|error| self.object_native_failure(error))?;
        self.charge(Work::AllocateLocator(platform.directory.as_os_str().len().saturating_mul(2).saturating_add(80)))
            .map_err(|error| self.object_native_failure(owner_error(&error)))?;
        let path = platform.directory.join(format!("{stem}.bin"));
        let mut context = OriginalObjectContext { native: self, first: None };
        read_legacy_revision_object_with_context(&platform, &mut context, &path, maximum_bytes)
            .map_err(NativeSelectedObjectFailure::Object)
    }

    fn object_native_failure(&self, cause: OwnerError) -> NativeSelectedObjectFailure {
        let operation = self.operation.borrow();
        NativeSelectedObjectFailure::Native {
            cause: operation.first_refusal.unwrap_or(cause),
            outcome_unknown: operation.output_attempted || cause == OwnerError::OwnerAcquireOutcomeUnknown,
        }
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(64);
    for byte in bytes {
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 15)]));
    }
    text
}
