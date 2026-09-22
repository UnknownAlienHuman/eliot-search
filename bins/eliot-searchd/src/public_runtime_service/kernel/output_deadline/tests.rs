//! Deterministic output-clock faults; no sleeps or native transport claims.

use super::*;
use super::super::session::{ServiceControl, serve};
use crate::service_output::write_line;
use std::cell::Cell;
use std::io::Cursor;
use std::rc::Rc;
use std::time::Duration;

const EXPIRED: &str = "DIRECT_RESULT_HANDLE_EXPIRED";

struct SlowWriter {
    clock: Rc<Cell<Instant>>,
    deadline: Instant,
    bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
    max_write: usize,
    expire_on_write: Option<usize>,
    expire_on_flush: Option<usize>,
}

impl SlowWriter {
    fn new() -> Self {
        let before = Instant::now();
        Self {
            clock: Rc::new(Cell::new(before)),
            deadline: before + Duration::from_secs(60),
            bytes: Vec::new(),
            writes: 0,
            flushes: 0,
            max_write: usize::MAX,
            expire_on_write: None,
            expire_on_flush: None,
        }
    }
}

impl Write for SlowWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        let count = bytes.len().min(self.max_write);
        self.bytes.extend_from_slice(&bytes[..count]);
        if self.expire_on_write == Some(self.writes) {
            self.clock.set(self.deadline);
        }
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.expire_on_flush == Some(self.flushes) {
            self.clock.set(self.deadline);
        }
        Ok(())
    }
}

#[test]
fn earliest_original_deadline_and_empty_batch_semantics() {
    let early = Instant::now();
    let late = early + Duration::from_secs(1);
    let continuation = ContinuationError::Expired.code();
    let handle = ResultHandleError::Expired.code();
    assert_eq!(page_deadline(None, None), None);
    assert_eq!(page_deadline(Some(early), None), Some((early, continuation)));
    assert_eq!(page_deadline(None, Some(early)), Some((early, handle)));
    assert_eq!(page_deadline(Some(early), Some(late)), Some((early, continuation)));
    assert_eq!(page_deadline(Some(late), Some(early)), Some((early, handle)));
    assert_eq!(page_deadline(Some(early), Some(early)), Some((early, continuation)));
}

#[test]
fn exact_expiry_refuses_the_first_write_without_touching_the_inner_writer() {
    let mut writer = SlowWriter::new();
    let deadline = writer.deadline;
    let result = with_clock(&mut writer, Some((deadline, EXPIRED)), || deadline, |out| {
        write_line(out, "must-not-appear")
    });
    assert_eq!(result, Err(EXPIRED.to_owned()));
    assert!(writer.bytes.is_empty());
    assert_eq!((writer.writes, writer.flushes), (0, 0));
}

#[test]
fn every_short_write_retry_rechecks_the_deadline() {
    let mut writer = SlowWriter::new();
    writer.max_write = 1;
    writer.expire_on_write = Some(1);
    let clock = Rc::clone(&writer.clock);
    let deadline = writer.deadline;
    let result = with_clock(&mut writer, Some((deadline, EXPIRED)), || clock.get(), |out| {
        write_line(out, "abcd")
    });
    assert_eq!(result, Err(EXPIRED.to_owned()));
    assert_eq!(writer.bytes, b"a");
    assert_eq!((writer.writes, writer.flushes), (1, 0));
}

#[test]
fn expiry_between_frames_prevents_the_next_frame() {
    let mut writer = SlowWriter::new();
    writer.expire_on_flush = Some(1);
    let clock = Rc::clone(&writer.clock);
    let deadline = writer.deadline;
    let result = with_clock(&mut writer, Some((deadline, EXPIRED)), || clock.get(), |out| {
        write_line(out, "first")?;
        write_line(out, "second")
    });
    assert_eq!(result, Err(EXPIRED.to_owned()));
    assert_eq!(writer.bytes, b"first\n");
    assert_eq!((writer.writes, writer.flushes), (2, 1));
}

#[test]
fn expiry_before_flush_does_not_start_a_new_flush() {
    let mut writer = SlowWriter::new();
    writer.expire_on_write = Some(2);
    let clock = Rc::clone(&writer.clock);
    let deadline = writer.deadline;
    let result = with_clock(&mut writer, Some((deadline, EXPIRED)), || clock.get(), |out| {
        write_line(out, "frame")
    });
    assert_eq!(result, Err(EXPIRED.to_owned()));
    assert_eq!(writer.bytes, b"frame\n");
    assert_eq!(writer.flushes, 0);
}

#[test]
fn a_successful_final_flush_is_not_retroactively_rejected() {
    let mut writer = SlowWriter::new();
    writer.expire_on_flush = Some(1);
    let clock = Rc::clone(&writer.clock);
    let deadline = writer.deadline;
    let result = with_clock(&mut writer, Some((deadline, EXPIRED)), || clock.get(), |out| {
        write_line(out, "complete")
    });
    assert_eq!(result, Ok(()));
    assert_eq!(writer.clock.get(), deadline);
    assert_eq!(writer.bytes, b"complete\n");
    assert_eq!(writer.flushes, 1);
}

#[test]
fn swallowed_expiry_and_a_regressed_test_clock_cannot_resume_output() {
    let mut writer = SlowWriter::new();
    let clock = Rc::clone(&writer.clock);
    let before = clock.get();
    let deadline = writer.deadline;
    clock.set(deadline);
    let result = with_clock(&mut writer, Some((deadline, EXPIRED)), || clock.get(), |out| {
        assert_eq!(out.write(b"first").unwrap_err().kind(), io::ErrorKind::TimedOut);
        clock.set(before);
        assert!(out.write(b"second").is_err());
        assert!(out.flush().is_err());
        Ok(())
    });
    assert_eq!(result, Err(EXPIRED.to_owned()));
    assert_eq!((writer.writes, writer.flushes), (0, 0));
}

#[test]
fn no_deadline_does_not_observe_the_clock_or_rewrite_a_logical_error() {
    let mut bytes = Vec::new();
    let result = with_clock(&mut bytes, None, || panic!("no locator lifetime"), |out| {
        write_line(out, "normal")?;
        Err("original-logical-error".to_owned())
    });
    assert_eq!(result, Err("original-logical-error".to_owned()));
    assert_eq!(bytes, b"normal\n");
}

#[test]
fn actual_session_stops_after_deadline_truncates_an_exchange() {
    for mutation in [false, true] {
        let mut reader = Cursor::new(b"first\nnext\n");
        let mut writer = SlowWriter::new();
        writer.expire_on_flush = Some(1);
        let clock = Rc::clone(&writer.clock);
        let deadline = writer.deadline;
        let result = serve(&mut reader, &mut writer, 32, |command, out, attempt| {
            assert_eq!(command, "first", "the next request must not be consumed");
            if mutation {
                attempt.arm();
            }
            with_clock(out, Some((deadline, EXPIRED)), || clock.get(), |out| {
                write_line(out, "started")?;
                write_line(out, "complete")
            })?;
            Ok(ServiceControl::Continue)
        });
        let expected = if mutation {
            "SERVICE_MUTATION_OUTCOME_UNKNOWN"
        } else {
            "SERVICE_OUTPUT_FAILED"
        };
        assert_eq!(result, Err(expected.to_owned()));
        assert_eq!(reader.position(), 6);
        assert_eq!(writer.bytes, b"started\n");
        assert_eq!((writer.writes, writer.flushes), (2, 1));
    }
}

#[test]
fn actual_session_keeps_pre_output_expiry_recoverable() {
    let mut reader = Cursor::new(b"expired\nshutdown\n");
    let mut bytes = Vec::new();
    let deadline = Instant::now();
    let mut calls = 0;
    serve(&mut reader, &mut bytes, 32, |command, out, _| {
        calls += 1;
        if command == "shutdown" {
            return Ok(ServiceControl::Stop);
        }
        with_clock(out, Some((deadline, EXPIRED)), || deadline, |out| {
            write_line(out, "must-not-appear")
        })?;
        Ok(ServiceControl::Continue)
    }).unwrap();
    assert_eq!(calls, 2);
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains(EXPIRED));
    assert!(!text.contains("must-not-appear"));
    assert!(!text.contains("SERVICE_OUTPUT_FAILED"));
}
