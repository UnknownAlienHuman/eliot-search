//! Deadline and byte ownership shared by pairing, hello and request exchanges.

use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use super::{IO_TIMEOUT, MAX_PROVIDER_LINE_BYTES, MAX_RESPONSE_LINES, ProviderSession};

const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const CANCELLATION_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

/// One non-renewable work budget, with at most one diagnostic-cancel cleanup.
/// Cleanup cannot turn expired work into success or reset byte/line accounting.
/// A partial frame survives the work-to-cleanup boundary on the same reader.
/// This bounds socket I/O, not blocking stdout or unrelated filesystem calls.
pub(super) struct ExchangeBudget {
    deadline: Instant,
    bytes: usize,
    lines: usize,
    partial: Vec<u8>,
    line_limit: Option<usize>,
    cleanup_started: bool,
}

impl ExchangeBudget {
    pub(super) fn new() -> Result<Self, String> {
        Ok(Self {
            deadline: Instant::now().checked_add(IO_TIMEOUT)
                .ok_or_else(|| "REMOTE_DEADLINE_EXPIRED".to_owned())?,
            bytes: 0,
            lines: 0,
            partial: Vec::new(),
            line_limit: None,
            cleanup_started: false,
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

    /// Starts one bounded cleanup only after the work deadline has expired.
    /// Keep the same parser state and finite response quota. Only the diagnostic
    /// exchange owner may use this to send cancel and consume its acknowledgements;
    /// it must still return the original deadline failure, never late success.
    pub(super) fn begin_cancel_cleanup(&mut self) -> Result<(), String> {
        if self.cleanup_started || self.remaining().is_ok() {
            return Err("REMOTE_CANCELLATION_STATE_INVALID".to_owned());
        }
        let deadline = Instant::now()
            .checked_add(CANCELLATION_CLEANUP_TIMEOUT)
            .ok_or_else(|| "REMOTE_DEADLINE_EXPIRED".to_owned())?;
        self.cleanup_started = true;
        self.deadline = deadline;
        Ok(())
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
        match self.line_limit {
            Some(limit) if limit != maximum_bytes => {
                return Err("REMOTE_RESPONSE_INVALID".to_owned());
            }
            Some(_) => {}
            None => {
                if self.lines >= MAX_RESPONSE_LINES {
                    return Err("REMOTE_RESPONSE_LINE_LIMIT_EXCEEDED".to_owned());
                }
                self.lines += 1;
                self.line_limit = Some(maximum_bytes);
            }
        }
        loop {
            self.check()?;
            // The deadline may have expired just after consume(), including
            // after LF. Resume this exact line before touching any later frame.
            if self.partial.last() == Some(&b'\n') {
                let mut line = std::mem::take(&mut self.partial);
                self.line_limit = None;
                line.pop();
                if line.last() == Some(&b'\r') { line.pop(); }
                return String::from_utf8(line)
                    .map_err(|_| "REMOTE_FRAME_INVALID_UTF8".to_owned());
            }
            if self.partial.len() >= maximum_bytes {
                return Err("REMOTE_FRAME_TOO_LARGE".to_owned());
            }
            if reader.buffer().is_empty() {
                reader.get_ref().set_read_timeout(Some(self.remaining()?))
                    .map_err(|_| "REMOTE_TIMEOUT_CONFIGURATION_ERROR".to_owned())?;
            }
            let count = {
                let bytes = match reader.fill_buf() {
                    Ok(bytes) => bytes,
                    Err(error) if matches!(error.kind(), io::ErrorKind::Interrupted
                        | io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {
                        // An OS timeout can return before the monotonic deadline.
                        // Retry only within the unchanged budget and same frame.
                        self.check()?;
                        continue;
                    }
                    Err(_) => {
                        self.check()?;
                        return Err("REMOTE_READ_ERROR".to_owned());
                    }
                };
                self.check()?;
                if bytes.is_empty() {
                    return Err(if self.partial.is_empty() {
                        missing
                    } else {
                        "REMOTE_RESPONSE_TRUNCATED"
                    }.to_owned());
                }
                let newline = bytes.iter().position(|byte| *byte == b'\n');
                let count = newline.map_or(bytes.len(), |position| position + 1);
                self.bytes = self.bytes.checked_add(count)
                    .filter(|bytes| *bytes <= MAX_RESPONSE_BYTES)
                    .ok_or_else(|| "REMOTE_RESPONSE_BYTES_EXCEEDED".to_owned())?;
                if self.partial.len().checked_add(count).is_none_or(|size| size > maximum_bytes) {
                    return Err("REMOTE_FRAME_TOO_LARGE".to_owned());
                }
                self.partial.extend_from_slice(&bytes[..count]);
                count
            };
            reader.consume(count);
            // The next iteration checks the clock before returning any line.
            // On expiry neither the prefix nor its accounting is discarded.
        }
    }
}
