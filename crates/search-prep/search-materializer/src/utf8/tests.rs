use super::*;
use crate::error::MaterializationError;
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

fn retained(bytes: &[u8]) -> RetainedRevision {
    RetainedRevision::new(
        OpaqueId::new("source:test").expect("source"),
        NonZeroRevision::new(1).expect("revision"),
        Some(Blake3Digest32::from_bytes([1; 32])),
        u64::try_from(bytes.len()).expect("length"),
        bytes.to_vec(),
        Some(ReceiptRef::new("receipt:revision").expect("receipt")),
    )
}

#[test]
fn exact_utf8_bytes_are_preserved() {
    let bytes = "alpha\r\nbeta\nγ".as_bytes();
    let result = materialize(retained(bytes), DEFAULT_MATERIALIZATION_LIMITS)
        .expect("materialize");
    assert_eq!(result.bytes(), bytes);
    assert_eq!(result.receipt.input_bytes, result.receipt.output_bytes);
}

#[test]
fn line_endings_and_offsets_are_exact() {
    let result = materialize(retained(b"a\r\nb\nc\rd"), DEFAULT_MATERIALIZATION_LIMITS)
        .expect("materialize");
    assert_eq!(result.lines.len(), 4);
    assert_eq!(
        (
            result.lines[0].source_start,
            result.lines[0].content_end,
            result.lines[0].source_end
        ),
        (0, 1, 3)
    );
    assert_eq!(result.lines[0].ending, LineEnding::CrLf);
    assert_eq!(result.lines[1].ending, LineEnding::Lf);
    assert_eq!(result.lines[2].ending, LineEnding::Cr);
    assert_eq!(result.lines[3].ending, LineEnding::None);
    assert!(result.receipt.line_endings.is_mixed());
}

#[test]
fn final_terminator_does_not_create_phantom_line() {
    let result = materialize(retained(b"a\n"), DEFAULT_MATERIALIZATION_LIMITS)
        .expect("materialize");
    assert_eq!(result.lines.len(), 1);
    assert_eq!(result.lines[0].ending, LineEnding::Lf);
    assert_eq!(result.receipt.line_endings.unterminated, 0);
}

#[test]
fn invalid_utf8_is_rejected() {
    assert_eq!(
        materialize(retained(&[0xff, 0xfe]), DEFAULT_MATERIALIZATION_LIMITS),
        Err(MaterializationError::InvalidUtf8)
    );
}

#[test]
fn nul_content_is_rejected_as_binary() {
    assert_eq!(
        materialize(retained(b"a\0b"), DEFAULT_MATERIALIZATION_LIMITS),
        Err(MaterializationError::BinaryContent)
    );
}

#[test]
fn byte_count_mismatch_is_rejected() {
    let mut input = retained(b"abc");
    input.byte_count = 2;
    assert_eq!(
        materialize(input, DEFAULT_MATERIALIZATION_LIMITS),
        Err(MaterializationError::InputLengthMismatch)
    );
}

#[test]
fn line_limit_is_fail_closed() {
    let limits = MaterializationLimits {
        max_lines: 1,
        ..DEFAULT_MATERIALIZATION_LIMITS
    };
    assert_eq!(
        materialize(retained(b"a\nb"), limits),
        Err(MaterializationError::TooManyLines)
    );
}

#[test]
fn debug_output_does_not_dump_source_text() {
    let input = retained(b"sensitive source text");
    assert!(!format!("{input:?}").contains("sensitive source text"));
    let result = materialize(input, DEFAULT_MATERIALIZATION_LIMITS).expect("materialize");
    assert!(!format!("{result:?}").contains("sensitive source text"));
    assert!(format!("{result:?}").contains("UTF-8 bytes"));
}

#[test]
fn receipt_free_and_bound_paths_use_identical_mapping() {
    for bytes in [b"a\r\nb\nc\rd".as_slice(), "αβ\n𐀀".as_bytes(), b"\n\n"] {
        let plain = materialize_utf8(bytes.to_vec(), DEFAULT_MATERIALIZATION_LIMITS).unwrap();
        let bound = materialize(retained(bytes), DEFAULT_MATERIALIZATION_LIMITS).unwrap();
        assert_eq!(plain.text(), bound.text());
        assert_eq!(plain.lines(), bound.lines.as_slice());
        assert_eq!(plain.line_endings(), bound.receipt.line_endings);
    }
}

#[test]
fn empty_text_does_not_create_a_revision_receipt() {
    let text = materialize_utf8(Vec::new(), DEFAULT_MATERIALIZATION_LIMITS).unwrap();
    assert!(text.text().is_empty());
    assert!(text.lines().is_empty());
    assert_eq!(
        materialize(retained(b""), DEFAULT_MATERIALIZATION_LIMITS),
        Err(MaterializationError::EmptyInput)
    );
}

#[test]
fn byte_preparation_keeps_finite_limits_and_redacted_debug() {
    let limits = MaterializationLimits {
        max_input_bytes: 2,
        ..DEFAULT_MATERIALIZATION_LIMITS
    };
    assert_eq!(
        materialize_utf8(b"abc".to_vec(), limits),
        Err(MaterializationError::InputTooLarge)
    );
    let text = materialize_utf8(
        b"private-sentinel".to_vec(),
        DEFAULT_MATERIALIZATION_LIMITS,
    )
    .unwrap();
    assert!(!format!("{text:?}").contains("private-sentinel"));
}
