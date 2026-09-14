use std::io::Cursor;

use crate::development::Health;

use super::protocol::serve_control;
use super::spec::MAX_COMMAND_BYTES;

#[test]
fn oversized_frame_terminates_session_without_executing_suffix() {
    let mut bytes = vec![b'x'; MAX_COMMAND_BYTES + 3];
    bytes.extend_from_slice(b"\nshutdown\n");
    let mut input = Cursor::new(bytes);
    let mut output = Vec::new();
    assert!(serve_control(Health::SHELL, &mut input, &mut output).is_err());
    let response = String::from_utf8(output).unwrap();
    assert!(response.contains("COMMAND_TOO_LARGE"));
    assert!(!response.contains("draining"));
    assert!(!response.contains("stopped"));
    assert_eq!(input.position(), (MAX_COMMAND_BYTES + 2) as u64);
}

#[test]
fn valid_control_session_reports_health_and_clean_shutdown() {
    let mut input = Cursor::new(b"health\r\nversion\nshutdown\n");
    let mut output = Vec::new();
    serve_control(Health::SHELL, &mut input, &mut output).unwrap();
    let response = String::from_utf8(output).unwrap();
    assert_eq!(response.lines().count(), 5);
    assert!(response.contains("\"source_backed_search_available\":false"));
    assert!(response.contains("\"clean\":true"));
}
