use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn real_identity_translation_stays_split_and_private() {
    let root = package_root();
    let facade = read(&root, "src/real/identity.rs");
    assert!(
        facade.len() < 1_000,
        "identity facade grew to {} bytes",
        facade.len()
    );
    for module in ["collection", "hex", "point"] {
        assert!(
            facade.contains(&format!("include!(\"identity/{module}.rs\");")),
            "identity facade lost {module} family"
        );
    }
    for forbidden in [
        "pub fn collection_name",
        "fn hex_to_32",
        "fn uuid_string",
        "fn bridge_point_id",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let collection = read(&root, "src/real/identity/collection.rs");
    assert!(collection.contains("pub fn validate_collection_name"));
    assert!(collection.contains("pub fn collection_name"));
    assert!(collection.contains("eliot-qdrant-collection/v1\\x00"));
    assert!(collection.contains("String::with_capacity(28)"));
    assert!(!collection.contains("PointId"));

    let hex = read(&root, "src/real/identity/hex.rs");
    assert!(hex.contains("fn hex_from_32"));
    assert!(hex.contains("fn hex_to_32"));
    assert!(hex.contains("const fn hex_val"));
    assert!(!hex.contains("CollectionRoute"));
    assert!(!hex.contains("PointId"));

    let point = read(&root, "src/real/identity/point.rs");
    for function in [
        "fn uuid_string",
        "fn parse_uuid",
        "fn vendor_point_id",
        "fn bridge_point_id",
    ] {
        assert!(point.contains(function));
    }
    assert!(point.contains("bytes[8..16].copy_from_slice(&number.to_be_bytes())"));
    assert!(!point.contains("Sha256"));

    for relative in [
        "src/real/identity/collection.rs",
        "src/real/identity/hex.rs",
        "src/real/identity/point.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            source.len() < 5_000,
            "identity module {relative} grew to {} bytes",
            source.len()
        );
        for forbidden in ["std::process", "Command::new", "NATIVE_EXE_PATH"] {
            assert!(
                !source.contains(forbidden),
                "identity module {relative} acquired process ownership: {forbidden}"
            );
        }
    }
}
