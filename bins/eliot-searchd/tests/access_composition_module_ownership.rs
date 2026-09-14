use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn access_composition_stays_split_between_gate_and_regressions() {
    let root = package_root();
    let facade = read(&root, "src/access_composition.rs");
    assert!(
        facade.len() < 1_500,
        "access facade grew to {} bytes",
        facade.len()
    );
    assert!(facade.contains("mod gate;"));
    assert!(facade.contains("pub use gate::*;"));
    assert!(facade.contains("mod tests;"));
    for forbidden in [
        "pub fn admit_pre_retrieval(",
        "pub fn recheck_before_emission(",
        "pub fn discard_contaminated_legs(",
        "struct Bundle",
    ] {
        assert!(
            !facade.contains(forbidden),
            "access facade reacquired {forbidden}"
        );
    }

    let gate = read(&root, "src/access_composition/gate.rs");
    assert!(gate.contains("pub fn admit_pre_retrieval("));
    assert!(gate.contains("pub fn recheck_before_emission("));
    assert!(gate.contains("pub fn recheck_before_expansion("));
    assert!(gate.contains("pub fn discard_contaminated_legs("));
    assert!(gate.contains("pub const fn deny_local_token_as_authority("));
    assert!(gate.contains("AccessError::LocalIdentityNotAuthority"));
    assert!(gate.len() < 12_000, "access gate grew to {} bytes", gate.len());
    for forbidden in [
        "#[cfg(test)]",
        "std::fs",
        "std::process",
        "qdrant_client",
        "search_qdrant",
        "reqwest::",
        "tokio::",
    ] {
        assert!(
            !gate.contains(forbidden),
            "access gate acquired forbidden token {forbidden}"
        );
    }

    let regressions = read(&root, "src/access_composition/tests.rs");
    for case in [
        "revoked_grant_denies_before_provider_dispatch",
        "live_purge_denies_admission",
        "emission_recheck_fires_on_new_revocation",
        "expansion_reauthorizes_live_state",
        "contaminated_legs_discarded_whole",
        "local_token_never_admits",
        "qdrant_parity_deferred",
    ] {
        assert!(
            regressions.contains(&format!("fn {case}(")),
            "access regression corpus lost {case}"
        );
    }
    assert!(
        regressions.len() < 20_000,
        "access regression corpus grew to {} bytes",
        regressions.len()
    );
}
