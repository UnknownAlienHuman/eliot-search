use super::*;
use super::detect::{UTF16BE_BOM, UTF16LE_BOM, UTF8_BOM};
use crate::MaterializationError;
use crate::profile::{
    LossBehavior, SourceEncoding, ValidatedMaterializerProfile, baseline_profile_descriptor,
    validate_materializer_profile,
};
use crate::request::{
    CancellationToken, DEFAULT_MATERIALIZATION_BUDGET, MaterializationBudget,
};

fn profile() -> ValidatedMaterializerProfile {
    validate_materializer_profile(&baseline_profile_descriptor("decode-test", 1)).expect("profile")
}

fn decide(bytes: &[u8], encoding: SourceEncoding) -> EncodingDecision {
    detect_or_validate_encoding(bytes, encoding, &profile()).expect("decision")
}

fn decode(bytes: &[u8], encoding: SourceEncoding) -> DecodedRepresentation {
    let decision = decide(bytes, encoding);
    decode_text_or_code(
        bytes,
        &decision,
        &profile(),
        &MaterializationBudget {
            max_input_bytes: 1024,
            max_output_bytes: 1024,
            max_lines: 64,
            max_map_segments: 128,
            max_loss_records: 128,
            max_steps: 1 << 20,
        },
        &mut StepCounter::new(1 << 20),
        CancellationToken::never(),
    )
    .expect("decode")
}

#[test]
fn utf8_golden_decodes_exactly() {
    let decoded = decode("hello\r\nworld\nγ".as_bytes(), SourceEncoding::Utf8);
    assert_eq!(decoded.text(), "hello\r\nworld\nγ");
    assert!(!decoded.bom_stripped());
    assert!(!decoded.transcoded());
    assert_eq!(decoded.lines().len(), 3);
    assert_eq!(decoded.lines()[0].ending, crate::LineEnding::CrLf);
    assert_eq!(
        (
            decoded.lines()[0].native_start,
            decoded.lines()[0].native_end
        ),
        (0, 7)
    );
    assert_eq!(
        (
            decoded.lines()[2].decoded_start,
            decoded.lines()[2].decoded_end
        ),
        (13, 14)
    );
    assert_eq!(
        (
            decoded.lines()[2].native_start,
            decoded.lines()[2].native_end
        ),
        (13, 15)
    );
}

#[test]
fn utf8_bom_is_recorded_not_silent() {
    let mut bytes = UTF8_BOM.to_vec();
    bytes.extend_from_slice(b"abc\n");
    let decoded = decode(&bytes, SourceEncoding::Utf8);
    assert!(decoded.bom_stripped());
    assert_eq!(decoded.bom_len_bytes(), 3);
    assert_eq!(decoded.text(), "abc\n");
    assert_eq!(decoded.lines()[0].native_start, 3);
}

#[test]
fn utf16le_transcodes_with_native_evidence() {
    let text = "Aπ\n";
    let mut bytes = Vec::new();
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let decoded = decode(&bytes, SourceEncoding::Utf16Le);
    assert_eq!(decoded.text(), text);
    assert!(decoded.transcoded());
    assert_eq!(decoded.lines().len(), 1);
    assert_eq!(
        (
            decoded.lines()[0].native_start,
            decoded.lines()[0].native_end
        ),
        (0, 6)
    );
    assert_eq!(
        (
            decoded.lines()[0].decoded_start,
            decoded.lines()[0].decoded_end
        ),
        (0, 3)
    );
}

#[test]
fn utf16be_with_bom_decodes() {
    let mut bytes = UTF16BE_BOM.to_vec();
    for unit in "Hi\r\n".encode_utf16() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    let decoded = decode(&bytes, SourceEncoding::Utf16Be);
    assert_eq!(decoded.text(), "Hi\r\n");
    assert!(decoded.bom_stripped());
    assert_eq!(decoded.lines()[0].ending, crate::LineEnding::CrLf);
}

#[test]
fn mismatched_bom_is_ambiguous() {
    let mut bytes = UTF16LE_BOM.to_vec();
    bytes.extend_from_slice(b"abc");
    assert_eq!(
        detect_or_validate_encoding(&bytes, SourceEncoding::Utf8, &profile()),
        Err(MaterializationError::EncodingAmbiguous)
    );
    assert_eq!(
        detect_or_validate_encoding(&bytes, SourceEncoding::Utf16Be, &profile()),
        Err(MaterializationError::EncodingAmbiguous)
    );
}

#[test]
fn undeclared_encoding_is_unsupported() {
    let mut narrow = baseline_profile_descriptor("narrow-decode", 1);
    narrow.encodings = vec![SourceEncoding::Utf8];
    let narrow = validate_materializer_profile(&narrow).expect("narrow");
    assert_eq!(
        detect_or_validate_encoding(b"abc", SourceEncoding::Utf16Le, &narrow),
        Err(MaterializationError::EncodingUnsupported)
    );
}

#[test]
fn malformed_and_truncated_inputs_are_typed_errors() {
    let decision = decide(&[0xFF, 0x41], SourceEncoding::Utf8);
    assert_eq!(
        decode_text_or_code(
            &[0xFF, 0x41],
            &decision,
            &profile(),
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never()
        ),
        Err(MaterializationError::InvalidSequence)
    );
    let units = "AB".encode_utf16().collect::<Vec<u16>>();
    let mut bytes = Vec::new();
    for unit in units {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes.push(0x41);
    let decision = decide(&bytes, SourceEncoding::Utf16Le);
    assert_eq!(
        decode_text_or_code(
            &bytes,
            &decision,
            &profile(),
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never()
        ),
        Err(MaterializationError::InvalidSequence)
    );
    let lone = [0x3D, 0xD8, 0x41, 0x00];
    let decision = decide(&lone, SourceEncoding::Utf16Le);
    assert_eq!(
        decode_text_or_code(
            &lone,
            &decision,
            &profile(),
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never()
        ),
        Err(MaterializationError::InvalidSequence)
    );
}

#[test]
fn strict_loss_profile_rejects_transcoding() {
    let mut strict = baseline_profile_descriptor("strict-decode", 1);
    strict.loss_behavior = LossBehavior::RejectOnAnyLoss;
    let strict = validate_materializer_profile(&strict).expect("strict");
    let text = "A\n";
    let mut bytes = Vec::new();
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let decision = detect_or_validate_encoding(&bytes, SourceEncoding::Utf16Le, &strict)
        .expect("decision");
    assert_eq!(
        decode_text_or_code(
            &bytes,
            &decision,
            &strict,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never()
        ),
        Err(MaterializationError::Loss)
    );
}

#[test]
fn tiny_step_budget_is_exhausted_not_truncated() {
    let decision = decide(b"aaaa\nbbbb\n", SourceEncoding::Utf8);
    let budget = MaterializationBudget {
        max_input_bytes: 64,
        max_output_bytes: 64,
        max_lines: 64,
        max_map_segments: 64,
        max_loss_records: 64,
        max_steps: 2,
    };
    assert_eq!(
        decode_text_or_code(
            b"aaaa\nbbbb\n",
            &decision,
            &profile(),
            &budget,
            &mut StepCounter::new(2),
            CancellationToken::never()
        ),
        Err(MaterializationError::BudgetExhausted)
    );
}

#[test]
fn debug_never_leaks_decoded_text() {
    let decoded = decode(b"private-bytes\n", SourceEncoding::Utf8);
    assert!(!format!("{decoded:?}").contains("private-bytes"));
}
