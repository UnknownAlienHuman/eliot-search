//! Explicit native authority and cooperative recipe ownership; no allow-all host.

use search_access::{AccessCheckpoint, AccessError, RequestSecurityFence};
use search_contracts::{ProviderBodyV1, RecipeIdV1, RequestBody};
use search_provider_protocol::{AdmittedProviderRequest, BindingContext, MonotonicMillis, RequestGuard, TerminalKind};
use std::task::Poll;

use crate::access_composition::NativeSecurityDomain;
use super::{CanonicalServingError, CanonicalTcpConnection, monotonic_millis};

/// One cooperative work slice, in the daemon's monotonic clock.
/// Blocking backends must use an already-owned bounded worker and poll its result.
#[derive(Clone, Copy, Debug)]
pub struct CanonicalWorkBudget {
    /// Earlier of the original request/authority deadline and this work slice.
    pub deadline: MonotonicMillis,
}

impl CanonicalWorkBudget {
    /// Check before each backend dispatch/read and between bounded CPU batches.
    pub fn check(self) -> Result<(), CanonicalServingError> {
        if monotonic_millis() >= self.deadline {
            return Err(CanonicalServingError::DeadlineExpired);
        }
        Ok(())
    }
}

/// Borrowed live authority held by the native host for one work/output turn.
/// None of these values may be reconstructed from client claims or capabilities.
pub struct CanonicalServingAuthority<'a> {
    /// Restored native domain under the actual serving/mutation lock.
    pub domain: &'a NativeSecurityDomain,
    /// Complete authoritative influence population, not only displayed hits.
    pub fence: &'a RequestSecurityFence,
    /// Conservative monotonic expiry of the current grant/binding authorization.
    pub valid_until: MonotonicMillis,
    /// Current scope/disclosure/byte-limit validation of the actual output body.
    /// This callback must not write output or release the host's authority lock.
    pub validate_output: &'a mut dyn FnMut(&RequestBody, &ProviderBodyV1) -> Result<(), AccessError>,
}

/// Native grant/plan owner injected into the serving loop. There is deliberately
/// no default implementation and no token-file or legacy-catalog fallback.
pub trait CanonicalRecipeHost {
    /// Task resources remain owned through cancellation and output completion.
    type Task: CanonicalRecipeTask;

    /// Prepare bounded local task state, without source/provider execution or
    /// publishing handles. Partial preparation must clean itself up on error.
    /// The actual source work starts only inside `with_current_authority`.
    fn prepare(
        &mut self,
        binding: &BindingContext,
        request: &AdmittedProviderRequest,
        budget: CanonicalWorkBudget,
    ) -> Result<Self::Task, CanonicalServingError>;

    /// Revalidate server-issued grant, binding, scope and plan; keep the native
    /// authority/domain lock held across `operation`, including socket output.
    /// Supply the complete influence fence and a conservative expiry, never a
    /// cached permit. The operation performs the concrete native checkpoints.
    ///
    /// The generic return and FnOnce callback cannot be replaced with a canned
    /// success. An error after invoking it still closes the serving connection.
    fn with_current_authority<R>(
        &mut self,
        binding: &BindingContext,
        request: &mut AdmittedProviderRequest,
        task: &mut Self::Task,
        operation: impl FnOnce(
            CanonicalServingAuthority<'_>,
            &mut AdmittedProviderRequest,
            &mut Self::Task,
        ) -> Result<R, CanonicalServingError>,
    ) -> Result<R, CanonicalServingError>;
}

/// One cooperative executor with mandatory cancellation ownership.
/// Implementations call existing recipe/source owners, not a second query engine.
pub trait CanonicalRecipeTask {
    /// Perform at most one bounded work slice and emit at most one event.
    /// Ready requires a successfully emitted terminal; Pending permits either
    /// no output or one progress event. Use `output.emit` inside existing
    /// prepared-handle/continuation delivery callbacks so their rollback and
    /// commit lifetimes span the actual write. Never dispatch detached work.
    fn poll(
        &mut self,
        output: &mut CanonicalWorkOutput<'_, '_>,
        budget: CanonicalWorkBudget,
    ) -> Result<Poll<()>, CanonicalServingError>;

    /// Finish cancellation/resource cleanup without reading or emitting source
    /// content. Ready proves this task stopped and all request-local cleanup
    /// completed; it is not a claim that external mutations rolled back.
    /// Failure/Pending must preserve ownership and retryable cleanup obligations.
    /// Repeated polls after completed cleanup must remain safe and idempotent.
    fn poll_cancel(&mut self, budget: CanonicalWorkBudget) -> Result<Poll<()>, CanonicalServingError>;

    /// Idempotent, infallible, non-panicking, nonblocking emergency signal. Do not discard
    /// unfinished cleanup. Task Drop must retain durable obligations with their
    /// real owner and must not detach workers; fallible cleanup uses poll_cancel.
    fn abort(&mut self);
}

/// Single-event output capability valid only inside the live host/domain borrow.
/// Executors never receive the socket or mutable protocol session.
pub struct CanonicalWorkOutput<'io, 'authority> {
    pub(super) transport: &'io mut CanonicalTcpConnection,
    pub(super) request: &'io mut AdmittedProviderRequest,
    pub(super) authority: CanonicalServingAuthority<'authority>,
    pub(super) budget: CanonicalWorkBudget,
    pub(super) emitted_terminal: Option<bool>,
    pub(super) failed: bool,
}

impl CanonicalWorkOutput<'_, '_> {
    /// Exact admitted request, still subject to the host's live authorization.
    #[must_use]
    pub fn request(&self) -> &RequestBody { self.request.body() }

    /// Original guard: its cancellation and deadline are never renewed here.
    #[must_use]
    pub fn guard(&self) -> &RequestGuard { self.request.guard() }

    /// Sequence to put in the next progress/result event.
    #[must_use]
    pub fn next_event_sequence(&self) -> Option<u64> { self.request.next_event_sequence() }

    /// Validate actual output and send it through the canonical transport.
    /// The native checkpoint and the host's lock span the complete frame/MAC
    /// write. Each write uses min(request deadline, current authority expiry).
    /// A swallowed error or second emission still poisons the owner turn.
    pub fn emit(
        &mut self,
        body: ProviderBodyV1,
        terminal: Option<TerminalKind>,
    ) -> Result<(), CanonicalServingError> {
        if self.failed || self.emitted_terminal.is_some() {
            self.failed = true;
            return Err(CanonicalServingError::InvalidCompletion);
        }
        self.failed = true;
        self.budget.check()?;
        (self.authority.validate_output)(self.request.body(), &body)
            .map_err(CanonicalServingError::Access)?;
        self.budget.check()?;
        let domain = self.authority.domain;
        let fence = self.authority.fence;
        let checkpoint = match &body {
            ProviderBodyV1::Result(value) if value.result.recipe_id() == RecipeIdV1::ExecuteExactScan => {
                AccessCheckpoint::BeforeExactEmission
            }
            _ => AccessCheckpoint::BeforeResultEmission,
        };
        domain.with_live_checkpoint(fence, checkpoint, |_| {
            self.transport.deliver_event_before(
                self.request, body, terminal, Some(self.authority.valid_until),
            ).map_err(CanonicalServingError::Transport)
        }).map_err(CanonicalServingError::Access)??;
        self.emitted_terminal = Some(terminal.is_some());
        self.failed = false;
        Ok(())
    }
}
