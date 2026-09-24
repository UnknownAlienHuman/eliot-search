//! One complete CLI exchange, with independent directional/transport sequences.
//!
//! Only an exact terminal followed by its exact transport acknowledgement can
//! leave the connection reusable. Ordinary fully framed refusals are outcomes;
//! protocol/I/O errors, unknown outcomes and unwinding poison the session.

use std::io::{self, Write};
use std::net::Shutdown;
use std::time::{SystemTime, UNIX_EPOCH};

use search_provider_protocol::negotiation::negotiate_hello;
use search_provider_protocol::request::{
    ControlCommand, RequestStatus, decode_response_json, encode_envelope_json,
    envelope_transcript, seal_envelope,
};

use super::{
    MAX_RESPONSE_LINES, ProofDigest, ProtocolRange,
    ProviderSession, REQUEST_ID_DOMAIN, RequestId, UnsignedRequest, hex_decode, hex_encode,
    keyed, provider_range, render_op_line, response, verify_sealed_response,
};

use super::transport::ExchangeBudget;

mod cancellation;

use cancellation::DeadlineCancellation;

// One diagnostic line including its normalized LF, matching the child limit.
const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024;

/// Holds only the diagnostic payload, not a whole streaming search result.
/// Release requires a matching outcome and complete exchange. This validates
/// event/outcome correspondence, not a general JSON schema or a new payload MAC.
struct DiagnosticReply {
    expected_event: &'static str,
    payload: Option<(bool, String)>,
}

impl DiagnosticReply {
    fn new(expected_event: &'static str) -> Self {
        Self {
            expected_event,
            payload: None,
        }
    }

    fn retain(&mut self, line: String) -> Result<(), String> {
        if self.payload.is_some() {
            return Err("REMOTE_RESPONSE_MISMATCH".to_owned());
        }
        if line.len() >= MAX_DIAGNOSTIC_BYTES {
            return Err("REMOTE_FRAME_TOO_LARGE".to_owned());
        }
        let event = response::event(&line)?;
        if event != self.expected_event && event != "error" {
            return Err("REMOTE_RESPONSE_MISMATCH".to_owned());
        }
        self.payload = Some((event == "error", line));
        Ok(())
    }

    fn publish(self, status: RequestStatus) -> Result<(), String> {
        if let Some(line) = self.into_output(status)? {
            print_payload(&line)?;
        }
        Ok(())
    }

    fn into_output(self, status: RequestStatus) -> Result<Option<String>, String> {
        match (status, self.payload) {
            // Pre-dispatch cancellation has no payload; late cancellation may
            // have a fully received one. Neither is a diagnostic result to print.
            (RequestStatus::Cancelled | RequestStatus::OutcomeUnknown, _)
            | (RequestStatus::Failed, None) => Ok(None),
            (RequestStatus::Ok | RequestStatus::Partial, Some((false, line)))
            | (RequestStatus::Failed, Some((true, line))) => Ok(Some(line)),
            _ => Err("REMOTE_RESPONSE_MISMATCH".to_owned()),
        }
    }
}

struct CompletedExchange {
    outcome: Result<(), String>,
    shutdown: bool,
    cancel_acknowledged: bool,
}

struct Exchange<'a> {
    session: &'a mut ProviderSession,
    completed: bool,
}

impl Drop for Exchange<'_> {
    fn drop(&mut self) {
        if !self.completed { self.session.close(); }
    }
}

impl ProviderSession {
    /// Permanently closes transport and removes the reusable key.
    pub(super) fn close(&mut self) {
        self.active = false;
        let _ = self.stream.shutdown(Shutdown::Both);
        self.key = None;
    }

    /// Installs negotiated state only after the complete hello exchange, using
    /// the original connection-opening budget rather than restarting its clock.
    pub(super) fn hello(&mut self, budget: &mut ExchangeBudget) -> Result<(), String> {
        let mut exchange = Exchange { session: self, completed: false };
        let sequence = exchange.session.endpoint_sequence;
        let next = next_sequence(sequence)?;
        budget.send(&mut exchange.session.stream, "op\thello\t1.0-1.0")?;
        response::started(&budget.recv(exchange.session)?, sequence)?;
        let line = budget.recv(exchange.session)?;
        let (version, nonce) = response::hello(&line)?;
        negotiate_hello(
            provider_range(),
            ProtocolRange::new(version, version).map_err(|_| response::invalid())?,
        ).map_err(|_| "REMOTE_VERSION_MISMATCH".to_owned())?;
        let complete = budget.recv(exchange.session)?;
        if response::complete(&complete, sequence)?.is_some() {
            return Err("REMOTE_HELLO_INVALID".to_owned());
        }
        budget.check()?;
        exchange.session.nonce = nonce;
        exchange.session.version = version;
        exchange.session.endpoint_sequence = next;
        exchange.session.active = true;
        exchange.completed = true;
        Ok(())
    }

    /// Sends one bounded request and consumes its complete response exchange.
    ///
    /// A fully framed refusal preserves the connection and its counters. Missing,
    /// duplicate, foreign or contradictory control frames, exhausted budgets,
    /// output errors and unwind close it; there is no implicit reconnect/replay.
    /// Expired health/version reads send one same-connection cancellation and
    /// allow five seconds only to drain/verify both exchanges. Complete cleanup
    /// preserves the session but still returns the work deadline failure. No
    /// diagnostic is printed after timeout; shutdown never enters this path.
    /// Diagnostics are held until outcome/acknowledgement verification. Other
    /// payload already written to stdout cannot be retracted on a later failure.
    pub fn invoke(&mut self, request: &UnsignedRequest) -> Result<(), String> {
        if !self.active { return Err("REMOTE_SESSION_CLOSED".to_owned()); }
        let mut exchange = Exchange { session: self, completed: false };
        let next = next_sequence(exchange.session.endpoint_sequence)?;
        let mut budget = ExchangeBudget::new()?;
        let completed = match request {
            UnsignedRequest::Health => exchange.session.invoke_envelope(ControlCommand::Health, &mut budget),
            UnsignedRequest::Version => exchange.session.invoke_envelope(ControlCommand::Version, &mut budget),
            UnsignedRequest::Shutdown => exchange.session.invoke_envelope(ControlCommand::Shutdown, &mut budget),
            _ => exchange.session.invoke_op(request, &mut budget),
        }?;
        budget.check()?;
        exchange.session.endpoint_sequence = if completed.cancel_acknowledged {
            next_sequence(next)?
        } else {
            next
        };
        if completed.shutdown { exchange.session.close(); }
        exchange.completed = true;
        completed.outcome
    }

    fn mint_id(&mut self) -> Result<RequestId, String> {
        self.request_counter = next_sequence(self.request_counter)?;
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_| "REMOTE_CLOCK_INVALID".to_owned())?.as_nanos();
        let mut input = Vec::with_capacity(REQUEST_ID_DOMAIN.len() + 40);
        input.extend_from_slice(REQUEST_ID_DOMAIN);
        input.extend_from_slice(self.nonce.as_bytes());
        input.extend_from_slice(&self.request_counter.to_le_bytes());
        input.extend_from_slice(&nanos.to_le_bytes());
        let digest = self.key.as_ref().ok_or_else(|| "REMOTE_SESSION_CLOSED".to_owned())?
            .with_bytes(|key| blake3::keyed_hash(key, &input));
        let mut raw = [0_u8; 16];
        raw.copy_from_slice(&digest.as_bytes()[..16]);
        if raw.iter().all(|byte| *byte == 0) { return Err("REMOTE_REQUEST_ID_INVALID".to_owned()); }
        Ok(RequestId::from_bytes(raw))
    }

    fn invoke_envelope(
        &mut self,
        command: ControlCommand,
        budget: &mut ExchangeBudget,
    ) -> Result<CompletedExchange, String> {
        let mut diagnostic = match command {
            ControlCommand::Health => Some(DiagnosticReply::new("health")),
            ControlCommand::Version => Some(DiagnosticReply::new("version")),
            ControlCommand::Shutdown => None,
        };
        let client_sequence = next_sequence(self.envelope_sequence)?;
        let provider_sequence = next_sequence(self.provider_sequence)?;
        let request_id = self.mint_id()?;
        let digest = ProofDigest::from_bytes(*blake3::hash(&[]).as_bytes());
        let stub = seal_envelope(
            self.version, self.nonce, request_id, command, digest, ProofDigest::from_bytes([0; 32]),
        );
        let proof = self.key.as_ref().ok_or_else(|| "REMOTE_SESSION_CLOSED".to_owned())?
            .with_bytes(|key| keyed(key, &envelope_transcript(&stub)));
        let sealed = seal_envelope(self.version, self.nonce, request_id, command, digest, proof);
        let frame = encode_envelope_json(&sealed);
        let length = u32::try_from(frame.len()).map_err(|_| "REMOTE_REQUEST_TOO_LARGE".to_owned())?;
        let mut framed = length.to_le_bytes().to_vec();
        framed.extend_from_slice(&frame);
        budget.send(&mut self.stream, &format!("envelope\t{client_sequence}\t{}", hex_encode(&framed)))?;
        // Cancellation can be sent only after the entire original request was
        // written. A partial request write still closes the stream without retry.
        let mut cancellation = DeadlineCancellation::new(command, request_id);
        response::started(&cancellation.recv(self, budget)?, self.endpoint_sequence)?;
        for _ in 0..MAX_RESPONSE_LINES {
            let line = cancellation.recv(self, budget)?;
            if let Some(hex) = line.strip_prefix("response\t") {
                let frame = hex_decode(hex).ok_or_else(response::invalid)?;
                let prefix: [u8; 4] = frame.get(..4).ok_or_else(response::invalid)?
                    .try_into().map_err(|_| response::invalid())?;
                let declared = usize::try_from(u32::from_le_bytes(prefix)).map_err(|_| response::invalid())?;
                if declared.checked_add(4) != Some(frame.len()) { return Err(response::invalid()); }
                let decoded = decode_response_json(&frame[4..], provider_range())
                    .map_err(|_| response::invalid())?;
                if decoded.request_id() != &request_id || decoded.version() != self.version {
                    return Err("REMOTE_RESPONSE_MISMATCH".to_owned());
                }
                self.key.as_ref().ok_or_else(|| "REMOTE_SESSION_CLOSED".to_owned())?
                    .with_bytes(|key| verify_sealed_response(key, &decoded, &self.nonce, provider_sequence))?;
                // A verified outcome-unknown terminal intentionally precedes
                // connection abort, not a normal endpoint acknowledgement.
                // Preserve its meaning instead of replacing it with an EOF error.
                if decoded.status() == RequestStatus::OutcomeUnknown {
                    return Err("REMOTE_OUTCOME_UNKNOWN".to_owned());
                }
                let complete = cancellation.recv(self, budget)?;
                if response::complete(&complete, self.endpoint_sequence)?.is_some() {
                    return Err("REMOTE_RESPONSE_MISMATCH".to_owned());
                }
                // Even discarded late diagnostics must agree with the terminal;
                // malformed output cannot buy a reusable connection via timeout.
                let output = diagnostic.take()
                    .map(|diagnostic| diagnostic.into_output(decoded.status()))
                    .transpose()?
                    .flatten();
                let cancel_acknowledged = cancellation.finish(self, budget)?;
                budget.check()?;
                if let (false, Some(line)) = (cancel_acknowledged, output) {
                    print_payload(&line)?;
                }
                self.envelope_sequence = client_sequence;
                self.provider_sequence = provider_sequence;
                return Ok(CompletedExchange {
                    outcome: if cancel_acknowledged {
                        Err("REMOTE_DEADLINE_EXPIRED".to_owned())
                    } else {
                        match decoded.status() {
                            RequestStatus::Ok => Ok(()),
                            RequestStatus::Partial => Err("REMOTE_PARTIAL_RESULT".to_owned()),
                            RequestStatus::Cancelled => Err("REMOTE_CANCELLED".to_owned()),
                            RequestStatus::Failed => Err("REMOTE_REQUEST_FAILED".to_owned()),
                            RequestStatus::OutcomeUnknown => Err("REMOTE_OUTCOME_UNKNOWN".to_owned()),
                        }
                    },
                    shutdown: command == ControlCommand::Shutdown && decoded.status() == RequestStatus::Ok,
                    cancel_acknowledged,
                });
            }
            match response::event(&line)? {
                "provider_error" => {
                    let reason = response::provider_error(&line)?;
                    let complete = budget.recv(self)?;
                    if response::complete(&complete, self.endpoint_sequence)? != Some(reason) {
                        return Err("REMOTE_RESPONSE_MISMATCH".to_owned());
                    }
                    // Admission may have refused before advancing its sequence.
                    // Do not guess a reusable client/provider sequence here.
                    return Err(reason.to_owned());
                }
                name if control_event(name) => return Err(response::invalid()),
                _ => {
                    if let Some(diagnostic) = diagnostic.as_mut() {
                        diagnostic.retain(line)?;
                    } else {
                        print_payload(&line)?;
                    }
                }
            }
        }
        Err("REMOTE_RESPONSE_LINE_LIMIT_EXCEEDED".to_owned())
    }

    fn invoke_op(
        &mut self,
        request: &UnsignedRequest,
        budget: &mut ExchangeBudget,
    ) -> Result<CompletedExchange, String> {
        let expected = match request {
            UnsignedRequest::Status => "status",
            UnsignedRequest::Cancel { .. } => "cancel",
            UnsignedRequest::Query { .. } => "query",
            UnsignedRequest::Ingest { .. } => "ingest",
            UnsignedRequest::Expand { .. } => "expand",
            _ => return Err("REMOTE_REQUEST_INVALID".to_owned()),
        };
        let mut diagnostic = matches!(request, UnsignedRequest::Status)
            .then_some(DiagnosticReply::new("provider_status"));
        let line = render_op_line(request).ok_or_else(|| "REMOTE_REQUEST_INVALID".to_owned())?;
        budget.send(&mut self.stream, &line)?;
        response::started(&budget.recv(self)?, self.endpoint_sequence)?;
        for _ in 0..MAX_RESPONSE_LINES {
            let line = budget.recv(self)?;
            match response::event(&line)? {
                "provider_op" => {
                    let reply = response::operation(&line, expected)?;
                    // Once a terminal arrives, only its exact acknowledgement
                    // may follow. Duplicate outcomes or trailing payload fail.
                    let complete = budget.recv(self)?;
                    reply.acknowledge(response::complete(&complete, self.endpoint_sequence)?)?;
                    let outcome = reply.result();
                    budget.check()?;
                    if let Some(diagnostic) = diagnostic.take() {
                        // The closed status-operation grammar permits only
                        // success or refusal; cancellation is a separate op.
                        diagnostic.publish(if outcome.is_ok() {
                            RequestStatus::Ok
                        } else {
                            RequestStatus::Failed
                        })?;
                    }
                    print_payload(&line)?;
                    return Ok(CompletedExchange { outcome, shutdown: false, cancel_acknowledged: false });
                }
                "provider_error" => {
                    let reason = response::provider_error(&line)?;
                    let complete = budget.recv(self)?;
                    if response::complete(&complete, self.endpoint_sequence)? != Some(reason) {
                        return Err("REMOTE_RESPONSE_MISMATCH".to_owned());
                    }
                    return Ok(CompletedExchange { outcome: Err(reason.to_owned()), shutdown: false, cancel_acknowledged: false });
                }
                name if control_event(name) => return Err(response::invalid()),
                _ => {
                    if let Some(diagnostic) = diagnostic.as_mut() {
                        diagnostic.retain(line)?;
                    } else {
                        print_payload(&line)?;
                    }
                }
            }
        }
        Err("REMOTE_RESPONSE_LINE_LIMIT_EXCEEDED".to_owned())
    }
}

fn control_event(name: &str) -> bool {
    matches!(name, "request_started" | "request_complete" | "provider_hello" | "provider_op" | "authenticated")
}

fn next_sequence(previous: u64) -> Result<u64, String> {
    previous.checked_add(1).ok_or_else(|| "REMOTE_SEQUENCE_EXHAUSTED".to_owned())
}

fn print_payload(line: &str) -> Result<(), String> {
    writeln!(io::stdout().lock(), "{line}").map_err(|_| "REMOTE_OUTPUT_ERROR".to_owned())
}
