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

#[test]
fn exact_reconstruction_has_no_gaps_or_overlap() {
    let text = "alpha\r\nbeta\nγamma";
    let result = unitize(input(text, simple_lines(text)), limits(7, 12)).unwrap();
    assert_eq!(result.units.iter().map(TextUnit::text).collect::<String>(), text);
    assert_eq!(result.receipt.input_bytes, result.receipt.emitted_bytes);
}

#[test]
fn line_boundaries_are_preferred_when_line_fits_hard_limit() {
    let text = "one\ntwo\nthree\n";
    let result = unitize(input(text, simple_lines(text)), limits(5, 10)).unwrap();
    assert_eq!(result.units[0].text(), "one\n");
    assert!(result.units[0].ends_at_line_boundary);
    assert_eq!(result.units[1].text(), "two\n");
}

#[test]
fn overlong_unicode_line_splits_only_at_character_boundaries() {
    let text = "αβγδεζηθ";
    let result = unitize(input(text, simple_lines(text)), limits(5, 6)).unwrap();
    assert!(result.units.len() > 1);
    assert!(result.units.iter().all(|unit| unit.len() <= 6));
    assert_eq!(result.units.iter().map(TextUnit::text).collect::<String>(), text);
}

#[test]
fn crlf_is_never_split_when_the_line_fits_hard_limit() {
    let text = "abcd\r\nef";
    let result = unitize(input(text, simple_lines(text)), limits(5, 8)).unwrap();
    assert_eq!(result.units[0].text(), "abcd\r\n");
}

#[test]
fn output_is_deterministic() {
    let text = "a\nbb\nccc\ndddd";
    let first = unitize(input(text, simple_lines(text)), limits(4, 7)).unwrap();
    let second = unitize(input(text, simple_lines(text)), limits(4, 7)).unwrap();
    assert_eq!(first, second);
}

#[test]
fn malformed_line_coverage_is_rejected() {
    let lines = vec![SourceLineSpan { line_index: 0, source_start: 1, source_end: 3, content_end: 3 }];
    assert_eq!(unitize(input("abc", lines), limits(2, 3)), Err(UnitizationError::InvalidLineSpan));
}

#[test]
fn line_inventory_must_cover_all_bytes() {
    let lines = vec![SourceLineSpan { line_index: 0, source_start: 0, source_end: 2, content_end: 2 }];
    assert_eq!(unitize(input("abc", lines), limits(2, 3)), Err(UnitizationError::LineCoverageMismatch));
}

#[test]
fn finite_unit_limit_is_fail_closed() {
    let text = "abcdefgh";
    let cap = UnitizationLimits { max_units: 3, ..limits(2, 2) };
    assert_eq!(unitize(input(text, simple_lines(text)), cap), Err(UnitizationError::TooManyUnits));
}

#[test]
fn debug_does_not_dump_source_or_unit_text() {
    let text = "sensitive source text";
    let input = input(text, simple_lines(text));
    assert!(!format!("{input:?}").contains(text));
    let result = unitize(input, limits(8, 16)).unwrap();
    assert!(!format!("{result:?}").contains(text));
}

#[test]
fn hidden_line_terminators_are_rejected() {
    let text = "a\nb";
    let lines = vec![SourceLineSpan { line_index: 0, source_start: 0, source_end: 3, content_end: 3 }];
    assert_eq!(unitize_text(text, &lines, limits(2, 4)), Err(UnitizationError::InvalidLineEnding));
}

#[test]
fn invented_unterminated_middle_lines_are_rejected() {
    let lines = vec![
        SourceLineSpan { line_index: 0, source_start: 0, source_end: 1, content_end: 1 },
        SourceLineSpan { line_index: 1, source_start: 1, source_end: 3, content_end: 3 },
    ];
    assert_eq!(unitize_text("abc", &lines, limits(2, 4)), Err(UnitizationError::InvalidLineEnding));
}

#[test]
fn split_crlf_line_inventory_is_rejected() {
    let lines = vec![
        SourceLineSpan { line_index: 0, source_start: 0, source_end: 2, content_end: 1 },
        SourceLineSpan { line_index: 1, source_start: 2, source_end: 3, content_end: 2 },
    ];
    assert_eq!(unitize_text("a\r\n", &lines, limits(2, 4)), Err(UnitizationError::InvalidLineEnding));
}

#[test]
fn raw_ranges_and_receipt_bound_units_agree_for_all_small_sizes() {
    let many_lines = "a\n".repeat(30);
    for text in ["a\r\nb\nc\rd", "αβγδεζηθ", many_lines.as_str()] {
        let lines = simple_lines(text);
        for preferred in 1..=12 {
            let caps = limits(preferred, preferred.max(4));
            let raw = unitize_text(text, &lines, caps).unwrap();
            let bound = unitize(input(text, lines.clone()), caps).unwrap();
            assert_eq!(raw.len(), bound.units.len());
            let mut cursor = 0;
            for (span, unit) in raw.iter().zip(&bound.units) {
                assert_eq!(span.source_start, cursor);
                assert_eq!(&text[span.source_start..span.source_end], unit.text());
                assert_eq!(span.logical_line_start, unit.logical_line_start);
                assert_eq!(span.logical_line_end, unit.logical_line_end);
                assert_eq!(span.starts_at_line_boundary, unit.starts_at_line_boundary);
                assert_eq!(span.ends_at_line_boundary, unit.ends_at_line_boundary);
                cursor = span.source_end;
            }
            assert_eq!(cursor, text.len());
        }
    }
}

#[test]
fn empty_layout_is_not_a_receipt_bound_revision() {
    assert!(unitize_text("", &[], limits(2, 4)).unwrap().is_empty());
    assert_eq!(unitize(input("", vec![]), limits(2, 4)), Err(UnitizationError::EmptyInput));
}

#[test]
fn hard_limit_smaller_than_one_scalar_fails_without_progress() {
    assert_eq!(unitize_text("𐀀", &simple_lines("𐀀"), limits(1, 3)), Err(UnitizationError::NoProgress));
}
