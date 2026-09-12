use crate::common::*;

#[test]
fn plan2_011_legacy_schema_field() {
    let fixture = FixtureRepository::new();
    fixture.replace_once(
        "swarm/ticket-drafts/p00/search-contracts.toml",
        "writer = \"UNASSIGNED\"",
        "lease_id = \"UNASSIGNED\"\nwriter = \"UNASSIGNED\"",
    );
    fixture.commit("legacy field");
    let build = build(&fixture, &options("search-contracts"));
    assert_reason(&build, "DRAFT_UNKNOWN_FIELD");
}


#[test]
fn plan2_012_exact_contract_pack_drift() {
    let fixture = FixtureRepository::new();
    fixture.replace_once(
        "swarm/context-drafts/p00/search-contracts.toml",
        "  \"docs/contracts/p00/TYPE_REGISTRY.md\"",
        "  \"docs/contracts/p00/CANONICAL_TYPES.md\"",
    );
    fixture.commit("contract pack drift");
    let build = build(&fixture, &options("search-contracts"));
    assert_reason(&build, "DRAFT_MANIFEST_MISMATCH");
}


#[test]
fn plan2_013_ordinary_source_ceiling_exceeded() {
    let fixture = FixtureRepository::new();
    let path = "swarm/context-drafts/p00/search-domain.toml";
    fixture.replace_once(path, "source_file_count = 2", "source_file_count = 17");
    let old = concat!(
        "source_files = [\n",
        "  \"AGENTS.md\",\n",
        "  \"crates/search-domain/AGENTS.md\"\n",
        "]",
    );
    let mut values = vec![
        "AGENTS.md".to_owned(),
        "crates/search-domain/AGENTS.md".to_owned(),
    ];
    for index in 0..15 {
        let relative = format!("docs/domain-extra-{index}.md");
        fixture.write_text(&relative, "# extra\n");
        values.push(relative);
    }
    let new = format!(
        "source_files = [\n{}\n]",
        values
            .iter()
            .map(|value| format!(
                "  {}",
                serde_json::to_string(value).expect("JSON string")
            ))
            .collect::<Vec<_>>()
            .join(",\n")
    );
    fixture.replace_once(path, old, &new);
    fixture.commit("ordinary source ceiling");
    let build = build(&fixture, &options("search-domain"));
    assert_reason(&build, "CONTEXT_BUDGET_EXCEEDED");
}


#[test]
fn plan2_014_context_source_missing() {
    let fixture = FixtureRepository::new();
    fixture.remove("crates/search-domain/AGENTS.md");
    fixture.commit("remove context source");
    let build = build(&fixture, &options("search-domain"));
    assert_reason(&build, "CONTEXT_SOURCE_MISSING");
}


#[test]
fn plan2_015_context_source_symlink() {
    let fixture = FixtureRepository::new();
    fixture.commit_index_symlink("crates/search-domain/AGENTS.md", "../target");
    let build = build(&fixture, &options("search-domain"));
    assert_reason(&build, "CONTEXT_SOURCE_NOT_REGULAR");
}


#[test]
fn plan2_016_context_source_not_utf8() {
    let fixture = FixtureRepository::new();
    fixture.write_bytes("crates/search-domain/AGENTS.md", &[0xff, 0xfe]);
    fixture.commit("non UTF-8 context source");
    let build = build(&fixture, &options("search-domain"));
    assert_reason(&build, "CONTEXT_SOURCE_NOT_UTF8");
}


#[test]
fn plan2_017_forbidden_context_source() {
    let fixture = FixtureRepository::new();
    fixture.write_text("docs/architecture/secret.md", "# no\n");
    fixture.replace_once(
        "swarm/context-drafts/p00/search-domain.toml",
        "  \"crates/search-domain/AGENTS.md\"",
        "  \"docs/architecture/secret.md\"",
    );
    fixture.commit("forbidden context source");
    let build = build(&fixture, &options("search-domain"));
    assert_reason(&build, "CONTEXT_SOURCE_FORBIDDEN");
}


#[test]
fn plan2_018_invalid_selector_grammar() {
    let fixture = FixtureRepository::new();
    fixture.replace_once(
        "swarm/context-drafts/p00/search-domain.toml",
        "swarm/crates.toml::package[name=search-domain]",
        "swarm/crates.toml::package[package=search-domain]",
    );
    fixture.commit("invalid selector");
    let build = build(&fixture, &options("search-domain"));
    assert_reason(&build, "CONTEXT_SELECTOR_INVALID");
}


#[test]
fn plan2_019_selector_not_unique() {
    let fixture = FixtureRepository::new();
    fixture.append_text(
        "swarm/crates.toml",
        concat!(
            "\n[[package]]\n",
            "name = \"search-domain\"\n",
            "path = \"crates/search-domain\"\n",
            "family = \"foundation\"\n",
            "wave = 0\n",
            "soft_src_line_target = 7000\n",
            "assignment = \"swarm/assignments/search-domain.md\"\n",
        ),
    );
    fixture.commit("duplicate package selector");
    let build = build(&fixture, &options("search-domain"));
    let actual = reasons(&build);
    assert!(
        actual.contains("PACKAGE_UNKNOWN")
            || actual.contains("CONTEXT_SELECTOR_NOT_UNIQUE"),
        "unexpected reasons: {actual:?}"
    );
}


#[test]
fn plan2_020_missing_conditional_handoff() {
    let fixture = FixtureRepository::new();
    let build = build(&fixture, &options("search-domain"));
    assert_eq!(decision(&build), DECISION_PREREQUISITE);
    assert_reason(&build, "HANDOFF_SLOT_UNSATISFIED");
}

