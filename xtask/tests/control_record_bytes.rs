//! Exact byte checks for signed control-record digest boundaries.

use xtask::control_record_bytes::{
    ControlRecordBytesError, ExactRecordFileDigest, SignedPayloadDigest,
    verify_control_record_bytes,
};
use xtask::ticket_planner::exact_sha256_hex;

const SIGNATURE_REFS: &str = concat!(
    "materializer_signature_ref = { approval_profile_ref = \"profile:test\", ",
    "approval_artifact_ref = { store_profile_ref = \"profile:test\", ",
    "artifact_id = \"approval:test\", bytes = 64, sha256 = \"",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" }, ",
    "signed_payload_sha256 = \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\", ",
    "actor_identity = \"actor:service:test\" }\n",
);

fn record(payload: &str) -> Vec<u8> {
    assert!(payload.ends_with('\n'));
    let payload_digest = exact_sha256_hex(payload.as_bytes());
    let mut raw = payload.as_bytes().to_vec();
    raw.extend_from_slice(
        format!("[signature]\nrecord_sha256 = \"{payload_digest}\"\n{SIGNATURE_REFS}").as_bytes(),
    );
    raw
}

fn error(raw: &[u8], max_bytes: usize) -> ControlRecordBytesError {
    verify_control_record_bytes(raw, max_bytes).unwrap_err()
}

#[test]
fn computes_distinct_digests_over_the_original_payload_and_complete_file() {
    let payload = "record_kind = \"sample_v1\"\nvalue = 7\n";
    let raw = record(payload);
    let verified = verify_control_record_bytes(&raw, raw.len()).unwrap();

    let signed_payload: SignedPayloadDigest = verified.signed_payload_sha256;
    let exact_file: ExactRecordFileDigest = verified.exact_record_file_sha256;
    assert_eq!(
        signed_payload.to_hex(),
        exact_sha256_hex(payload.as_bytes())
    );
    assert_eq!(exact_file.to_hex(), exact_sha256_hex(&raw));
    assert_ne!(signed_payload.to_hex(), exact_file.to_hex());
}

#[test]
fn accepts_multiline_string_markers_and_generated_nested_signature_references() {
    let payload = concat!(
        "record_kind = \"sample_v1\"\n",
        "quoted_hash = \"# inside a basic string\"\n",
        "description = \"\"\"\n",
        "[signature]\n",
        "# inside a multiline string\n",
        "\"\"\"\n",
    );
    let raw = record(payload);
    assert!(verify_control_record_bytes(&raw, raw.len()).is_ok());
}

#[test]
fn requires_a_positive_caller_bound_and_checks_it_before_parsing() {
    assert_eq!(
        error(b"not toml", 0),
        ControlRecordBytesError::ZeroByteLimit
    );
    assert_eq!(
        error(b"not toml", 1),
        ControlRecordBytesError::ByteLimitExceeded
    );

    let raw = record("record_kind = \"sample_v1\"\n");
    assert_eq!(
        error(&raw, raw.len() - 1),
        ControlRecordBytesError::ByteLimitExceeded
    );
}

#[test]
fn rejects_a_signature_header_without_a_preceding_payload_line_feed() {
    let raw = concat!(
        "[signature]\n",
        "record_sha256 = \"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\"\n",
    )
    .as_bytes();
    assert_eq!(
        error(raw, raw.len()),
        ControlRecordBytesError::SignatureHeaderInvalid
    );
}

#[test]
fn rejects_noncanonical_byte_encodings_and_whitespace() {
    let raw = record("record_kind = \"sample_v1\"\n");

    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(&raw);
    assert_eq!(
        error(&bom, bom.len()),
        ControlRecordBytesError::ByteOrderMarkForbidden
    );

    let mut crlf = Vec::with_capacity(raw.len() * 2);
    for byte in &raw {
        if *byte == b'\n' {
            crlf.push(b'\r');
        }
        crlf.push(*byte);
    }
    assert_eq!(
        error(&crlf, crlf.len()),
        ControlRecordBytesError::CarriageReturnForbidden
    );

    let mut missing_final_lf = raw.clone();
    missing_final_lf.pop();
    assert_eq!(
        error(&missing_final_lf, missing_final_lf.len()),
        ControlRecordBytesError::FinalLineFeedRequired
    );

    let trailing_space = record("record_kind = \"sample_v1\" \n");
    assert_eq!(
        error(&trailing_space, trailing_space.len()),
        ControlRecordBytesError::TrailingWhitespaceForbidden
    );

    let trailing_tab = record("record_kind = \"sample_v1\"\t\n");
    assert_eq!(
        error(&trailing_tab, trailing_tab.len()),
        ControlRecordBytesError::TrailingWhitespaceForbidden
    );

    let non_utf8 = [raw.as_slice(), &[0xFF]].concat();
    assert_eq!(
        error(&non_utf8, non_utf8.len()),
        ControlRecordBytesError::InvalidUtf8
    );
}

#[test]
fn rejects_toml_comments_but_does_not_confuse_quoted_hashes_for_comments() {
    let commented = record("record_kind = \"sample_v1\" # forbidden\n");
    assert_eq!(
        error(&commented, commented.len()),
        ControlRecordBytesError::CommentsForbidden
    );

    let multiline_comment_text = concat!(
        "record_kind = \"sample_v1\"\n",
        "description = \"\"\"\n",
        "# this is string content\n",
        "\"\"\"\n",
    );
    let raw = record(multiline_comment_text);
    assert!(verify_control_record_bytes(&raw, raw.len()).is_ok());
}

#[test]
fn only_counts_a_real_signature_header_outside_multiline_strings() {
    let fake_only = concat!(
        "description = \"\"\"\n",
        "[signature]\n",
        "record_sha256 = \"not-a-root-field\"\n",
        "\"\"\"\n",
    )
    .as_bytes();
    assert_eq!(
        error(fake_only, fake_only.len()),
        ControlRecordBytesError::SignatureHeaderInvalid
    );

    let payload = concat!("description = \"\"\"\n", "[signature]\n", "\"\"\"\n",);
    let raw = record(payload);
    assert!(verify_control_record_bytes(&raw, raw.len()).is_ok());
}

#[test]
fn rejects_invalid_or_nonterminal_signature_boundaries_and_payload_mismatch() {
    let payload = "record_kind = \"sample_v1\"\n";
    let mut duplicate = record(payload);
    duplicate.extend_from_slice(b"[signature]\nrecord_sha256 = \"duplicate\"\n");
    assert!(verify_control_record_bytes(&duplicate, duplicate.len()).is_err());

    let mut nonterminal = record(payload);
    nonterminal.extend_from_slice(b"[after_signature]\nvalue = 1\n");
    assert_eq!(
        error(&nonterminal, nonterminal.len()),
        ControlRecordBytesError::SignatureNotTerminal
    );

    let mut mismatch = record(payload);
    let digest_start = mismatch
        .windows(b"record_sha256 = \"".len())
        .position(|window| window == b"record_sha256 = \"")
        .unwrap()
        + b"record_sha256 = \"".len();
    mismatch[digest_start] = if mismatch[digest_start] == b'0' {
        b'1'
    } else {
        b'0'
    };
    assert_eq!(
        error(&mismatch, mismatch.len()),
        ControlRecordBytesError::PayloadDigestMismatch
    );
}

#[test]
fn errors_are_closed_and_do_not_include_record_bytes() {
    let raw = b"private-record-body\n";
    let displayed = error(raw, raw.len()).to_string();
    assert!(!displayed.contains("private-record-body"));
}
