use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

use super::spec::MAX_LINE_BYTES;
use super::time::remaining;

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
