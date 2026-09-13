use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn crate_root_is_a_public_facade() {
    let root = package_root();
    let facade = read(&root, "src/lib.rs");
    assert!(
        facade.len() < 3_500,
        "unitizer facade grew to {} bytes",
        facade.len()
    );
    for module in ["error", "unitization", "layout", "manifest"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    for forbidden in [
        "pub enum UnitizationError",
        "pub struct UnitizationInput",
        "pub struct TextUnit",
        "pub fn unitize(",
        "std::fs",
        "std::process",
    ] {
        assert!(
            !facade.contains(forbidden),
            "crate facade owns implementation token {forbidden}"
        );
    }
}

#[test]
fn error_and_receipt_bound_unitization_have_distinct_owners() {
    let root = package_root();
    let error = read(&root, "src/error.rs");
    assert!(error.contains("pub enum UnitizationError"));
    assert!(error.contains("UNITIZATION_MISSING_MATERIALIZATION_RECEIPT"));
    assert!(!error.contains("pub struct UnitizationInput"));
    assert!(!error.contains("pub fn unitize("));

    let unitization = read(&root, "src/unitization.rs");
    for required in [
        "pub const DEFAULT_UNITIZATION_LIMITS",
        "pub struct UnitizationLimits",
        "pub struct SourceLineSpan",
        "pub struct UnitizationInput",
        "pub struct UnitIdentity",
        "pub struct TextUnit",
        "pub struct UnitizationReceipt",
        "pub struct UnitizationResult",
        "pub fn unitize(",
        "unitize_text(input.text(), &input.lines, limits)",
    ] {
        assert!(
            unitization.contains(required),
            "unitization owner missing {required}"
        );
    }
    for forbidden in [
        "std::fs",
        "std::process",
        "qdrant_client",
        "search_qdrant",
        "tokio::",
        "reqwest::",
    ] {
        assert!(
            !unitization.contains(forbidden),
            "unitization owner acquired forbidden dependency token {forbidden}"
        );
    }
}

#[test]
fn existing_public_surface_is_reexported() {
    let root = package_root();
    let facade = read(&root, "src/lib.rs");
    for public in [
        "UnitizationError",
        "DEFAULT_UNITIZATION_LIMITS",
        "SourceLineSpan",
        "TextUnit",
        "UnitIdentity",
        "UnitizationInput",
        "UnitizationLimits",
        "UnitizationReceipt",
        "UnitizationResult",
        "unitize",
        "UnitSpan",
        "unitize_text",
        "UnitManifest",
        "build_unit_manifest",
        "decode_unit_manifest",
        "verify_unit_manifest",
    ] {
        assert!(facade.contains(public), "crate root no longer exports {public}");
    }
}
