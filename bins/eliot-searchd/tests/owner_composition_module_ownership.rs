use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn owner_composition_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/owner_composition.rs");
    assert!(entry.contains("#[path = \"owner_composition/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::{LiveOwner, ShutdownReceipt, establish};"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "struct DurableOwnerRecord",
        "fn load_or_create_installation",
        "fn observe_physical_root",
        "fn newest_valid",
        "fn publish_transition",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "owner implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/owner_composition/kernel.rs");
    for module in [
        "codec",
        "installation",
        "lifecycle",
        "observation",
        "record",
        "slots",
        "spec",
        "succession",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use lifecycle::{LiveOwner, ShutdownReceipt};"));
    assert!(kernel.contains("pub use succession::establish;"));
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 3_000, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn owner_responsibilities_stay_separated_and_bounded() {
    let root = crate_root();
    let owners = [
        ("src/owner_composition/kernel/spec.rs", "enum LifecycleState"),
        (
            "src/owner_composition/kernel/record.rs",
            "struct DurableOwnerRecord",
        ),
        (
            "src/owner_composition/kernel/installation.rs",
            "fn load_or_create_installation(",
        ),
        (
            "src/owner_composition/kernel/observation.rs",
            "fn observe_physical_root(",
        ),
        ("src/owner_composition/kernel/slots.rs", "fn newest_valid("),
        ("src/owner_composition/kernel/lifecycle.rs", "pub struct LiveOwner"),
        ("src/owner_composition/kernel/succession.rs", "pub fn establish("),
        ("src/owner_composition/kernel/codec.rs", "fn native_volume_material("),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 18_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden vendor token {forbidden}"
            );
        }
    }

    let record = read(&root, "src/owner_composition/kernel/record.rs");
    assert!(record.contains("lines.len() != 17"));
    assert!(record.contains("record_digest="));
    assert!(record.contains("OWNER_STATE_MAGIC"));
    assert!(!record.contains("OpenOptions"));
    assert!(!record.contains("current_exe"));

    let installation = read(&root, "src/owner_composition/kernel/installation.rs");
    assert!(installation.contains("create_new(true)"));
    assert!(installation.contains("INSTALLATION_MAGIC"));
    assert!(!installation.contains("OWNER_STATE_MAGIC"));

    let observation = read(&root, "src/owner_composition/kernel/observation.rs");
    assert!(observation.contains("owner-canonical-path/v1"));
    assert!(observation.contains("owner-volume-identity/v1"));
    assert!(observation.contains("owner-executable/v1"));
    assert!(observation.contains("owner-token/v1"));
    assert!(!observation.contains("OWNER_SLOT_A"));

    let slots = read(&root, "src/owner_composition/kernel/slots.rs");
    assert!(slots.contains("SlotRead"));
    assert!(slots.contains("sync_all"));
    assert!(slots.contains("publish_transition"));
    assert!(!slots.contains("current_exe"));

    let lifecycle = read(&root, "src/owner_composition/kernel/lifecycle.rs");
    assert!(lifecycle.contains("begin_drain"));
    assert!(lifecycle.contains("release_cleanly"));
    assert!(lifecycle.contains("OwnerDrainRequired"));
    assert!(!lifecycle.contains("load_or_create_installation"));

    let succession = read(&root, "src/owner_composition/kernel/succession.rs");
    assert!(succession.contains("verify_sealed_head_agrees"));
    assert!(succession.contains("plan_successor"));
    assert!(succession.contains("OwnerEpoch::new"));
    assert!(!succession.contains("OpenOptions"));
}

#[test]
fn owner_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/owner_composition/kernel/tests.rs");
    assert!(tests.len() < 28_000, "owner tests grew to {} bytes", tests.len());
    for case in [
        "fresh_acquisition_starts_at_epoch_one_with_zero_predecessor",
        "record_encoding_round_trips_and_has_exact_shape",
        "strict_decode_rejects_non_canonical_records",
        "successor_advances_epoch_and_links_previous_digest",
        "foreign_installation_denies_succession",
        "copied_state_files_deny_on_a_relocated_root",
        "corrupt_slots_quarantine_without_repair",
        "torn_non_authority_slot_heals_by_guarded_succession",
        "drain_release_lifecycle_is_guarded_and_idempotent",
        "journal_inputs_construct_a_valid_redb_identity_without_relabelling",
        "owner_debug_redacts_the_creation_token",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}
