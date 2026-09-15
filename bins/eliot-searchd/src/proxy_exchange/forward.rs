use std::io::Write;

use super::parser::{event_name, FATAL_CHILD_FRAMES};
use super::reply::Reply;

pub(in super::super) fn forward_reply(
    mut read: impl FnMut() -> Result<Option<String>, String>,
    writer: &mut impl Write,
    terminal: impl Fn(&str) -> bool,
    shutdown: bool,
    max_lines: usize,
    max_bytes: usize,
) -> Result<Reply, String> {
    let mut total_bytes = 0_usize;
    for _ in 0..max_lines {
        let line = read()?
            .ok_or_else(|| "LOOPBACK_DIRECT_CHILD_CLOSED_MID_RESPONSE".to_owned())?;
        total_bytes = total_bytes
            .checked_add(line.len())
            .and_then(|bytes| bytes.checked_add(1))
            .filter(|bytes| *bytes <= max_bytes)
            .ok_or_else(|| "LOOPBACK_DIRECT_RESPONSE_BYTES_EXCEEDED".to_owned())?;
        writer
            .write_all(line.as_bytes())
            .and_then(|()| writer.write_all(b"\n"))
            .and_then(|()| writer.flush())
            .map_err(|_| "LOOPBACK_PROXY_WRITE_ERROR".to_owned())?;
        // Exact canonical frames emitted by service_session's fail-stop paths.
        // A fully transmitted OUTCOME_UNKNOWN is terminal for this child; it
        // is never a recoverable command validation error.
        if FATAL_CHILD_FRAMES.contains(&line.as_str()) {
            return Ok(Reply::Fatal);
        }
        // An ordinary command rejection is a complete frame, not channel loss.
        if event_name(&line) == Some("error") {
            return Ok(Reply::Rejected);
        }
        if terminal(&line) {
            return Ok(if shutdown {
                Reply::Shutdown
            } else {
                Reply::Complete
            });
        }
    }
    Err("LOOPBACK_DIRECT_RESPONSE_LINE_LIMIT_EXCEEDED".to_owned())
}
