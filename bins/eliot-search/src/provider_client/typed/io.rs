//! Single-owner typed record I/O. Each operation keeps its original deadline.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant};

use search_provider_protocol::{ProofDigest, ProtocolLimits, TypedTransportProfileV1};

use super::TypedClientError;

pub(super) struct SocketIo(TcpStream);

impl SocketIo {
    pub(super) const fn new(stream: TcpStream) -> Self { Self(stream) }

    pub(super) fn configure(&self) -> Result<(), TypedClientError> {
        if !self.0.peer_addr()?.ip().is_loopback() || !self.0.local_addr()?.ip().is_loopback() {
            return Err(TypedClientError::NonLoopback);
        }
        self.0.set_nonblocking(false)?;
        Ok(())
    }

    pub(super) fn read_exact(
        &mut self,
        mut bytes: &mut [u8],
        deadline: Instant,
    ) -> Result<(), TypedClientError> {
        while !bytes.is_empty() {
            self.0.set_read_timeout(Some(remaining(deadline)?))?;
            match self.0.read(bytes) {
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

    pub(super) fn read_record(
        &mut self,
        limits: ProtocolLimits,
        deadline: Instant,
    ) -> Result<(Vec<u8>, ProofDigest), TypedClientError> {
        let mut prefix = [0_u8; 4];
        self.read_exact(&mut prefix, deadline)?;
        let size = TypedTransportProfileV1::frame_length(prefix, limits)?;
        let mut frame = Vec::new();
        frame.try_reserve_exact(size).map_err(|_| TypedClientError::Allocation)?;
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
                self.0.set_write_timeout(Some(remaining(deadline)?))?;
                match self.0.write(bytes) {
                    Ok(0) => return Err(TypedClientError::WriteZero),
                    Ok(count) => bytes = &bytes[count..],
                    Err(error) if retryable(&error) => continue,
                    Err(error) => return Err(error.into()),
                }
                remaining(deadline)?;
            }
        }
        loop {
            self.0.set_write_timeout(Some(remaining(deadline)?))?;
            match self.0.flush() {
                Ok(()) => break,
                Err(error) if retryable(&error) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        remaining(deadline).map(|_| ())
    }
}

impl Drop for SocketIo {
    fn drop(&mut self) { let _ = self.0.shutdown(Shutdown::Both); }
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
