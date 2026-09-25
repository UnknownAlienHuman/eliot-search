//! Single-owner typed record I/O. Each operation keeps its original deadline.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::task::Poll;
use std::time::{Duration, Instant};

use search_provider_protocol::{ProofDigest, ProtocolError, ProtocolLimits, TypedRecordBuffer, TypedTransportProfileV1};

use super::TypedClientError;

pub(super) const POLL_INTERVAL: Duration = Duration::from_millis(25);
const POLL_BYTES: usize = 64 * 1024;
const READ_BYTES: usize = 16 * 1024;

struct Incoming {
    record: TypedRecordBuffer,
    deadline: Instant,
}

pub(super) struct SocketIo {
    stream: TcpStream,
    incoming: Option<Incoming>,
}

impl SocketIo {
    pub(super) const fn new(stream: TcpStream) -> Self { Self { stream, incoming: None } }

    pub(super) fn configure(&self) -> Result<(), TypedClientError> {
        if !self.stream.peer_addr()?.ip().is_loopback() || !self.stream.local_addr()?.ip().is_loopback() {
            return Err(TypedClientError::NonLoopback);
        }
        self.stream.set_nonblocking(false)?;
        Ok(())
    }

    pub(super) fn read_exact(
        &mut self,
        mut bytes: &mut [u8],
        deadline: Instant,
    ) -> Result<(), TypedClientError> {
        while !bytes.is_empty() {
            self.stream.set_read_timeout(Some(remaining(deadline)?))?;
            match self.stream.read(bytes) {
                Ok(0) => return Err(TypedClientError::PeerClosed),
                Ok(count) => {
                    let buffer = bytes;
                    bytes = &mut buffer[count..];
                }
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(error.into()),
            }
            remaining(deadline)?;
        }
        remaining(deadline).map(|_| ())
    }

    pub(super) fn poll_record(
        &mut self,
        limits: ProtocolLimits,
        deadline: Instant,
        quantum: Duration,
    ) -> Result<Poll<(Vec<u8>, ProofDigest)>, TypedClientError> {
        if quantum.is_zero() { return Err(ProtocolError::InvalidLimits.into()); }
        let turn_end = Instant::now().checked_add(quantum.min(POLL_INTERVAL))
            .ok_or(TypedClientError::DeadlineExpired)?;
        if let Some(incoming) = &mut self.incoming {
            // Sending another control/request must never renew a partial frame.
            incoming.deadline = incoming.deadline.min(deadline);
        } else {
            remaining(deadline)?;
            self.incoming = Some(Incoming { record: TypedRecordBuffer::new(limits)?, deadline });
        }
        let mut left = POLL_BYTES;
        loop {
            let incoming = self.incoming.as_mut().expect("record retained for polling");
            remaining(incoming.deadline)?;
            if incoming.record.is_complete() {
                let incoming = self.incoming.take().expect("complete retained record");
                return incoming.record.finish().map(Poll::Ready).map_err(Into::into);
            }
            if left == 0 || Instant::now() >= turn_end { return Ok(Poll::Pending); }
            let buffer = incoming.record.read_buffer(left.min(READ_BYTES)).map_err(|error| {
                if error == ProtocolError::ResourceExhausted { TypedClientError::Allocation }
                else { error.into() }
            })?;
            // Allocation time also consumes the same read budget and quantum.
            let absolute_left = remaining(incoming.deadline)?;
            let Some(turn_left) = turn_end.checked_duration_since(Instant::now())
                .filter(|value| !value.is_zero()) else { return Ok(Poll::Pending); };
            self.stream.set_read_timeout(Some(absolute_left.min(turn_left)))?;
            match self.stream.read(buffer) {
                Ok(0) => return Err(TypedClientError::PeerClosed),
                Ok(count) => { incoming.record.advance(count)?; left -= count; }
                Err(error) if retryable(&error) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub(super) fn write_record(
        &mut self,
        frame: &[u8],
        proof: &ProofDigest,
        limits: ProtocolLimits,
        deadline: Instant,
    ) -> Result<(), TypedClientError> {
        TypedTransportProfileV1::validate_frame(frame, limits)?;
        self.write_parts(&[frame, proof.as_bytes()], deadline)
    }

    pub(super) fn write_parts(
        &mut self,
        parts: &[&[u8]],
        deadline: Instant,
    ) -> Result<(), TypedClientError> {
        for &part in parts {
            let mut bytes = part;
            while !bytes.is_empty() {
                self.stream.set_write_timeout(Some(remaining(deadline)?))?;
                match self.stream.write(bytes) {
                    Ok(0) => return Err(TypedClientError::WriteZero),
                    Ok(count) => bytes = &bytes[count..],
                    Err(error) if retryable(&error) => continue,
                    Err(error) => return Err(error.into()),
                }
                remaining(deadline)?;
            }
        }
        loop {
            self.stream.set_write_timeout(Some(remaining(deadline)?))?;
            match self.stream.flush() {
                Ok(()) => break,
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        remaining(deadline).map(|_| ())
    }
}

impl Drop for SocketIo {
    fn drop(&mut self) { let _ = self.stream.shutdown(Shutdown::Both); }
}

pub(super) fn budget(duration: Duration) -> Result<(Instant, u64), TypedClientError> {
    let started = Instant::now();
    let millis = u64::try_from(duration.as_millis()).ok().filter(|value| *value > 0)
        .ok_or(TypedClientError::DeadlineExpired)?;
    let deadline = started.checked_add(Duration::from_millis(millis))
        .ok_or(TypedClientError::DeadlineExpired)?;
    Ok((deadline, millis))
}

pub(super) fn remaining(deadline: Instant) -> Result<Duration, TypedClientError> {
    deadline.checked_duration_since(Instant::now()).filter(|left| !left.is_zero())
        .ok_or(TypedClientError::DeadlineExpired)
}

fn retryable(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::Interrupted | io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock)
}
