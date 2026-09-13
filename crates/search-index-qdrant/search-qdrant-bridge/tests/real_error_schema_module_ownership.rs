use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn real_error_mapping_and_schema_verification_stay_split() {
    let root = package_root();
    let facade = read(&root, "src/real/errors_schema.rs");
    assert!(
        facade.len() < 1_000,
        "error/schema facade grew to {} bytes",
        facade.len()
    );
    for module in ["read", "mutation", "create", "verify"] {
        assert!(
            facade.contains(&format!("include!(\"errors_schema/{module}.rs\");")),
            "error/schema facade lost {module} family"
        );
    }
    for forbidden in [
        "fn map_read_error",
        "fn map_mutation_error",
        "fn map_create_error",
        "fn verify_server_schema",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let expectations = [
        ("src/real/errors_schema/read.rs", "fn map_read_error"),
        (
            "src/real/errors_schema/mutation.rs",
            "fn map_mutation_error",
        ),
        ("src/real/errors_schema/create.rs", "fn map_create_error"),
        (
            "src/real/errors_schema/verify.rs",
            "fn verify_server_schema",
        ),
    ];
    for (relative, function) in expectations {
        let source = read(&root, relative);
        assert!(source.contains(function), "{relative} lost {function}");
        assert!(
            source.len() < 5_000,
            "error module {relative} grew to {} bytes",
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
                "error module {relative} acquired forbidden surface: {forbidden}"
            );
        }
    }
}

#[test]
fn read_and_mutation_transport_failures_remain_distinct() {
    let root = package_root();
    let read = read(&root, "src/real/errors_schema/read.rs");
    assert!(read.contains("QdrantError::Io(_) => BridgeError::TransportFailed"));
    assert!(!read.contains("QdrantError::Io(_) => BridgeError::MutationOutcomeUnknown"));

    let mutation = read(&root, "src/real/errors_schema/mutation.rs");
    assert!(mutation.contains("BridgeError::MutationOutcomeUnknown"));
    assert!(!mutation.contains("QdrantError::Io(_) => BridgeError::TransportFailed"));

    let create = read(&root, "src/real/errors_schema/create.rs");
    assert!(create.contains("BridgeError::MutationOutcomeUnknown"));
    assert!(create.contains("BridgeError::CollectionSchemaMismatch"));
}
