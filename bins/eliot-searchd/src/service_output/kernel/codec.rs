use std::io::Write;

pub const MAX_RESPONSE_BYTES: usize = 64 * 1024;

pub fn write_line(writer: &mut impl Write, value: &str) -> Result<(), String> {
    if value.len() > MAX_RESPONSE_BYTES {
        return Err("SERVICE_RESPONSE_TOO_LARGE".to_owned());
    }
    writer
        .write_all(value.as_bytes())
        .and_then(|()| writer.write_all(b"\n"))
        .and_then(|()| writer.flush())
        .map_err(|error| format!("SERVICE_WRITE_ERROR:{error}"))
}

pub fn write_error(writer: &mut impl Write, error: &str) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            "{{\"event\":\"error\",\"error\":{}}}",
            json_string(&eliot_searchd::diagnostics::sanitize_code(error))
        ),
    )
}

pub fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(&mut output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}
