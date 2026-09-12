//! Pure package-map helper parity with the retired Python implementation.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Per-package map root.
pub const MAP_ROOT: &str = "swarm/coverage/package-maps";
/// Package-to-map index.
pub const INDEX_PATH: &str = "swarm/coverage/package-map-index.toml";
/// Documentation file reverse index.
pub const DOC_INDEX_PATH: &str =
    "swarm/coverage/documentation-file-index-v2.toml";
/// Explicit non-crate documentation map.
pub const INTEGRATION_PATH: &str =
    "swarm/coverage/integration-map-v2.toml";
/// Human package-map index.
pub const HUMAN_INDEX_PATH: &str = "docs/handoff/PACKAGE_MAP_INDEX_V2.md";

/// Returns the canonical TOML boolean text.
#[must_use]
pub const fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// Keeps only string members of a JSON array; other values yield an empty list.
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

/// Returns sorted Kahn residuals for a package dependency map.
///
/// Unknown producers deliberately increase the consumer indegree to preserve
/// the retired implementation's fail-closed behavior.
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

/// Four package-local map paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePaths {
    /// Per-package overview.
    pub overview: String,
    /// Per-package operation map.
    pub operations: String,
    /// Per-package documentation map.
    pub documents: String,
    /// Per-package relation map.
    pub relations: String,
}

/// Returns the four deterministic package-local paths.
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

/// Returns the sorted symmetric difference of expected and actual files.
#[must_use]
pub fn stale_package_files(expected: &[&str], actual: &[&str]) -> Vec<String> {
    let expected: BTreeSet<&str> = expected.iter().copied().collect();
    let actual: BTreeSet<&str> = actual.iter().copied().collect();
    expected
        .symmetric_difference(&actual)
        .map(|path| (*path).to_owned())
        .collect()
}
