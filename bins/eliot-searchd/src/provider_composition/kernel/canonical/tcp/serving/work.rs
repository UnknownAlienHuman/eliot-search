//! Explicit native authority and cooperative recipe ownership; no allow-all host.

use search_access::{AccessCheckpoint, AccessError, RequestSecurityFence};
use search_contracts::{ProviderBodyV1, RecipeIdV1, RequestBody};
use search_provider_protocol::{AdmittedProviderRequest, BindingContext, MonotonicMillis, RequestGuard, TerminalKind};
use std::task::Poll;

use crate::access_composition::{AuthoritativeGrantPolicy, NativeSecurityDomain};
use super::{CanonicalServingError, CanonicalTcpConnection, monotonic_millis};

/// One owner-issued work turn with separate lifetime and scheduling bounds.
///
/// The hard deadline is the original request/cleanup deadline, tightened by
/// current authority. The earlier yield point only asks the task to return
/// Pending. Yielding does not expire a request, release resources or renew its
/// lifetime. Blocking backends must use an already-owned bounded worker and
/// poll its result; the cooperative quantum is not a preemption guarantee.
#[derive(Clone, Copy, Debug)]
pub struct CanonicalWorkBudget {
    started: MonotonicMillis,
    deadline: MonotonicMillis,
    yield_at: MonotonicMillis,
}

impl CanonicalWorkBudget {
    pub(super) fn new(
        deadline: MonotonicMillis,
        quantum_ms: u64,
    ) -> Result<Self, CanonicalServingError> {
        if !(1..=25).contains(&quantum_ms) {
            return Err(CanonicalServingError::InvalidConfiguration);
        }
        let started = monotonic_millis();
        let yield_at = started.get().checked_add(quantum_ms)
            .map(MonotonicMillis::new).unwrap_or(deadline).min(deadline);
        let budget = Self { started, deadline, yield_at };
        budget.check_at(started)?;
        Ok(budget)
    }

    /// Absolute lifetime ceiling for backend operations in this turn.
    /// This is not the scheduling quantum and cannot be extended by the task.
    #[must_use]
    pub const fn deadline(self) -> MonotonicMillis { self.deadline }

    /// Check the hard request/authority or cleanup lifetime before each backend
    /// operation and after returning from it. A completed work slice is not an
    /// error; use should_yield between bounded batches to return Pending.
    pub fn check(self) -> Result<(), CanonicalServingError> {
        self.check_at(monotonic_millis())
    }

    /// Whether the current turn should yield without starting another batch.
    /// Hard expiry or clock regression remains an error, even at the yield point.
    /// Pending must retain task state and all unfinished cleanup obligations.
    pub fn should_yield(self) -> Result<bool, CanonicalServingError> {
        let now = monotonic_millis();
        self.check_at(now)?;
        Ok(now >= self.yield_at)
    }

    // Tightening authority never widens either bound or changes the origin.
    // Only the serving owner can issue the next task slice; tasks cannot renew it.
    pub(super) fn bounded_by(self, valid_until: MonotonicMillis) -> Self {
        Self {
            started: self.started,
            deadline: self.deadline.min(valid_until),
            yield_at: self.yield_at.min(valid_until),
        }
    }

    fn check_at(self, now: MonotonicMillis) -> Result<(), CanonicalServingError> {
        if now < self.started || now >= self.deadline {
            return Err(CanonicalServingError::DeadlineExpired);
        }
        Ok(())
    }
}

/// Borrowed live authority held by the native host for one work/output turn.
/// None of these values may be reconstructed from client claims or capabilities.
pub struct CanonicalServingAuthority<'a> {
    /// Current active standalone policy borrowed from the held native lock.
    /// The standalone adapter requires Some; other peer roles may use None.
    /// Never supply a detached policy snapshot or reconstruct one from claims.
    pub standalone_policy: Option<&'a AuthoritativeGrantPolicy>,
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
    /// Keep preparation bounded. If it consumes the scheduling quantum, the
    /// owner retains the returned task and schedules its first poll next turn.
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
    /// For standalone serving, also borrow the actual active binding policy
    /// into standalone_policy; grant checks run inside this same lock scope.
    /// new_standalone supplies issuer verification through its adapter, without
    /// re-entering the policy source. Other roles retain their own grant checks.
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
    /// Check budget.should_yield() between batches and return Pending when it
    /// requests a yield. Do not translate that yield into DeadlineExpired.
    /// budget.check() enforces the separate hard request/authority lifetime.
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
    /// Use should_yield for cooperative Pending and check for hard expiry; a
    /// slice boundary alone must not lose or fail unfinished cleanup.
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
    /// Once entered, output is governed by the hard lifetime, not the yield
    /// point. Do not roll back a prepared delivery merely because its work
    /// quantum elapsed; actual output failure still closes the connection.
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
