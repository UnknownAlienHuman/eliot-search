use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    EXPECTED_LAYOUT_DIRECTORIES, EXPECTED_PROFILES,
    validate_integration_bootstrap,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "eliot-search-bootstrap-{}-{id}",
            std::process::id()
        ));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("remove stale fixture");
        }
        std::fs::create_dir_all(root.join(".cargo"))
            .expect("create Cargo fixture directory");
        std::fs::create_dir_all(root.join(".github/workflows"))
            .expect("create workflow fixture directory");
        std::fs::create_dir_all(root.join("config"))
            .expect("create config fixture directory");
        let fixture = Self { root };
        fixture.write_valid_files();
        fixture
    }

    fn write(&self, relative: &str, content: &str) {
        std::fs::write(self.root.join(relative), content)
            .expect("write fixture file");
    }

    fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.root.join(relative))
            .expect("read fixture file")
    }

    fn write_valid_files(&self) {
        self.write(
            "rust-toolchain.toml",
            "[toolchain]\nchannel = \"1.98.0\"\nprofile = \"minimal\"\ncomponents = [\"clippy\", \"rustfmt\"]\ntargets = [\"x86_64-pc-windows-msvc\"]\n",
        );
        self.write(
            ".cargo/config.toml",
            "[alias]\ncheck-all = \"check --workspace --all-targets --locked\"\ntest-all = \"test --workspace --all-targets --locked\"\nclippy-all = \"clippy --workspace --all-targets --locked -- -D warnings\"\ndoc-all = \"doc --workspace --no-deps --locked\"\n",
        );

        let members = (0..45)
            .map(|index| format!("  \"crate-{index}\""))
            .collect::<Vec<_>>()
            .join(",\n");
        self.write(
            "Cargo.toml",
            &format!(
                "[workspace]\nresolver = \"3\"\nmembers = [\n{members}\n]\n[workspace.package]\nedition = \"2024\"\n"
            ),
        );

        let mut profiles = String::from(
            "status = \"FROZEN_BOOTSTRAP_NOT_PRODUCT_ACCEPTED\"\ndefault_profile = \"P00_FOUNDATION\"\nautomatic_profile_upgrade = false\n",
        );
        for profile in EXPECTED_PROFILES {
            profiles.push_str(&format!(
                "[[profile]]\nid = \"{profile}\"\ndefault = {}\n",
                profile == "P00_FOUNDATION"
            ));
        }
        self.write("config/build-profiles-v1.toml", &profiles);

        let directories = EXPECTED_LAYOUT_DIRECTORIES
            .iter()
            .map(|(key, value)| format!("{key} = \"{value}\""))
            .collect::<Vec<_>>()
            .join("\n");
        self.write(
            "config/data-layout-v1.toml",
            &format!(
                "root_must_be_dedicated = true\nroot_must_not_be_repository_checkout = true\nroot_must_not_be_source_identity = true\nowner_only_acl_required = true\ninherited_broad_acl_forbidden = true\nsymlink_or_reparse_escape_forbidden = true\nplaintext_secret_storage_forbidden = true\n[directories]\n{directories}\n[control]\nredb_role = \"CONTROL_JOURNAL_ONLY\"\nsearchable_corpus_forbidden = true\n[qdrant]\nsole_search_index = true\n[runtime]\nunsaved_bytes_must_remain_memory_only = true\n[migration]\nunknown_outcome_requires_quarantine = true\n"
            ),
        );
        self.write(
            ".github/workflows/integration-bootstrap.yml",
            "on:\n  workflow_dispatch:\npermissions:\n  contents: read\nsteps:\n  persist-credentials: false\n",
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn codes(root: &Path, allow_missing_lock: bool) -> BTreeSet<String> {
    validate_integration_bootstrap(root, allow_missing_lock)
        .findings
        .into_iter()
        .map(|finding| finding.code)
        .collect()
}

#[test]
fn valid_preview_without_lock() {
    let fixture = Fixture::new();
    assert!(codes(&fixture.root, true).is_empty());
}

#[test]
fn missing_lock_fails_verification() {
    let fixture = Fixture::new();
    assert!(codes(&fixture.root, false).contains("CARGO_LOCK_MISSING"));
}

#[test]
fn toolchain_drift_fails() {
    let fixture = Fixture::new();
    fixture.write(
        "rust-toolchain.toml",
        "[toolchain]\nchannel = \"stable\"\n",
    );
    assert!(codes(&fixture.root, true).contains("TOOLCHAIN_NOT_EXACT"));
}

#[test]
fn automatic_workflow_trigger_fails() {
    let fixture = Fixture::new();
    let workflow = format!(
        "{}push:\n",
        fixture.read(".github/workflows/integration-bootstrap.yml")
    );
    fixture.write(".github/workflows/integration-bootstrap.yml", &workflow);
    assert!(
        codes(&fixture.root, true)
            .contains("BOOTSTRAP_WORKFLOW_AUTOMATIC_TRIGGER")
    );
}

#[test]
fn redb_search_role_fails() {
    let fixture = Fixture::new();
    let layout = fixture
        .read("config/data-layout-v1.toml")
        .replace("CONTROL_JOURNAL_ONLY", "SEARCH_INDEX");
    fixture.write("config/data-layout-v1.toml", &layout);
    assert!(codes(&fixture.root, true).contains("REDB_ROLE_INVALID"));
}

#[test]
fn optional_profile_cannot_be_default() {
    let fixture = Fixture::new();
    let profiles = fixture
        .read("config/build-profiles-v1.toml")
        .replace(
            "id = \"OPTIONAL_DEPTH\"\ndefault = false",
            "id = \"OPTIONAL_DEPTH\"\ndefault = true",
        );
    fixture.write("config/build-profiles-v1.toml", &profiles);
    assert!(
        codes(&fixture.root, true)
            .contains("BUILD_PROFILE_DEFAULT_NOT_UNIQUE")
    );
}
