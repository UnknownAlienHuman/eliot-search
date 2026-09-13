use std::path::PathBuf;

use search_materializer::api::{
    DEFAULT_MATERIALIZATION_LIMITS, LineEnding, LineEndingEvidence, LineSpan,
    MaterializationLimits, MaterializationReceipt, MaterializedRevision, MaterializedText,
    RetainedRevision, materialize, materialize_utf8,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn utf8_entry_is_a_thin_stable_facade() {
    let entry = std::fs::read_to_string(crate_root().join("src/utf8.rs"))
        .expect("UTF-8 facade exists");

    for module in [
        "mod materialize;",
        "mod model;",
        "mod scan;",
        "mod tests;",
    ] {
        assert!(entry.contains(module), "missing UTF-8 owner: {module}");
    }
    assert!(entry.lines().count() <= 32);

    for implementation_marker in [
        "pub struct RetainedRevision",
        "fn scan_lines",
        "pub fn materialize_utf8",
        "#[test]",
    ] {
        assert!(
            !entry.contains(implementation_marker),
            "implementation returned to UTF-8 facade: {implementation_marker}"
        );
    }
}

#[test]
fn utf8_responsibilities_have_distinct_private_owners() {
    let root = crate_root().join("src/utf8");
    let expectations = [
        ("model.rs", "pub struct RetainedRevision"),
        ("scan.rs", "fn scan_lines"),
        ("materialize.rs", "pub fn materialize_utf8"),
        ("tests.rs", "fn exact_utf8_bytes_are_preserved"),
    ];
    for (path, marker) in expectations {
        let source = std::fs::read_to_string(root.join(path)).expect("owner module exists");
        assert!(source.contains(marker), "{path} lost owner marker: {marker}");
    }
}

#[test]
fn public_utf8_surface_remains_importable_and_exact() {
    let prepared = materialize_utf8(b"a\r\nb\n".to_vec(), DEFAULT_MATERIALIZATION_LIMITS)
        .expect("UTF-8 preparation remains available");
    assert_eq!(prepared.text(), "a\r\nb\n");
    assert_eq!(prepared.lines().len(), 2);
    assert_eq!(prepared.lines()[0].ending, LineEnding::CrLf);

    let _ = core::mem::size_of::<LineEndingEvidence>();
    let _ = core::mem::size_of::<LineSpan>();
    let _ = core::mem::size_of::<MaterializationLimits>();
    let _ = core::mem::size_of::<MaterializationReceipt>();
    let _ = core::mem::size_of::<MaterializedRevision>();
    let _ = core::mem::size_of::<MaterializedText>();
    let _ = core::mem::size_of::<RetainedRevision>();
    let _ = materialize;
}
