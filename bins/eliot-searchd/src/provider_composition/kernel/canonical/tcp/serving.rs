//! Bounded single-owner serving loop over the actual canonical TCP transport.

mod work;
pub use work::{CanonicalRecipeHost, CanonicalRecipeTask, CanonicalServingAuthority, CanonicalWorkBudget, CanonicalWorkOutput};

use std::collections::VecDeque;
use std::task::Poll;
use std::time::Duration;

use search_access::{AccessCheckpoint, AccessError};
use search_contracts::{CancelledBody, ExpandHandleTarget, ProviderBodyV1, RecipeBodyV1};
use search_provider_protocol::{AdmittedProviderRequest, MonotonicMillis, ProtocolError, TerminalKind};

use crate::access_composition::GrantUseError;
use super::{CanonicalTcpConnection, CanonicalTcpError, monotonic_millis};

/// Serving failure. Backend diagnostics are static codes, never request content.
#[derive(Debug)]
pub enum CanonicalServingError {
    /// Canonical transport failed; no alternate route is attempted.
    Transport(CanonicalTcpError),
    /// The live grant/scope/domain check denied this turn.
    Access(AccessError),
    /// Standalone grant refusal before the current task/output callback ran.
    /// Earlier work slices may already have produced effects.
    GrantRefused(GrantUseError),
    /// Grant time/currentness failed after the current callback returned.
    /// Already emitted bytes and possible effects cannot be retracted.
    GrantAfterOperation(GrantUseError),
    /// Original request or cooperative work budget expired.
    DeadlineExpired,
    /// Invalid owner configuration or a connection with unowned live requests.
    InvalidConfiguration,
    /// Worker completion did not match actual output, or output was repeated.
    InvalidCompletion,
    /// A backend failed; only a registered static diagnostic code is allowed.
    Backend(&'static str),
    /// Possible external effects cannot be represented as an ordinary P00 error.
    OutcomeUnknown,
    /// Cleanup exceeded its one fixed deadline; tasks are still retained.
    CleanupExpired,
    /// Normal polling is forbidden after closure.
    Closed,
}

/// Runtime limits supplied by daemon configuration, not client request fields.
#[derive(Clone, Copy, Debug)]
pub struct CanonicalServingLimits {
    /// Maximum queued/running/cleaning tasks; cannot exceed transport work slots.
    pub tasks: usize,
    /// Finite server cap for a new incoming request, including its frame read.
    pub request_deadline_ms: u64,
    /// Cooperative task quantum and maximum input wait; at most 25 ms.
    pub quantum_ms: u64,
    /// One cleanup deadline after closure, never refreshed by retrying cleanup.
    pub cleanup_deadline_ms: u64,
}

struct Slot<T> {
    request: AdmittedProviderRequest,
    task: Option<T>,
}

/// Owns the real transport and a bounded round-robin work queue. A cancelled
/// task occupies capacity until cleanup and its own terminal are complete.
/// There is no additional protocol replay/sequence registry or reader thread.
///
/// Errors/unwinding close the socket and signal tasks before returning. Cleanup
/// remains owned here and can be polled after failure; it is not falsely marked
/// complete or dropped to make room. Native bootstrap and a real host are required.
pub struct CanonicalServingOwner<H: CanonicalRecipeHost> {
    transport: CanonicalTcpConnection,
    queue: VecDeque<Slot<H::Task>>,
    // A popped task remains owned during callbacks, including panic unwinding.
    active: Option<Slot<H::Task>>,
    // Drop task resources before their backing host/store owners.
    host: H,
    limits: CanonicalServingLimits,
    closed: bool,
    cleanup_deadline: Option<MonotonicMillis>,
}

impl<H: CanonicalRecipeHost> CanonicalServingOwner<H> {
    /// Take a freshly negotiated transport and explicit native recipe host.
    /// No startup listener, grant or default backend is created by this method.
    pub fn new(
        transport: CanonicalTcpConnection,
        host: H,
        limits: CanonicalServingLimits,
    ) -> Result<Self, CanonicalServingError> {
        let state = transport.state.as_ref().ok_or(CanonicalServingError::Closed)?;
        if !state.connection.session.is_active() || state.connection.session.in_flight_len() != 0
            || limits.tasks == 0 || limits.tasks > state.connection.limits.max_in_flight_requests
            || limits.request_deadline_ms == 0 || limits.cleanup_deadline_ms == 0
            || !(1..=25).contains(&limits.quantum_ms)
        {
            return Err(CanonicalServingError::InvalidConfiguration);
        }
        let mut queue = VecDeque::new();
        queue.try_reserve_exact(limits.tasks)
            .map_err(|_| CanonicalServingError::Transport(CanonicalTcpError::Allocation))?;
        Ok(Self { transport, host, queue, active: None, limits, closed: false, cleanup_deadline: None })
    }

    /// Service at most one input record and one task slice, in that order.
    /// Even sustained incoming controls cannot skip the queued task turn.
    /// Input Pending does not prevent execution of already accepted work.
    /// Quantum bounds input waiting and cooperative work, not blocking output;
    /// actual frame writes keep their original request/authority deadlines.
    pub fn tick(&mut self) -> Result<(), CanonicalServingError> {
        let mut turn = Turn { owner: self, complete: false };
        let result = turn.owner.tick_inner();
        turn.complete = result.is_ok();
        result
    }

    fn tick_inner(&mut self) -> Result<(), CanonicalServingError> {
        if self.closed { return Err(CanonicalServingError::Closed); }
        let mut wait_ms = self.limits.quantum_ms;
        let now = monotonic_millis();
        for slot in &self.queue {
            let remaining = request_deadline(&slot.request)?.get().checked_sub(now.get())
                .filter(|left| *left > 0).ok_or(CanonicalServingError::DeadlineExpired)?;
            wait_ms = wait_ms.min(remaining);
        }
        match self.transport.poll_receive(self.limits.request_deadline_ms, Duration::from_millis(wait_ms))
            .map_err(CanonicalServingError::Transport)?
        {
            Poll::Ready(Some(request)) => {
                if self.queue.len() >= self.limits.tasks {
                    // Cancellation may free a protocol slot before its worker
                    // stops. Never translate that into unbounded work capacity.
                    return Err(CanonicalServingError::Transport(CanonicalTcpError::Protocol(
                        ProtocolError::ResourceExhausted,
                    )));
                }
                self.queue.push_back(Slot { request, task: None });
            }
            Poll::Pending | Poll::Ready(None) => {}
        }
        self.active = self.queue.pop_front();
        if self.active.is_none() { return Ok(()); }
        let complete = self.service_active()?;
        if let Some(slot) = self.active.take() {
            if !complete { self.queue.push_back(slot); }
        }
        Ok(())
    }

    fn service_active(&mut self) -> Result<bool, CanonicalServingError> {
        let slot = self.active.as_mut().ok_or(CanonicalServingError::InvalidCompletion)?;
        let deadline = request_deadline(&slot.request)?;
        let budget = work_budget(deadline, self.limits.quantum_ms)?;
        if slot.request.guard().is_cancelled() {
            let cleaned = match slot.task.as_mut() {
                Some(task) => {
                    task.abort();
                    task.poll_cancel(budget)?
                }
                None => Poll::Ready(()), // No preparation or source work ever started.
            };
            budget.check()?;
            if cleaned.is_pending() { return Ok(false); }
            let target_request_id = *slot.request.guard().request_id();
            self.transport.deliver_event(&mut slot.request, ProviderBodyV1::Cancelled(CancelledBody {
                target_request_id, terminal: true,
            }), Some(TerminalKind::Cancelled)).map_err(CanonicalServingError::Transport)?;
            return Ok(true);
        }
        let session = self.transport.session().ok_or(CanonicalServingError::Closed)?;
        session.revalidate_request_guard(slot.request.guard(), monotonic_millis())
            .map_err(|error| CanonicalServingError::Transport(CanonicalTcpError::Protocol(error)))?;
        let binding = session.binding_context();
        if slot.task.is_none() {
            slot.task = Some(self.host.prepare(&binding, &slot.request, budget)?);
            budget.check()?;
        }
        let task = slot.task.as_mut().ok_or(CanonicalServingError::InvalidCompletion)?;
        let transport = &mut self.transport;
        self.host.with_current_authority(&binding, &mut slot.request, task, |authority, request, task| {
            let budget = CanonicalWorkBudget { deadline: budget.deadline.min(authority.valid_until) };
            budget.check()?;
            let domain = authority.domain;
            let fence = authority.fence;
            let checkpoint = match &request.body().recipe_request.body {
                RecipeBodyV1::ExpandHandle(expansion) => match &expansion.handle {
                    ExpandHandleTarget::Source(_) => AccessCheckpoint::HandleExpansion,
                    ExpandHandleTarget::Continuation(_) => AccessCheckpoint::ContinuationExpansion,
                },
                _ => AccessCheckpoint::BeforeLegDispatch,
            };
            domain.with_live_checkpoint(fence, checkpoint, |_| {
                let mut output = CanonicalWorkOutput { transport, request, authority, budget,
                    emitted_terminal: None, failed: false };
                let result = task.poll(&mut output, budget)?;
                if output.failed { return Err(CanonicalServingError::InvalidCompletion); }
                let after = monotonic_millis();
                if output.request.guard().is_expired(after) || after >= output.authority.valid_until {
                    return Err(CanonicalServingError::DeadlineExpired);
                }
                if output.emitted_terminal.is_none() { budget.check()?; }
                match (result, output.emitted_terminal) {
                    (Poll::Ready(()), Some(true)) => Ok(true),
                    (Poll::Pending, None | Some(false)) => Ok(false),
                    _ => Err(CanonicalServingError::InvalidCompletion),
                }
            }).map_err(CanonicalServingError::Access)?
        })
    }

    /// Run on the calling thread until explicitly stopped. Failure closes the
    /// connection but retains tasks; call poll_cleanup to finish their disposal.
    /// A normal stop drains cleanup under one finite deadline before returning.
    pub fn run(&mut self, mut stop: impl FnMut() -> bool) -> Result<(), CanonicalServingError> {
        let mut run = CloseOnReturn(self);
        let owner = &mut run.0;
        while !stop() { owner.tick()?; }
        owner.close();
        loop {
            if owner.poll_cleanup()?.is_ready() { return Ok(()); }
            std::thread::sleep(Duration::from_millis(owner.limits.quantum_ms));
        }
    }

    /// Close first (signalling every session guard), then signal worker resources.
    /// This is idempotent; no retry extends the cleanup deadline or clears tasks.
    pub fn close(&mut self) {
        if self.closed { return; }
        self.closed = true;
        self.transport.close();
        self.cleanup_deadline = monotonic_millis().get().checked_add(self.limits.cleanup_deadline_ms)
            .map(MonotonicMillis::new);
        for slot in self.active.iter_mut().chain(self.queue.iter_mut()) {
            if let Some(task) = slot.task.as_mut() { task.abort(); }
        }
    }

    /// Clean at most one task after closure. Ready means every retained task is
    /// cleaned; Pending/Err retain the task, including when the deadline expires.
    /// No source authority is needed to dispose resources, and no data is sent.
    pub fn poll_cleanup(&mut self) -> Result<Poll<()>, CanonicalServingError> {
        if !self.closed { return Err(CanonicalServingError::InvalidConfiguration); }
        if self.active.is_none() { self.active = self.queue.pop_front(); }
        let Some(slot) = self.active.as_mut() else { return Ok(Poll::Ready(())); };
        let deadline = self.cleanup_deadline.filter(|end| monotonic_millis() < *end)
            .ok_or(CanonicalServingError::CleanupExpired)?;
        let budget = work_budget(deadline, self.limits.quantum_ms)?;
        let cleaned = match slot.task.as_mut() {
            Some(task) => task.poll_cancel(budget)?,
            None => Poll::Ready(()),
        };
        budget.check()?;
        if let Some(slot) = self.active.take() {
            if cleaned.is_pending() { self.queue.push_back(slot); }
        }
        Ok(if self.queue.is_empty() { Poll::Ready(()) } else { Poll::Pending })
    }

    /// Queued, running and cleanup-pending task count; not a success metric.
    #[must_use]
    pub fn retained_tasks(&self) -> usize { self.queue.len() + usize::from(self.active.is_some()) }
}

fn request_deadline(request: &AdmittedProviderRequest) -> Result<MonotonicMillis, CanonicalServingError> {
    request.guard().deadline().ok_or(CanonicalServingError::InvalidConfiguration)
}

fn work_budget(deadline: MonotonicMillis, quantum: u64) -> Result<CanonicalWorkBudget, CanonicalServingError> {
    let now = monotonic_millis();
    if now >= deadline { return Err(CanonicalServingError::DeadlineExpired); }
    let slice = now.get().checked_add(quantum).map(MonotonicMillis::new).unwrap_or(deadline);
    Ok(CanonicalWorkBudget { deadline: deadline.min(slice) })
}

impl<H: CanonicalRecipeHost> Drop for CanonicalServingOwner<H> {
    fn drop(&mut self) { self.close(); }
}

struct Turn<'a, H: CanonicalRecipeHost> { owner: &'a mut CanonicalServingOwner<H>, complete: bool }
impl<H: CanonicalRecipeHost> Drop for Turn<'_, H> {
    fn drop(&mut self) { if !self.complete { self.owner.close(); } }
}

struct CloseOnReturn<'a, H: CanonicalRecipeHost>(&'a mut CanonicalServingOwner<H>);
impl<H: CanonicalRecipeHost> Drop for CloseOnReturn<'_, H> {
    fn drop(&mut self) { self.0.close(); }
}
