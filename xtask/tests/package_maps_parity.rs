//! Parity tests for the T41 package-maps slice (`xtask` vs Python).
//!
//! Covers only the ported pure helpers from `tools/package_maps_v2.py` and the
//! shared stale-file symmetric difference from
//! `tools/generate-package-maps-v2.py` / `tools/validate-package-maps-v2.py`.
//! Vectors in `fixtures/tooling/package-maps/vectors.json` were captured from
//! `CPython` 3.12 (`probe_pm.py`) and asserted byte-exactly.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};
use xtask::package_maps::{
    DOC_INDEX_PATH, HUMAN_INDEX_PATH, INDEX_PATH, INTEGRATION_PATH, MAP_ROOT, bool_text,
    dependency_cycle, package_paths, stale_package_files, string_list,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at the repository root")
        .to_owned()
}

fn vectors() -> Value {
    let path = repo_root().join("fixtures/tooling/package-maps/vectors.json");
    let text = std::fs::read_to_string(&path).expect("parity vectors exist");
    serde_json::from_str(&text).expect("parity vectors parse")
}

fn str_vec(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}

fn deps_map(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    pairs
        .iter()
        .map(|(consumer, producers)| {
            (
                (*consumer).to_owned(),
                producers.iter().map(|p| (*p).to_owned()).collect(),
            )
        })
        .collect()
}

// constants: all five registry paths byte-exact.
#[test]
fn constants_match_python_pass() {
    let v = vectors();
    assert_eq!(MAP_ROOT, v["map_root"].as_str().unwrap());
    assert_eq!(INDEX_PATH, v["index_path"].as_str().unwrap());
    assert_eq!(DOC_INDEX_PATH, v["doc_index_path"].as_str().unwrap());
    assert_eq!(INTEGRATION_PATH, v["integration_path"].as_str().unwrap());
    assert_eq!(HUMAN_INDEX_PATH, v["human_index_path"].as_str().unwrap());
}

// digest: reuse coverage_graph helper, never duplicated here.
#[test]
fn digest_reuse_not_duplicated_pass() {
    let v = vectors();
    assert_eq!(
        xtask::coverage_graph::digest_text("abc"),
        v["digest_dup_check"].as_str().unwrap()
    );
}

// bool_text: true/false.
#[test]
fn bool_text_both_pass() {
    let v = vectors();
    assert_eq!(bool_text(true), v["bool_true"].as_str().unwrap());
    assert_eq!(bool_text(false), v["bool_false"].as_str().unwrap());
}

// string_list: mixed array filtered, empty/all-non-string empty.
#[test]
fn string_list_mixed_and_empty_pass() {
    let v = vectors();
    assert_eq!(
        string_list(&json!(["a", 1, null, "b", true, 2.5, "c"])),
        str_vec(&v["str_mixed"])
    );
    assert_eq!(string_list(&json!([])), str_vec(&v["str_empty"]));
    assert_eq!(
        string_list(&json!([1, null, true])),
        str_vec(&v["str_all_nonstring"])
    );
}

// string_list: non-array rejected to empty (negative).
#[test]
fn string_list_non_list_rejected() {
    let v = vectors();
    assert_eq!(
        string_list(&json!({"a": 1})),
        str_vec(&v["str_nonlist_dict"])
    );
    assert_eq!(string_list(&Value::Null), str_vec(&v["str_nonlist_none"]));
    assert_eq!(string_list(&json!("ab")), str_vec(&v["str_nonlist_str"]));
    assert_ne!(string_list(&json!(["a"])), string_list(&json!(["b"])));
}

// package_paths: search-contracts byte-exact.
#[test]
fn package_paths_search_contracts_pass() {
    let v = vectors();
    let got = package_paths("search-contracts");
    let want = &v["paths_search_contracts"];
    assert_eq!(got.overview, want["overview"].as_str().unwrap());
    assert_eq!(got.operations, want["operations"].as_str().unwrap());
    assert_eq!(got.documents, want["documents"].as_str().unwrap());
    assert_eq!(got.relations, want["relations"].as_str().unwrap());
}

// package_paths: prefix/suffix/distinct shape (negative).
#[test]
fn package_paths_shape_rejected_when_violated() {
    for package in ["search-contracts", "eliot-searchd"] {
        let got = package_paths(package);
        let paths = [
            &got.overview,
            &got.operations,
            &got.documents,
            &got.relations,
        ];
        for path in paths {
            assert!(path.starts_with(&format!("{MAP_ROOT}/{package}/")));
            assert_eq!(
                std::path::Path::new(path)
                    .extension()
                    .and_then(|e| e.to_str()),
                Some("toml")
            );
        }
        let unique: std::collections::BTreeSet<&str> = paths.iter().map(AsRef::as_ref).collect();
        assert_eq!(unique.len(), 4);
    }
    assert_ne!(
        package_paths("search-contracts"),
        package_paths("eliot-searchd")
    );
}

// cycle: empty/single/DAG/diamond yield none.
#[test]
fn cycle_empty_and_dag_pass() {
    let v = vectors();
    assert_eq!(dependency_cycle(&deps_map(&[])), str_vec(&v["cycle_empty"]));
    assert_eq!(
        dependency_cycle(&deps_map(&[("a", &[])])),
        str_vec(&v["cycle_single"])
    );
    assert_eq!(
        dependency_cycle(&deps_map(&[("a", &["b"]), ("b", &[]), ("c", &["a", "b"]),])),
        str_vec(&v["cycle_dag"])
    );
    assert_eq!(
        dependency_cycle(&deps_map(&[
            ("a", &["b", "c"]),
            ("b", &["d"]),
            ("c", &["d"]),
            ("d", &[]),
        ])),
        str_vec(&v["cycle_diamond"])
    );
}

// cycle: two-cycle/self-loop/three-cycle reported sorted.
#[test]
fn cycle_two_self_three_pass() {
    let v = vectors();
    assert_eq!(
        dependency_cycle(&deps_map(&[("a", &["b"]), ("b", &["a"]), ("c", &[])])),
        str_vec(&v["cycle_two"])
    );
    assert_eq!(
        dependency_cycle(&deps_map(&[("a", &["a"])])),
        str_vec(&v["cycle_self"])
    );
    assert_eq!(
        dependency_cycle(&deps_map(&[("a", &["b"]), ("b", &["c"]), ("c", &["a"])])),
        str_vec(&v["cycle_three"])
    );
}

// cycle: unknown-producer quirk + non-string deps filtered (negative shape).
#[test]
fn cycle_unknown_producer_quirk_pass() {
    let v = vectors();
    assert_eq!(
        dependency_cycle(&deps_map(&[("a", &["zzz"])])),
        str_vec(&v["cycle_unknown"])
    );
    assert_eq!(
        dependency_cycle(&deps_map(&[("a", &["b"]), ("b", &[])])),
        Vec::<String>::new()
    );
    let filtered = string_list(&json!(["b", 1, null]));
    assert_eq!(filtered, vec!["b".to_owned()]);
    let mut projected = BTreeMap::new();
    projected.insert("a".to_owned(), filtered);
    projected.insert("b".to_owned(), Vec::new());
    assert_eq!(
        dependency_cycle(&projected),
        str_vec(&v["cycle_nonstring_deps"])
    );
}

// stale: equal/both-empty yield none.
#[test]
fn stale_equal_and_empty_pass() {
    let v = vectors();
    assert_eq!(
        stale_package_files(&["a", "b"], &["a", "b"]),
        str_vec(&v["stale_equal"])
    );
    assert_eq!(
        stale_package_files(&[], &[]),
        str_vec(&v["stale_both_empty"])
    );
}

// stale: orphan+missing reported, symmetric (negative).
#[test]
fn stale_diff_and_symmetry_pass() {
    let v = vectors();
    assert_eq!(
        stale_package_files(&["a", "b", "c"], &["b", "c", "d"]),
        str_vec(&v["stale_diff"])
    );
    assert_eq!(
        stale_package_files(&["b", "c", "d"], &["a", "b", "c"]),
        str_vec(&v["stale_diff"])
    );
    assert_eq!(
        stale_package_files(&["a", "a", "b"], &["b"]),
        vec!["a".to_owned()]
    );
}
