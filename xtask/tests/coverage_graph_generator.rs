use std::path::{Path, PathBuf};

use xtask::coverage_graph_generation::{
    CoverageGraphGenerationMode, generate_coverage_graph,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under repository root")
        .to_owned()
}

#[test]
fn committed_coverage_derived_files_are_current() {
    let report = generate_coverage_graph(
        &repository_root(),
        CoverageGraphGenerationMode::Check,
    );
    assert!(
        report.passed(),
        "coverage reconciliation failed: stale={:?}, errors={:?}",
        report.stale,
        report.errors
    );
    assert_eq!(report.operations, 664);
    assert_eq!(report.documentation_nodes, 3499);
    assert_eq!(report.dependency_edges, 206);
    assert_eq!(report.modules, 479);
    assert!(report.weak_modules.is_empty());
}
