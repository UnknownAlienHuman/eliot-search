//! Cross-file Qdrant public-surface regressions on disposable repositories.

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
                "eliot-qdrant-cross-file-{}-{}",
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

#[test]
fn private_vendor_alias_cannot_be_reexported_from_lib() {
    let fixture = Fixture::new();
    fixture.write(
        &format!("{BRIDGE}/src/private.rs"),
        "pub(crate) type VendorClient = qdrant_client::Qdrant;\n",
    );
    fixture.write(
        &format!("{BRIDGE}/src/lib.rs"),
        "mod private;\npub use crate::private::VendorClient;\n",
    );

    assert_cross_file_failure(&fixture, &format!("{BRIDGE}/src/lib.rs"), 2);
}

#[test]
fn private_cross_file_import_cannot_enter_a_public_signature() {
    let fixture = Fixture::new();
    fixture.write(
        &format!("{BRIDGE}/src/private.rs"),
        "pub(crate) type VendorClient = qdrant_client::Qdrant;\n",
    );
    fixture.write(
        &format!("{BRIDGE}/src/api.rs"),
        concat!(
            "use crate::private::VendorClient as LocalClient;\n",
            "pub fn client() -> LocalClient;\n",
        ),
    );

    assert_cross_file_failure(&fixture, &format!("{BRIDGE}/src/api.rs"), 2);
}

#[test]
fn an_unrelated_same_named_bridge_type_remains_allowed() {
    let fixture = Fixture::new();
    fixture.write(
        &format!("{BRIDGE}/src/private.rs"),
        "type VendorClient = qdrant_client::Qdrant;\n",
    );
    fixture.write(
        &format!("{BRIDGE}/src/owned.rs"),
        "pub struct VendorClient;\n",
    );
    fixture.write(
        &format!("{BRIDGE}/src/lib.rs"),
        "pub use crate::owned::VendorClient;\n",
    );

    let report = validate_qdrant_boundary(&fixture.root);
    assert!(report.passed(), "{:?}", report.errors);
}
