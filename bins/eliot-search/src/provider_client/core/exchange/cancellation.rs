//! Same-connection cleanup for an expired diagnostic request, never a retry.

use super::{
    ControlCommand, ExchangeBudget, ProviderSession, RequestId, UnsignedRequest,
    hex_encode, next_sequence, render_op_line, response,
};

pub(super) struct DeadlineCancellation {
    target: RequestId,
    allowed: bool,
    sent: bool,
}

impl DeadlineCancellation {
    pub(super) fn new(command: ControlCommand, target: RequestId) -> Self {
        Self {
            target,
            // The server can drain only these exact admitted diagnostics without
            // killing its owner. Never infer safety from a generic read label.
            allowed: matches!(command, ControlCommand::Health | ControlCommand::Version),
            sent: false,
        }
    }

    pub(super) fn recv(
        &mut self,
        session: &mut ProviderSession,
        budget: &mut ExchangeBudget,
    ) -> Result<String, String> {
        match budget.recv(session) {
            Err(error) if error == "REMOTE_DEADLINE_EXPIRED" && self.allowed && !self.sent => {
                // Reserve the counter for both the original exchange and cancel
                // before sending. The cancel uses this connection's current ID,
                // never a reconnect, a new request identity or a repeated query.
                next_sequence(next_sequence(session.endpoint_sequence)?)?;
                let cancel = UnsignedRequest::cancel(&hex_encode(self.target.as_bytes()))?;
                let line = render_op_line(&cancel)
                    .ok_or_else(|| "REMOTE_REQUEST_INVALID".to_owned())?;
                budget.begin_cancel_cleanup()?;
                self.sent = true;
                budget.send(&mut session.stream, &line)?;
                // The budget retains a partly consumed response, including a LF
                // consumed just before expiry. Do not start a second reader or
                // interpret the old frame's suffix as a new response.
                budget.recv(session)
            }
            result => result,
        }
    }

    /// Called only after the target's authenticated terminal and exact endpoint
    /// acknowledgement. Cancel remains queued at the server and owns a separate
    /// next endpoint sequence, but consumes no signed-envelope sequence.
    pub(super) fn finish(
        &self,
        session: &mut ProviderSession,
        budget: &mut ExchangeBudget,
    ) -> Result<bool, String> {
        if !self.sent {
            return Ok(false);
        }
        let sequence = next_sequence(session.endpoint_sequence)?;
        response::started(&budget.recv(session)?, sequence)?;
        let line = budget.recv(session)?;
        let reply = response::operation(&line, "cancel")?;
        let complete = budget.recv(session)?;
        reply.acknowledge(response::complete(&complete, sequence)?)?;
        match reply.result() {
            Ok(()) => {}
            // The target often completed its cancelled terminal before the
            // queued cancel was dispatched. This is a valid idempotent outcome,
            // not proof that its result met the original work deadline.
            Err(reason) if reason == "PROVIDER_CANCEL_UNKNOWN_OR_TERMINAL" => {}
            Err(reason) => return Err(reason),
        }
        budget.check()?;
        Ok(true)
    }
}
