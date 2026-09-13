use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn real_query_operations_stay_split_and_bounded() {
    let root = package_root();
    let facade = read(&root, "src/real/queries.rs");
    assert!(
        facade.len() < 1_000,
        "query facade grew to {} bytes",
        facade.len()
    );
    for module in ["readback", "count", "scroll", "search"] {
        assert!(
            facade.contains(&format!("include!(\"queries/{module}.rs\");")),
            "query facade lost {module} operation family"
        );
    }
    for forbidden in [
        "pub async fn readback_exact",
        "pub async fn count_exact",
        "pub async fn scroll_exact",
        "pub async fn query_filtered",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let expectations = [
        ("src/real/queries/readback.rs", "pub async fn readback_exact"),
        ("src/real/queries/count.rs", "pub async fn count_exact"),
        ("src/real/queries/scroll.rs", "pub async fn scroll_exact"),
        ("src/real/queries/search.rs", "pub async fn query_filtered"),
    ];
    for (relative, operation) in expectations {
        let source = read(&root, relative);
        assert!(source.contains(operation), "{relative} lost {operation}");
        assert!(
            source.len() < 7_500,
            "query module {relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "pub struct",
            "pub enum",
            "std::process",
            "Command::new",
            "NATIVE_EXE_PATH",
        ] {
            assert!(
                !source.contains(forbidden),
                "query module {relative} acquired forbidden surface: {forbidden}"
            );
        }
    }
}

#[test]
fn exact_readback_and_nomination_reject_duplicate_vendor_ids() {
    let root = package_root();
    let readback = read(&root, "src/real/queries/readback.rs");
    assert!(readback.contains("BTreeMap::<QdrantPointId, PointRecord>::new()"));
    assert!(readback.contains("returned.insert(id, point).is_some()"));
    assert!(readback.contains("unexpected.insert(id)"));
    assert!(readback.contains("for id in ids"));

    let search = read(&root, "src/real/queries/search.rs");
    assert!(search.contains("let mut seen = BTreeSet::new();"));
    assert!(search.contains("if !seen.insert(point_id)"));
}
