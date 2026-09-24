//! One unbuffered socket owner; partial reads never borrow the next record.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

use search_provider_protocol::{MonotonicMillis, ProofDigest, ProtocolLimits, TypedTransportProfileV1};
use search_provider_protocol::request::RequestCancellation;

use super::{CanonicalTcpError, monotonic_millis};

const POLL: Duration = Duration::from_millis(25);

pub(super) struct SocketIo {
    stream: TcpStream,
}

impl SocketIo {
    pub(super) const fn new(stream: TcpStream) -> Self { Self { stream } }

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
    ) -> Result<(), CanonicalTcpError> {
        while !bytes.is_empty() {
            self.stream.set_read_timeout(Some(remaining(deadline)?)).map_err(CanonicalTcpError::Io)?;
            match self.stream.read(bytes) {
                Ok(0) => return Err(CanonicalTcpError::PeerClosed),
                Ok(count) => {
                    let buffer = bytes;
                    bytes = &mut buffer[count..];
                }
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(CanonicalTcpError::Io(error)),
            }
            remaining(deadline)?;
        }
        remaining(deadline).map(|_| ())
    }

    pub(super) fn read_record(
        &mut self,
        limits: ProtocolLimits,
        deadline: MonotonicMillis,
    ) -> Result<(Vec<u8>, ProofDigest), CanonicalTcpError> {
        let mut prefix = [0_u8; 4];
        self.read_exact(&mut prefix, deadline)?;
        let size = TypedTransportProfileV1::frame_length(prefix, limits)
            .map_err(CanonicalTcpError::Protocol)?;
        let mut frame = Vec::new();
        frame.try_reserve_exact(size).map_err(|_| CanonicalTcpError::Allocation)?;
        frame.extend_from_slice(&prefix);
        frame.resize(size, 0);
        self.read_exact(&mut frame[4..], deadline)?;
        let mut proof = [0_u8; TypedTransportProfileV1::PROOF_BYTES];
        self.read_exact(&mut proof, deadline)?;
        Ok((frame, ProofDigest::from_bytes(proof)))
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
        self.write_parts(&[frame, proof.as_bytes()], deadline, cancellation)
    }

    pub(super) fn write_parts(
        &mut self,
        parts: &[&[u8]],
        deadline: MonotonicMillis,
        cancellation: Option<&RequestCancellation>,
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

fn check_cancel(cancellation: Option<&RequestCancellation>) -> Result<(), CanonicalTcpError> {
    if cancellation.is_some_and(RequestCancellation::is_cancelled) {
        return Err(CanonicalTcpError::Cancelled);
    }
    Ok(())
}

fn retryable(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::Interrupted | io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock)
}
