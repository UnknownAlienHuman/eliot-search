//! Strict bounded line I/O and redacted transport diagnostics.

use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use super::spec::{MAX_COMMAND_LINE_BYTES, READ_TIMEOUT, WRITE_TIMEOUT};

/// One absolute socket budget. Reuse it across every step of a handshake;
/// a partial read/write, interruption or buffered frame never restarts it.
pub(super) struct SocketDeadline {
    expires_at: Instant,
}

impl SocketDeadline {
    pub(super) fn new(timeout: Duration) -> io::Result<Self> {
        if timeout.is_zero() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let expires_at = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        Ok(Self { expires_at })
    }

    fn remaining(&self, operation_limit: Duration) -> io::Result<Duration> {
        self.expires_at
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .map(|remaining| remaining.min(operation_limit))
            .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))
    }

    pub(super) fn check(&self) -> io::Result<()> {
        self.remaining(READ_TIMEOUT).map(|_| ())
    }

    /// The wire ceiling includes LF/CRLF, as in the existing endpoint protocol.
    /// Only the current line is consumed; any prefetched next frame stays in the
    /// original reader. EOF after a prefix is never a complete command.
    pub(super) fn read_line(
        &self,
        reader: &mut BufReader<TcpStream>,
        maximum_bytes: usize,
    ) -> Result<Option<String>, String> {
        let mut line = Vec::new();
        loop {
            match self.read_step(reader, &mut line, maximum_bytes, READ_TIMEOUT)? {
                LineRead::Pending => {}
                LineRead::Eof => return Ok(None),
                LineRead::Complete(line) => return Ok(Some(line)),
            }
        }
    }

    /// Consumes at most one buffer chunk and never discards a partial frame.
    /// A short polling timeout is not frame expiry; the original deadline still
    /// governs every later attempt. Only read timeout is changed, not socket
    /// nonblocking mode or the independent worker's write timeout.
    pub(super) fn read_step(
        &self,
        reader: &mut BufReader<TcpStream>,
        line: &mut Vec<u8>,
        maximum_bytes: usize,
        wait: Duration,
    ) -> Result<LineRead, String> {
        if maximum_bytes == 0 || maximum_bytes > MAX_COMMAND_LINE_BYTES || wait.is_zero() {
            return Err("ENDPOINT_FRAME_LIMIT_INVALID".to_owned());
        }
        if line.len() >= maximum_bytes {
            return Err("ENDPOINT_FRAME_TOO_LARGE".to_owned());
        }
        self.check().map_err(read_error)?;
        if reader.buffer().is_empty() {
            reader.get_ref()
                .set_read_timeout(Some(self.remaining(wait).map_err(read_error)?))
                .map_err(|error| redacted_io_error("ENDPOINT_TIMEOUT_CONFIGURATION_ERROR", &error))?;
        }
        let (consumed, ended) = {
            let available = match reader.fill_buf() {
                Ok(available) => available,
                Err(error) if matches!(error.kind(), io::ErrorKind::Interrupted
                    | io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {
                    self.check().map_err(read_error)?;
                    return Ok(LineRead::Pending);
                }
                Err(error) => return Err(read_error(error)),
            };
            self.check().map_err(read_error)?;
            if available.is_empty() {
                return if line.is_empty() {
                    Ok(LineRead::Eof)
                } else {
                    Err("ENDPOINT_FRAME_TOO_LARGE".to_owned())
                };
            }
            let remaining = maximum_bytes - line.len();
            let offered = &available[..available.len().min(remaining)];
            let newline = offered.iter().position(|byte| *byte == b'\n');
            let consumed = newline.map_or(offered.len(), |position| position + 1);
            line.extend_from_slice(&offered[..consumed]);
            (consumed, newline.is_some())
        };
        reader.consume(consumed);
        self.check().map_err(read_error)?;
        if ended {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            return String::from_utf8(std::mem::take(line))
                .map(LineRead::Complete)
                .map_err(|_| "ENDPOINT_FRAME_INVALID_UTF8".to_owned());
        }
        if line.len() == maximum_bytes {
            return Err("ENDPOINT_FRAME_TOO_LARGE".to_owned());
        }
        Ok(LineRead::Pending)
    }

    /// Each underlying write and flush uses this same deadline. Expiry after a
    /// partial write returns an error; the caller must close, not append a reply.
    pub(super) fn write_line(&self, stream: &mut TcpStream, value: &str) -> io::Result<()> {
        write_line(&mut DeadlineWriter { stream, deadline: self }, value)
    }
}

pub(super) enum LineRead {
    Pending,
    Eof,
    Complete(String),
}

struct DeadlineWriter<'a> {
    stream: &'a mut TcpStream,
    deadline: &'a SocketDeadline,
}

impl Write for DeadlineWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream
            .set_write_timeout(Some(self.deadline.remaining(WRITE_TIMEOUT)?))?;
        let result = self.stream.write(bytes);
        self.deadline.check()?;
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream
            .set_write_timeout(Some(self.deadline.remaining(WRITE_TIMEOUT)?))?;
        let result = self.stream.flush();
        self.deadline.check()?;
        result
    }
}

#[cfg(test)]
pub(super) fn read_bounded_line(
    reader: &mut BufReader<TcpStream>,
    maximum_bytes: usize,
) -> Result<Option<String>, String> {
    SocketDeadline::new(READ_TIMEOUT)
        .map_err(|error| redacted_io_error("ENDPOINT_TIMEOUT_CONFIGURATION_ERROR", &error))?
        .read_line(reader, maximum_bytes)
}

fn read_error(error: io::Error) -> String {
    match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => "ENDPOINT_READ_TIMEOUT".to_owned(),
        _ => redacted_io_error("ENDPOINT_READ_ERROR", &error),
    }
}

pub(super) fn write_line(stream: &mut impl Write, value: &str) -> io::Result<()> {
    stream.write_all(value.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

/// T40 closed transport error carrying only a redacted OS detail class.
pub(super) fn redacted_io_error(code: &str, error: &io::Error) -> String {
    format!("{code}:{:?}", error.kind())
}

pub(super) fn sanitize_code(error: &str) -> String {
    let code = error.split(':').next().unwrap_or("ENDPOINT_ERROR");
    let mut output = String::with_capacity(code.len().min(128));
    for character in code.chars().take(128) {
        if character.is_ascii_uppercase()
            || character.is_ascii_digit()
            || matches!(character, '_' | '-' | '.')
        {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "ENDPOINT_ERROR".to_owned()
    } else {
        output
    }
}
