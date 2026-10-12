//! Mandatory caller-context accounting for the frozen legacy DPAPI owner.

use core::convert::Infallible;
use core::fmt;
use std::collections::TryReserveError;

use super::super::LegacyRevisionDpapiError;
use super::super::model::clear_bytes;

/// Cumulative logical work reserved before the corresponding owner action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyRevisionDpapiWork {
    /// Reserve one possible native call, including error capture or release.
    NativeCall,
    /// Reserve requested Rust buffer capacity before allocation.
    Allocate(usize),
    /// Reserve copy/clear work before processing owned bytes.
    Process(usize),
}

/// The original invocation's mandatory context; no default callbacks or renewal.
///
/// Implementations retain the original absolute deadline, cancellation flag and
/// cumulative meter. Refusal must remain sticky. Native calls are cooperative:
/// the context cannot predict or preempt an OS-internal DPAPI allocation.
pub trait LegacyRevisionDpapiContext {
    /// The caller's typed original refusal.
    type Error;
    /// Poll the original context.
    fn checkpoint(&mut self) -> Result<(), Self::Error>;
    /// Debit cumulative requested work without refunds or fresh per-call budgets.
    fn charge(&mut self, work: LegacyRevisionDpapiWork) -> Result<(), Self::Error>;
    /// Record actual attempted bounded native clearing before it runs, even
    /// after sticky refusal. Use the SAME cumulative processing meter. Retain
    /// overflow/exhaustion evidence and the first refusal; never gate cleanup.
    /// This grants no allocation, normal work, refund or renewed allowance.
    fn account_native_clear(&mut self, actual_bytes: usize) -> Result<(), Self::Error>;
}

/// Typed contextual failure; native and post-refusal causes remain inspectable.
pub enum ContextualLegacyRevisionDpapiError<E> {
    /// Original caller refusal.
    Context(E),
    /// Preserved legacy native/validation failure.
    Native(LegacyRevisionDpapiError),
    /// Native failure and original post-call/refusal together.
    NativeAndContext {
        /// Exact legacy native cause.
        native: LegacyRevisionDpapiError,
        /// Original post-action refusal.
        context: E,
    },
    /// Rust output capacity could not be reserved.
    Allocation(TryReserveError),
    /// Output allocation failure and post-allocation refusal together.
    AllocationAndContext {
        /// Exact allocation failure.
        allocation: TryReserveError,
        /// Original post-allocation refusal.
        context: E,
    },
}
impl<E> ContextualLegacyRevisionDpapiError<E> {
    /// Closed redacted diagnostic, never raw context or native numeric text.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Context(_) => "DPAPI_LEGACY_REVISION_CONTEXT_REFUSED",
            Self::Native(e) => e.code(),
            Self::NativeAndContext { .. } => "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT",
            Self::Allocation(_) => "DPAPI_LEGACY_REVISION_ALLOCATION_FAILED",
            Self::AllocationAndContext { .. } => "DPAPI_LEGACY_REVISION_ALLOCATION_AND_CONTEXT",
        }
    }
}
impl<E> fmt::Debug for ContextualLegacyRevisionDpapiError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl<E> fmt::Display for ContextualLegacyRevisionDpapiError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl<E> std::error::Error for ContextualLegacyRevisionDpapiError<E> {}

/// Contextual DPAPI result, retaining both object and original caller causes.
pub type ContextualLegacyRevisionDpapiResult<T, E> =
    Result<T, ContextualLegacyRevisionDpapiError<E>>;

pub(super) enum KernelError<O, E> {
    Object(O),
    Context(E),
    AfterObject {
        object: O,
        context: E,
    },
    Allocation(TryReserveError),
    AfterAllocation {
        allocation: TryReserveError,
        context: E,
    },
}
impl<O, E> fmt::Debug for KernelError<O, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LegacyRevisionDpapiKernelError(<opaque>)")
    }
}
impl<O, E> KernelError<O, E> {
    pub(super) fn map_object<N>(self, map: impl FnOnce(O) -> N) -> KernelError<N, E> {
        match self {
            Self::Object(e) => KernelError::Object(map(e)),
            Self::Context(e) => KernelError::Context(e),
            Self::AfterObject { object, context } => KernelError::AfterObject {
                object: map(object),
                context,
            },
            Self::Allocation(e) => KernelError::Allocation(e),
            Self::AfterAllocation {
                allocation,
                context,
            } => KernelError::AfterAllocation {
                allocation,
                context,
            },
        }
    }
}
impl<E> From<KernelError<LegacyRevisionDpapiError, E>> for ContextualLegacyRevisionDpapiError<E> {
    fn from(e: KernelError<LegacyRevisionDpapiError, E>) -> Self {
        match e {
            KernelError::Object(e) => Self::Native(e),
            KernelError::Context(e) => Self::Context(e),
            KernelError::AfterObject { object, context } => Self::NativeAndContext {
                native: object,
                context,
            },
            KernelError::Allocation(e) => Self::Allocation(e),
            KernelError::AfterAllocation {
                allocation,
                context,
            } => Self::AllocationAndContext {
                allocation,
                context,
            },
        }
    }
}
pub(super) fn before<C: LegacyRevisionDpapiContext, O>(
    c: &mut C,
) -> Result<(), KernelError<O, C::Error>> {
    c.checkpoint().map_err(KernelError::Context)
}
pub(super) fn charge<C: LegacyRevisionDpapiContext, O>(
    c: &mut C,
    work: LegacyRevisionDpapiWork,
) -> Result<(), KernelError<O, C::Error>> {
    before(c)?;
    let debit = c.charge(work);
    let post = c.checkpoint();
    debit.and(post).map_err(KernelError::Context)
}
pub(super) fn after<C: LegacyRevisionDpapiContext, O, T>(
    c: &mut C,
    result: Result<T, KernelError<O, C::Error>>,
) -> Result<T, KernelError<O, C::Error>> {
    merge_context(result, c.checkpoint())
}
fn merge_context<O, E, T>(
    result: Result<T, KernelError<O, E>>,
    post: Result<(), E>,
) -> Result<T, KernelError<O, E>> {
    match (result, post) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(context)) => Err(KernelError::Context(context)),
        (Err(KernelError::Object(object)), Err(context)) => {
            Err(KernelError::AfterObject { object, context })
        }
        (Err(KernelError::Allocation(allocation)), Err(context)) => {
            Err(KernelError::AfterAllocation {
                allocation,
                context,
            })
        }
        (Err(first), _) => Err(first),
    }
}
pub(super) fn finish_native_clear<C: LegacyRevisionDpapiContext, O, T>(
    c: &mut C,
    actual_bytes: usize,
    result: Result<T, KernelError<O, C::Error>>,
    clear: impl FnOnce(),
) -> Result<T, KernelError<O, C::Error>> {
    // Do not go through before/charge: this mandatory debit must be recorded
    // even if cancellation, expiry or an earlier budget refusal is sticky.
    let accounted = c.account_native_clear(actual_bytes);
    clear();
    merge_context(result, accounted)
}
pub(super) fn stage<C: LegacyRevisionDpapiContext, O, T>(
    c: &mut C,
    action: impl FnOnce() -> Result<T, O>,
) -> Result<T, KernelError<O, C::Error>> {
    before(c)?;
    let result = action().map_err(KernelError::Object);
    after(c, result)
}
pub(super) fn reserve<C: LegacyRevisionDpapiContext, O>(
    c: &mut C,
    unprotecting: bool,
) -> Result<(), KernelError<O, C::Error>> {
    // Native clear length is unknown until return. Explicit finish accounts
    // its actual attempted bytes; refusal cannot veto already-owned cleanup.
    // Crypt32 + possible GetLastError + output LocalFree; unprotect also owns
    // a possible description LocalFree. Every possible attempt is reserved.
    for _ in 0..if unprotecting { 4 } else { 3 } {
        charge(c, LegacyRevisionDpapiWork::NativeCall)?;
    }
    Ok(())
}
pub(super) struct CopiedBytes(Option<Vec<u8>>);
impl CopiedBytes {
    pub(super) fn take(mut self) -> Vec<u8> {
        self.0.take().expect("copied bytes remain owned")
    }
}
impl Drop for CopiedBytes {
    fn drop(&mut self) {
        if let Some(bytes) = self.0.as_mut() {
            clear_bytes(bytes);
        }
    }
}
pub(super) fn copy<C: LegacyRevisionDpapiContext, O>(
    c: &mut C,
    bytes: &[u8],
    contextual: bool,
) -> Result<CopiedBytes, KernelError<O, C::Error>> {
    charge(c, LegacyRevisionDpapiWork::Allocate(bytes.len()))?;
    // Copy and possible clear-on-refusal are reserved before constructing Vec.
    charge(
        c,
        LegacyRevisionDpapiWork::Process(bytes.len().saturating_mul(2)),
    )?;
    before(c)?;
    let result = if contextual {
        let mut output = Vec::new();
        match output.try_reserve_exact(bytes.len()) {
            Ok(()) => {
                output.extend_from_slice(bytes);
                Ok(CopiedBytes(Some(output)))
            }
            Err(error) => Err(KernelError::Allocation(error)),
        }
    } else {
        // Preserve the legacy infallible-allocation behavior and exact bytes.
        Ok(CopiedBytes(Some(bytes.to_vec())))
    };
    after(c, result)
}
pub(super) struct LegacyContext;
impl LegacyRevisionDpapiContext for LegacyContext {
    type Error = Infallible;
    fn checkpoint(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
    fn charge(&mut self, _: LegacyRevisionDpapiWork) -> Result<(), Infallible> {
        Ok(())
    }
    fn account_native_clear(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
}
pub(super) fn legacy<O, T>(result: Result<T, KernelError<O, Infallible>>) -> Result<T, O> {
    match result {
        Ok(value) => Ok(value),
        Err(KernelError::Object(e)) => Err(e),
        Err(
            KernelError::Context(e)
            | KernelError::AfterObject { context: e, .. }
            | KernelError::AfterAllocation { context: e, .. },
        ) => match e {},
        Err(KernelError::Allocation(_)) => {
            unreachable!("legacy copies use the original infallible allocator")
        }
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
