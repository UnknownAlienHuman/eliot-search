//! Source-manifest invariants supplement, not replace, Cargo metadata checks.

fn table<'a>(manifest: &'a str, kind: &str, name: &str) -> &'a str {
    let heading = format!("[[{kind}]]");
    let name = format!("name = \"{name}\"");
    manifest.split(&heading).skip(1)
        .map(|tail| tail.split("\n[").next().unwrap())
        .find(|body| body.lines().any(|line| line == name))
        .expect("explicit target must remain present")
}

#[test]
fn only_primary_entrypoints_are_installable_binaries() {
    for (manifest, primary) in [
        (include_str!("../Cargo.toml"), "eliot-searchd"),
        (include_str!("../../eliot-search/Cargo.toml"), "eliot-search"),
    ] {
        assert!(manifest.contains("autobins = false"));
        assert!(manifest.contains("autoexamples = false"));
        assert_eq!(manifest.matches("[[bin]]").count(), 1);
        assert!(!manifest.contains("[[example]]"));
        assert!(table(manifest, "bin", primary).contains("path = \"src/entry.rs\""));
    }
}

#[test]
fn all_legacy_targets_retain_harness_tests_without_feature_hiding() {
    let daemon = include_str!("../Cargo.toml");
    let cli = include_str!("../../eliot-search/Cargo.toml");
    assert_eq!(daemon.matches("[[test]]").count(), 7);
    assert_eq!(cli.matches("[[test]]").count(), 1);
    for (manifest, name) in [
        (daemon, "eliot-search-snapshotd"),
        (daemon, "eliot-search-sealed-authority"),
        (daemon, "eliot-search-sealed-catalog"),
        (daemon, "eliot-search-sealed-direct"),
        (daemon, "eliot-search-sealed-recover"),
        (daemon, "eliot-search-sealed-store"),
        (daemon, "eliot-search-sealed-transaction"),
        (cli, "eliot-search-snapshot"),
    ] {
        let target = table(manifest, "test", name);
        assert!(target.contains("harness = true"));
        assert!(target.contains("test = true"));
        assert!(!target.contains("required-features"));
    }
}
