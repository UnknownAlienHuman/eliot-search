//! Original-context Native observations for C07's sole immutable-object lifecycle.

use std::{cell::RefCell, ffi::OsStr, fs::File, path::{Component, Path, PathBuf}};
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
use search_revision_store::derived_artifact::{
    DerivedArtifactErrorV1, DerivedArtifactIoErrorV1, DerivedArtifactObjectRefV1,
    DerivedArtifactPayloadCandidateV1, DerivedArtifactReadReceiptV1,
    DerivedArtifactVerificationPortV1, DerivedArtifactWriteReceiptV1,
    MAX_DERIVED_DESCRIPTOR_BYTES, publish_derived_artifact_v1, read_derived_artifact_v1,
};
use search_runtime_owner::OwnerError;
use super::{FreshRootContext, FreshRootError};

const DERIVED_DIRECTORY_DEPTH: usize = 3;

/// Original refusal and possible effects remain distinct from the object reason.
#[allow(clippy::redundant_pub_crate)] // Sibling kernel callers inspect failures returned by FreshRootContext.
pub(crate) enum NativeSelectedObjectFailure {
    Native { cause: OwnerError, outcome_unknown: bool },
    Object(ContextualRevisionObjectError<OwnerError, OwnerError>),
}
impl NativeSelectedObjectFailure {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Native { cause, .. } => cause.code(),
            Self::Object(error) => error.object_code().unwrap_or_else(|| error.code()),
        }
    }
    pub(crate) const fn outcome_unknown(&self) -> bool {
        match self {
            Self::Native { outcome_unknown, .. } => *outcome_unknown,
            Self::Object(error) => error.outcome_unknown(),
        }
    }
    #[allow(dead_code)] // Retain the existing native failure-inspection API.
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

/// Native caller context passed to C07's actual verifier and sole I/O kernel.
#[allow(clippy::redundant_pub_crate)] // Parent-owned verifier implementations borrow this context.
pub(crate) struct OriginalObjectContext<'a, 'operation, 'owner> {
    native: &'a FreshRootContext<'operation, 'owner>,
    first: Option<OwnerError>,
}
impl<'a, 'operation, 'owner> OriginalObjectContext<'a, 'operation, 'owner> {
    /// Borrow the actual root context for parent-owned `C02`/`Crypto`/`Windows` checks.
    /// Native custody stays in `FreshRootContext` and all work uses its original meter.
    #[allow(dead_code)] // Used by the parent verifier at reverse-consumer cutover.
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) const fn fresh_root(&self) -> &'a FreshRootContext<'operation, 'owner> {
        self.native
    }
}

/// C07 error plus the original Native operation's possible-effect fence.
#[allow(dead_code)] // Reverse-consumer integration is intentionally outside this prerequisite.
#[allow(clippy::redundant_pub_crate)] // Returned to the sibling kernel consumer.
pub(crate) enum NativeDerivedArtifactFailure<E> {
    Native { cause: OwnerError, outcome_unknown: bool },
    Artifact { cause: DerivedArtifactIoErrorV1<OwnerError, OwnerError, E>, outcome_unknown: bool },
}
#[allow(dead_code)] // The reverse consumer will inspect this result.
impl<E> NativeDerivedArtifactFailure<E> {
    pub(crate) const fn outcome_unknown(&self) -> bool {
        match self {
            Self::Native { outcome_unknown, .. } | Self::Artifact { outcome_unknown, .. } => *outcome_unknown,
        }
    }
}
impl<E> std::fmt::Debug for NativeDerivedArtifactFailure<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.outcome_unknown() { "NATIVE_DERIVED_ARTIFACT_OUTCOME_UNKNOWN" }
            else { "NATIVE_DERIVED_ARTIFACT_REFUSED" })
    }
}
impl<E> std::fmt::Display for NativeDerivedArtifactFailure<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { std::fmt::Debug::fmt(self, f) }
}

#[allow(dead_code)] // Used by the deferred reverse-consumer facade.
enum DerivedPlatformError { Native(OwnerError), Profile(DerivedArtifactErrorV1) }

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
    // Exact root-relative prefixes for one object: one raw shard or C07's three components.
    directories: [Option<PathBuf>; DERIVED_DIRECTORY_DEPTH],
    directory_pins: RefCell<[Option<File>; DERIVED_DIRECTORY_DEPTH]>,
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
            let child_index = self.directories.iter().position(|directory| directory.as_deref() == Some(path));
            if path != self.root && child_index.is_none() { return Err(OwnerError::OwnerGuardMismatch); }
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
                    let index = child_index.ok_or(OwnerError::OwnerGuardMismatch)?;
                    let mut pins = self.directory_pins.borrow_mut();
                    if pins[..index].iter().any(Option::is_none) {
                        return Err(OwnerError::OwnerGuardMismatch);
                    }
                    if let Some(previous) = &pins[index] {
                        // Two 20-byte identity allocations/constructions and
                        // their complete comparison share the original meter.
                        meter.read(request, 120)?;
                        let before = meter.call(request, || crate::owner_composition::kernel::native_objects::native_identity(previous))?;
                        let after = meter.call(request, || crate::owner_composition::kernel::native_objects::native_identity(&file))?;
                        if before != after { return Err(OwnerError::OwnerGuardMismatch); }
                    } else { pins[index] = Some(file); }
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
        let root = self.native_cas_root()?;
        let directory = root.join(shard);
        Ok((NativeObjectPlatform {
            native: self, root, directory: directory.clone(),
            directories: [Some(directory), None, None],
            directory_pins: RefCell::new([None, None, None]),
        }, stem))
    }

    fn native_cas_root(&self) -> Result<PathBuf, OwnerError> {
        self.operation.borrow_mut().with_binding(|binding, request, meter, _| {
            if binding.pins.direct.is_none() || binding.storage_acl.is_none() {
                return Err(OwnerError::OwnerInvalidTransition);
            }
            meter.read(request, binding.root.as_os_str().len().saturating_mul(4).saturating_add(64))?;
            Ok(binding.root.join("revisions"))
        })
    }

    #[allow(dead_code)] // Used by the deferred reverse-consumer facade.
    fn derived_object_platform(&self, reference: &DerivedArtifactObjectRefV1)
        -> Result<(NativeObjectPlatform<'_, 'operation, 'owner>, PathBuf), DerivedPlatformError> {
        self.verify_owner().map_err(|error| DerivedPlatformError::Native(owner_error(&error)))?;
        // Address calculation and all bounded path construction use the original work meter.
        self.charge(Work::AllocateLocator(1024))
            .map_err(|error| DerivedPlatformError::Native(owner_error(&error)))?;
        self.charge(Work::Compare(MAX_DERIVED_DESCRIPTOR_BYTES * 16))
            .map_err(|error| DerivedPlatformError::Native(owner_error(&error)))?;
        let address = reference.address().map_err(DerivedPlatformError::Profile)?;
        let relative = address.relative_path();
        let mut parts = relative.split('/');
        let components = [
            parts.next().ok_or(DerivedPlatformError::Native(OwnerError::OwnerGuardMismatch))?,
            parts.next().ok_or(DerivedPlatformError::Native(OwnerError::OwnerGuardMismatch))?,
            parts.next().ok_or(DerivedPlatformError::Native(OwnerError::OwnerGuardMismatch))?,
        ];
        let final_name = parts.next().ok_or(DerivedPlatformError::Native(OwnerError::OwnerGuardMismatch))?;
        if parts.next().is_some()
            || components.iter().chain(std::iter::once(&final_name)).any(|component| !one_local_component(component))
        {
            return Err(DerivedPlatformError::Native(OwnerError::OwnerGuardMismatch));
        }
        let root = self.native_cas_root().map_err(DerivedPlatformError::Native)?;
        let mut directory = root.clone();
        let mut directories: [Option<PathBuf>; DERIVED_DIRECTORY_DEPTH] = [None, None, None];
        for (index, component) in components.iter().enumerate() {
            directory.push(component);
            directories[index] = Some(directory.clone());
        }
        Ok((NativeObjectPlatform {
            native: self, root: root.clone(), directory, directories,
            directory_pins: RefCell::new([None, None, None]),
        }, root))
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

    #[allow(dead_code)] // Narrow C07 reverse-consumer entrypoint.
    pub(crate) fn publish_derived_artifact_v1<'native, V>(&'native self, verifier: &mut V,
        temporary_name: &str, candidate: &DerivedArtifactPayloadCandidateV1, maximum_bytes: usize,
    ) -> Result<DerivedArtifactWriteReceiptV1<Vec<u8>>,
        NativeDerivedArtifactFailure<<V as DerivedArtifactVerificationPortV1<
            OriginalObjectContext<'native, 'operation, 'owner>,
        >>::Error>>
    where V: DerivedArtifactVerificationPortV1<OriginalObjectContext<'native, 'operation, 'owner>> {
        let (platform, root) = self.derived_object_platform(candidate.object_ref()).map_err(|error| match error {
            DerivedPlatformError::Native(cause) => self.derived_native_failure(cause),
            DerivedPlatformError::Profile(error) => self.derived_artifact_failure(DerivedArtifactIoErrorV1::Profile(error)),
        })?;
        let mut context = OriginalObjectContext { native: self, first: None };
        publish_derived_artifact_v1(&platform, verifier, &mut context, &root, temporary_name, candidate, maximum_bytes)
            .map_err(|cause| self.derived_artifact_failure(cause))
    }

    #[allow(dead_code)] // Narrow C07 reverse-consumer entrypoint.
    pub(crate) fn read_derived_artifact_v1<'native, V>(&'native self, verifier: &mut V,
        expected: &DerivedArtifactObjectRefV1, maximum_bytes: usize,
    ) -> Result<DerivedArtifactReadReceiptV1<Vec<u8>>,
        NativeDerivedArtifactFailure<<V as DerivedArtifactVerificationPortV1<
            OriginalObjectContext<'native, 'operation, 'owner>,
        >>::Error>>
    where V: DerivedArtifactVerificationPortV1<OriginalObjectContext<'native, 'operation, 'owner>> {
        let (platform, root) = self.derived_object_platform(expected).map_err(|error| match error {
            DerivedPlatformError::Native(cause) => self.derived_native_failure(cause),
            DerivedPlatformError::Profile(error) => self.derived_artifact_failure(DerivedArtifactIoErrorV1::Profile(error)),
        })?;
        let mut context = OriginalObjectContext { native: self, first: None };
        read_derived_artifact_v1(&platform, verifier, &mut context, &root, expected, maximum_bytes)
            .map_err(|cause| self.derived_artifact_failure(cause))
    }

    #[allow(dead_code)] // Called by the deferred reverse-consumer facade.
    fn derived_native_failure<E>(&self, cause: OwnerError) -> NativeDerivedArtifactFailure<E> {
        let operation = self.operation.borrow();
        NativeDerivedArtifactFailure::Native { cause: operation.first_refusal.unwrap_or(cause),
            outcome_unknown: operation.output_attempted || cause == OwnerError::OwnerAcquireOutcomeUnknown }
    }

    #[allow(dead_code)] // Called by the deferred reverse-consumer facade.
    fn derived_artifact_failure<E>(&self, cause: DerivedArtifactIoErrorV1<OwnerError, OwnerError, E>)
        -> NativeDerivedArtifactFailure<E> {
        let operation = self.operation.borrow();
        NativeDerivedArtifactFailure::Artifact { outcome_unknown: operation.output_attempted || cause.outcome_unknown(), cause }
    }

    fn object_native_failure(&self, cause: OwnerError) -> NativeSelectedObjectFailure {
        let operation = self.operation.borrow();
        NativeSelectedObjectFailure::Native {
            cause: operation.first_refusal.unwrap_or(cause),
            outcome_unknown: operation.output_attempted || cause == OwnerError::OwnerAcquireOutcomeUnknown,
        }
    }
}

#[allow(dead_code)] // Used by the deferred reverse-consumer facade.
fn one_local_component(value: &str) -> bool {
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(component)) if component == OsStr::new(value))
        && components.next().is_none()
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
