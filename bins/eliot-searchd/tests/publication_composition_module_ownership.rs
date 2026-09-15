use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn publication_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/publication_composition.rs");
    assert!(entry.contains("#[path = \"publication_composition/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 2_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub struct Publisher",
        "pub trait QdrantCompensate",
        "pub enum RecoveryDecision",
        "PublicationFloor",
        "FileJournal",
        "qdrant_client",
        "tokio::",
    ] {
        assert!(
            !entry.contains(forbidden),
            "publication implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/publication_composition/kernel.rs");
    for module in [
        "compensation",
        "guards",
        "publisher",
        "recovery",
        "retirement",
        "spec",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use search_contracts::PublicationGuards;"));
    assert!(kernel.contains("PublicationFloor, cas"));
    assert!(kernel.len() < 3_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn publication_responsibilities_remain_bounded_and_vendor_neutral() {
    let root = crate_root();
    let owners = [
        (
            "src/publication_composition/kernel/spec.rs",
            "pub enum PublisherError",
        ),
        (
            "src/publication_composition/kernel/guards.rs",
            "pub struct LiveGuardRead",
        ),
        (
            "src/publication_composition/kernel/compensation.rs",
            "pub trait QdrantCompensate",
        ),
        (
            "src/publication_composition/kernel/retirement.rs",
            "pub struct RetiredManifest",
        ),
        (
            "src/publication_composition/kernel/recovery.rs",
            "pub enum RecoveryDecision",
        ),
        (
            "src/publication_composition/kernel/publisher.rs",
            "pub struct Publisher",
        ),
    ];

    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 14_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "qdrant_client",
            "search_qdrant_bridge",
            "reqwest::",
            "tokio::",
            "RealDataPlane::",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired vendor/runtime token {forbidden}"
            );
        }
    }

    let compensation =
        read(&root, "src/publication_composition/kernel/compensation.rs");
    assert!(compensation.contains("not itself a production `RealDataPlane`"));
    assert!(compensation.contains("QDRANT_MUTATION_OUTCOME_UNKNOWN"));
    assert!(compensation.contains("QDRANT_OPERATION_CONFLICT"));
    assert!(compensation.contains("QDRANT_EXACT_READBACK_MISMATCH"));
    assert!(!compensation.contains("CollectionRoute"));
    assert!(!compensation.contains("PointRecord"));
    assert!(!compensation.contains("OpContext"));

    let publisher = read(&root, "src/publication_composition/kernel/publisher.rs");
    assert!(publisher.contains("FileJournal"));
    assert!(publisher.contains("JournalPersistOutcome::ReplayIdentical"));
    assert!(publisher.contains("cas(&self.floor"));
    assert!(!publisher.contains("CompensatePointId"));
    assert!(!publisher.contains("QdrantCompensate"));
    assert!(!publisher.contains("std::fs"));

    let recovery = read(&root, "src/publication_composition/kernel/recovery.rs");
    assert!(recovery.contains("CompensateExact"));
    assert!(!recovery.contains("FileJournal"));
    assert!(!recovery.contains("PublicationFloor"));

    let retirement =
        read(&root, "src/publication_composition/kernel/retirement.rs");
    assert!(retirement.contains("MAX_RETIRED_IDS"));
    assert!(!retirement.contains("fn delete"));
    assert!(!retirement.contains("remove_file"));
}

#[test]
fn publication_contracts_and_false_adapter_claim_stay_closed() {
    let root = crate_root();
    let spec = read(&root, "src/publication_composition/kernel/spec.rs");
    for code in [
        "CONTROL_CONFLICT",
        "EPOCH_MISMATCH",
        "ABANDON_FENCE_MISSING",
        "JOURNAL_CONFLICT",
        "JOURNAL_CORRUPT",
        "JOURNAL_OUTCOME_UNKNOWN",
        "PUBLISHER_BUDGET_EXCEEDED",
    ] {
        assert!(spec.contains(code), "lost publisher reason {code}");
    }
    assert!(spec.contains("pub const MAX_RETIRED_IDS: usize = 1_024;"));

    let entry = read(&root, "src/publication_composition.rs");
    assert!(!entry.contains("forward to"));
    assert!(!entry.contains("methods of the same names"));
    assert!(entry.contains("point identifiers alone"));

    let publisher = read(&root, "src/publication_composition/kernel/publisher.rs");
    for invariant in [
        "if self.active.is_some()",
        "if epoch != expected",
        "guards != self.live.guards()",
        "self.last_reserved = epoch",
        "if active.kind == CommitKind::Full",
        "if !fence.is_full()",
    ] {
        assert!(publisher.contains(invariant), "lost invariant {invariant}");
    }
}
