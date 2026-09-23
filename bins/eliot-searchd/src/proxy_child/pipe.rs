use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

use search_provider_protocol::request::RequestCancellation;

use super::spec::MAX_LINE_BYTES;
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
) -> Result<(), String> {
    check_request(deadline, Some(cancellation))?;
    let original_timeout = socket.write_timeout()
        .map_err(|_| "LOOPBACK_SOCKET_CONFIGURATION_ERROR".to_owned())?;
    let mut writer = RequestWriter {
        inner: DeadlineWriter {
            socket: socket.try_clone()
                .map_err(|_| "LOOPBACK_STREAM_CLONE_ERROR".to_owned())?,
            deadline,
        },
        deadline,
        cancellation: Some(cancellation),
    };
    let result = writer.write_all(line.as_bytes())
        .and_then(|()| writer.write_all(b"\n"))
        .and_then(|()| writer.flush())
        .map_err(|_| "LOOPBACK_PROXY_WRITE_ERROR".to_owned());
    // The endpoint's outer acknowledgement has its own transport policy.
    // Restoring a socket option must never renew this request's deadline.
    let restored = socket.set_write_timeout(original_timeout)
        .map_err(|_| "LOOPBACK_SOCKET_CONFIGURATION_ERROR".to_owned());
    result?;
    restored?;
    check_request(deadline, Some(cancellation))
}
