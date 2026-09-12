//! Manifest ceilings, handoff topology and exact P00 contract-pack rules.

use std::collections::BTreeSet;

use super::spec::{
    CONTEXT_TOTAL_BYTE_CEILING, HARD_TOTAL_LINES,
    SPLIT_REVIEW_TOTAL_LINES,
};

/// Manifest-owned ceiling selection.
#[must_use]
pub fn select_ceiling(
    ceiling_class: &str,
    package: &str,
    exceptions: &[&str],
    ordinary: i64,
    exact: i64,
) -> (i64, bool) {
    if ceiling_class == "P00_EXACT_CONTRACT_PACK" {
        let class_ok = exceptions.iter().filter(|exception| **exception == package).count() == 1
            && package == "search-contracts";
        return (exact, class_ok);
    }
    (ordinary, ceiling_class == "ORDINARY")
}

/// Expected accepted-handoff slots per P00 package.
#[must_use]
pub fn expected_handoff_slots(package: &str) -> &'static [&'static str] {
    if package == "search-contracts" {
        &[]
    } else {
        &["search-contracts::accepted_package_and_api_handoff"]
    }
}

/// Expected required-handoff packages per P00 ticket.
#[must_use]
pub fn expected_required_handoffs(package: &str) -> &'static [&'static str] {
    if package == "search-contracts" {
        &[]
    } else {
        &["search-contracts"]
    }
}

/// Draft repository-fence line-limit coherence.
#[must_use]
pub const fn line_limits_ok(
    registry_soft: i64,
    ticket_soft: i64,
    split_review: i64,
    hard: i64,
) -> bool {
    0 < registry_soft
        && registry_soft <= ticket_soft
        && ticket_soft <= split_review
        && split_review <= hard
        && split_review == SPLIT_REVIEW_TOTAL_LINES
        && hard == HARD_TOTAL_LINES
}

/// Declared-context total byte ceiling check.
#[must_use]
pub const fn context_total_bytes_ok(total: u64) -> bool {
    total <= CONTEXT_TOTAL_BYTE_CEILING
}

/// Manifest-closed exact P00 source list.
///
/// # Errors
///
/// Returns `DRAFT_MANIFEST_MISMATCH` unless `required_files` is non-empty,
/// duplicate-free and `README.md`-first.
pub fn contract_pack_sources(
    package: &str,
    required_files: &[&str],
) -> Result<Vec<String>, &'static str> {
    const MISMATCH: &str = "DRAFT_MANIFEST_MISMATCH";
    if required_files.is_empty() || required_files[0] != "README.md" {
        return Err(MISMATCH);
    }
    let unique: BTreeSet<&&str> = required_files.iter().collect();
    if unique.len() != required_files.len() {
        return Err(MISMATCH);
    }
    let mut sources = vec![
        "AGENTS.md".to_owned(),
        format!("crates/{package}/AGENTS.md"),
        "docs/handoff/AUTHORITY_MAP.md".to_owned(),
        "swarm/ASSIGNMENT_PROTOCOL.md".to_owned(),
        format!("swarm/assignments/{package}.md"),
        "docs/handoff/P00_BOOTSTRAP.md".to_owned(),
        "docs/contracts/p00/README.md".to_owned(),
        "docs/contracts/p00/manifest.toml".to_owned(),
    ];
    sources.extend(
        required_files[1..]
            .iter()
            .map(|name| format!("docs/contracts/p00/{name}")),
    );
    Ok(sources)
}
