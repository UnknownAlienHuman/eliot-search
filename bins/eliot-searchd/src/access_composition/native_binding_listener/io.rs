//! Restricted unbuffered I/O for native pairing on one accepted socket.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::MonotonicMillis;

use crate::provider_composition::monotonic_millis;

const SOCKET_POLL: Duration = Duration::from_millis(25);

/// Closed exact-I/O failure during native pairing or listener polling.
#[derive(Debug)]
pub enum StandalonePairingIoError {
    /// Socket or listener operation failed; peer bytes are never included.
    Io(io::Error),
    /// The original finite setup deadline elapsed.
    DeadlineExpired,
    /// The process-local cancellation capability was signalled.
    Cancelled,
    /// Peer closed before the exact requested bytes arrived.
    PeerClosed,
    /// A nonempty write made no progress.
    WriteZero,
    /// Listener, local endpoint or peer endpoint was not loopback.
    NonLoopback,
    /// Poll/accept configuration was zero, excessive or otherwise invalid.
    InvalidConfiguration,
    /// Monotone connection identity could not advance.
    ConnectionSequenceExhausted,
}

impl StandalonePairingIoError {
    /// Stable redacted diagnostic code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "STANDALONE_LISTENER_IO_ERROR",
            Self::DeadlineExpired => "STANDALONE_PAIRING_DEADLINE_EXPIRED",
            Self::Cancelled => "STANDALONE_PAIRING_CANCELLED",
            Self::PeerClosed => "STANDALONE_PAIRING_TRUNCATED",
            Self::WriteZero => "STANDALONE_PAIRING_WRITE_ZERO",
            Self::NonLoopback => "STANDALONE_LISTENER_LOOPBACK_REQUIRED",
            Self::InvalidConfiguration => "STANDALONE_LISTENER_CONFIGURATION_INVALID",
            Self::ConnectionSequenceExhausted => {
                "STANDALONE_CONNECTION_SEQUENCE_EXHAUSTED"
            }
        }
    }
}

impl std::fmt::Display for StandalonePairingIoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for StandalonePairingIoError {}

/// Restricted unbuffered pairing I/O over the original accepted socket.
///
/// Reads consume exactly the requested bytes. The wrapper exposes no socket
/// clone, raw descriptor or inner-stream accessor. Every partial read/write and
/// flush observes one absolute deadline and the original cancellation probe.
pub struct StandalonePairingIo<'a, C: CancellationProbe> {
    stream: &'a mut TcpStream,
    context: &'a OperationContext<C>,
    started: MonotonicMillis,
    deadline: MonotonicMillis,
}

impl<'a, C: CancellationProbe> StandalonePairingIo<'a, C> {
    pub(super) fn new(
        stream: &'a mut TcpStream,
        context: &'a OperationContext<C>,
    ) -> Result<Self, StandalonePairingIoError> {
        if !stream
            .peer_addr()
            .map_err(StandalonePairingIoError::Io)?
            .ip()
            .is_loopback()
            || !stream
                .local_addr()
                .map_err(StandalonePairingIoError::Io)?
                .ip()
                .is_loopback()
        {
            return Err(StandalonePairingIoError::NonLoopback);
        }
        stream
            .set_nonblocking(false)
            .map_err(StandalonePairingIoError::Io)?;
        let started = monotonic_millis();
        let deadline = started
            .get()
            .checked_add(context.relative_deadline_ms().get())
            .map(MonotonicMillis::new)
            .ok_or(StandalonePairingIoError::DeadlineExpired)?;
        let owner = Self { stream, context, started, deadline };
        owner.remaining()?;
        Ok(owner)
    }

    /// Read exactly one caller-sized field without prefetching the next field.
    pub fn read_exact(&mut self, output: &mut [u8]) -> Result<(), StandalonePairingIoError> {
        let mut offset = 0;
        while offset < output.len() {
            self.stream
                .set_read_timeout(Some(self.remaining()?.min(SOCKET_POLL)))
                .map_err(StandalonePairingIoError::Io)?;
            match self.stream.read(&mut output[offset..]) {
                Ok(0) => return Err(StandalonePairingIoError::PeerClosed),
                Ok(count) => offset += count,
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(StandalonePairingIoError::Io(error)),
            }
            self.remaining()?;
        }
        Ok(())
    }

    /// Write one exact field under the unchanged setup deadline.
    pub fn write_all(&mut self, input: &[u8]) -> Result<(), StandalonePairingIoError> {
        let mut offset = 0;
        while offset < input.len() {
            self.stream
                .set_write_timeout(Some(self.remaining()?.min(SOCKET_POLL)))
                .map_err(StandalonePairingIoError::Io)?;
            match self.stream.write(&input[offset..]) {
                Ok(0) => return Err(StandalonePairingIoError::WriteZero),
                Ok(count) => offset += count,
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(StandalonePairingIoError::Io(error)),
            }
            self.remaining()?;
        }
        Ok(())
    }

    /// Flush all preceding pairing bytes under the same deadline.
    pub fn flush(&mut self) -> Result<(), StandalonePairingIoError> {
        loop {
            self.stream
                .set_write_timeout(Some(self.remaining()?.min(SOCKET_POLL)))
                .map_err(StandalonePairingIoError::Io)?;
            match self.stream.flush() {
                Ok(()) => return self.remaining().map(|_| ()),
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(StandalonePairingIoError::Io(error)),
            }
        }
    }

    fn remaining(&self) -> Result<Duration, StandalonePairingIoError> {
        if self.context.cancellation().is_cancelled() {
            return Err(StandalonePairingIoError::Cancelled);
        }
        let now = monotonic_millis();
        if now < self.started {
            return Err(StandalonePairingIoError::DeadlineExpired);
        }
        self.deadline
            .get()
            .checked_sub(now.get())
            .filter(|left| *left > 0)
            .map(Duration::from_millis)
            .ok_or(StandalonePairingIoError::DeadlineExpired)
    }

    pub(super) fn remaining_context(&self) -> Result<OperationContext<C>, StandalonePairingIoError>
    where
        C: Clone,
    {
        let remaining = u64::try_from(self.remaining()?.as_millis())
            .map_err(|_| StandalonePairingIoError::DeadlineExpired)?;
        OperationContext::new(
            self.context.request_id(),
            remaining,
            self.context.cancellation().clone(),
            self.context.budget_ref().clone(),
        )
        .map_err(|_| StandalonePairingIoError::InvalidConfiguration)
    }
}

fn retryable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}
