//! Structural guard regressions on disposable synthetic repositories.
//!
//! These do not build or qualify a Qdrant client/server. Version numbers below
//! are fixture data, not additions to the product qualification registry.
//!
//! Scope is the static pin/inheritance/source/artifact policy only. The Cargo
//! dependency and lockfile graph cases moved to the embedded `CargoInventory`
//! tests, because a synthetic root here is not a resolvable workspace and
//! therefore cannot feed `cargo metadata`. Nothing in this module asserts graph
//! resolution, lockfile consistency or dependency placement.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use xtask::qdrant_boundary::{
    BoundaryScope, QdrantBoundaryReport, exit_code, render_report_json,
    validate_qdrant_static_boundary,
};

const BRIDGE: &str = "crates/search-index-qdrant/search-qdrant-bridge";

/// A synthetic root is not a resolvable Cargo workspace: the lockfile below is
/// a deliberately inert stub, so no dependency or resolution expectation is
/// attached to it. It exists so that file-collection and read paths that do
/// not read the lockfile see a bounded, well-formed placeholder.
const LOCKFILE: &str =
    "version = 4\n\n[[package]]\nname = \"qdrant-client\"\nversion = \"1.2.3\"\n";
const ROOT_MANIFEST: &str = concat!(
    "[workspace.dependencies]\n",
    "qdrant-client = { version = \"=1.2.3\", default-features = false }\n",
);
const BRIDGE_MANIFEST: &str = "[dependencies]\nqdrant-client.workspace = true\n";
const QUALIFIED: &str = concat!(
    "pub const QUALIFIED_CLIENT_VERSION: &str = \"1.2.3\";\n",
    "pub const QUALIFIED_SERVER_VERSION: &str = \"2.3.4\";\n",
);
const ARTIFACT: &str = concat!(
    "[client]\ncrate_name = \"qdrant-client\"\n",
    "version = \"1.2.3\"\n",
    "[server]\nversion = \"2.3.4\"\n",
);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..128 {
            let root = std::env::temp_dir().join(format!(
                "eliot-qdrant-static-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let fixture = Self { root };
                    fixture.write("Cargo.toml", ROOT_MANIFEST);
                    fixture.write(&format!("{BRIDGE}/Cargo.toml"), BRIDGE_MANIFEST);
                    fixture.write(&format!("{BRIDGE}/src/qualified.rs"), QUALIFIED);
                    fixture.write("qualification/qdrant/artifact.toml", ARTIFACT);
                    return fixture;
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create fixture: {error}"),
            }
        }
        panic!("fixture directory collision budget exhausted");
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
        fs::write(path, text).expect("write fixture");
    }

    fn validate(&self) -> QdrantBoundaryReport {
        validate_qdrant_static_boundary(&self.root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_failure(report: &QdrantBoundaryReport, location: &str) {
    assert!(!report.passed(), "expected a failure at {location}");
    assert_eq!(exit_code(report), 1);
    assert!(
        report.errors.iter().any(|error| error.contains(location)),
        "missing {location}: {:?}",
        report.errors,
    );
}

/// The static scan reads exactly the two manifests, and the report names its
/// own scope so a caller cannot mistake a static PASS for a graph PASS.
#[test]
fn clean_fixture_passes_without_writing_any_file() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture.root);
    let first = fixture.validate();
    assert!(first.passed(), "{:?}", first.errors);
    assert_eq!(exit_code(&first), 0);
    assert_eq!(first.scope, BoundaryScope::StaticPolicy);
    assert_eq!(first.manifests_scanned, 2);
    assert_eq!(first.rust_files_scanned, 1);
    assert_eq!(first, fixture.validate());
    assert_eq!(before, snapshot(&fixture.root));
    let json = render_report_json(&first).expect("render report");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid report JSON");
    assert_eq!(parsed["scope"], "static-policy");
    assert_eq!(parsed["status"], "PASS");
}

/// The workspace pin must stay an exact `=major.minor.patch` declaration with
/// `default-features = false`. Each variant below is a syntactically valid
/// TOML entry that would still resolve, so only the static pin policy rejects
/// them and a graph-only reader would miss all five.
#[test]
fn workspace_pin_must_stay_an_exact_default_less_declaration() {
    for declaration in [
        "qdrant-client = \"1.2.3\"",
        "qdrant-client = { version = \"1.2.3\", default-features = false }",
        "qdrant-client = { version = \"=1.2.3\" }",
        "qdrant-client = { version = \"^1.2.3\", default-features = false }",
        "qdrant-client = { version = \"=1.2\", default-features = false }",
    ] {
        let fixture = Fixture::new();
        fixture.write(
            "Cargo.toml",
            &format!("[workspace.dependencies]\n{declaration}\n"),
        );
        assert_failure(&fixture.validate(), "Cargo.toml");
    }
}

/// The bridge manifest may only inherit. Any extra key alongside
/// `workspace = true` is a static override, regardless of whether it names
/// the same version and would therefore resolve identically.
#[test]
fn bridge_cannot_override_the_workspace_inheritance() {
    for extra in [
        "features = [\"unqualified\"]",
        "default-features = false",
        "package = \"another-client\"",
    ] {
        let fixture = Fixture::new();
        fixture.write(
            &format!("{BRIDGE}/Cargo.toml"),
            &format!("[dependencies]\nqdrant-client = {{ workspace = true, {extra} }}\n"),
        );
        assert_failure(&fixture.validate(), &format!("{BRIDGE}/Cargo.toml"));
    }
    let fixture = Fixture::new();
    fixture.write(
        &format!("{BRIDGE}/Cargo.toml"),
        "[dependencies]\nqdrant-client = \"1.2.3\"\n",
    );
    assert_failure(&fixture.validate(), &format!("{BRIDGE}/Cargo.toml"));
}

/// A bridge manifest that drops the vendor dependency entirely is a static
/// violation, so the boundary cannot be satisfied by deleting the entry.
#[test]
fn bridge_must_declare_the_inherited_vendor_dependency() {
    let fixture = Fixture::new();
    fixture.write(&format!("{BRIDGE}/Cargo.toml"), "[dependencies]\n");
    assert_failure(&fixture.validate(), &format!("{BRIDGE}/Cargo.toml"));
}

/// `[patch]` and `[replace]` substitute the qualified source. Both are
/// declared in the manifest text, so the static scan rejects them before any
/// resolution would even run.
#[test]
fn patch_and_replace_cannot_substitute_the_qualified_client() {
    for declaration in [
        "[patch.crates-io]\nqdrant-client = { path = \"vendor\" }\n",
        concat!(
            "[patch.crates-io]\n",
            "sdk = { package = \"qdrant-client\", path = \"vendor\" }\n",
        ),
        "[replace]\n\"qdrant-client:1.2.3\" = { path = \"vendor\" }\n",
    ] {
        let fixture = Fixture::new();
        fixture.write("Cargo.toml", &format!("{ROOT_MANIFEST}\n{declaration}"));
        assert_failure(&fixture.validate(), "vendor dependency declared");
    }
}

/// A second workspace entry renamed through `package` to the vendor crate is
/// a rename the graph would honour, so only the static pin policy sees it.
#[test]
fn renamed_workspace_pin_cannot_shadow_the_vendor_dependency() {
    let fixture = Fixture::new();
    fixture.write(
        "Cargo.toml",
        &format!("{ROOT_MANIFEST}sdk = {{ package = \"qdrant-client\", version = \"=1.2.3\" }}\n"),
    );
    assert_failure(&fixture.validate(), "renamed workspace vendor pin");
}

/// Renaming under `dev-`, `build-` or target scopes changes the Cargo graph,
/// not the pin policy, so this case now belongs to the embedded inventory
/// tests. What remains here is that the static scan never treats such a
/// section as satisfying the workspace pin requirement.
#[test]
fn dependency_sections_do_not_satisfy_the_workspace_pin() {
    for section in [
        "dev-dependencies",
        "build-dependencies",
        "target.'cfg(windows)'.dependencies",
        "target.'cfg(unix)'.build-dependencies",
    ] {
        let fixture = Fixture::new();
        fixture.write(
            &format!("{BRIDGE}/Cargo.toml"),
            &format!("[{section}]\nqdrant-client.workspace = true\n"),
        );
        assert_failure(&fixture.validate(), "must inherit the workspace dependency");
    }
}

#[test]
fn workspace_pin_rejects_source_substitution_with_the_same_version() {
    for extra in [
        "path = \"vendor\"",
        "git = \"https://example.invalid/client\"",
        "registry = \"other\"",
        "package = \"not-the-qualified-client\"",
    ] {
        let fixture = Fixture::new();
        fixture.write(
            "Cargo.toml",
            &format!(
                "[workspace.dependencies]\nqdrant-client = {{ \
                 version = \"=1.2.3\", default-features = false, {extra} }}\n"
            ),
        );
        assert_failure(&fixture.validate(), "Cargo.toml");
    }
}

#[test]
fn spaced_sdk_reference_outside_bridge_fails_with_its_source_path() {
    let fixture = Fixture::new();
    fixture.write(
        "crates/consumer/src/lib.rs",
        "type Client = qdrant_client /* separated */ :: Qdrant;\n",
    );
    let report = fixture.validate();
    assert_failure(&report, "crates/consumer/src/lib.rs");
    assert_eq!(
        report.sdk_source_files,
        vec!["crates/consumer/src/lib.rs".to_owned()]
    );
}

#[test]
fn comment_cannot_spoof_the_client_version_used_for_comparison() {
    let fixture = Fixture::new();
    fixture.write(
        &format!("{BRIDGE}/src/qualified.rs"),
        &format!("/*\n{QUALIFIED}*/\n{}", QUALIFIED.replace("1.2.3", "9.9.9")),
    );
    assert_failure(&fixture.validate(), "Qdrant version mismatch");
}

#[test]
fn comments_strings_and_similarly_named_modules_are_inert() {
    let fixture = Fixture::new();
    fixture.write(
        "crates/consumer/src/lib.rs",
        r##"
use qdrant_client_helpers::Client;
// use qdrant_client::Qdrant;
const NOTE: &str = "use qdrant_client::Qdrant;";
const RAW: &str = r#"qdrant_client :: Qdrant"#;
"##,
    );
    let report = fixture.validate();
    assert!(report.passed(), "{:?}", report.errors);
    assert!(report.sdk_source_files.is_empty());
}

fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).expect("enumerate fixture") {
            let entry = entry.expect("fixture entry");
            let path = entry.path();
            if entry.file_type().expect("fixture type").is_dir() {
                directories.push(path);
            } else {
                let bytes = fs::read(&path).expect("read fixture");
                files.push((
                    path.strip_prefix(root).expect("fixture path").to_path_buf(),
                    bytes,
                ));
            }
        }
    }
    files.sort();
    files
}

#[test]
fn public_sdk_surface_still_fails() {
    let fixture = Fixture::new();
    fixture.write(
        &format!("{BRIDGE}/src/lib.rs"),
        "pub type Client = qdrant_client::Qdrant;\n",
    );
    assert_failure(&fixture.validate(), &format!("{BRIDGE}/src/lib.rs:1:"));
}

#[test]
fn static_policy_ignores_lockfile_graph_text() {
    let fixture = Fixture::new();
    for text in [LOCKFILE, "not a Cargo lockfile"] {
        fixture.write("Cargo.lock", text);
        let report = fixture.validate();
        assert!(report.passed(), "{:?}", report.errors);
        assert_eq!(report.scope, BoundaryScope::StaticPolicy);
    }
}

#[test]
fn unused_alternate_replacement_keys_are_rejected() {
    for key in [
        "qdrant-client@1.2.3",
        "registry+https://github.com/rust-lang/crates.io-index#qdrant-client@1.2.3",
        "https://github.com/example/qdrant-client#1.2.3",
        "git+https://example.invalid/qdrant-client?branch=dev#1.2.3",
    ] {
        let fixture = Fixture::new();
        fixture.write(
            "Cargo.toml",
            &format!("{ROOT_MANIFEST}\n[replace]\n\"{key}\" = {{ path = \"vendor\" }}\n"),
        );
        assert_failure(&fixture.validate(), "vendor dependency declared");
    }
}
