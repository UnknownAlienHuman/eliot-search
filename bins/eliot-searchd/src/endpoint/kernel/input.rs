//! Single-reader, bounded lookahead during synchronous provider execution.

use std::collections::VecDeque;
use std::io::BufReader;
use std::net::TcpStream;
use std::time::Duration;

use search_contracts::MAX_PROTOCOL_IN_FLIGHT;

use super::spec::{MAX_COMMAND_LINE_BYTES, READ_TIMEOUT};
use super::wire::{LineRead, SocketDeadline, redacted_io_error};

const INPUT_POLL: Duration = Duration::from_millis(1);
const MAX_QUEUED_BYTES: usize = MAX_COMMAND_LINE_BYTES * MAX_PROTOCOL_IN_FLIGHT;

/// Connection-owned input, including bytes prefetched during pairing.
///
/// Lookahead never admits or executes a request. Complete non-control commands
/// retain FIFO order in a bounded queue; partial framing and its original clock
/// survive return to ordinary dispatch. No second socket reader is created.
pub struct EndpointInput {
    reader: BufReader<TcpStream>,
    queued: VecDeque<String>,
    queued_bytes: usize,
    partial: Vec<u8>,
    frame_deadline: Option<SocketDeadline>,
}

impl EndpointInput {
    pub(super) fn new(reader: BufReader<TcpStream>) -> Self {
        Self {
            reader,
            queued: VecDeque::new(),
            queued_bytes: 0,
            partial: Vec::new(),
            frame_deadline: None,
        }
    }

    pub(super) fn read_command(&mut self) -> Result<Option<String>, String> {
        if let Some(command) = self.queued.pop_front() {
            self.queued_bytes -= command.len();
            return Ok(Some(command));
        }
        loop {
            match self.step(READ_TIMEOUT, true)? {
                LineRead::Pending => {}
                LineRead::Eof => return Ok(None),
                LineRead::Complete(command) => return Ok(Some(command)),
            }
        }
    }

    /// Polls a paired connection without executing its pending requests.
    ///
    /// `matches` must identify only a control action for the current admitted
    /// request. A match is a cancellation observation, not acknowledgement or
    /// proof that work stopped. All complete commands, including matches, remain
    /// queued in FIFO order for normal dispatch if the current exchange safely
    /// drains. The execution owner decides whether cancellation requires abort.
    ///
    /// A short idle poll is harmless. EOF, malformed/expired framing and queue
    /// overflow fail closed. The queue holds at most 32 frames / 4 MiB under the
    /// existing protocol ceilings, plus one partial frame and the input buffer.
    ///
    /// # Errors
    ///
    /// Returns a closed endpoint error on EOF, invalid framing, expiry or full
    /// lookahead capacity. The current exchange must not be reused on failure.
    pub fn poll_control(
        &mut self,
        mut matches: impl FnMut(&str) -> bool,
    ) -> Result<bool, String> {
        // A cancel may already have been read while an earlier request ran.
        // Reconsider it only after its exact target has actually been admitted.
        let queued_match = self.queued.iter().any(|command| matches(command));
        // Keep observing EOF/framing even after a matching cancel was queued;
        // otherwise a queued cancel would mask disconnect until the drain times out.
        match self.step(INPUT_POLL, false)? {
            LineRead::Pending => Ok(queued_match),
            LineRead::Eof => Err("ENDPOINT_CONNECTION_CLOSED".to_owned()),
            LineRead::Complete(command) => {
                let matched = queued_match || matches(&command);
                let bytes = self.queued_bytes.checked_add(command.len())
                    .filter(|bytes| *bytes <= MAX_QUEUED_BYTES)
                    .ok_or_else(|| "ENDPOINT_INPUT_QUEUE_EXHAUSTED".to_owned())?;
                if self.queued.len() >= MAX_PROTOCOL_IN_FLIGHT {
                    return Err("ENDPOINT_INPUT_QUEUE_EXHAUSTED".to_owned());
                }
                self.queued.push_back(command);
                self.queued_bytes = bytes;
                Ok(matched)
            }
        }
    }

    fn step(&mut self, wait: Duration, keep_idle: bool) -> Result<LineRead, String> {
        if self.frame_deadline.is_none() {
            self.frame_deadline = Some(SocketDeadline::new(READ_TIMEOUT)
                .map_err(|error| redacted_io_error("ENDPOINT_TIMEOUT_CONFIGURATION_ERROR", &error))?);
        }
        let result = self.frame_deadline.as_ref()
            .expect("frame deadline installed")
            .read_step(&mut self.reader, &mut self.partial, MAX_COMMAND_LINE_BYTES, wait)?;
        // Silence during an active long-running request is not an idle command
        // timeout. Once any prefix is consumed, however, its deadline is fixed.
        if !matches!(&result, LineRead::Pending) || (!keep_idle && self.partial.is_empty()) {
            self.frame_deadline = None;
        }
        Ok(result)
    }
}
