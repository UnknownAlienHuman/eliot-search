use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn real_vendor_codec_stays_split_and_private() {
    let root = package_root();
    let facade = read(&root, "src/real/codec.rs");
    assert!(
        facade.len() < 1_000,
        "codec facade grew to {} bytes",
        facade.len()
    );
    for module in ["status", "payload", "vectors", "point"] {
        assert!(
            facade.contains(&format!("include!(\"codec/{module}.rs\");")),
            "codec facade lost {module} family"
        );
    }
    for forbidden in [
        "fn encode_payload",
        "fn decode_payload",
        "fn encode_vectors",
        "fn decode_vectors",
        "fn decode_point",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let expectations = [
        ("src/real/codec/status.rs", "fn update_completed"),
        ("src/real/codec/payload.rs", "fn encode_payload"),
        ("src/real/codec/payload.rs", "fn decode_payload"),
        ("src/real/codec/vectors.rs", "fn encode_vectors"),
        ("src/real/codec/vectors.rs", "fn decode_vectors"),
        ("src/real/codec/point.rs", "fn decode_point"),
    ];
    for (relative, function) in expectations {
        let source = read(&root, relative);
        assert!(source.contains(function), "{relative} lost {function}");
        assert!(
            source.len() < 7_500,
            "codec module {relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "pub struct",
            "pub enum",
            "std::process",
            "Command::new",
            "NATIVE_EXE_PATH",
        ] {
            assert!(
                !source.contains(forbidden),
                "codec module {relative} acquired forbidden surface: {forbidden}"
            );
        }
    }
}

#[test]
fn sparse_vector_readback_remains_fail_closed() {
    let root = package_root();
    let vectors = read(&root, "src/real/codec/vectors.rs");
    for invariant in [
        "named.len() != schema.named_vectors.len()",
        "sparse.indices.len() != sparse.values.len()",
        "!score.is_finite()",
        "pair[0].0 >= pair[1].0",
        "*index >= vector_schema.dimensions",
    ] {
        assert!(vectors.contains(invariant), "lost vector invariant: {invariant}");
    }

    let payload = read(&root, "src/real/codec/payload.rs");
    for field in [
        "projection_membership_id",
        "source_revision",
        "unit_ordinal",
        "payload_digest",
        "identity_digest",
        "vector_digest_",
    ] {
        assert!(payload.contains(field), "lost payload field binding: {field}");
    }
}
