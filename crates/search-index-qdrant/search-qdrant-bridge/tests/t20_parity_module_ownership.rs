use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn t20_live_parity_stays_split_and_discriminating() {
    let root = package_root();
    let facade = read(&root, "tests/t20_parity_live.rs");
    assert!(
        facade.len() < 1_500,
        "T20 parity facade grew to {} bytes",
        facade.len()
    );
    assert!(facade.contains("t20_parity_live/support.rs"));
    assert!(facade.contains("t20_parity_live/scenario.rs"));
    assert!(!facade.contains("spawn_disposable_server"));
    assert!(!facade.contains("#[tokio::test]"));

    let support = read(&root, "tests/t20_parity_live/support.rs");
    for module in ["live", "model", "snapshot"] {
        assert!(support.contains(&format!("mod {module};")));
    }

    for relative in [
        "tests/t20_parity_live/support/live.rs",
        "tests/t20_parity_live/support/model.rs",
        "tests/t20_parity_live/support/snapshot.rs",
        "tests/t20_parity_live/scenario.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            source.len() < 8_500,
            "T20 parity module {relative} grew to {} bytes",
            source.len()
        );
    }

    let scenario = read(&root, "tests/t20_parity_live/scenario.rs");
    assert_eq!(scenario.matches("#[tokio::test]").count(), 1);
    assert!(scenario.contains(
        "fn t24_live_t20_parity_denied_cannot_move_permitted"
    ));
    assert!(scenario.contains("permitted_point(1, vec![(0, 2.0), (1, 1.0)])"));
    assert!(scenario.contains("permitted_point(2, vec![(0, 1.0)])"));
    assert!(scenario.contains("global IDF must observe the denied population"));
    assert!(scenario.contains("denied docs cannot move permitted scores/order/IDF"));
}
