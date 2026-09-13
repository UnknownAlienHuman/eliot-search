use std::path::PathBuf;

use search_materializer::{
    DEFAULT_MATERIALIZATION_LIMITS, MaterializationError, materialize_utf8,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn crate_root_is_a_bounded_facade() {
    let entry = std::fs::read_to_string(crate_root().join("src/lib.rs"))
        .expect("crate root exists");

    assert!(entry.contains("mod error;"));
    assert!(entry.contains("mod utf8;"));
    assert!(entry.contains("pub use error::MaterializationError;"));
    assert!(entry.lines().count() <= 48);

    for implementation_marker in [
        "pub enum MaterializationError",
        "pub struct RetainedRevision",
        "fn scan_lines",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(implementation_marker),
            "implementation returned to crate root: {implementation_marker}"
        );
    }
}

#[test]
fn modules_own_error_and_utf8_behavior() {
    let error = std::fs::read_to_string(crate_root().join("src/error.rs"))
        .expect("error module exists");
    let utf8 = std::fs::read_to_string(crate_root().join("src/utf8.rs"))
        .expect("utf8 module exists");

    assert!(error.contains("pub enum MaterializationError"));
    for required in [
        "pub struct RetainedRevision",
        "pub fn materialize_utf8",
        "pub fn materialize",
        "fn scan_lines",
        "#[cfg(test)]",
    ] {
        assert!(utf8.contains(required), "utf8 module lost owner: {required}");
    }
}

#[test]
fn existing_root_api_remains_usable() {
    let text = materialize_utf8(b"a\r\nb".to_vec(), DEFAULT_MATERIALIZATION_LIMITS)
        .expect("exact UTF-8 remains supported");
    assert_eq!(text.text(), "a\r\nb");
    assert_eq!(text.lines().len(), 2);
    assert_eq!(
        MaterializationError::InvalidUtf8.code(),
        "MATERIALIZATION_INVALID_UTF8"
    );
}
