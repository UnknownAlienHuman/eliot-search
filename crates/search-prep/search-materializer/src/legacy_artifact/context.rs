//! Original caller context through the sole legacy immutable-object kernel.

use core::fmt;
use std::convert::Infallible;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use super::{
    LegacyPreparationArtifactError as ObjectError, LegacyPreparationArtifactPlatform as Platform,
    LegacyPreparationArtifactRead as ObjectRead, LegacyPreparationArtifactReceipt as ObjectReceipt,
    TemporaryArtifactGuard, regular, valid_local_name,
};

/// One attempted owner seam, charged before dispatch. Byte counts describe
/// requested I/O or logical buffer capacity, not observed transfers or RSS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyPreparationArtifactWork {
    /// A filesystem metadata or qualified platform observation.
    Observe,
    /// One file open attempt.
    Open,
    /// Requested read bytes; short and interrupted attempts are charged again.
    Read(usize),
    /// Requested write bytes; short and interrupted attempts are charged again.
    Write(usize),
    /// Logical object-buffer capacity, before allocation.
    Allocate(usize),
    /// Logical encoded locator storage, before constructing a child path.
    AllocateLocator(usize),
    /// Bounded exact byte comparison.
    Compare(usize),
    /// One directory creation, temporary creation, link or durability attempt.
    Effect,
}

/// Borrow the caller's original deadline, cancellation and cumulative allowance.
///
/// The implementation must not renew them or mint root/source/recovery authority.
/// Platform callbacks remain cooperatively observed rather than preemptible.
pub trait LegacyPreparationArtifactContext {
    /// Private caller refusal; ordinary error rendering never exposes it.
    type Error;
    /// Poll the original context without replacing its identity or allowance.
    fn checkpoint(&mut self) -> Result<(), Self::Error>;
    /// Consume this attempt's allowance before allocation or dispatch.
    fn charge(&mut self, work: LegacyPreparationArtifactWork) -> Result<(), Self::Error>;
    /// Latch possible effects in the original parent before the first dispatch.
    /// This notification neither certifies completion nor authorizes replay.
    fn effect_attempted(&mut self);
}

/// Failure inside a qualified contextual platform observation. Rendering is
/// content-free; implementations retain their own original caller refusal.
pub enum LegacyPreparationArtifactPlatformFailure<E, C> {
    /// Qualified platform observation failed.
    Platform(E),
    /// Original context refused an internal platform attempt.
    Context(C),
    /// Preserve a platform failure when its post-call context poll also refuses.
    AfterPlatform {
        /// Private platform failure; ordinary rendering stays content-free.
        platform: E,
        /// Original caller refusal, without a replacement deadline or allowance.
        context: C,
    },
}
impl<E, C> fmt::Debug for LegacyPreparationArtifactPlatformFailure<E, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LegacyPreparationArtifactPlatformFailure(<opaque>)")
    }
}

/// Qualified platform observations explicitly borrowing the same context.
///
/// There is no default unmetered implementation: nested opens, native calls and
/// allocations must be charged and polled by the actual platform owner.
pub trait ContextualLegacyPreparationArtifactPlatform<C: LegacyPreparationArtifactContext>:
    Platform
{
    /// Validate an admitted directory and meter the actual platform closure.
    fn validate_directory_contextual(
        &self,
        context: &mut C,
        path: &Path,
    ) -> Result<(), LegacyPreparationArtifactPlatformFailure<Self::Error, C::Error>>;
    /// Revalidate the locator against the retained file under the original context.
    fn verify_locator_contextual(
        &self,
        context: &mut C,
        file: &File,
        path: &Path,
    ) -> Result<(), LegacyPreparationArtifactPlatformFailure<Self::Error, C::Error>>;
    /// Observe native identity, charging any internal platform attempts.
    fn identity_contextual(
        &self,
        context: &mut C,
        file: &File,
    ) -> Result<Self::Identity, LegacyPreparationArtifactPlatformFailure<Self::Error, C::Error>>;
    /// Apply the qualified directory durability contract under the same context.
    fn sync_directory_contextual(
        &self,
        context: &mut C,
        path: &Path,
    ) -> Result<(), LegacyPreparationArtifactPlatformFailure<Self::Error, C::Error>>;
}

/// Furthest component-local mutation boundary actually dispatched by this call.
/// These observations do not identify or reconcile the enclosing sync request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ContextualPreparationArtifactEffect {
    /// No artifact-local mutation was dispatched by this call.
    NotAttempted,
    /// Child directory creation or its durability may have applied.
    DirectoryAttempted,
    /// Temporary creation, writes or staging durability may have applied.
    StagingAttempted,
    /// The final link or its durability may have applied.
    PublicationAttempted,
}

enum Failure<E, C> {
    Object(ObjectError<E>),
    Context(C),
    AfterObject { object: ObjectError<E>, context: C },
}

/// Content-free contextual failure. This path performs no owner cleanup after
/// uncertainty and grants no cleanup, reconciliation or retry authority.
pub struct ContextualPreparationArtifactError<E, C> {
    failure: Failure<E, C>,
    effect: ContextualPreparationArtifactEffect,
}

/// One contextual result preserving the private platform and original caller
/// failure types behind the redacted owner error.
pub type ContextualPreparationArtifactResult<T, E, C> =
    Result<T, ContextualPreparationArtifactError<E, C>>;

impl<E, C> ContextualPreparationArtifactError<E, C> {
    /// Stable artifact-local failure code, including a failure preceding refusal.
    /// Raw platform and filesystem errors remain private.
    #[must_use]
    pub const fn object_code(&self) -> Option<&'static str> {
        let object = match &self.failure {
            Failure::Object(object) | Failure::AfterObject { object, .. } => object,
            Failure::Context(_) => return None,
        };
        Some(match object {
            ObjectError::Platform(_) => "PREPARATION_ARTIFACT_PLATFORM_FAILED",
            ObjectError::ParentMissing => "PREPARATION_ARTIFACT_PARENT_MISSING",
            ObjectError::LocalNameInvalid => "PREPARATION_ARTIFACT_NAME_INVALID",
            ObjectError::SizeInvalid => "PREPARATION_ARTIFACT_SIZE_INVALID",
            ObjectError::DirectoryCreate(_) => "PREPARATION_ARTIFACT_DIRECTORY_FAILED",
            ObjectError::ObjectInspect(_) => "PREPARATION_ARTIFACT_INSPECT_FAILED",
            ObjectError::ObjectInvalid => "PREPARATION_ARTIFACT_INVALID",
            ObjectError::ObjectRead(_) => "PREPARATION_ARTIFACT_READ_FAILED",
            ObjectError::ReadbackMismatch => "PREPARATION_ARTIFACT_READBACK_MISMATCH",
            ObjectError::Create(_) => "PREPARATION_ARTIFACT_CREATE_FAILED",
            ObjectError::Write(_) => "PREPARATION_ARTIFACT_WRITE_FAILED",
            ObjectError::PublishOutcomeUnknown(_) => "PREPARATION_ARTIFACT_PUBLICATION_FAILED",
            ObjectError::PublishPlatformOutcomeUnknown(_) => {
                "PREPARATION_ARTIFACT_DURABILITY_FAILED"
            }
            ObjectError::ImmutableConflict => "PREPARATION_ARTIFACT_CONFLICT",
            ObjectError::IdentityChanged => "PREPARATION_ARTIFACT_IDENTITY_CHANGED",
            ObjectError::Cleanup(_) => "PREPARATION_ARTIFACT_CLEANUP_FAILED",
        })
    }
    /// Explicitly borrow the caller's own original refusal. This is not exposed
    /// by ordinary Debug, Display or the standard error chain.
    #[must_use]
    pub const fn context_cause(&self) -> Option<&C> {
        match &self.failure {
            Failure::Context(context) | Failure::AfterObject { context, .. } => Some(context),
            Failure::Object(_) => None,
        }
    }
    /// Whether this call attempted any filesystem mutation before failing.
    #[must_use]
    pub const fn outcome_unknown(&self) -> bool {
        !matches!(
            self.effect,
            ContextualPreparationArtifactEffect::NotAttempted
        )
    }
    /// Furthest actual artifact-local dispatch boundary, without recovery authority.
    #[must_use]
    pub const fn effect(&self) -> ContextualPreparationArtifactEffect {
        self.effect
    }
    /// Stable content-free reason, including possible earlier directory effects.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        if self.outcome_unknown() {
            "PREPARATION_ARTIFACT_OUTCOME_UNKNOWN"
        } else {
            match &self.failure {
                Failure::Context(_) | Failure::AfterObject { .. } => {
                    "PREPARATION_ARTIFACT_CONTEXT_REFUSED"
                }
                Failure::Object(_) => "PREPARATION_ARTIFACT_FAILED",
            }
        }
    }
}

impl<E, C> fmt::Display for ContextualPreparationArtifactError<E, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl<E, C> fmt::Debug for ContextualPreparationArtifactError<E, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl<E, C> std::error::Error for ContextualPreparationArtifactError<E, C> {}

/// Exact final-artifact readback only, without source or operation reconciliation.
/// A newly created stage is not unlinked by this call, even after verified success.
#[must_use]
pub struct ContextualPreparationArtifactReceipt<I> {
    object: ObjectReceipt<I>,
    staging_cleanup_deferred: bool,
}
impl<I> ContextualPreparationArtifactReceipt<I> {
    /// Stable final identity observed through the platform's exact readback.
    #[must_use]
    pub const fn identity(&self) -> &I {
        self.object.identity()
    }
    /// Exact final object byte length.
    #[must_use]
    pub const fn encoded_bytes(&self) -> u64 {
        self.object.encoded_bytes()
    }
    /// Whether the final object already existed and matched exactly.
    #[must_use]
    pub const fn reused(&self) -> bool {
        self.object.reused()
    }
    /// This call created a stage and performed no unlink. This reports the call's
    /// disposition, not continued physical presence or cleanup authority. False
    /// does not certify absence of stages left by earlier attempts.
    #[must_use]
    pub const fn staging_cleanup_deferred(&self) -> bool {
        self.staging_cleanup_deferred
    }
}
impl<I> fmt::Debug for ContextualPreparationArtifactReceipt<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextualPreparationArtifactReceipt")
            .field("encoded_bytes", &self.encoded_bytes())
            .field("reused", &self.reused())
            .field("staging_cleanup_deferred", &self.staging_cleanup_deferred())
            .finish_non_exhaustive()
    }
}

/// Exact opaque bytes observed under the caller context. The private legacy
/// value cannot escape through Debug; no source/recovery authority is implied.
pub struct ContextualPreparationArtifactRead<I> {
    object: ObjectRead<I>,
}
impl<I> ContextualPreparationArtifactRead<I> {
    /// Exact object bytes, without interpretation or source authorization.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.object.bytes()
    }
    /// Stable observed native identity.
    #[must_use]
    pub const fn identity(&self) -> &I {
        self.object.identity()
    }
    /// Exact encoded length.
    #[must_use]
    pub fn encoded_bytes(&self) -> u64 {
        self.object.encoded_bytes()
    }
    /// Consume the observation, returning its still-charged object bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.object.into_bytes()
    }
}
impl<I> fmt::Debug for ContextualPreparationArtifactRead<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ContextualPreparationArtifactRead(<opaque>)")
    }
}

/// Read one exact opaque preparation artifact under the original borrowed context.
///
/// All filesystem/platform attempts, short reads and logical object-buffer
/// capacity are charged. No durable write or current-path source read occurs.
pub fn read_legacy_preparation_artifact_with_context<
    P: ContextualLegacyPreparationArtifactPlatform<C>,
    C: LegacyPreparationArtifactContext,
>(
    platform: &P,
    context: &mut C,
    path: &Path,
    maximum_bytes: usize,
) -> ContextualPreparationArtifactResult<
    ContextualPreparationArtifactRead<P::Identity>,
    P::Error,
    C::Error,
> {
    let mut kernel = Kernel::new(platform, context, true);
    let result = kernel.read(path, maximum_bytes, None);
    kernel
        .finish(result)
        .map(|object| ContextualPreparationArtifactRead { object })
}

/// Publish and verify one exact opaque immutable object under the original
/// borrowed context. No stage is automatically unlinked, including on success.
///
/// Possible effects remain unresolved; this API supplies no replay/recovery
/// authority and does not certify continued physical presence of a stage.
pub fn publish_legacy_preparation_artifact_with_context<
    P: ContextualLegacyPreparationArtifactPlatform<C>,
    C: LegacyPreparationArtifactContext,
>(
    platform: &P,
    context: &mut C,
    directory: &Path,
    final_name: &str,
    temporary_name: &str,
    bytes: &[u8],
    maximum_bytes: usize,
) -> ContextualPreparationArtifactResult<
    ContextualPreparationArtifactReceipt<P::Identity>,
    P::Error,
    C::Error,
> {
    let mut kernel = Kernel::new(platform, context, true);
    let result = kernel.publish(directory, final_name, temporary_name, bytes, maximum_bytes);
    kernel.finish(result)
}

// Compatibility is private and usable only by the existing legacy entrypoints.
// It grants no contextual/native capability and never enters the new APIs.
struct LegacyContext;
impl LegacyPreparationArtifactContext for LegacyContext {
    type Error = Infallible;
    fn checkpoint(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
    fn charge(&mut self, _: LegacyPreparationArtifactWork) -> Result<(), Infallible> {
        Ok(())
    }
    fn effect_attempted(&mut self) {}
}

struct LegacyPlatform<'a, P>(&'a P);
impl<P: Platform> Platform for LegacyPlatform<'_, P> {
    type Identity = P::Identity;
    type Error = P::Error;
    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
        self.0.validate_directory(path)
    }
    fn verify_locator(&self, file: &File, path: &Path) -> Result<(), Self::Error> {
        self.0.verify_locator(file, path)
    }
    fn identity(&self, file: &File) -> Result<Self::Identity, Self::Error> {
        self.0.identity(file)
    }
    fn sync_directory(&self, path: &Path) -> Result<(), Self::Error> {
        self.0.sync_directory(path)
    }
}
impl<P: Platform> ContextualLegacyPreparationArtifactPlatform<LegacyContext>
    for LegacyPlatform<'_, P>
{
    fn validate_directory_contextual(
        &self,
        _: &mut LegacyContext,
        path: &Path,
    ) -> Result<(), LegacyPreparationArtifactPlatformFailure<P::Error, Infallible>> {
        self.0
            .validate_directory(path)
            .map_err(LegacyPreparationArtifactPlatformFailure::Platform)
    }
    fn verify_locator_contextual(
        &self,
        _: &mut LegacyContext,
        file: &File,
        path: &Path,
    ) -> Result<(), LegacyPreparationArtifactPlatformFailure<P::Error, Infallible>> {
        self.0
            .verify_locator(file, path)
            .map_err(LegacyPreparationArtifactPlatformFailure::Platform)
    }
    fn identity_contextual(
        &self,
        _: &mut LegacyContext,
        file: &File,
    ) -> Result<P::Identity, LegacyPreparationArtifactPlatformFailure<P::Error, Infallible>> {
        self.0
            .identity(file)
            .map_err(LegacyPreparationArtifactPlatformFailure::Platform)
    }
    fn sync_directory_contextual(
        &self,
        _: &mut LegacyContext,
        path: &Path,
    ) -> Result<(), LegacyPreparationArtifactPlatformFailure<P::Error, Infallible>> {
        self.0
            .sync_directory(path)
            .map_err(LegacyPreparationArtifactPlatformFailure::Platform)
    }
}

pub(super) fn legacy_read<P: Platform>(
    platform: &P,
    path: &Path,
    maximum_bytes: usize,
) -> Result<ObjectRead<P::Identity>, ObjectError<P::Error>> {
    let mut context = LegacyContext;
    let platform = LegacyPlatform(platform);
    let mut kernel = Kernel::new(&platform, &mut context, false);
    legacy_result(kernel.read(path, maximum_bytes, None))
}
pub(super) fn legacy_publish<P: Platform>(
    platform: &P,
    directory: &Path,
    final_name: &str,
    temporary_name: &str,
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<ObjectReceipt<P::Identity>, ObjectError<P::Error>> {
    let mut context = LegacyContext;
    let platform = LegacyPlatform(platform);
    let mut kernel = Kernel::new(&platform, &mut context, false);
    legacy_result(kernel.publish(directory, final_name, temporary_name, bytes, maximum_bytes))
        .map(|receipt| receipt.object)
}
fn legacy_result<T, E>(result: Result<T, Failure<E, Infallible>>) -> Result<T, ObjectError<E>> {
    result.map_err(|failure| match failure {
        Failure::Object(error) => error,
        Failure::Context(never) | Failure::AfterObject { context: never, .. } => match never {},
    })
}

type ResultFor<P, C, T> =
    Result<T, Failure<<P as Platform>::Error, <C as LegacyPreparationArtifactContext>::Error>>;
struct Kernel<
    'a,
    P: ContextualLegacyPreparationArtifactPlatform<C>,
    C: LegacyPreparationArtifactContext,
> {
    platform: &'a P,
    context: &'a mut C,
    retain_stage: bool,
    effect: ContextualPreparationArtifactEffect,
}
impl<'a, P: ContextualLegacyPreparationArtifactPlatform<C>, C: LegacyPreparationArtifactContext>
    Kernel<'a, P, C>
{
    const fn new(platform: &'a P, context: &'a mut C, retain_stage: bool) -> Self {
        Self {
            platform,
            context,
            retain_stage,
            effect: ContextualPreparationArtifactEffect::NotAttempted,
        }
    }
    fn finish<T>(
        &mut self,
        result: ResultFor<P, C, T>,
    ) -> Result<T, ContextualPreparationArtifactError<P::Error, C::Error>> {
        let checkpoint = self.context.checkpoint().map_err(Failure::Context);
        let result = match (result, checkpoint) {
            (Err(Failure::Object(object)), Err(Failure::Context(context))) => {
                Err(Failure::AfterObject { object, context })
            }
            (Err(error), _) | (_, Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        };
        result.map_err(|failure| ContextualPreparationArtifactError {
            failure,
            effect: self.effect,
        })
    }
    fn call<T>(
        &mut self,
        work: LegacyPreparationArtifactWork,
        effect: ContextualPreparationArtifactEffect,
        action: impl FnOnce() -> Result<T, ObjectError<P::Error>>,
    ) -> ResultFor<P, C, T> {
        self.context.checkpoint().map_err(Failure::Context)?;
        self.context.charge(work).map_err(Failure::Context)?;
        self.context.checkpoint().map_err(Failure::Context)?;
        if effect != ContextualPreparationArtifactEffect::NotAttempted {
            self.context.effect_attempted();
            self.effect = self.effect.max(effect);
        }
        let result = action().map_err(Failure::Object);
        let checkpoint = self.context.checkpoint().map_err(Failure::Context);
        match (result, checkpoint) {
            (Err(Failure::Object(object)), Err(Failure::Context(context))) => {
                Err(Failure::AfterObject { object, context })
            }
            (Err(error), _) | (_, Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
    fn platform_call<T>(
        &mut self,
        effect: ContextualPreparationArtifactEffect,
        map_error: fn(P::Error) -> ObjectError<P::Error>,
        action: impl FnOnce(
            &P,
            &mut C,
        )
            -> Result<T, LegacyPreparationArtifactPlatformFailure<P::Error, C::Error>>,
    ) -> ResultFor<P, C, T> {
        self.context.checkpoint().map_err(Failure::Context)?;
        self.context
            .charge(LegacyPreparationArtifactWork::Observe)
            .map_err(Failure::Context)?;
        self.context.checkpoint().map_err(Failure::Context)?;
        if effect != ContextualPreparationArtifactEffect::NotAttempted {
            self.context.effect_attempted();
            self.effect = self.effect.max(effect);
        }
        let result = action(self.platform, self.context).map_err(|error| match error {
            LegacyPreparationArtifactPlatformFailure::Platform(error) => {
                Failure::Object(map_error(error))
            }
            LegacyPreparationArtifactPlatformFailure::Context(error) => Failure::Context(error),
            LegacyPreparationArtifactPlatformFailure::AfterPlatform { platform, context } => {
                Failure::AfterObject {
                    object: map_error(platform),
                    context,
                }
            }
        });
        let checkpoint = self.context.checkpoint().map_err(Failure::Context);
        match (result, checkpoint) {
            (Err(Failure::Object(object)), Err(Failure::Context(context))) => {
                Err(Failure::AfterObject { object, context })
            }
            (Err(error), _) | (_, Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
    fn join(&mut self, parent: &Path, name: &str) -> ResultFor<P, C, std::path::PathBuf> {
        let bytes = parent
            .as_os_str()
            .as_encoded_bytes()
            .len()
            .checked_add(1)
            .and_then(|bytes| bytes.checked_add(name.len()))
            .ok_or(Failure::Object(ObjectError::SizeInvalid))?;
        self.call(
            LegacyPreparationArtifactWork::AllocateLocator(bytes),
            ContextualPreparationArtifactEffect::NotAttempted,
            || {
                let mut path = std::path::PathBuf::new();
                path.try_reserve_exact(bytes)
                    .map_err(|_| ObjectError::SizeInvalid)?;
                path.push(parent);
                path.push(name);
                Ok(path)
            },
        )
    }
    fn compare(&mut self, left: &[u8], right: &[u8]) -> ResultFor<P, C, bool> {
        self.call(
            LegacyPreparationArtifactWork::Compare(left.len().min(right.len())),
            ContextualPreparationArtifactEffect::NotAttempted,
            || Ok(left == right),
        )
    }
    fn read(
        &mut self,
        path: &Path,
        maximum_bytes: usize,
        expected: Option<&P::Identity>,
    ) -> ResultFor<P, C, ObjectRead<P::Identity>> {
        let parent = path
            .parent()
            .ok_or(Failure::Object(ObjectError::ParentMissing))?;
        self.platform_call(
            ContextualPreparationArtifactEffect::NotAttempted,
            ObjectError::Platform,
            |platform, context| platform.validate_directory_contextual(context, parent),
        )?;
        let before = self.call(
            LegacyPreparationArtifactWork::Observe,
            ContextualPreparationArtifactEffect::NotAttempted,
            || fs::symlink_metadata(path).map_err(ObjectError::ObjectInspect),
        )?;
        let size =
            usize::try_from(before.len()).map_err(|_| Failure::Object(ObjectError::SizeInvalid))?;
        if !regular(&before) || size > maximum_bytes {
            return Err(Failure::Object(if size > maximum_bytes {
                ObjectError::SizeInvalid
            } else {
                ObjectError::ObjectInvalid
            }));
        }
        let mut file = self.call(
            LegacyPreparationArtifactWork::Open,
            ContextualPreparationArtifactEffect::NotAttempted,
            || File::open(path).map_err(ObjectError::ObjectRead),
        )?;
        let opened = self.call(
            LegacyPreparationArtifactWork::Observe,
            ContextualPreparationArtifactEffect::NotAttempted,
            || file.metadata().map_err(ObjectError::ObjectRead),
        )?;
        if !regular(&opened)
            || opened.len() != before.len()
            || opened.modified().ok() != before.modified().ok()
        {
            return Err(Failure::Object(ObjectError::ReadbackMismatch));
        }
        self.platform_call(
            ContextualPreparationArtifactEffect::NotAttempted,
            ObjectError::Platform,
            |platform, context| platform.verify_locator_contextual(context, &file, path),
        )?;
        let identity = self.platform_call(
            ContextualPreparationArtifactEffect::NotAttempted,
            ObjectError::Platform,
            |platform, context| platform.identity_contextual(context, &file),
        )?;
        if expected.is_some_and(|value| value != &identity) {
            return Err(Failure::Object(ObjectError::IdentityChanged));
        }
        let bytes = self.read_data(&mut file, size)?;
        let used = bytes.len();
        let after = self.call(
            LegacyPreparationArtifactWork::Observe,
            ContextualPreparationArtifactEffect::NotAttempted,
            || file.metadata().map_err(ObjectError::ObjectRead),
        )?;
        if used != size
            || used > maximum_bytes
            || after.len() != before.len()
            || after.modified().ok() != before.modified().ok()
        {
            return Err(Failure::Object(ObjectError::ReadbackMismatch));
        }
        self.platform_call(
            ContextualPreparationArtifactEffect::NotAttempted,
            ObjectError::Platform,
            |platform, context| platform.verify_locator_contextual(context, &file, path),
        )?;
        let after_identity = self.platform_call(
            ContextualPreparationArtifactEffect::NotAttempted,
            ObjectError::Platform,
            |platform, context| platform.identity_contextual(context, &file),
        )?;
        if after_identity != identity {
            return Err(Failure::Object(ObjectError::IdentityChanged));
        }
        drop(file);
        self.context.checkpoint().map_err(Failure::Context)?;
        Ok(ObjectRead { bytes, identity })
    }
    fn read_data(&mut self, reader: &mut impl Read, size: usize) -> ResultFor<P, C, Vec<u8>> {
        let capacity = size
            .checked_add(1)
            .ok_or(Failure::Object(ObjectError::SizeInvalid))?;
        let mut bytes = self.call(
            LegacyPreparationArtifactWork::Allocate(capacity),
            ContextualPreparationArtifactEffect::NotAttempted,
            || {
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(capacity)
                    .map_err(|_| ObjectError::SizeInvalid)?;
                bytes.resize(capacity, 0);
                Ok(bytes)
            },
        )?;
        let mut used = 0;
        while used < capacity {
            let end = used.saturating_add(64 * 1024).min(capacity);
            let attempted = end - used;
            let result = self.call(
                LegacyPreparationArtifactWork::Read(attempted),
                ContextualPreparationArtifactEffect::NotAttempted,
                || {
                    reader
                        .read(&mut bytes[used..end])
                        .map_err(ObjectError::ObjectRead)
                },
            );
            let count = match result {
                Err(Failure::Object(ObjectError::ObjectRead(error)))
                    if error.kind() == io::ErrorKind::Interrupted =>
                {
                    continue;
                }
                other => other?,
            };
            if count == 0 {
                break;
            }
            if count > attempted {
                return Err(Failure::Object(ObjectError::ReadbackMismatch));
            }
            used += count;
        }
        bytes.truncate(used);
        if used != size {
            return Err(Failure::Object(ObjectError::ReadbackMismatch));
        }
        Ok(bytes)
    }
    fn write_data(&mut self, writer: &mut impl Write, bytes: &[u8]) -> ResultFor<P, C, ()> {
        let mut written = 0;
        while written < bytes.len() {
            let end = written.saturating_add(64 * 1024).min(bytes.len());
            let attempted = end - written;
            let result = self.call(
                LegacyPreparationArtifactWork::Write(attempted),
                ContextualPreparationArtifactEffect::StagingAttempted,
                || {
                    writer
                        .write(&bytes[written..end])
                        .map_err(ObjectError::Write)
                },
            );
            let count = match result {
                Err(Failure::Object(ObjectError::Write(error)))
                    if error.kind() == io::ErrorKind::Interrupted =>
                {
                    continue;
                }
                other => other?,
            };
            if count == 0 || count > attempted {
                let error = if count == 0 && !self.retain_stage {
                    io::Error::new(io::ErrorKind::WriteZero, "failed to write whole buffer")
                } else {
                    io::Error::from(io::ErrorKind::WriteZero)
                };
                return Err(Failure::Object(ObjectError::Write(error)));
            }
            written += count;
        }
        Ok(())
    }
    fn ensure_directory(&mut self, directory: &Path) -> ResultFor<P, C, ()> {
        let parent = directory
            .parent()
            .ok_or(Failure::Object(ObjectError::ParentMissing))?;
        self.platform_call(
            ContextualPreparationArtifactEffect::NotAttempted,
            ObjectError::Platform,
            |platform, context| platform.validate_directory_contextual(context, parent),
        )?;
        let observed = self.call(
            LegacyPreparationArtifactWork::Observe,
            ContextualPreparationArtifactEffect::NotAttempted,
            || fs::symlink_metadata(directory).map_err(ObjectError::ObjectInspect),
        );
        match observed {
            Ok(_) => {}
            Err(Failure::Object(ObjectError::ObjectInspect(error)))
                if error.kind() == io::ErrorKind::NotFound =>
            {
                let result = self.call(
                    LegacyPreparationArtifactWork::Effect,
                    ContextualPreparationArtifactEffect::DirectoryAttempted,
                    || fs::create_dir(directory).map_err(ObjectError::DirectoryCreate),
                );
                match result {
                    Ok(()) => {}
                    Err(Failure::Object(ObjectError::DirectoryCreate(error)))
                        if error.kind() == io::ErrorKind::AlreadyExists => {}
                    other => other?,
                }
                self.platform_call(
                    ContextualPreparationArtifactEffect::DirectoryAttempted,
                    ObjectError::Platform,
                    |platform, context| platform.sync_directory_contextual(context, parent),
                )?;
            }
            Err(failure) => return Err(failure),
        }
        self.platform_call(
            ContextualPreparationArtifactEffect::NotAttempted,
            ObjectError::Platform,
            |platform, context| platform.validate_directory_contextual(context, directory),
        )
    }
    fn publish(
        &mut self,
        directory: &Path,
        final_name: &str,
        temporary_name: &str,
        bytes: &[u8],
        maximum_bytes: usize,
    ) -> ResultFor<P, C, ContextualPreparationArtifactReceipt<P::Identity>> {
        if bytes.len() > maximum_bytes {
            return Err(Failure::Object(ObjectError::SizeInvalid));
        }
        if !valid_local_name(final_name)
            || !valid_local_name(temporary_name)
            || final_name == temporary_name
            || !temporary_name.starts_with('.')
            || !temporary_name.as_bytes().ends_with(b".tmp")
        {
            return Err(Failure::Object(ObjectError::LocalNameInvalid));
        }
        self.ensure_directory(directory)?;
        let final_path = self.join(directory, final_name)?;
        let temporary_path = self.join(directory, temporary_name)?;
        let observed = self.call(
            LegacyPreparationArtifactWork::Observe,
            ContextualPreparationArtifactEffect::NotAttempted,
            || fs::symlink_metadata(&final_path).map_err(ObjectError::ObjectInspect),
        );
        match observed {
            Ok(_) => {
                let existing = self.read(&final_path, maximum_bytes, None)?;
                if !self.compare(existing.bytes(), bytes)? {
                    return Err(Failure::Object(ObjectError::ImmutableConflict));
                }
                let encoded_bytes = existing.encoded_bytes();
                return Ok(ContextualPreparationArtifactReceipt {
                    object: ObjectReceipt {
                        identity: existing.identity,
                        encoded_bytes,
                        reused: true,
                    },
                    staging_cleanup_deferred: false,
                });
            }
            Err(Failure::Object(ObjectError::ObjectInspect(error)))
                if error.kind() == io::ErrorKind::NotFound => {}
            Err(failure) => return Err(failure),
        }
        // Creating a new stage is both an open and an effect. Charge the open
        // separately; the effect latch is immediately before actual dispatch.
        self.context.checkpoint().map_err(Failure::Context)?;
        self.context
            .charge(LegacyPreparationArtifactWork::Open)
            .map_err(Failure::Context)?;

        let mut temporary = self.call(
            LegacyPreparationArtifactWork::Effect,
            ContextualPreparationArtifactEffect::StagingAttempted,
            || {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(&temporary_path)
                    .map_err(ObjectError::Create)
            },
        )?;
        let metadata = self.call(
            LegacyPreparationArtifactWork::Observe,
            ContextualPreparationArtifactEffect::NotAttempted,
            || temporary.metadata().map_err(ObjectError::Create),
        )?;
        if !regular(&metadata) {
            return Err(Failure::Object(ObjectError::ObjectInvalid));
        }
        let temporary_identity = self.platform_call(
            ContextualPreparationArtifactEffect::NotAttempted,
            ObjectError::Platform,
            |platform, context| platform.identity_contextual(context, &temporary),
        )?;
        // Preserve the preparation adapter's legacy declaration/unwind order.
        // Contextual calls never construct a cleanup guard.
        let mut staging = if self.retain_stage {
            None
        } else {
            Some(TemporaryArtifactGuard {
                platform: self.platform,
                path: temporary_path.clone(),
                identity: temporary_identity.clone(),
                cleanup_armed: true,
            })
        };
        self.platform_call(
            ContextualPreparationArtifactEffect::NotAttempted,
            ObjectError::Platform,
            |platform, context| {
                platform.verify_locator_contextual(context, &temporary, &temporary_path)
            },
        )?;
        // Legacy write/sync failure closes the file before guard cleanup.
        let written = self.write_data(&mut temporary, bytes).and_then(|()| {
            self.call(
                LegacyPreparationArtifactWork::Effect,
                ContextualPreparationArtifactEffect::StagingAttempted,
                || temporary.sync_all().map_err(ObjectError::Write),
            )
        });
        if let Err(failure) = written {
            drop(temporary);
            return Err(failure);
        }
        let temporary = if self.retain_stage {
            Some(temporary)
        } else {
            // Preserve the old close-before-readback/cleanup boundary.
            drop(temporary);
            None
        };
        let staged = self.read(&temporary_path, maximum_bytes, Some(&temporary_identity))?;
        if !self.compare(staged.bytes(), bytes)? {
            return Err(Failure::Object(ObjectError::ReadbackMismatch));
        }
        drop(staged);
        if let Some(temporary) = &temporary {
            self.platform_call(
                ContextualPreparationArtifactEffect::NotAttempted,
                ObjectError::Platform,
                |platform, context| {
                    platform.verify_locator_contextual(context, temporary, &temporary_path)
                },
            )?;
        }
        let linked = self.call(
            LegacyPreparationArtifactWork::Effect,
            ContextualPreparationArtifactEffect::PublicationAttempted,
            || {
                fs::hard_link(&temporary_path, &final_path)
                    .map_err(ObjectError::PublishOutcomeUnknown)
            },
        );
        let reused = match linked {
            Ok(()) => false,
            Err(Failure::Object(ObjectError::PublishOutcomeUnknown(error)))
                if error.kind() == io::ErrorKind::AlreadyExists =>
            {
                true
            }
            Err(failure) => return Err(failure),
        };
        self.platform_call(
            ContextualPreparationArtifactEffect::PublicationAttempted,
            ObjectError::PublishPlatformOutcomeUnknown,
            |platform, context| platform.sync_directory_contextual(context, directory),
        )?;
        let final_object = self.read(&final_path, maximum_bytes, None)?;
        if !self.compare(final_object.bytes(), bytes)? {
            return Err(Failure::Object(ObjectError::ImmutableConflict));
        }
        if !reused && final_object.identity() != &temporary_identity {
            return Err(Failure::Object(ObjectError::IdentityChanged));
        }
        let encoded_bytes = final_object.encoded_bytes();
        let identity = final_object.identity;
        drop(temporary);
        if !self.retain_stage {
            staging
                .as_mut()
                .ok_or(Failure::Object(ObjectError::ObjectInvalid))?
                .cleanup()
                .map_err(Failure::Object)?;
            self.platform_call(
                ContextualPreparationArtifactEffect::PublicationAttempted,
                ObjectError::PublishPlatformOutcomeUnknown,
                |platform, context| platform.sync_directory_contextual(context, directory),
            )?;
        }
        drop(staging);
        self.context.checkpoint().map_err(Failure::Context)?;
        Ok(ContextualPreparationArtifactReceipt {
            object: ObjectReceipt {
                identity,
                encoded_bytes,
                reused,
            },
            staging_cleanup_deferred: self.retain_stage,
        })
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
