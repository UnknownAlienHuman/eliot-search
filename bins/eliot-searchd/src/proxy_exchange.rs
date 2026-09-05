//! A child stream is reusable only after its entire response is consumed.

use std::io::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Reply {
    Complete,
    Rejected,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ExchangeFence {
    blocked: bool,
}

impl ExchangeFence {
    pub(super) fn blocked(self) -> bool {
        self.blocked
    }

    /// Arm before writing any command byte. Every early return stays blocked.
    /// Only a fully consumed response can release the fence; no write is retried.
    pub(super) fn run(
        &mut self,
        exchange: impl FnOnce() -> Result<Reply, String>,
    ) -> Result<Reply, String> {
        if self.blocked {
            return Err("LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART".to_owned());
        }
        self.blocked = true;
        let reply = exchange()?;
        if reply != Reply::Shutdown {
            self.blocked = false;
        }
        Ok(reply)
    }
}

pub(super) fn forward_reply(
    mut read: impl FnMut() -> Result<Option<String>, String>,
    writer: &mut impl Write,
    terminal: impl Fn(&str) -> bool,
    shutdown: bool,
    max_lines: usize,
) -> Result<Reply, String> {
    for _ in 0..max_lines {
        let line = read()?
            .ok_or_else(|| "LOOPBACK_DIRECT_CHILD_CLOSED_MID_RESPONSE".to_owned())?;
        writer.write_all(line.as_bytes())
            .and_then(|()| writer.write_all(b"\n"))
            .and_then(|()| writer.flush())
            .map_err(|_| "LOOPBACK_PROXY_WRITE_ERROR".to_owned())?;
        // An ordinary command rejection is a complete frame, not channel loss.
        if line.contains("\"event\":\"error\"") {
            return Ok(Reply::Rejected);
        }
        if terminal(&line) {
            return Ok(if shutdown { Reply::Shutdown } else { Reply::Complete });
        }
    }
    Err("LOOPBACK_DIRECT_RESPONSE_LINE_LIMIT_EXCEEDED".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    struct Disconnected;
    impl Write for Disconnected {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected disconnect"))
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    #[test]
    fn disconnect_blocks_next_request_without_consuming_old_reply() {
        let mut fence = ExchangeFence::default();
        let mut old_reply = ["match", "complete"].into_iter();
        let mut reads = 0;
        let result = fence.run(|| forward_reply(
            || { reads += 1; Ok(old_reply.next().map(str::to_owned)) },
            &mut Disconnected, |line| line == "complete", false, 10,
        ));
        assert!(result.is_err());
        assert!(fence.blocked());
        assert_eq!(reads, 1);
        let mut new_command_written = false;
        assert!(fence.run(|| {
            new_command_written = true;
            Ok(Reply::Complete)
        }).is_err());
        assert!(!new_command_written);
        assert_eq!(old_reply.next(), Some("complete"));
    }

    #[test]
    fn incomplete_command_write_cannot_be_replayed_automatically() {
        let mut fence = ExchangeFence::default();
        assert!(fence.run(|| Err("write failed after prefix".to_owned())).is_err());
        assert!(fence.blocked());
        assert!(fence.run(|| panic!("must not retry uncertain command")).is_err());
    }

    #[test]
    fn complete_response_releases_fence_and_stops_at_its_terminal() {
        let mut lines = ["match", "complete", "next-response"].into_iter();
        let mut output = Vec::new();
        let mut fence = ExchangeFence::default();
        assert_eq!(fence.run(|| forward_reply(
            || Ok(lines.next().map(str::to_owned)), &mut output,
            |line| line == "complete", false, 10,
        )).unwrap(), Reply::Complete);
        assert!(!fence.blocked());
        assert_eq!(output, b"match\ncomplete\n");
        assert_eq!(lines.next(), Some("next-response"));
    }

    #[test]
    fn fully_forwarded_command_rejection_does_not_poison_stream() {
        let mut fence = ExchangeFence::default();
        let mut output = Vec::new();
        assert_eq!(fence.run(|| forward_reply(
            || Ok(Some("{\"event\":\"error\"}".to_owned())), &mut output,
            |_| false, false, 1,
        )).unwrap(), Reply::Rejected);
        assert!(!fence.blocked());
    }

    #[test]
    fn eof_and_exhausted_line_budget_leave_stream_blocked() {
        for eof in [false, true] {
            let mut fence = ExchangeFence::default();
            assert!(fence.run(|| forward_reply(
                || Ok(if eof { None } else { Some("not-terminal".to_owned()) }),
                &mut Vec::new(), |_| false, false, 2,
            )).is_err());
            assert!(fence.blocked());
        }
    }

    #[test]
    fn shutdown_never_reopens_the_exchange_fence() {
        let mut fence = ExchangeFence::default();
        assert_eq!(fence.run(|| Ok(Reply::Shutdown)).unwrap(), Reply::Shutdown);
        assert!(fence.blocked());
    }
}
