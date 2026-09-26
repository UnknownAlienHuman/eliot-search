//! Connection-wide monotonic drain signal shared with admitted request guards.

use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Internal owner shared by one bound session, its external drain handle and
/// every admitted request cancellation probe.
#[derive(Clone)]
pub(crate) struct SessionDrainState {
    requested: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
}

impl SessionDrainState {
    pub(crate) fn new() -> Self {
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn handle(&self) -> SessionDrainHandle {
        SessionDrainHandle { state: self.clone() }
    }

    pub(crate) fn request(&self) -> bool {
        !self.requested.swap(true, Ordering::AcqRel)
    }

    pub(crate) fn mark_closed(&self) {
        self.requested.store(true, Ordering::Release);
        self.closed.store(true, Ordering::Release);
    }

    pub(crate) fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub(crate) fn same_signal(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.requested, &other.requested)
            && Arc::ptr_eq(&self.closed, &other.closed)
    }
}

impl fmt::Debug for SessionDrainState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionDrainState")
            .field("requested", &self.is_requested())
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}

/// Read-only external lifecycle capability for one exact authenticated session.
///
/// Cloning shares the same monotonic signal. Requesting drain immediately denies
/// new work and makes every cancellation probe issued by this session observable
/// as cancelled. The mutable session/socket owner remains responsible for terminal
/// delivery and deterministic close. This handle carries no key, grant, source
/// authority, request identity or transport descriptor.
#[derive(Clone)]
pub struct SessionDrainHandle {
    state: SessionDrainState,
}

impl SessionDrainHandle {
    /// Requests fail-closed drain. Returns `true` only for the first request.
    ///
    /// This operation is idempotent and cannot be reset. It does not claim that
    /// the socket is already closed or that possible external effects rolled back.
    pub fn request_drain(&self) -> bool {
        self.state.request()
    }

    /// Whether any owner has requested drain for this exact session.
    #[must_use]
    pub fn is_drain_requested(&self) -> bool {
        self.state.is_requested()
    }

    /// Whether the canonical mutable session owner completed teardown.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state.is_closed()
    }
}

impl fmt::Debug for SessionDrainHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionDrainHandle")
            .field("requested", &self.is_drain_requested())
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}
