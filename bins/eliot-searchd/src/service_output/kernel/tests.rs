use super::{MAX_RESPONSE_BYTES, write_error, write_line};

#[test]
fn error_frames_carry_closed_codes_only() {
    for code in [
        "SERVICE_COMMAND_TOO_LARGE",
        "SERVICE_MUTATION_OUTCOME_UNKNOWN",
        "SERVICE_STATUS_TOO_LARGE",
    ] {
        let mut output = Vec::new();
        write_error(&mut output, code).expect("bounded error frame");
        assert_eq!(
            String::from_utf8(output).expect("error frame is UTF-8"),
            format!("{{\"event\":\"error\",\"error\":\"{code}\"}}\n"),
        );
    }
}

#[test]
fn error_frames_redact_suffixes_paths_and_secrets() {
    for (input, expected) in [
        (
            "SERVICE_HEX_INVALID:C:\\temp\\corpus\\secret.txt (os error 3)",
            "SERVICE_HEX_INVALID",
        ),
        (
            "SOURCE_ADMISSION_DENIED:query bytes needle-alpha",
            "SOURCE_ADMISSION_DENIED",
        ),
        ("bearer-token-abc123", "______-_____-___123"),
    ] {
        let mut output = Vec::new();
        write_error(&mut output, input).expect("bounded error frame");
        let frame = String::from_utf8(output).expect("error frame is UTF-8");
        assert_eq!(
            frame,
            format!("{{\"event\":\"error\",\"error\":\"{expected}\"}}\n"),
            "input={input:?}",
        );
        assert!(!frame.contains("secret"), "input={input:?}");
        assert!(!frame.contains("needle"), "input={input:?}");
    }
}

#[test]
fn response_ceiling_is_typed_and_bounded() {
    let mut output = Vec::new();
    assert_eq!(
        write_line(&mut output, &"m".repeat(MAX_RESPONSE_BYTES + 1)),
        Err("SERVICE_RESPONSE_TOO_LARGE".to_owned())
    );
    assert!(output.is_empty(), "oversize responses emit nothing");
}
