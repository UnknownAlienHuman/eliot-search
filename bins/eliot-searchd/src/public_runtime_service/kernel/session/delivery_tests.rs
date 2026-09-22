//! Exercises the real fail-stop session loop with in-memory I/O and callbacks.

use std::io::{Cursor, Write};

use super::{MUTATION_UNKNOWN, OUTPUT_FAILED, ServiceControl, serve};

#[test]
fn logical_failure_after_output_does_not_append_error_or_consume_next_request() {
    let mut reader = Cursor::new(b"first\nsecond\n");
    let mut writer = Vec::new();
    let mut calls = 0;
    let result = serve(&mut reader, &mut writer, 1024, |_, output, _| {
        calls += 1;
        output.write_all(b"{\"event\":\"search_page_started\"}\n").unwrap();
        Err("SERVICE_RESPONSE_TOO_LARGE".into())
    });
    assert_eq!(result, Err(OUTPUT_FAILED.to_owned()));
    assert_eq!(calls, 1);
    assert_eq!(reader.position(), 6);
    assert_eq!(writer.as_slice(), b"{\"event\":\"search_page_started\"}\n".as_slice());
}

#[test]
fn partial_mutation_output_keeps_unknown_outcome_without_an_extra_frame() {
    let mut reader = Cursor::new(b"first\nsecond\n");
    let mut writer = Vec::new();
    let mut calls = 0;
    let result = serve(&mut reader, &mut writer, 1024, |_, output, attempt| {
        calls += 1;
        attempt.arm();
        output.write_all(b"partial").unwrap();
        Err("MUTATION_RECEIPT_FORMAT_FAILED".into())
    });
    assert_eq!(result, Err(MUTATION_UNKNOWN.to_owned()));
    assert_eq!(calls, 1);
    assert_eq!(reader.position(), 6);
    assert_eq!(writer.as_slice(), b"partial".as_slice());
}

#[test]
fn pre_output_validation_error_can_be_followed_by_a_new_complete_exchange() {
    let mut reader = Cursor::new(b"invalid\nvalid\n");
    let mut writer = Vec::new();
    let mut calls = 0;
    let result = serve(&mut reader, &mut writer, 1024, |_, output, _| {
        calls += 1;
        if calls == 1 {
            return Err("SERVICE_RESPONSE_TOO_LARGE".into());
        }
        output.write_all(b"complete\n").unwrap();
        Ok(ServiceControl::Stop)
    });
    assert_eq!(result, Ok(()));
    assert_eq!(calls, 2);
    let rendered = String::from_utf8(writer).unwrap();
    assert!(rendered.contains("SERVICE_RESPONSE_TOO_LARGE"));
    assert!(rendered.ends_with("complete\n"));
}

#[test]
fn successful_output_does_not_mark_the_next_command_as_started() {
    let mut reader = Cursor::new(b"valid\ninvalid\nstop\n");
    let mut writer = Vec::new();
    let mut calls = 0;
    let result = serve(&mut reader, &mut writer, 1024, |_, output, _| {
        calls += 1;
        match calls {
            1 => {
                output.write_all(b"complete\n").unwrap();
                Ok(ServiceControl::Continue)
            }
            2 => Err("SERVICE_ARGUMENT_INVALID".into()),
            _ => Ok(ServiceControl::Stop),
        }
    });
    assert_eq!(result, Ok(()));
    assert_eq!(calls, 3);
    let rendered = String::from_utf8(writer).unwrap();
    assert!(rendered.starts_with("complete\n"));
    assert!(rendered.contains("SERVICE_ARGUMENT_INVALID"));
}

#[test]
fn zero_length_successful_write_and_flush_do_not_start_a_response() {
    let mut reader = Cursor::new(b"invalid\nstop\n");
    let mut writer = Vec::new();
    let mut calls = 0;
    let result = serve(&mut reader, &mut writer, 1024, |_, output, _| {
        calls += 1;
        if calls == 1 {
            assert_eq!(output.write(b"").unwrap(), 0);
            output.flush().unwrap();
            Err("SERVICE_ARGUMENT_INVALID".into())
        } else {
            Ok(ServiceControl::Stop)
        }
    });
    assert_eq!(result, Ok(()));
    assert_eq!(calls, 2);
    assert!(String::from_utf8(writer).unwrap().contains("SERVICE_ARGUMENT_INVALID"));
}
