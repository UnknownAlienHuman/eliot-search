//! Fail-stop session boundary for the primary DIRECT service.
//!
//! Once a mutation is dispatched, an error cannot prove it had no effects.
//! Do not consume another command or retry output after a failed exchange.

use std::io::{self, BufRead, Write};

use crate::protocol_io::{self, LineError};
use crate::service_output::write_error;

pub(super) const MUTATION_UNKNOWN: &str = "SERVICE_MUTATION_OUTCOME_UNKNOWN";
const OUTPUT_FAILED: &str = "SERVICE_OUTPUT_FAILED";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ServiceControl {
    Continue,
    Stop,
}

/// Armed immediately before a call that can change durable state. This is
/// deliberately not a rollback receipt or a persisted recovery decision.
#[derive(Default)]
pub(super) struct MutationAttempt {
    dispatched: bool,
}

impl MutationAttempt {
    pub(super) fn arm(&mut self) {
        self.dispatched = true;
    }
}

/// Remembers the first output failure even if a caller catches its error.
/// A later successful write cannot make a partial response reusable.
pub(super) struct SessionOutput<'a, W> {
    inner: &'a mut W,
    failed: bool,
}

impl<W: Write> Write for SessionOutput<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.failed {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        let result = self.inner.write(bytes);
        if result.is_err() || (matches!(result, Ok(0)) && !bytes.is_empty()) {
            self.failed = true;
        }
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        let result = self.inner.flush();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}

/// A successful return means EOF/shutdown after only complete exchanges.
/// The owner must discard handles and exit on Err, not resume this reader.
pub(super) fn serve<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    max_command_bytes: usize,
    mut execute: impl FnMut(
        &str,
        &mut SessionOutput<'_, W>,
        &mut MutationAttempt,
    ) -> Result<ServiceControl, String>,
) -> Result<(), String> {
    loop {
        let command = match protocol_io::read_line(reader, max_command_bytes) {
            Ok(Some(command)) => command,
            Ok(None) => return Ok(()),
            Err(error) => {
                let code = match error {
                    LineError::InvalidLimit => "SERVICE_COMMAND_LIMIT_INVALID",
                    LineError::TooLarge => "SERVICE_COMMAND_TOO_LARGE",
                    LineError::InvalidUtf8 => "SERVICE_COMMAND_NOT_UTF8",
                    LineError::Io(_) => "SERVICE_READ_ERROR",
                };
                write_error(writer, code)?;
                return Err(code.to_owned());
            }
        };
        let mut attempt = MutationAttempt::default();
        let mut output = SessionOutput {
            inner: &mut *writer,
            failed: false,
        };
        let result = execute(&command, &mut output, &mut attempt);
        if output.failed {
            // There may already be a partial JSON frame on the channel. Do
            // not append an error frame or consume a following request.
            return Err(if attempt.dispatched {
                MUTATION_UNKNOWN
            } else {
                OUTPUT_FAILED
            }
            .to_owned());
        }
        match result {
            Ok(ServiceControl::Continue) => {}
            Ok(ServiceControl::Stop) => return Ok(()),
            Err(_) if attempt.dispatched => {
                // Legacy storage errors are strings, not trustworthy no-effect
                // receipts. Classify conservatively without inspecting wording.
                let _ = write_error(&mut output, MUTATION_UNKNOWN);
                return Err(MUTATION_UNKNOWN.to_owned());
            }
            Err(error) => write_error(&mut output, &error)?,
        }
    }
}

#[cfg(test)]
#[path = "service_session_tests.rs"]
mod tests;
