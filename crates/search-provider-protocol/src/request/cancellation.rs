//! Monotonic request cancellation linked to its owning connection drain.

use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use search_ports::{CancellationProbe, PackageOpaque};

use crate::binding::SessionDrainState;

use super::RequestGuard;

/// Read-only cancellation capability for an admitted request.
///
/// Obtain it from `RequestGuard::cancellation` and pass it to an existing
/// `OperationContext`. Cloning shares both the request-local signal and, for
/// a session-admitted guard, its connection drain signal. It does not copy the
/// cancellation state, grant authority or mutable connection lifecycle.
///
/// Cancellation is monotonic and cooperative: observing it does not interrupt
/// a blocked system call or prove that an external mutation had no effects.
/// Deadlines, output barriers and unknown-outcome recovery remain their owners'
/// duties. A connection drain cancels all guards from that exact session without
/// making another connection's equal request ID related.
#[derive(Clone)]
pub struct RequestCancellation {
    cancelled: Arc<AtomicBool>,
    session: Option<SessionDrainState>,
}

impl RequestCancellation {
    pub(super) fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            session: None,
        }
    }

    /// Observes request-local cancellation or drain of its owning session.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
            || self.session.as_ref().is_some_and(|state| {
                state.is_requested() || state.is_closed()
            })
    }

    /// Same original request/session signal; this grants no admission authority.
    #[must_use]
    pub fn same_request(&self, other: &Self) -> bool {
        self.same_signal(other)
    }

    // Identity, not the observed boolean: equal IDs/timestamps in another
    // connection do not make a constructed guard an admitted one.
    pub(crate) fn same_signal(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cancelled, &other.cancelled)
            && match (&self.session, &other.session) {
                (Some(left), Some(right)) => left.same_signal(right),
                (None, None) => true,
                _ => false,
            }
    }

    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl RequestGuard {
    /// Links an otherwise fully validated guard to its exact owning session.
    /// Called once before the guard is inserted or cloned into public observers.
    pub(crate) fn attach_session_drain(&mut self, state: &SessionDrainState) {
        debug_assert!(self.cancelled.session.is_none());
        self.cancelled.session = Some(state.clone());
    }
}

impl fmt::Debug for RequestCancellation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestCancellation")
            .field("cancelled", &self.is_cancelled())
            .field("session_linked", &self.session.is_some())
            .finish_non_exhaustive()
    }
}

impl PackageOpaque for RequestCancellation {
    fn owner_package(&self) -> &'static str {
        "search-provider-protocol"
    }
}

impl CancellationProbe for RequestCancellation {
    fn is_cancelled(&self) -> bool {
        Self::is_cancelled(self)
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use crate::{DEFAULT_PROTOCOL_LIMITS, MonotonicMillis};
    use search_contracts::RequestId;

    #[test]
    fn provider_read_context_identity_accepts_clone_and_refuses_equal_id_foreign_request() {
        let guard = || RequestGuard::new(RequestId::from_bytes([0x61; 16]), 1,
            MonotonicMillis::new(1), Some(100), DEFAULT_PROTOCOL_LIMITS).unwrap();
        let original = guard().cancellation();
        assert!(original.same_request(&original.clone()));
        assert!(!original.same_request(&guard().cancellation()));
    }
}
