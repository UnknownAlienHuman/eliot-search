//! Bounded port of pure `tools/package_maps_v2.py` helpers (T41, package-maps).
//!
//! Covers only deterministic helpers: registry path constants, `bool_text`,
//! `string_list`, `dependency_cycle` (Kahn, faithful including the
//! unknown-producer indegree quirk), `package_paths`, and the
//! expected-vs-actual symmetric difference shared by `generate --check`
//! (`check_outputs`) and the validator orphan check.
//! `digest_text` is intentionally NOT duplicated: reuse
//! [`crate::coverage_graph::digest_text`] (identical
//! `hashlib.sha256(...).hexdigest()` semantics, pinned by `digest_dup_check`
//! in the parity vectors).
//! Graph derivation (`build_graph`), TOML renders, manifest patching,
//! `write_outputs` and both `generate`/`validate` entrypoints remain
//! Python-owned (see report).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// `MAP_ROOT` registry location for per-package maps.
pub const MAP_ROOT: &str = "swarm/coverage/package-maps";
/// `INDEX_PATH` package-to-map index with exact digests.
pub const INDEX_PATH: &str = "swarm/coverage/package-map-index.toml";
/// `DOC_INDEX_PATH` documentation file reverse index.
pub const DOC_INDEX_PATH: &str = "swarm/coverage/documentation-file-index-v2.toml";
/// `INTEGRATION_PATH` explicit non-crate documentation map.
pub const INTEGRATION_PATH: &str = "swarm/coverage/integration-map-v2.toml";
/// `HUMAN_INDEX_PATH` Markdown entry point for package writers.
pub const HUMAN_INDEX_PATH: &str = "docs/handoff/PACKAGE_MAP_INDEX_V2.md";

/// `bool_text`: `"true"` / `"false"`.
#[must_use]
pub const fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// `string_list`: keep the string items of a JSON array, else empty.
///
/// Non-string items are dropped; any non-array value yields empty
/// (mirrors `isinstance(value, list)` returning `[]`).
#[must_use]
pub fn string_list(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

/// `dependency_cycle`: Kahn residuals over `{package: [deps]}`, sorted.
///
/// Every listed producer raises consumer indegree, even producers outside the
/// known package set (faithful Python quirk: such consumers are reported as
/// cyclic). Callers pass `{package: string_list(row["deps"])}` projections.
#[must_use]
pub fn dependency_cycle(deps: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut indegree: BTreeMap<&str, usize> =
        deps.keys().map(|package| (package.as_str(), 0)).collect();
    let mut outgoing: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (consumer, producers) in deps {
        for producer in producers {
            outgoing
                .entry(producer.as_str())
                .or_default()
                .push(consumer.as_str());
            if let Some(degree) = indegree.get_mut(consumer.as_str()) {
                *degree += 1;
            }
        }
    }
    // `BTreeMap` iteration is key-sorted, matching Python's
    // `sorted(... indegree == 0)` seed order.
    let mut queue: VecDeque<&str> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(package, _)| *package)
        .collect();
    while let Some(package) = queue.pop_front() {
        if let Some(consumers) = outgoing.get(package) {
            let mut ordered = consumers.clone();
            ordered.sort_unstable();
            for consumer in ordered {
                if let Some(degree) = indegree.get_mut(consumer) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(consumer);
                    }
                }
            }
        }
    }
    indegree
        .into_iter()
        .filter(|(_, degree)| *degree > 0)
        .map(|(package, _)| package.to_owned())
        .collect()
}

/// Four package-local map paths for `package`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePaths {
    /// Per-package overview map path.
    pub overview: String,
    /// Per-package operations map path.
    pub operations: String,
    /// Per-package documents map path.
    pub documents: String,
    /// Per-package relations map path.
    pub relations: String,
}

/// `package_paths`: `{MAP_ROOT}/{package}/{overview,operations,documents,relations}.toml`.
#[must_use]
pub fn package_paths(package: &str) -> PackagePaths {
    let root = format!("{MAP_ROOT}/{package}");
    PackagePaths {
        overview: format!("{root}/overview.toml"),
        operations: format!("{root}/operations.toml"),
        documents: format!("{root}/documents.toml"),
        relations: format!("{root}/relations.toml"),
    }
}

/// Sorted symmetric difference of expected vs actual generated map files.
///
/// Ports `sorted(expected_package_files ^ actual_package_files)` shared by
/// `generate --check` and the validator orphan/missing check.
#[must_use]
pub fn stale_package_files(expected: &[&str], actual: &[&str]) -> Vec<String> {
    let expected: BTreeSet<&str> = expected.iter().copied().collect();
    let actual: BTreeSet<&str> = actual.iter().copied().collect();
    expected
        .symmetric_difference(&actual)
        .map(|path| (*path).to_owned())
        .collect()
}
