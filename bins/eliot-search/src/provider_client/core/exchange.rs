//! One complete CLI exchange, with independent directional/transport sequences.
//!
//! Only an exact terminal followed by its exact transport acknowledgement can
//! leave the connection reusable. Ordinary fully framed refusals are outcomes;
//! protocol/I/O errors, unknown outcomes and unwinding poison the session.

use std::io::{self, BufRead, Write};
use std::net::Shutdown;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use search_provider_protocol::negotiation::negotiate_hello;
use search_provider_protocol::request::{
    ControlCommand, RequestStatus, decode_response_json, encode_envelope_json,
    envelope_transcript, seal_envelope,
};

use super::{
    IO_TIMEOUT, MAX_PROVIDER_LINE_BYTES, MAX_RESPONSE_LINES, ProofDigest, ProtocolRange,
    ProviderSession, REQUEST_ID_DOMAIN, RequestId, UnsignedRequest, hex_decode, hex_encode,
    keyed, provider_range, render_op_line, response, verify_sealed_response,
};

const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

struct CompletedExchange {
    outcome: Result<(), String>,
    shutdown: bool,
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
        self.key.fill(0);
    }

    /// Installs negotiated state only after the complete hello exchange.
    pub(super) fn hello(&mut self) -> Result<(), String> {
        let mut exchange = Exchange { session: self, completed: false };
        let mut budget = ExchangeBudget::new()?;
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
    /// Payload already written to stdout cannot be retracted on a later failure.
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
        exchange.session.endpoint_sequence = next;
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
        let digest = blake3::keyed_hash(&self.key, &input);
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
        let client_sequence = next_sequence(self.envelope_sequence)?;
        let provider_sequence = next_sequence(self.provider_sequence)?;
        let request_id = self.mint_id()?;
        let digest = ProofDigest::from_bytes(*blake3::hash(&[]).as_bytes());
        let stub = seal_envelope(
            self.version, self.nonce, request_id, command, digest, ProofDigest::from_bytes([0; 32]),
        );
        let proof = keyed(&self.key, &envelope_transcript(&stub));
        let sealed = seal_envelope(self.version, self.nonce, request_id, command, digest, proof);
        let frame = encode_envelope_json(&sealed);
        let length = u32::try_from(frame.len()).map_err(|_| "REMOTE_REQUEST_TOO_LARGE".to_owned())?;
        let mut framed = length.to_le_bytes().to_vec();
        framed.extend_from_slice(&frame);
        budget.send(&mut self.stream, &format!("envelope\t{client_sequence}\t{}", hex_encode(&framed)))?;
        response::started(&budget.recv(self)?, self.endpoint_sequence)?;
        for _ in 0..MAX_RESPONSE_LINES {
            let line = budget.recv(self)?;
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
                verify_sealed_response(&self.key, &decoded, &self.nonce, provider_sequence)?;
                // A verified outcome-unknown terminal intentionally precedes
                // connection abort, not a normal endpoint acknowledgement.
                // Preserve its meaning instead of replacing it with an EOF error.
                if decoded.status() == RequestStatus::OutcomeUnknown {
                    return Err("REMOTE_OUTCOME_UNKNOWN".to_owned());
                }
                let complete = budget.recv(self)?;
                if response::complete(&complete, self.endpoint_sequence)?.is_some() {
                    return Err("REMOTE_RESPONSE_MISMATCH".to_owned());
                }
                self.envelope_sequence = client_sequence;
                self.provider_sequence = provider_sequence;
                return Ok(CompletedExchange {
                    outcome: match decoded.status() {
                        RequestStatus::Ok => Ok(()),
                        RequestStatus::Partial => Err("REMOTE_PARTIAL_RESULT".to_owned()),
                        RequestStatus::Cancelled => Err("REMOTE_CANCELLED".to_owned()),
                        RequestStatus::Failed => Err("REMOTE_REQUEST_FAILED".to_owned()),
                        RequestStatus::OutcomeUnknown => Err("REMOTE_OUTCOME_UNKNOWN".to_owned()),
                    },
                    shutdown: command == ControlCommand::Shutdown && decoded.status() == RequestStatus::Ok,
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
                _ => print_payload(&line)?,
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
                    print_payload(&line)?;
                    return Ok(CompletedExchange { outcome: reply.result(), shutdown: false });
                }
                "provider_error" => {
                    let reason = response::provider_error(&line)?;
                    let complete = budget.recv(self)?;
                    if response::complete(&complete, self.endpoint_sequence)? != Some(reason) {
                        return Err("REMOTE_RESPONSE_MISMATCH".to_owned());
                    }
                    return Ok(CompletedExchange { outcome: Err(reason.to_owned()), shutdown: false });
                }
                name if control_event(name) => return Err(response::invalid()),
                _ => print_payload(&line)?,
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

/// Counts actual consumed wire bytes (including newlines), without buffering a
/// whole result. One absolute deadline covers all post-pairing reads/writes.
/// Socket timeouts use only its remainder; partial I/O never renews the budget.
struct ExchangeBudget {
    deadline: Instant,
    bytes: usize,
    lines: usize,
}

impl ExchangeBudget {
    fn new() -> Result<Self, String> {
        Ok(Self {
            deadline: Instant::now().checked_add(IO_TIMEOUT)
                .ok_or_else(|| "REMOTE_DEADLINE_EXPIRED".to_owned())?,
            bytes: 0,
            lines: 0,
        })
    }

    fn remaining(&self) -> Result<Duration, String> {
        self.deadline.checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| "REMOTE_DEADLINE_EXPIRED".to_owned())
    }

    fn check(&self) -> Result<(), String> { self.remaining().map(|_| ()) }

    fn send(&self, stream: &mut std::net::TcpStream, line: &str) -> Result<(), String> {
        if line.is_empty() || line.len() >= MAX_PROVIDER_LINE_BYTES || line.contains('\n') || line.contains('\r') {
            return Err("REMOTE_REQUEST_TOO_LARGE".to_owned());
        }
        for mut bytes in [line.as_bytes(), b"\n".as_slice()] {
            while !bytes.is_empty() {
                stream.set_write_timeout(Some(self.remaining()?))
                    .map_err(|_| "REMOTE_TIMEOUT_CONFIGURATION_ERROR".to_owned())?;
                match stream.write(bytes) {
                    Ok(0) => return Err("REMOTE_WRITE_ERROR".to_owned()),
                    Ok(count) => bytes = &bytes[count..],
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => return Err("REMOTE_WRITE_ERROR".to_owned()),
                }
            }
        }
        stream.set_write_timeout(Some(self.remaining()?))
            .map_err(|_| "REMOTE_TIMEOUT_CONFIGURATION_ERROR".to_owned())?;
        stream.flush().map_err(|_| "REMOTE_WRITE_ERROR".to_owned())?;
        self.check()
    }

    fn recv(&mut self, session: &mut ProviderSession) -> Result<String, String> {
        if self.lines >= MAX_RESPONSE_LINES { return Err("REMOTE_RESPONSE_LINE_LIMIT_EXCEEDED".to_owned()); }
        self.lines += 1;
        let reader = &mut session.reader;
        let mut line = Vec::new();
        loop {
            self.check()?;
            if reader.buffer().is_empty() {
                reader.get_ref().set_read_timeout(Some(self.remaining()?))
                    .map_err(|_| "REMOTE_TIMEOUT_CONFIGURATION_ERROR".to_owned())?;
            }
            let (count, ended) = {
                let bytes = match reader.fill_buf() {
                    Ok(bytes) => bytes,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => return Err("REMOTE_READ_ERROR".to_owned()),
                };
                if bytes.is_empty() { return Err("REMOTE_RESPONSE_TRUNCATED".to_owned()); }
                let newline = bytes.iter().position(|byte| *byte == b'\n');
                let count = newline.map_or(bytes.len(), |position| position + 1);
                self.bytes = self.bytes.checked_add(count)
                    .filter(|bytes| *bytes <= MAX_RESPONSE_BYTES)
                    .ok_or_else(|| "REMOTE_RESPONSE_BYTES_EXCEEDED".to_owned())?;
                if line.len().checked_add(count).is_none_or(|size| size > MAX_PROVIDER_LINE_BYTES) {
                    return Err("REMOTE_FRAME_TOO_LARGE".to_owned());
                }
                line.extend_from_slice(&bytes[..count]);
                (count, newline.is_some())
            };
            reader.consume(count);
            self.check()?;
            if ended {
                line.pop();
                if line.last() == Some(&b'\r') { line.pop(); }
                return String::from_utf8(line).map_err(|_| "REMOTE_FRAME_INVALID_UTF8".to_owned());
            }
        }
    }
}
