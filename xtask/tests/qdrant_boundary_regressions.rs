//! Structural guard regressions on disposable synthetic repositories.
//!
//! These do not build or qualify a Qdrant client/server. Version numbers below
//! are fixture data, not additions to the product qualification registry.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use xtask::qdrant_boundary::{
    QdrantBoundaryReport, exit_code, render_report_json,
    validate_qdrant_boundary,
};

const BRIDGE: &str =
    "crates/search-index-qdrant/search-qdrant-bridge";
const ROOT_MANIFEST: &str = concat!(
    "[workspace.dependencies]\n",
    "qdrant-client = { version = \"=1.2.3\", default-features = false }\n",
);
const LOCKFILE: &str = concat!(
    "version = 4\n\n[[package]]\n",
    "name = \"qdrant-client\"\nversion = \"1.2.3\"\n",
);
const QUALIFIED: &str = concat!(
    "pub const QUALIFIED_CLIENT_VERSION: &str = \"1.2.3\";\n",
    "pub const QUALIFIED_SERVER_VERSION: &str = \"2.3.4\";\n",
);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..128 {
            let root = std::env::temp_dir().join(format!(
                "eliot-qdrant-boundary-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let fixture = Self { root };
                    fixture.write("Cargo.toml", ROOT_MANIFEST);
                    fixture.write("Cargo.lock", LOCKFILE);
                    fixture.write(
                        &format!("{BRIDGE}/Cargo.toml"),
                        "[dependencies]\nqdrant-client.workspace = true\n",
                    );
                    fixture.write(
                        &format!("{BRIDGE}/src/qualified.rs"),
                        QUALIFIED,
                    );
                    fixture.write(
                        "qualification/qdrant/artifact.toml",
                        concat!(
                            "[client]\ncrate_name = \"qdrant-client\"\n",
                            "version = \"1.2.3\"\n",
                            "[server]\nversion = \"2.3.4\"\n",
                        ),
                    );
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
        fs::create_dir_all(path.parent().expect("fixture parent"))
            .expect("create fixture parent");
        fs::write(path, text).expect("write fixture");
    }

    fn validate(&self) -> QdrantBoundaryReport {
        validate_qdrant_boundary(&self.root)
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

#[test]
fn clean_fixture_passes_repeatedly_without_writing_any_file() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture.root);
    let first = fixture.validate();
    assert!(first.passed(), "{:?}", first.errors);
    assert_eq!(exit_code(&first), 0);
    assert_eq!(first.manifests_scanned, 2);
    assert_eq!(first.rust_files_scanned, 1);
    assert_eq!(first, fixture.validate());
    assert_eq!(before, snapshot(&fixture.root));
    let json: serde_json::Value =
        serde_json::from_str(&render_report_json(&first))
            .expect("valid report JSON");
    assert_eq!(json["status"], "PASS");
}

#[test]
fn renamed_dependencies_fail_in_every_cargo_dependency_scope() {
    for section in [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "target.'cfg(windows)'.dependencies",
        "target.'cfg(unix)'.build-dependencies",
    ] {
        let fixture = Fixture::new();
        fixture.write(
            "crates/consumer/Cargo.toml",
            &format!(
                "[{section}]\nsdk = {{ package = \"qdrant-client\", \
                 version = \"=1.2.3\" }}\n"
            ),
        );
        assert_failure(&fixture.validate(), "crates/consumer/Cargo.toml");
    }
}

#[test]
fn renamed_workspace_pin_cannot_be_inherited_under_another_name() {
    let fixture = Fixture::new();
    fixture.write(
        "Cargo.toml",
        &format!(
            "{ROOT_MANIFEST}sdk = {{ package = \"qdrant-client\", \
             version = \"=1.2.3\" }}\n"
        ),
    );
    fixture.write(
        "crates/consumer/Cargo.toml",
        "[dependencies]\nsdk.workspace = true\n",
    );
    assert_failure(&fixture.validate(), "workspace.dependencies.sdk");
}

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
fn bridge_cannot_override_the_workspace_source_or_features() {
    for extra in [
        "path = \"vendor\"",
        "git = \"https://example.invalid/client\"",
        "features = [\"unqualified\"]",
        "package = \"another-client\"",
    ] {
        let fixture = Fixture::new();
        fixture.write(
            &format!("{BRIDGE}/Cargo.toml"),
            &format!(
                "[dependencies]\nqdrant-client = {{ workspace = true, \
                 {extra} }}\n"
            ),
        );
        assert_failure(&fixture.validate(), &format!("{BRIDGE}/Cargo.toml"));
    }
}

#[test]
fn duplicate_lockfile_client_records_never_select_the_first_match() {
    for version in ["1.2.3", "9.9.9"] {
        let fixture = Fixture::new();
        fixture.write(
            "Cargo.lock",
            &format!(
                "{LOCKFILE}\n[[package]]\nname = \"qdrant-client\"\n\
                 version = \"{version}\"\n"
            ),
        );
        assert_failure(&fixture.validate(), "Cargo.lock");
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
        &format!(
            "/*\n{QUALIFIED}*/\n{}",
            QUALIFIED.replace("1.2.3", "9.9.9"),
        ),
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

#[test]
fn direct_dependency_and_public_sdk_surface_still_fail() {
    let fixture = Fixture::new();
    fixture.write(
        "crates/consumer/Cargo.toml",
        "[dependencies]\nqdrant-client.workspace = true\n",
    );
    fixture.write(
        &format!("{BRIDGE}/src/lib.rs"),
        "pub type Client = qdrant_client::Qdrant;\n",
    );
    let report = fixture.validate();
    assert_failure(&report, "crates/consumer/Cargo.toml");
    assert_failure(&report, &format!("{BRIDGE}/src/lib.rs:1:"));
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
