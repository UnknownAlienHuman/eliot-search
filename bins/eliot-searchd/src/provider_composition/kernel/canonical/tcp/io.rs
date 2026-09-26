//! One unbuffered socket owner; partial reads never borrow the next record.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::task::Poll;
use std::time::{Duration, Instant};

use search_ports::CancellationProbe;

use search_provider_protocol::{MonotonicMillis, ProofDigest, ProtocolError, ProtocolLimits, TypedRecordBuffer, TypedTransportProfileV1};
use search_provider_protocol::request::RequestCancellation;

use super::{CanonicalTcpError, monotonic_millis};

pub(super) const POLL: Duration = Duration::from_millis(25);
const POLL_BYTES: usize = 64 * 1024;
const READ_BYTES: usize = 16 * 1024;

struct Incoming {
    record: TypedRecordBuffer,
    started: MonotonicMillis,
    maximum_deadline_ms: u64,
    deadline: MonotonicMillis,
}

pub(super) struct ReceivedRecord {
    pub(super) frame: Vec<u8>,
    pub(super) proof: ProofDigest,
    pub(super) started: MonotonicMillis,
    pub(super) maximum_deadline_ms: u64,
}

pub(super) struct SocketIo {
    stream: TcpStream,
    incoming: Option<Incoming>,
}

impl SocketIo {
    pub(super) const fn new(stream: TcpStream) -> Self { Self { stream, incoming: None } }

    pub(super) fn configure(&self) -> Result<(), CanonicalTcpError> {
        if !self.stream.peer_addr().map_err(CanonicalTcpError::Io)?.ip().is_loopback()
            || !self.stream.local_addr().map_err(CanonicalTcpError::Io)?.ip().is_loopback()
        {
            return Err(CanonicalTcpError::NonLoopback);
        }
        self.stream.set_nonblocking(false).map_err(CanonicalTcpError::Io)
    }

    pub(super) fn read_exact(
        &mut self,
        mut bytes: &mut [u8],
        deadline: MonotonicMillis,
        cancellation: Option<&dyn CancellationProbe>,
    ) -> Result<(), CanonicalTcpError> {
        while !bytes.is_empty() {
            check_cancel(cancellation)?;
            self.stream.set_read_timeout(Some(remaining(deadline)?.min(POLL)))
                .map_err(CanonicalTcpError::Io)?;
            match self.stream.read(bytes) {
                Ok(0) => return Err(CanonicalTcpError::PeerClosed),
                Ok(count) => {
                    let buffer = bytes;
                    bytes = &mut buffer[count..];
                }
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(CanonicalTcpError::Io(error)),
            }
            check_cancel(cancellation)?;
            remaining(deadline)?;
        }
        check_cancel(cancellation)?;
        remaining(deadline).map(|_| ())
    }

    pub(super) fn poll_record(
        &mut self,
        limits: ProtocolLimits,
        maximum_deadline_ms: u64,
        quantum: Duration,
    ) -> Result<Poll<ReceivedRecord>, CanonicalTcpError> {
        if quantum.is_zero() {
            return Err(CanonicalTcpError::Protocol(ProtocolError::InvalidLimits));
        }
        let turn_end = Instant::now().checked_add(quantum.min(POLL))
            .ok_or(CanonicalTcpError::DeadlineExpired)?;
        if let Some(incoming) = &mut self.incoming {
            // Polling can tighten the cap, never restart its original clock.
            incoming.maximum_deadline_ms = incoming.maximum_deadline_ms.min(maximum_deadline_ms);
            incoming.deadline = deadline(incoming.started, incoming.maximum_deadline_ms)?;
        } else {
            let started = monotonic_millis();
            self.incoming = Some(Incoming {
                record: TypedRecordBuffer::new(limits).map_err(CanonicalTcpError::Protocol)?,
                started, maximum_deadline_ms, deadline: deadline(started, maximum_deadline_ms)?,
            });
        }
        let mut left = POLL_BYTES;
        loop {
            let incoming = self.incoming.as_mut().expect("record retained for polling");
            remaining(incoming.deadline)?;
            if incoming.record.is_complete() {
                let incoming = self.incoming.take().expect("complete retained record");
                let (frame, proof) = incoming.record.finish().map_err(CanonicalTcpError::Protocol)?;
                return Ok(Poll::Ready(ReceivedRecord {
                    frame, proof, started: incoming.started,
                    maximum_deadline_ms: incoming.maximum_deadline_ms,
                }));
            }
            if left == 0 || Instant::now() >= turn_end { return Ok(Poll::Pending); }
            let buffer = incoming.record.read_buffer(left.min(READ_BYTES)).map_err(|error| {
                if error == ProtocolError::ResourceExhausted { CanonicalTcpError::Allocation }
                else { CanonicalTcpError::Protocol(error) }
            })?;
            let absolute_left = remaining(incoming.deadline)?;
            let Some(turn_left) = turn_end.checked_duration_since(Instant::now())
                .filter(|value| !value.is_zero()) else { return Ok(Poll::Pending); };
            self.stream.set_read_timeout(Some(absolute_left.min(turn_left))).map_err(CanonicalTcpError::Io)?;
            match self.stream.read(buffer) {
                Ok(0) => return Err(CanonicalTcpError::PeerClosed),
                Ok(count) => {
                    incoming.record.advance(count).map_err(CanonicalTcpError::Protocol)?;
                    left -= count;
                }
                Err(error) if retryable(&error) => {}
                Err(error) => return Err(CanonicalTcpError::Io(error)),
            }
        }
    }

    pub(super) fn write_record(
        &mut self,
        frame: &[u8],
        proof: &ProofDigest,
        limits: ProtocolLimits,
        deadline: MonotonicMillis,
        cancellation: Option<&RequestCancellation>,
    ) -> Result<(), CanonicalTcpError> {
        TypedTransportProfileV1::validate_frame(frame, limits).map_err(CanonicalTcpError::Protocol)?;
        self.write_parts(
            &[frame, proof.as_bytes()], deadline,
            cancellation.map(|probe| probe as &dyn CancellationProbe),
        )
    }

    pub(super) fn write_parts(
        &mut self,
        parts: &[&[u8]],
        deadline: MonotonicMillis,
        cancellation: Option<&dyn CancellationProbe>,
    ) -> Result<(), CanonicalTcpError> {
        for &part in parts {
            let mut bytes = part;
            while !bytes.is_empty() {
                check_cancel(cancellation)?;
                self.stream.set_write_timeout(Some(remaining(deadline)?.min(POLL)))
                    .map_err(CanonicalTcpError::Io)?;
                match self.stream.write(bytes) {
                    Ok(0) => return Err(CanonicalTcpError::WriteZero),
                    Ok(count) => bytes = &bytes[count..],
                    Err(error) if retryable(&error) => continue,
                    Err(error) => return Err(CanonicalTcpError::Io(error)),
                }
                check_cancel(cancellation)?;
                remaining(deadline)?;
            }
        }
        loop {
            check_cancel(cancellation)?;
            self.stream.set_write_timeout(Some(remaining(deadline)?.min(POLL)))
                .map_err(CanonicalTcpError::Io)?;
            match self.stream.flush() {
                Ok(()) => break,
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(CanonicalTcpError::Io(error)),
            }
        }
        check_cancel(cancellation)?;
        remaining(deadline).map(|_| ())
    }
}

impl Drop for SocketIo {
    fn drop(&mut self) { let _ = self.stream.shutdown(Shutdown::Both); }
}

pub(super) fn deadline(started: MonotonicMillis, millis: u64) -> Result<MonotonicMillis, CanonicalTcpError> {
    started.get().checked_add(millis).filter(|end| *end > started.get())
        .map(MonotonicMillis::new).ok_or(CanonicalTcpError::DeadlineExpired)
}

fn remaining(deadline: MonotonicMillis) -> Result<Duration, CanonicalTcpError> {
    deadline.get().checked_sub(monotonic_millis().get()).filter(|left| *left > 0)
        .map(Duration::from_millis).ok_or(CanonicalTcpError::DeadlineExpired)
}

fn check_cancel(cancellation: Option<&dyn CancellationProbe>) -> Result<(), CanonicalTcpError> {
    if cancellation.is_some_and(CancellationProbe::is_cancelled) {
        return Err(CanonicalTcpError::Cancelled);
    }
    Ok(())
}

fn retryable(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::Interrupted | io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock)
}
