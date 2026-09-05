use super::*;
use std::io::Cursor;

#[test]
fn rejected_validation_does_not_poison_a_complete_exchange() {
    let mut reader = Cursor::new(b"invalid\nhealth\nshutdown\n");
    let mut output = Vec::new();
    let mut calls = Vec::new();
    serve(&mut reader, &mut output, 32, |command, writer, _| {
        calls.push(command.to_owned());
        match command {
            "invalid" => Err("SERVICE_COMMAND_INVALID".to_owned()),
            "shutdown" => Ok(ServiceControl::Stop),
            _ => {
                writeln!(writer, "health").map_err(|_| "write".to_owned())?;
                Ok(ServiceControl::Continue)
            }
        }
    })
    .unwrap();
    assert_eq!(calls, ["invalid", "health", "shutdown"]);
}

#[test]
fn failed_mutation_never_consumes_the_next_command_or_exposes_error_details() {
    let mut reader = Cursor::new(b"mutate\nhealth\ngc\n");
    let mut output = Vec::new();
    let mut effects = Vec::new();
    let result = serve(&mut reader, &mut output, 32, |_, _, attempt| {
        attempt.arm();
        effects.push("effect before acknowledgement");
        Err("PRIVATE_PATH_AND_CONTENT_SENTINEL".to_owned())
    });
    assert_eq!(result, Err(MUTATION_UNKNOWN.to_owned()));
    assert_eq!(effects.len(), 1);
    assert_eq!(reader.position(), 7);
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(MUTATION_UNKNOWN));
    assert!(!output.contains("PRIVATE_PATH_AND_CONTENT_SENTINEL"));
}

#[test]
fn acknowledged_mutation_does_not_poison_the_next_validation_error() {
    let mut reader = Cursor::new(b"mutate\ninvalid\nshutdown\n");
    let mut output = Vec::new();
    let mut calls = 0;
    serve(&mut reader, &mut output, 32, |command, _, attempt| {
        calls += 1;
        match command {
            "mutate" => {
                attempt.arm();
                Ok(ServiceControl::Continue)
            }
            "invalid" => Err("SERVICE_COMMAND_INVALID".to_owned()),
            _ => Ok(ServiceControl::Stop),
        }
    })
    .unwrap();
    assert_eq!(calls, 3);
}

#[test]
fn oversized_frame_is_not_drained_or_followed_by_a_second_command() {
    let mut bytes = vec![b'x'; 1_000_000];
    bytes.extend_from_slice(b"\nhealth\n");
    let mut reader = Cursor::new(bytes);
    let mut output = Vec::new();
    let result = serve(&mut reader, &mut output, 1024, |_, _, _| {
        panic!("an invalid frame must never be dispatched")
    });
    assert_eq!(result, Err("SERVICE_COMMAND_TOO_LARGE".to_owned()));
    assert!(reader.position() <= 1026);
}

#[test]
fn invalid_utf8_terminates_the_session_without_dispatch() {
    let mut reader = Cursor::new(b"\xff\nhealth\n");
    let mut output = Vec::new();
    let result = serve(&mut reader, &mut output, 32, |_, _, _| {
        panic!("invalid UTF-8 must not be repaired or dispatched")
    });
    assert_eq!(result, Err("SERVICE_COMMAND_NOT_UTF8".to_owned()));
    assert_eq!(reader.position(), 2);
}

struct FailingOutput {
    writes: usize,
    zero: bool,
    fail_flush: bool,
}

impl Write for FailingOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.fail_flush {
            Ok(bytes.len())
        } else if self.zero {
            Ok(0)
        } else if self.writes == 1 {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        } else {
            Ok(bytes.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        } else {
            Ok(())
        }
    }
}

#[test]
fn output_failure_is_latched_even_when_dispatch_ignores_it() {
    for (zero, fail_flush) in [(false, false), (true, false), (false, true)] {
        let mut reader = Cursor::new(b"first\nsecond\n");
        let mut output = FailingOutput { writes: 0, zero, fail_flush };
        let result = serve(&mut reader, &mut output, 32, |_, writer, _| {
            let _ = writer.write_all(b"prefix");
            let _ = writer.flush();
            let _ = writer.write_all(b"must-not-be-written");
            Ok(ServiceControl::Continue)
        });
        assert_eq!(result, Err(OUTPUT_FAILED.to_owned()));
        assert_eq!(reader.position(), 6);
        assert_eq!(output.writes, 1);
    }
}

#[test]
fn failed_mutation_output_is_unknown_not_a_clean_shutdown() {
    let mut reader = Cursor::new(b"mutate\nshutdown\n");
    let mut output = FailingOutput { writes: 0, zero: false, fail_flush: false };
    let result = serve(&mut reader, &mut output, 32, |_, writer, attempt| {
        attempt.arm();
        let _ = writer.write_all(b"ack");
        Ok(ServiceControl::Continue)
    });
    assert_eq!(result, Err(MUTATION_UNKNOWN.to_owned()));
    assert_eq!(output.writes, 1);
    assert_eq!(reader.position(), 7);
}
