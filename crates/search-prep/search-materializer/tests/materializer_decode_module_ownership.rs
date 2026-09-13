use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn decode_entry_is_a_thin_owner_facade() {
    let entry = std::fs::read_to_string(crate_root().join("src/decode.rs"))
        .expect("decode entry exists");

    for declaration in ["mod detect;", "mod engine;", "mod model;"] {
        assert!(entry.contains(declaration), "missing module: {declaration}");
    }
    assert!(entry.lines().count() <= 32);
    for implementation_marker in [
        "pub struct EncodingDecision",
        "pub fn detect_or_validate_encoding",
        "pub fn decode_text_or_code",
        "fn decode_utf16_units",
    ] {
        assert!(
            !entry.contains(implementation_marker),
            "implementation returned to decode facade: {implementation_marker}"
        );
    }
}

#[test]
fn decode_responsibilities_have_single_module_owners() {
    let model = std::fs::read_to_string(crate_root().join("src/decode/model.rs"))
        .expect("decode model exists");
    let detect = std::fs::read_to_string(crate_root().join("src/decode/detect.rs"))
        .expect("encoding detector exists");
    let engine = std::fs::read_to_string(crate_root().join("src/decode/engine.rs"))
        .expect("decode engine exists");
    let tests = std::fs::read_to_string(crate_root().join("src/decode/tests.rs"))
        .expect("decode tests exist");

    assert!(model.contains("pub struct EncodingDecision"));
    assert!(model.contains("pub struct DecodedRepresentation"));
    assert!(detect.contains("pub fn detect_or_validate_encoding"));
    assert!(engine.contains("pub fn decode_text_or_code"));
    assert!(engine.contains("fn decode_utf16_units"));
    assert!(tests.contains("fn malformed_and_truncated_inputs_are_typed_errors"));
}
