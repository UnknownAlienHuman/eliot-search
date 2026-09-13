use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn layout_codec_stays_split_by_wire_direction() {
    let root = package_root();
    let facade = read(&root, "src/layout_manifest.rs");
    assert!(facade.len() < 1_000);
    for module in ["decode", "encode", "format", "wire"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    for forbidden in [
        "pub fn encode_layout",
        "pub fn decode_layout",
        "const MAGIC",
        "std::fs",
        "std::process",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let encode = read(&root, "src/layout_manifest/encode.rs");
    assert!(encode.contains("pub fn encode_layout"));
    assert!(!encode.contains("pub fn decode_layout"));
    assert!(encode.contains("unitize_text"));

    let decode = read(&root, "src/layout_manifest/decode.rs");
    assert!(decode.contains("pub fn decode_layout"));
    assert!(!decode.contains("pub fn encode_layout"));
    assert!(decode.contains("validate_text"));
    assert!(decode.contains("choose_end"));

    let format = read(&root, "src/layout_manifest/format.rs");
    assert!(format.contains("exact-utf8-line-unit-layout/v1"));
    assert!(!format.contains("encode_layout"));
    assert!(!format.contains("decode_layout"));

    let wire = read(&root, "src/layout_manifest/wire.rs");
    for token in ["ELSLAY01", "encoded_size", "fn boolean"] {
        assert!(wire.contains(token));
    }
    assert!(!wire.contains("unitize_text"));
    assert!(!wire.contains("choose_end"));

    for relative in [
        "src/layout_manifest/decode.rs",
        "src/layout_manifest/encode.rs",
        "src/layout_manifest/format.rs",
        "src/layout_manifest/wire.rs",
    ] {
        let source = read(&root, relative);
        assert!(source.len() < 6_000, "{relative} grew to {} bytes", source.len());
        for forbidden in ["std::fs", "std::process", "qdrant_client", "tokio::"] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }
}
