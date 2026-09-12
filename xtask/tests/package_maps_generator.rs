use std::path::{Path, PathBuf};

use xtask::package_maps::{
    PackageMapGenerationMode, generate_package_maps,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under repository root")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn checked_in_package_maps_match_the_rust_generator() {
    let root = repository_root();
    let report = generate_package_maps(&root, PackageMapGenerationMode::Check);
    assert!(
        report.passed(),
        "stale={:?} errors={:?}",
        report.stale_files,
        report.errors
    );
}

#[test]
fn package_map_generation_no_longer_requires_python() {
    let root = repository_root();
    let wrapper = read(&root, "tools/generate-package-maps-v2.ps1");
    let lower = wrapper.to_ascii_lowercase();
    assert!(lower.contains("cargo"));
    assert!(lower.contains("--locked"));
    assert!(lower.contains("generate"));
    assert!(lower.contains("package-maps"));
    assert!(!lower.contains("python"));

    assert!(!root.join("tools/package_maps_v2.py").exists());
    assert!(!root.join("tools/generate-package-maps-v2.py").exists());

    let workflow = read(&root, ".github/workflows/package-map-coverage-v2.yml");
    assert!(workflow.contains("generate package-maps --check --json"));
    assert!(!workflow.contains("generate-package-maps-v2.py"));
    assert!(!workflow.contains("package_maps_v2.py"));
}
