//! A single monotonic cancellation signal shared by one request's observers.

use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use search_ports::{CancellationProbe, PackageOpaque};

/// Read-only cancellation capability for an admitted request.
///
/// Obtain it from `RequestGuard::cancellation` and pass it to an existing
/// `OperationContext`. Cloning shares the same signal; it does not copy the
/// cancellation state, grant authority or connection lifecycle. The signal
/// cannot be reset and a newly admitted request gets a distinct allocation,
/// even if another connection reuses the same request identifier.
///
/// Cancellation is cooperative: observing it does not interrupt a blocked
/// system call or prove that an external mutation had no effects. Deadlines,
/// output barriers and unknown-outcome recovery remain their owners' duties.
#[derive(Clone)]
pub struct RequestCancellation {
    cancelled: Arc<AtomicBool>,
}

impl RequestCancellation {
    pub(super) fn new() -> Self {
        Self { cancelled: Arc::new(AtomicBool::new(false)) }
    }

    /// Observes cancellation recorded by any guard for this exact request.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl fmt::Debug for RequestCancellation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestCancellation")
            .field("cancelled", &self.is_cancelled())
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
        RequestCancellation::is_cancelled(self)
    }
}
