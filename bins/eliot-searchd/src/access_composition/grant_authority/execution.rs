//! Admission-bound observation around the existing grant policy/issuer owners.

use std::cell::{Cell, RefCell};

use search_contracts::SearchReadGrantClaims;
use search_provider_protocol::{
    BindingContext, BoundSession, MonotonicMillis, ProtocolError, RequestGuard,
    StandaloneGrantRequestV1,
};

use super::{GrantAuthorityError, SessionBoundGrantAuthority, StandaloneGrantPolicySource};
use super::super::grant::{
    AuthoritativeGrantPolicy, GrantIssuerError, StandaloneGrantIssuer,
    StandaloneGrantMaterial, StandaloneGrantTemplate,
};

/// Closed reason why an admitted grant request can no longer run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantRequestInterruption {
    /// The exact admitted request's shared signal was cancelled.
    Cancelled,
    /// The original request deadline expired or its clock regressed.
    DeadlineExpired,
    /// The supplied guard is not a live admission in this session.
    NotAdmitted,
}

/// Grant execution outcome, preserving whether issuance may have had effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantExecutionError {
    /// Existing policy/intersection/issuer failure, unchanged.
    Authority(GrantAuthorityError),
    /// Interrupted before the underlying issuer was ever called.
    Interrupted(GrantRequestInterruption),
    /// Interrupted after entering the issuer; exact recovery is still required.
    OutcomeUnknown(GrantRequestInterruption),
}

impl GrantExecutionError {
    /// Stable reason; external effects cannot be relabelled clean cancellation.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Authority(error) => error.code(),
            Self::Interrupted(GrantRequestInterruption::Cancelled) => {
                "DAEMON_GRANT_REQUEST_CANCELLED"
            }
            Self::Interrupted(GrantRequestInterruption::DeadlineExpired) => {
                "DAEMON_GRANT_REQUEST_DEADLINE_EXPIRED"
            }
            Self::Interrupted(GrantRequestInterruption::NotAdmitted) => {
                "DAEMON_GRANT_REQUEST_NOT_ADMITTED"
            }
            Self::OutcomeUnknown(_) => "DAEMON_GRANT_REQUEST_OUTCOME_UNKNOWN",
        }
    }
}

impl<P, I> SessionBoundGrantAuthority<P, I>
where
    P: StandaloneGrantPolicySource,
    I: StandaloneGrantIssuer,
{
    /// Mints through the original policy/intersection path for an exact live guard.
    ///
    /// Checks bracket both policy reads and the actual issuer call, not merely
    /// the beginning/end of a potentially blocking composite operation. The clock
    /// uses admission's monotonic origin. No new policy, issuer, operation ID or
    /// receipt owner is created: adapters below borrow the existing owners.
    ///
    /// # Errors
    ///
    /// Refuses foreign/reconstructed guards, cancellation, expiry and regressed
    /// time. Interruption after the issuer was entered returns `OutcomeUnknown`
    /// and never exposes late claims or retries the issuer. Checks are cooperative;
    /// they cannot preempt a blocked call or prove rollback of possible effects.
    pub fn mint_admitted_protocol_request(
        &mut self,
        session: &BoundSession,
        request: &RequestGuard,
        body: StandaloneGrantRequestV1,
        now: impl FnMut() -> MonotonicMillis,
    ) -> Result<SearchReadGrantClaims, GrantExecutionError> {
        let check = ExecutionCheck {
            session,
            request,
            clock: RefCell::new(now),
            last_observed: Cell::new(request.admitted_at()),
            issuer_entered: Cell::new(false),
            interrupted: Cell::new(None),
        };
        check.check().map_err(GrantExecutionError::Interrupted)?;
        let mut observed = SessionBoundGrantAuthority::new(
            ObservedPolicy { inner: &mut self.policy_source, check: &check },
            ObservedIssuer { inner: &mut self.issuer, check: &check },
        );
        let result = observed.mint_protocol_request(session, *request.request_id(), body);
        // Also cover pure mapping/intersection/receipt validation between the
        // callbacks. An earlier interruption is sticky and cannot be cleared.
        let _ = check.check();
        if let Some(reason) = check.interrupted.get() {
            return Err(if check.issuer_entered.get() {
                GrantExecutionError::OutcomeUnknown(reason)
            } else {
                GrantExecutionError::Interrupted(reason)
            });
        }
        result.map_err(GrantExecutionError::Authority)
    }
}

struct ExecutionCheck<'a, F> {
    session: &'a BoundSession,
    request: &'a RequestGuard,
    clock: RefCell<F>,
    last_observed: Cell<MonotonicMillis>,
    issuer_entered: Cell<bool>,
    interrupted: Cell<Option<GrantRequestInterruption>>,
}

impl<F: FnMut() -> MonotonicMillis> ExecutionCheck<'_, F> {
    fn check(&self) -> Result<(), GrantRequestInterruption> {
        if let Some(reason) = self.interrupted.get() {
            return Err(reason);
        }
        let now = (self.clock.borrow_mut())();
        let result = if now < self.last_observed.get() {
            Err(GrantRequestInterruption::DeadlineExpired)
        } else {
            self.last_observed.set(now);
            self.session.revalidate_request_guard(self.request, now).map_err(|error| {
                if self.request.is_cancelled() {
                    GrantRequestInterruption::Cancelled
                } else if error == ProtocolError::DeadlineExpired {
                    GrantRequestInterruption::DeadlineExpired
                } else {
                    GrantRequestInterruption::NotAdmitted
                }
            })
        };
        if let Err(reason) = result {
            self.interrupted.set(Some(reason));
        }
        result
    }
}

struct ObservedPolicy<'a, 'b, P, F> {
    inner: &'a mut P,
    check: &'a ExecutionCheck<'b, F>,
}

impl<P: StandaloneGrantPolicySource, F: FnMut() -> MonotonicMillis>
    StandaloneGrantPolicySource for ObservedPolicy<'_, '_, P, F>
{
    type Error = ();

    fn snapshot(&mut self, binding: &BindingContext) -> Result<AuthoritativeGrantPolicy, ()> {
        self.check.check().map_err(|_| ())?;
        let result = self.inner.snapshot(binding).map_err(|_| ());
        self.check.check().map_err(|_| ())?;
        result
    }
}

struct ObservedIssuer<'a, 'b, I, F> {
    inner: &'a mut I,
    check: &'a ExecutionCheck<'b, F>,
}

impl<I: StandaloneGrantIssuer, F: FnMut() -> MonotonicMillis>
    StandaloneGrantIssuer for ObservedIssuer<'_, '_, I, F>
{
    fn issue(
        &mut self,
        template: &StandaloneGrantTemplate,
    ) -> Result<StandaloneGrantMaterial, GrantIssuerError> {
        self.check.check().map_err(|_| GrantIssuerError::Unavailable)?;
        // Arm before the only underlying mutation call. Errors/unwind do not
        // prove that an external implementation performed no durable writes.
        self.check.issuer_entered.set(true);
        let result = self.inner.issue(template);
        self.check.check().map_err(|_| GrantIssuerError::OutcomeUnknown)?;
        result
    }
}
