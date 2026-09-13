use std::path::PathBuf;

use search_materializer::api::{
    CanonicalMaterializationBytes, MaterializationAdmissionPlan, MaterializationContext,
    MaterializationProduct, MaterializationVerificationReceipt, MaterializationWarning,
    ResourceReceipt, RevisionBytesGuard, RevisionReadPort, StoredRevisionBytes,
    canonicalize_materialization, materialize_text_or_code, open_exact_revision,
    prepare_admission, verify_materialization,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn product_entry_is_a_thin_stable_facade() {
    let entry = std::fs::read_to_string(crate_root().join("src/product.rs"))
        .expect("product facade exists");

    for module in [
        "mod codec;",
        "mod digest;",
        "mod model;",
        "mod pipeline;",
        "mod read;",
        "mod verify;",
        "mod tests;",
    ] {
        assert!(entry.contains(module), "missing product owner: {module}");
    }
    assert!(entry.lines().count() <= 40);

    for implementation_marker in [
        "pub struct MaterializationProduct",
        "pub trait RevisionReadPort",
        "fn digest_coordinate_map",
        "pub fn materialize_text_or_code",
        "pub fn verify_materialization",
        "#[test]",
    ] {
        assert!(
            !entry.contains(implementation_marker),
            "implementation returned to product facade: {implementation_marker}"
        );
    }
}

#[test]
fn product_responsibilities_have_distinct_private_owners() {
    let root = crate_root().join("src/product");
    let expectations = [
        ("model.rs", "pub struct MaterializationProduct"),
        ("read.rs", "pub trait RevisionReadPort"),
        ("digest.rs", "fn digest_coordinate_map"),
        ("pipeline.rs", "pub fn materialize_text_or_code"),
        ("codec.rs", "pub fn canonicalize_materialization"),
        ("verify.rs", "pub fn verify_materialization"),
        ("tests.rs", "fn end_to_end_product_is_deterministic"),
    ];
    for (path, marker) in expectations {
        let source = std::fs::read_to_string(root.join(path)).expect("owner module exists");
        assert!(source.contains(marker), "{path} lost owner marker: {marker}");
    }
}

#[test]
fn public_product_surface_remains_importable() {
    let _ = core::mem::size_of::<CanonicalMaterializationBytes>();
    let _ = core::mem::size_of::<MaterializationAdmissionPlan>();
    let _ = core::mem::size_of::<MaterializationContext<'static>>();
    let _ = core::mem::size_of::<MaterializationProduct>();
    let _ = core::mem::size_of::<MaterializationVerificationReceipt>();
    let _ = core::mem::size_of::<MaterializationWarning>();
    let _ = core::mem::size_of::<ResourceReceipt>();
    let _ = core::mem::size_of::<RevisionBytesGuard>();
    let _ = core::mem::size_of::<StoredRevisionBytes>();
    let _ = canonicalize_materialization;
    let _ = materialize_text_or_code;
    let _ = open_exact_revision;
    let _ = prepare_admission;
    let _ = verify_materialization;

    fn assert_read_port<T: RevisionReadPort>() {}
    let _ = assert_read_port::<NeverPort>;
}

struct NeverPort;

impl RevisionReadPort for NeverPort {
    fn read_exact(
        &self,
        _source: &search_contracts::OpaqueId,
        _revision: search_contracts::NonZeroRevision,
        _byte_count: u64,
    ) -> Result<StoredRevisionBytes, search_materializer::MaterializationError> {
        Err(search_materializer::MaterializationError::RevisionUnavailable)
    }
}
