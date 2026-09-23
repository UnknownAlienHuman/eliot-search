//! Deadline and byte ownership shared by pairing, hello and request exchanges.

use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use super::{IO_TIMEOUT, MAX_PROVIDER_LINE_BYTES, MAX_RESPONSE_LINES, ProviderSession};

const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

/// One non-renewable budget; connecting and negotiating never create a new one.
/// Counts consumed wire bytes, including delimiters, without buffering a result.
/// This bounds socket I/O, not blocking stdout or unrelated filesystem calls.
pub(super) struct ExchangeBudget {
    deadline: Instant,
    bytes: usize,
    lines: usize,
}

impl ExchangeBudget {
    pub(super) fn new() -> Result<Self, String> {
        Ok(Self {
            deadline: Instant::now().checked_add(IO_TIMEOUT)
                .ok_or_else(|| "REMOTE_DEADLINE_EXPIRED".to_owned())?,
            bytes: 0,
            lines: 0,
        })
    }

    pub(super) fn remaining(&self) -> Result<Duration, String> {
        self.deadline.checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| "REMOTE_DEADLINE_EXPIRED".to_owned())
    }

    pub(super) fn check(&self) -> Result<(), String> {
        self.remaining().map(|_| ())
    }

    pub(super) fn send(&self, stream: &mut TcpStream, line: &str) -> Result<(), String> {
        if line.is_empty() || line.len() >= MAX_PROVIDER_LINE_BYTES
            || line.contains('\n') || line.contains('\r')
        {
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
                    Err(_) => {
                        self.check()?;
                        return Err("REMOTE_WRITE_ERROR".to_owned());
                    }
                }
            }
        }
        loop {
            stream.set_write_timeout(Some(self.remaining()?))
                .map_err(|_| "REMOTE_TIMEOUT_CONFIGURATION_ERROR".to_owned())?;
            match stream.flush() {
                Ok(()) => return self.check(),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.check()?;
                    return Err("REMOTE_WRITE_ERROR".to_owned());
                }
            }
        }
    }

    pub(super) fn recv(&mut self, session: &mut ProviderSession) -> Result<String, String> {
        self.read_line(&mut session.reader, MAX_PROVIDER_LINE_BYTES, "REMOTE_RESPONSE_TRUNCATED")
    }

    /// Reads one frame with the phase's actual bound; no per-byte timeout reset.
    /// Buffered input is checked too. EOF before any bytes retains the phase's
    /// missing-frame code; partial EOF is always a truncated exchange.
    pub(super) fn read_line(
        &mut self,
        reader: &mut BufReader<TcpStream>,
        maximum_bytes: usize,
        missing: &'static str,
    ) -> Result<String, String> {
        if maximum_bytes == 0 || maximum_bytes > MAX_PROVIDER_LINE_BYTES {
            return Err("REMOTE_FRAME_TOO_LARGE".to_owned());
        }
        if self.lines >= MAX_RESPONSE_LINES {
            return Err("REMOTE_RESPONSE_LINE_LIMIT_EXCEEDED".to_owned());
        }
        self.lines += 1;
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
                    Err(_) => {
                        self.check()?;
                        return Err("REMOTE_READ_ERROR".to_owned());
                    }
                };
                self.check()?;
                if bytes.is_empty() {
                    return Err(if line.is_empty() { missing } else { "REMOTE_RESPONSE_TRUNCATED" }.to_owned());
                }
                let newline = bytes.iter().position(|byte| *byte == b'\n');
                let count = newline.map_or(bytes.len(), |position| position + 1);
                self.bytes = self.bytes.checked_add(count)
                    .filter(|bytes| *bytes <= MAX_RESPONSE_BYTES)
                    .ok_or_else(|| "REMOTE_RESPONSE_BYTES_EXCEEDED".to_owned())?;
                if line.len().checked_add(count).is_none_or(|size| size > maximum_bytes) {
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
            if line.len() == maximum_bytes {
                // There is no remaining room for the mandatory newline. Reject
                // now rather than waiting for another byte until the deadline.
                return Err("REMOTE_FRAME_TOO_LARGE".to_owned());
            }
        }
    }
}
