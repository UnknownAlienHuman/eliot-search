/// Recognizes the fixed event-first header emitted by the owned child.
///
/// A nested event field or payload substring is not a terminal/rejection
/// signal. This is not a general JSON validator or provider-envelope parser.
pub(in super::super) fn event_name(line: &str) -> Option<&str> {
    let (event, tail) = line.strip_prefix("{\"event\":\"")?.split_once('"')?;
    if event.is_empty()
        || !event.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
        })
    {
        return None;
    }
    if tail == "}" || (tail.starts_with(',') && line.ends_with('}')) {
        Some(event)
    } else {
        None
    }
}

/// Closed fail-stop frames emitted by `service_session`.
pub(super) const FATAL_CHILD_FRAMES: &[&str] = &[
    r#"{"event":"error","error":"SERVICE_MUTATION_OUTCOME_UNKNOWN"}"#,
    r#"{"event":"error","error":"SERVICE_COMMAND_LIMIT_INVALID"}"#,
    r#"{"event":"error","error":"SERVICE_COMMAND_TOO_LARGE"}"#,
    r#"{"event":"error","error":"SERVICE_COMMAND_NOT_UTF8"}"#,
    r#"{"event":"error","error":"SERVICE_READ_ERROR"}"#,
];
