//! Exclusive terminal preparation: output succeeds before completion is recorded.

use core::fmt;

use search_provider_protocol::SessionState;

use super::{ProviderRouter, ProtocolError, RequestId, RequestStatus, SequenceTracker, TerminalKind};

/// One terminal response reserved against the exact oldest pending request.
///
/// The exclusive router borrow prevents cancellation, another admission or a
/// competing terminal while the response is encoded and delivered. Dropping the
/// preparation, including output error or unwind, closes the session and clears
/// all in-flight guards. It never permits retry on a possibly partial stream.
#[must_use = "deliver the terminal or the connection will be closed"]
pub struct PreparedProviderTerminal<'a> {
    router: &'a mut ProviderRouter,
    request_id: RequestId,
    status: RequestStatus,
    sequence: u64,
    next_sequence: SequenceTracker,
    committed: bool,
}

impl fmt::Debug for PreparedProviderTerminal<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedProviderTerminal")
            .field("status", &self.status)
            .field("sequence", &self.sequence)
            .finish_non_exhaustive()
    }
}

impl ProviderRouter {
    /// Validates one terminal without consuming its slot or response sequence.
    ///
    /// Validation failures leave pending work unchanged. The transport must
    /// abort rather than append another frame if a child response has already
    /// started. Once preparation succeeds, its drop guard enforces fail-stop.
    pub fn prepare_terminal(
        &mut self,
        request_id: &RequestId,
        terminal: TerminalKind,
    ) -> Result<PreparedProviderTerminal<'_>, ProtocolError> {
        if self.completed.contains(request_id) {
            return Err(ProtocolError::DuplicateTerminal);
        }
        match self.session.state() {
            SessionState::Active | SessionState::Draining => {}
            SessionState::Closed => return Err(ProtocolError::SessionClosed),
            SessionState::Quarantined => return Err(ProtocolError::Quarantined),
            _ => return Err(ProtocolError::AuthenticationRequired),
        }
        match self.pending.front() {
            Some(oldest) if oldest == request_id => {}
            Some(_) if self.pending.contains(request_id) => {
                return Err(ProtocolError::SequenceGap);
            }
            _ => return Err(ProtocolError::InvalidSessionTransition),
        }
        if !self.inflight.contains(request_id) {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        // RequestGuard is only small lifecycle/progress metadata, not a result
        // window. Validate a copy so a later sequence error cannot finish the
        // retained guard or release its slot without a terminal response.
        let mut guard = self.guards.get(request_id)
            .ok_or(ProtocolError::InvalidSessionTransition)?.clone();
        guard.finish(terminal, self.limits)?;
        let mut next_sequence = self.provider_sequence;
        let sequence = next_sequence.next_expected()
            .ok_or(ProtocolError::SequenceExhausted)?;
        SequenceTracker::require_accepted(next_sequence.observe(sequence))?;
        Ok(PreparedProviderTerminal {
            router: self,
            request_id: *request_id,
            status: RequestStatus::from_terminal(terminal),
            sequence,
            next_sequence,
            committed: false,
        })
    }
}

impl PreparedProviderTerminal<'_> {
    /// Encodes, writes and flushes through the supplied real transport callback.
    ///
    /// Only callback success records completion. Any returned error or unwind
    /// disconnects the router; the callback must not claim success after partial
    /// output, and its caller must close the transport on failure. This is local
    /// write completion, not proof that the remote client consumed the bytes.
    pub fn deliver<E>(
        self,
        output: impl FnOnce(RequestStatus, u64) -> Result<(), E>,
    ) -> Result<(RequestStatus, u64), E> {
        output(self.status, self.sequence)?;
        Ok(self.commit())
    }

    pub(super) fn commit(mut self) -> (RequestStatus, u64) {
        // Every recoverable check was completed before the output callback,
        // under this same exclusive borrow. No fallible work follows delivery.
        self.router.completed.insert(self.request_id);
        self.router.provider_sequence = self.next_sequence;
        self.router.pending.pop_front();
        self.router.inflight.remove(&self.request_id);
        self.router.guards.remove(&self.request_id);
        self.committed = true;
        (self.status, self.sequence)
    }
}

impl Drop for PreparedProviderTerminal<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.router.disconnect();
        }
    }
}
