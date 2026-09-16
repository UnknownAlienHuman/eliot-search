//! Direct-qualified Qdrant public-surface regressions on disposable repositories.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use xtask::qdrant_boundary::validate_qdrant_boundary;

const BRIDGE: &str =
    "crates/search-index-qdrant/search-qdrant-bridge";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..128 {
            let root = std::env::temp_dir().join(format!(
                "eliot-qdrant-qualified-path-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let fixture = Self { root };
                    fixture.write(
                        "Cargo.toml",
                        concat!(
                            "[workspace.dependencies]\n",
                            "qdrant-client = { version = \"=1.2.3\", default-features = false }\n",
                        ),
                    );
                    fixture.write(
                        "Cargo.lock",
                        concat!(
                            "version = 4\n\n[[package]]\n",
                            "name = \"qdrant-client\"\nversion = \"1.2.3\"\n",
                        ),
                    );
                    fixture.write(
                        &format!("{BRIDGE}/Cargo.toml"),
                        "[dependencies]\nqdrant-client.workspace = true\n",
                    );
                    fixture.write(
                        &format!("{BRIDGE}/src/qualified.rs"),
                        concat!(
                            "pub const QUALIFIED_CLIENT_VERSION: &str = \"1.2.3\";\n",
                            "pub const QUALIFIED_SERVER_VERSION: &str = \"2.3.4\";\n",
                        ),
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
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_cross_file_failure(fixture: &Fixture, relative: &str, line: usize) {
    let report = validate_qdrant_boundary(&fixture.root);
    let location = format!("{relative}:{line}:");
    assert!(
        report.errors.iter().any(|error| {
            error.contains(&location) && error.contains("cross-file alias")
        }),
        "missing {location}: {:?}",
        report.errors,
    );
}

fn write_vendor_alias(fixture: &Fixture) {
    fixture.write(
        &format!("{BRIDGE}/src/private.rs"),
        "pub(crate) type VendorClient = qdrant_client::Qdrant;\n",
    );
}

#[test]
fn direct_crate_path_cannot_enter_a_public_signature() {
    let fixture = Fixture::new();
    write_vendor_alias(&fixture);
    fixture.write(
        &format!("{BRIDGE}/src/lib.rs"),
        concat!(
            "mod private;\n",
            "pub fn client() -> crate::private::VendorClient;\n",
        ),
    );

    assert_cross_file_failure(&fixture, &format!("{BRIDGE}/src/lib.rs"), 2);
}

#[test]
fn split_qualified_path_cannot_enter_a_public_enum() {
    let fixture = Fixture::new();
    write_vendor_alias(&fixture);
    fixture.write(
        &format!("{BRIDGE}/src/lib.rs"),
        concat!(
            "mod private;\n",
            "pub enum ResultValue {\n",
            "    Client(crate::\n",
            "        private::VendorClient),\n",
            "}\n",
        ),
    );

    assert_cross_file_failure(&fixture, &format!("{BRIDGE}/src/lib.rs"), 2);
}

#[test]
fn exported_macro_cannot_hide_a_qualified_vendor_alias() {
    let fixture = Fixture::new();
    write_vendor_alias(&fixture);
    fixture.write(
        &format!("{BRIDGE}/src/lib.rs"),
        concat!(
            "mod private;\n",
            "#[macro_export]\n",
            "macro_rules! leaked_client {\n",
            "    () => { $crate::private::VendorClient };\n",
            "}\n",
        ),
    );

    assert_cross_file_failure(&fixture, &format!("{BRIDGE}/src/lib.rs"), 2);
}

#[test]
fn super_path_is_resolved_from_the_current_module() {
    let fixture = Fixture::new();
    write_vendor_alias(&fixture);
    fixture.write(
        &format!("{BRIDGE}/src/api/mod.rs"),
        "pub fn client() -> super::private::VendorClient;\n",
    );

    assert_cross_file_failure(
        &fixture,
        &format!("{BRIDGE}/src/api/mod.rs"),
        1,
    );
}

#[test]
fn unrelated_qualified_type_and_private_use_remain_allowed() {
    let fixture = Fixture::new();
    write_vendor_alias(&fixture);
    fixture.write(
        &format!("{BRIDGE}/src/owned.rs"),
        "pub struct VendorClient;\n",
    );
    fixture.write(
        &format!("{BRIDGE}/src/lib.rs"),
        concat!(
            "mod owned;\n",
            "mod private;\n",
            "pub fn client() -> crate::owned::VendorClient;\n",
            "fn internal() -> crate::private::VendorClient { todo!() }\n",
        ),
    );

    let report = validate_qdrant_boundary(&fixture.root);
    assert!(report.passed(), "{:?}", report.errors);
}
