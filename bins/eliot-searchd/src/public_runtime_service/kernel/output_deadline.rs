//! Enforce original locator lifetimes at every output operation.
//!
//! This is not a timer that can interrupt an already-blocked writer. No new
//! write or flush begins after observed expiry. A partial exchange is handled
//! by the surrounding fail-stop session; successful output is not rolled back.

use std::io::{self, Write};
use std::time::Instant;

use crate::continuation::ContinuationError;
use crate::result_handles::ResultHandleError;

/// The earlier original page/handle deadline wins, including final pages.
/// No handle deadline applies to an empty batch. Rounded TTLs are not clocks.
pub(super) fn page_deadline(
    page: Option<Instant>,
    handles: Option<Instant>,
) -> Option<(Instant, &'static str)> {
    page.map(|at| (at, ContinuationError::Expired.code()))
        .into_iter()
        .chain(handles.map(|at| (at, ResultHandleError::Expired.code())))
        .min_by_key(|(at, _)| *at)
}

pub(super) fn with_deadline<W: Write>(
    writer: &mut W,
    deadline: Option<(Instant, &'static str)>,
    emit: impl FnOnce(&mut DeadlineOutput<'_, W>) -> Result<(), String>,
) -> Result<(), String> {
    with_clock(writer, deadline, Instant::now, emit)
}

fn with_clock<W: Write>(
    writer: &mut W,
    deadline: Option<(Instant, &'static str)>,
    mut now: impl FnMut() -> Instant,
    emit: impl FnOnce(&mut DeadlineOutput<'_, W>) -> Result<(), String>,
) -> Result<(), String> {
    let mut output = DeadlineOutput {
        inner: writer,
        deadline,
        now: &mut now,
        expired: None,
    };
    let result = emit(&mut output);
    // The emitter cannot turn a caught expiry error into a successful commit.
    output.expired.map_or(result, |code| Err(code.to_owned()))
}

/// Borrowed output adapter. It buffers nothing and retains no token or bytes.
pub(super) struct DeadlineOutput<'a, W> {
    inner: &'a mut W,
    deadline: Option<(Instant, &'static str)>,
    now: &'a mut dyn FnMut() -> Instant,
    expired: Option<&'static str>,
}

impl<W> DeadlineOutput<'_, W> {
    fn check(&mut self) -> io::Result<()> {
        if self.expired.is_none()
            && let Some((deadline, code)) = self.deadline
            && (self.now)() >= deadline
        {
            self.expired = Some(code);
        }
        if self.expired.is_some() {
            Err(io::ErrorKind::TimedOut.into())
        } else {
            Ok(())
        }
    }
}

impl<W: Write> Write for DeadlineOutput<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.check()?;
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.check()?;
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests;
