use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn real_connection_and_schema_operations_stay_split() {
    let root = package_root();
    let facade = read(&root, "src/real/connect_schema.rs");
    assert!(
        facade.len() < 1_000,
        "connect/schema facade grew to {} bytes",
        facade.len()
    );
    for module in ["connect", "create", "verify"] {
        assert!(
            facade.contains(&format!("include!(\"connect_schema/{module}.rs\");")),
            "connect/schema facade lost {module} family"
        );
    }
    for forbidden in [
        "pub async fn connect",
        "pub async fn create_collection",
        "pub async fn verify_schema",
        "health_check()",
        "CreateCollection",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let connect = read(&root, "src/real/connect_schema/connect.rs");
    assert!(connect.contains("pub async fn connect"));
    assert!(connect.contains("health_check()"));
    assert!(connect.contains("QUALIFIED_SERVER_VERSION"));
    assert!(connect.contains("QUALIFIED_SERVER_BUILD"));
    assert!(!connect.contains("CreateCollection"));
    assert!(!connect.contains("verify_server_schema"));

    let create = read(&root, "src/real/connect_schema/create.rs");
    assert!(create.contains("pub async fn create_collection"));
    assert!(create.contains("CreateCollection"));
    assert!(create.contains("CreateFieldIndexCollection"));
    assert!(create.contains("wait: Some(true)"));
    assert!(create.contains("ordering: Some(strong_ordering())"));
    assert!(create.contains("fn post_create_check"));
    assert!(create.contains("fn map_post_create_error"));
    assert!(create.contains("BridgeError::MutationOutcomeUnknown"));
    assert!(!create.contains("health_check()"));

    let verify = read(&root, "src/real/connect_schema/verify.rs");
    assert!(verify.contains("async fn verify_server_schema"));
    assert!(verify.contains("pub async fn verify_schema"));
    assert!(verify.contains("collection_info(name)"));
    assert!(!verify.contains("CreateCollection"));

    for relative in [
        "src/real/connect_schema/connect.rs",
        "src/real/connect_schema/create.rs",
        "src/real/connect_schema/verify.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            source.len() < 7_500,
            "schema module {relative} grew to {} bytes",
            source.len()
        );
        for forbidden in ["std::process", "Command::new", "NATIVE_EXE_PATH"] {
            assert!(
                !source.contains(forbidden),
                "schema module {relative} acquired process ownership: {forbidden}"
            );
        }
    }
}

#[test]
fn post_create_no_write_errors_are_not_returned_after_possible_effects() {
    let root = package_root();
    let create = read(&root, "src/real/connect_schema/create.rs");
    for error in [
        "BridgeError::Cancelled",
        "BridgeError::TransportFailed",
        "BridgeError::MalformedResponse",
    ] {
        assert!(create.contains(error), "missing post-create mapping for {error}");
    }
    assert!(create.contains("=> BridgeError::MutationOutcomeUnknown"));
    assert!(create.contains("post_create_check(context)?;"));
    assert!(create.contains(".map_err(map_post_create_error)?;"));
}
