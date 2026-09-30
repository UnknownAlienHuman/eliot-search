//! Single-owner typed record I/O over one local byte stream.
//!
//! Protocol/session code depends only on this private bounded stream seam.
//! Concrete TCP and future named-pipe adapters own transport-specific setup.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::task::Poll;
use std::time::{Duration, Instant};

use search_provider_protocol::{
    ProofDigest, ProtocolError, ProtocolLimits, TypedRecordBuffer, TypedTransportProfileV1,
};

use super::TypedClientError;

pub(super) const POLL_INTERVAL: Duration = Duration::from_millis(25);
const POLL_BYTES: usize = 64 * 1024;
const READ_BYTES: usize = 16 * 1024;

/// Private byte-stream capability required by the typed client engine.
///
/// Implementations own transport-specific validation, timeout mapping and
/// shutdown. They must represent one original local connection; cloning,
/// reconnecting, buffering ahead and hidden fallback are forbidden.
pub(super) trait ProviderByteStream: Read + Write + Send + Sync + 'static {
    /// Validate and configure the concrete stream before protocol I/O.
    fn configure(&self) -> Result<(), TypedClientError>;

    /// Bound the next receive operation.
    fn set_receive_timeout(&self, timeout: Option<Duration>) -> io::Result<()>;

    /// Bound the next send operation.
    fn set_send_timeout(&self, timeout: Option<Duration>) -> io::Result<()>;

    /// Close both directions without implying rollback or remote receipt.
    fn close(&self) -> io::Result<()>;
}

impl ProviderByteStream for TcpStream {
    fn configure(&self) -> Result<(), TypedClientError> {
        if !self.peer_addr()?.ip().is_loopback() || !self.local_addr()?.ip().is_loopback() {
            return Err(TypedClientError::NonLoopback);
        }
        self.set_nonblocking(false)?;
        Ok(())
    }

    fn set_receive_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(timeout)
    }

    fn set_send_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.set_write_timeout(timeout)
    }

    fn close(&self) -> io::Result<()> {
        self.shutdown(Shutdown::Both)
    }
}

struct Incoming {
    record: TypedRecordBuffer,
    deadline: Instant,
}

/// One absolute setup deadline plus an injected cancellation observation.
///
/// The cancellation callback owns no transport or request state. Every partial
/// setup read/write consults it between bounded 25 ms waits; it cannot renew the
/// deadline or turn a possible peer-visible write into rollback.
pub(super) struct SetupBudget<'a> {
    deadline: Instant,
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> SetupBudget<'a> {
    pub(super) fn new(
        duration: Duration,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, TypedClientError> {
        let (deadline, _) = budget(duration)?;
        let value = Self {
            deadline,
            cancelled,
        };
        value.remaining()?;
        Ok(value)
    }

    pub(super) fn remaining(&self) -> Result<Duration, TypedClientError> {
        if (self.cancelled)() {
            return Err(TypedClientError::Cancelled);
        }
        remaining(self.deadline)
    }
}

/// Sole bounded I/O owner used by pairing and typed-session state.
///
/// The concrete stream remains private behind [`ProviderByteStream`]. No caller
/// can downcast it, clone it or change transport behavior after handoff.
pub(super) struct ProviderIo {
    stream: Box<dyn ProviderByteStream>,
    incoming: Option<Incoming>,
}

impl ProviderIo {
    pub(super) fn new<S>(stream: S) -> Self
    where
        S: ProviderByteStream,
    {
        Self {
            stream: Box::new(stream),
            incoming: None,
        }
    }

    pub(super) fn configure(&self) -> Result<(), TypedClientError> {
        self.stream.configure()
    }

    pub(super) fn read_exact_setup(
        &mut self,
        mut bytes: &mut [u8],
        budget: &SetupBudget<'_>,
    ) -> Result<(), TypedClientError> {
        while !bytes.is_empty() {
            self.stream
                .set_receive_timeout(Some(budget.remaining()?.min(POLL_INTERVAL)))?;
            match self.stream.read(bytes) {
                Ok(0) => return Err(TypedClientError::PeerClosed),
                Ok(count) => {
                    let buffer = bytes;
                    bytes = &mut buffer[count..];
                }
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(error.into()),
            }
            budget.remaining()?;
        }
        budget.remaining().map(|_| ())
    }

    pub(super) fn poll_record(
        &mut self,
        limits: ProtocolLimits,
        deadline: Instant,
        quantum: Duration,
    ) -> Result<Poll<(Vec<u8>, ProofDigest)>, TypedClientError> {
        if quantum.is_zero() {
            return Err(ProtocolError::InvalidLimits.into());
        }
        let turn_end = Instant::now()
            .checked_add(quantum.min(POLL_INTERVAL))
            .ok_or(TypedClientError::DeadlineExpired)?;
        if let Some(incoming) = &mut self.incoming {
            // Sending another control/request must never renew a partial frame.
            incoming.deadline = incoming.deadline.min(deadline);
        } else {
            remaining(deadline)?;
            self.incoming = Some(Incoming {
                record: TypedRecordBuffer::new(limits)?,
                deadline,
            });
        }
        let mut left = POLL_BYTES;
        loop {
            let incoming = self
                .incoming
                .as_mut()
                .expect("record retained for polling");
            remaining(incoming.deadline)?;
            if incoming.record.is_complete() {
                let incoming = self
                    .incoming
                    .take()
                    .expect("complete retained record");
                return incoming
                    .record
                    .finish()
                    .map(Poll::Ready)
                    .map_err(Into::into);
            }
            if left == 0 || Instant::now() >= turn_end {
                return Ok(Poll::Pending);
            }
            let buffer = incoming
                .record
                .read_buffer(left.min(READ_BYTES))
                .map_err(|error| {
                    if error == ProtocolError::ResourceExhausted {
                        TypedClientError::Allocation
                    } else {
                        error.into()
                    }
                })?;
            // Allocation time also consumes the same read budget and quantum.
            let absolute_left = remaining(incoming.deadline)?;
            let Some(turn_left) = turn_end
                .checked_duration_since(Instant::now())
                .filter(|value| !value.is_zero())
            else {
                return Ok(Poll::Pending);
            };
            self.stream
                .set_receive_timeout(Some(absolute_left.min(turn_left)))?;
            match self.stream.read(buffer) {
                Ok(0) => return Err(TypedClientError::PeerClosed),
                Ok(count) => {
                    incoming.record.advance(count)?;
                    left -= count;
                }
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
                self.stream
                    .set_send_timeout(Some(remaining(deadline)?))?;
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
            self.stream
                .set_send_timeout(Some(remaining(deadline)?))?;
            match self.stream.flush() {
                Ok(()) => break,
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        remaining(deadline).map(|_| ())
    }

    pub(super) fn write_parts_setup(
        &mut self,
        parts: &[&[u8]],
        budget: &SetupBudget<'_>,
    ) -> Result<(), TypedClientError> {
        for &part in parts {
            let mut bytes = part;
            while !bytes.is_empty() {
                self.stream
                    .set_send_timeout(Some(budget.remaining()?.min(POLL_INTERVAL)))?;
                match self.stream.write(bytes) {
                    Ok(0) => return Err(TypedClientError::WriteZero),
                    Ok(count) => bytes = &bytes[count..],
                    Err(error) if retryable(&error) => continue,
                    Err(error) => return Err(error.into()),
                }
                budget.remaining()?;
            }
        }
        loop {
            self.stream
                .set_send_timeout(Some(budget.remaining()?.min(POLL_INTERVAL)))?;
            match self.stream.flush() {
                Ok(()) => break,
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        budget.remaining().map(|_| ())
    }
}

impl Drop for ProviderIo {
    fn drop(&mut self) {
        let _ = self.stream.close();
    }
}

pub(super) fn budget(duration: Duration) -> Result<(Instant, u64), TypedClientError> {
    let started = Instant::now();
    let millis = u64::try_from(duration.as_millis())
        .ok()
        .filter(|value| *value > 0)
        .ok_or(TypedClientError::DeadlineExpired)?;
    let deadline = started
        .checked_add(Duration::from_millis(millis))
        .ok_or(TypedClientError::DeadlineExpired)?;
    Ok((deadline, millis))
}

pub(super) fn remaining(deadline: Instant) -> Result<Duration, TypedClientError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or(TypedClientError::DeadlineExpired)
}

fn retryable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}
