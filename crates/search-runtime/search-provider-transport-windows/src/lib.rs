//! Single-owner synchronous Windows provider byte transport.
//!
//! Opening a pipe proves no registration, pairing, grant or peer receipt.
//! The typed client owns all framing, deadlines, cancellation and authentication.

#![deny(missing_docs, unsafe_op_in_unsafe_fn)]

use core::fmt;
use std::io::{self, Read, Write};
use std::time::Instant;

use search_provider_protocol::NativeEndpointNameV1;

// Only this checked synchronous ABI adapter may use unsafe code.
#[cfg(windows)]
#[allow(unsafe_code)]
#[path = "windows.rs"]
mod platform;

#[cfg(not(windows))]
mod platform {
    use super::{NativeEndpointNameV1, NativeTransportError, io};
    pub struct Pipe;
    pub fn open(_: NativeEndpointNameV1) -> Result<Pipe, NativeTransportError> {
        Err(NativeTransportError::UnsupportedPlatform)
    }
    impl Pipe {
        pub(super) fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::Unsupported.into())
        }
        pub(super) fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::Unsupported.into())
        }
    }
}

/// Closed failure without paths, installation coordinates or raw OS diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTransportError {
    /// This target has no Windows pipe adapter.
    UnsupportedPlatform,
    /// The caller's original absolute setup deadline expired.
    DeadlineExpired,
    /// The exact local endpoint could not be opened/configured.
    Unavailable,
    /// The endpoint is not a byte-type client pipe.
    UnsupportedPipe,
}

impl NativeTransportError {
    /// Content-free stable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "NATIVE_TRANSPORT_UNSUPPORTED_PLATFORM",
            Self::DeadlineExpired => "NATIVE_TRANSPORT_DEADLINE_EXPIRED",
            Self::Unavailable => "NATIVE_TRANSPORT_UNAVAILABLE",
            Self::UnsupportedPipe => "NATIVE_TRANSPORT_UNSUPPORTED_PIPE",
        }
    }
}

impl fmt::Display for NativeTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}
impl std::error::Error for NativeTransportError {}

/// Sole owner of one canonical local pipe, with no clone or raw-handle escape.
///
/// Synchronous `PIPE_NOWAIT` polling is not asynchronous/overlapped I/O.
/// The caller must retain finite operation deadlines and cancellation checks.
/// Dropping this owner closes its handle, never rolls back submitted bytes.
pub struct NativeLocalConnection {
    pipe: platform::Pipe,
}

impl NativeLocalConnection {
    /// Open one canonical local endpoint once under the original setup deadline.
    ///
    /// No arbitrary path/File/handle, remote prefix, discovery, wait or retry.
    /// Deadline checks bracket the native work; OS call latency is not a hard
    /// real-time guarantee. Success is transport only, never authentication.
    ///
    /// # Errors
    /// Returns a closed failure for an expired deadline, unsupported target or
    /// unavailable/wrong-shape pipe. No credential or raw OS error is retained.
    pub fn open(
        endpoint: NativeEndpointNameV1,
        absolute_deadline: Instant,
    ) -> Result<Self, NativeTransportError> {
        check_deadline(absolute_deadline)?;
        let pipe = platform::open(endpoint)?;
        check_deadline(absolute_deadline)?;
        Ok(Self { pipe })
    }
}

impl fmt::Debug for NativeLocalConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeLocalConnection")
            .finish_non_exhaustive()
    }
}

impl Read for NativeLocalConnection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.pipe.read(buffer)
    }
}
impl Write for NativeLocalConnection {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.pipe.write(buffer)
    }
    fn flush(&mut self) -> io::Result<()> {
        // No userspace write buffer. Never call blocking FlushFileBuffers;
        // this return value is not a peer acknowledgement or durable receipt.
        Ok(())
    }
}

fn check_deadline(deadline: Instant) -> Result<(), NativeTransportError> {
    if Instant::now() >= deadline {
        Err(NativeTransportError::DeadlineExpired)
    } else {
        Ok(())
    }
}

#[cfg(any(windows, test))]
fn read_result(result: io::Result<usize>) -> io::Result<usize> {
    match result {
        // Empty PIPE_NOWAIT reads report NO_DATA. If closure also reports this,
        // the caller still stops at its original deadline with unknown outcome.
        Err(error) if error.raw_os_error() == Some(232) => Err(io::ErrorKind::WouldBlock.into()),
        Err(error) if matches!(error.raw_os_error(), Some(109 | 233)) => Ok(0),
        Err(error) => Err(error.kind().into()),
        value => value,
    }
}

#[cfg(any(windows, test))]
fn write_result(result: io::Result<usize>, empty: bool) -> io::Result<usize> {
    match result {
        Ok(0) if !empty => Err(io::ErrorKind::WouldBlock.into()),
        Err(error) if error.raw_os_error() == Some(232) => Err(io::ErrorKind::WouldBlock.into()),
        Err(error) if matches!(error.raw_os_error(), Some(109 | 233)) => {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        Err(error) => Err(error.kind().into()),
        value => value,
    }
}

#[cfg(test)]
mod tests;
