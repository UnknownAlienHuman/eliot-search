use super::*;

fn input(text: &str, lines: Vec<SourceLineSpan>) -> UnitizationInput {
    UnitizationInput::new(
        OpaqueId::new("source:test").expect("source"),
        NonZeroRevision::new(1).expect("revision"),
        Blake3Digest32::from_bytes([1; 32]),
        text.to_owned(),
        lines,
        Some(ReceiptRef::new("receipt:materialization").expect("receipt")),
    )
}

fn simple_lines(text: &str) -> Vec<SourceLineSpan> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        let (content_end, end) = if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            (index, index + 2)
        } else if matches!(bytes[index], b'\n' | b'\r') {
            (index, index + 1)
        } else {
            index += 1;
            continue;
        };
        spans.push(SourceLineSpan {
            line_index: u64::try_from(spans.len()).unwrap(),
            source_start: u64::try_from(start).unwrap(),
            source_end: u64::try_from(end).unwrap(),
            content_end: u64::try_from(content_end).unwrap(),
        });
        start = end;
        index = end;
    }
    if start < bytes.len() {
        spans.push(SourceLineSpan {
            line_index: u64::try_from(spans.len()).unwrap(),
            source_start: u64::try_from(start).unwrap(),
            source_end: u64::try_from(bytes.len()).unwrap(),
            content_end: u64::try_from(bytes.len()).unwrap(),
        });
    }
    spans
}

fn limits(preferred: usize, maximum: usize) -> UnitizationLimits {
    UnitizationLimits {
        preferred_unit_bytes: preferred,
        max_unit_bytes: maximum,
        ..DEFAULT_UNITIZATION_LIMITS
    }
}

mod layout_cases;
mod manifest_cases;
