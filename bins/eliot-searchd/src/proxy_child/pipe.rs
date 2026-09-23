use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

use search_provider_protocol::request::RequestCancellation;

use super::spec::{MAX_LINE_BYTES, POLL};
use super::time::{check_request, remaining};

pub(super) struct DeadlineWriter {
    pub(super) socket: TcpStream,
    pub(super) deadline: Instant,
}

impl Write for DeadlineWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let left = remaining(self.deadline)
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?;
        self.socket.set_write_timeout(Some(left))?;
        self.socket.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        remaining(self.deadline)
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?;
        self.socket.flush()
    }
}

pub(super) fn read_child_line(output: &mut impl BufRead) -> Result<Option<String>, String> {
    let mut bytes = Vec::new();
    let read = Read::take(&mut *output, (MAX_LINE_BYTES + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(|_| "LOOPBACK_DIRECT_CHILD_READ_ERROR".to_owned())?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > MAX_LINE_BYTES || !bytes.ends_with(b"\n") {
        return Err("LOOPBACK_DIRECT_CHILD_FRAME_TOO_LARGE".to_owned());
    }
    bytes.pop();
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "LOOPBACK_DIRECT_CHILD_FRAME_NOT_UTF8".to_owned())
}

/// Applies the same admitted cancellation signal before and after every actual
/// write/flush, including partial writes to stdin and client output. A blocked
/// child pipe is interrupted by the process owner, not by this cooperative check.
pub(super) struct RequestWriter<'a, W> {
    pub(super) inner: W,
    pub(super) deadline: Instant,
    pub(super) cancellation: Option<&'a RequestCancellation>,
}

impl<W> RequestWriter<'_, W> {
    fn check(&self) -> io::Result<()> {
        if self.cancellation.is_some_and(RequestCancellation::is_cancelled) {
            // Interrupted would cause write_all to retry forever on cancellation.
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        remaining(self.deadline)
            .map(|_| ())
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))
    }
}

impl<W: Write> Write for RequestWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.check()?;
        let written = self.inner.write(bytes)?;
        self.check()?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.check()?;
        self.inner.flush()?;
        self.check()
    }
}

/// Writes a sealed terminal within the original admitted execution budget.
/// The caller must abort the exchange on any error, including a late return
/// after bytes were sent. Successful local I/O is not remote acknowledgement.
pub(in super::super) fn write_admitted_line(
    socket: &TcpStream,
    line: &str,
    deadline: Instant,
    cancellation: &RequestCancellation,
    poll: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    write_observed(socket, &[line.as_bytes(), b"\n"], deadline, Some(cancellation), poll)
}

/// Parent-thread output keeps servicing incoming cancel/EOF even when the client
/// stops draining its receive buffer. The worker owns output until its reply is
/// consumed; only then may the parent use this writer for deferred/terminal bytes.
pub(super) fn write_observed(
    socket: &TcpStream,
    pieces: &[&[u8]],
    deadline: Instant,
    cancellation: Option<&RequestCancellation>,
    poll: &mut dyn FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let original_timeout = socket.write_timeout()
        .map_err(|_| "LOOPBACK_SOCKET_CONFIGURATION_ERROR".to_owned())?;
    let result = (|| {
        let mut writer = socket;
        for &piece in pieces {
            let mut bytes = piece;
            while !bytes.is_empty() {
                check_request(deadline, cancellation)?;
                poll()?;
                check_request(deadline, cancellation)?;
                socket.set_write_timeout(Some(remaining(deadline)?.min(POLL)))
                    .map_err(|_| "LOOPBACK_SOCKET_CONFIGURATION_ERROR".to_owned())?;
                match writer.write(bytes) {
                    Ok(0) => return Err("LOOPBACK_PROXY_WRITE_ERROR".to_owned()),
                    Ok(count) => bytes = &bytes[count..],
                    Err(error) if matches!(error.kind(), io::ErrorKind::Interrupted
                        | io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => continue,
                    Err(_) => return Err("LOOPBACK_PROXY_WRITE_ERROR".to_owned()),
                }
                check_request(deadline, cancellation)?;
            }
        }
        loop {
            poll()?;
            check_request(deadline, cancellation)?;
            socket.set_write_timeout(Some(remaining(deadline)?.min(POLL)))
                .map_err(|_| "LOOPBACK_SOCKET_CONFIGURATION_ERROR".to_owned())?;
            match writer.flush() {
                Ok(()) => break,
                Err(error) if matches!(error.kind(), io::ErrorKind::Interrupted
                    | io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {}
                Err(_) => return Err("LOOPBACK_PROXY_WRITE_ERROR".to_owned()),
            }
        }
        poll()?;
        check_request(deadline, cancellation)
    })();
    // Restore only the socket option, never the original deadline. Any error
    // still requires caller fail-stop, including errors after partial delivery.
    let restored = socket.set_write_timeout(original_timeout)
        .map_err(|_| "LOOPBACK_SOCKET_CONFIGURATION_ERROR".to_owned());
    result?;
    restored
}
