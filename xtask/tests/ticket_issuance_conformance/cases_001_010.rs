use crate::common::*;

#[test]
fn plan2_001_missing_selection() {
    let fixture = FixtureRepository::new();
    let build = build(&fixture, &options("search-contracts"));
    assert_eq!(decision(&build), DECISION_MISSING);
    assert!(reasons(&build).is_empty());
    assert_non_authoritative(&build);
}


#[test]
fn plan2_002_deterministic_replay() {
    let fixture = FixtureRepository::new();
    let options = options("search-contracts");
    let first = build(&fixture, &options);
    let second = build(&fixture, &options);
    assert_eq!(first.plan_bytes(), second.plan_bytes());
}


#[test]
fn plan2_003_complete_contracts_selection() {
    let fixture = FixtureRepository::new();
    let build = build(
        &fixture,
        &complete_options(&fixture, "search-contracts"),
    );
    assert_eq!(decision(&build), DECISION_READY);
    assert!(reasons(&build).is_empty());
}


#[test]
fn plan2_004_partial_selection() {
    let fixture = FixtureRepository::new();
    let mut options = options("search-contracts");
    options.base_commit = Some(fixture.tagged_head());
    let build = build(&fixture, &options);
    assert_eq!(decision(&build), DECISION_CONFLICT);
    assert_reason(&build, "PARTIAL_ISSUANCE_SELECTION");
}


#[test]
fn plan2_005_invalid_actor_identity() {
    let fixture = FixtureRepository::new();
    let mut options = complete_options(&fixture, "search-contracts");
    options.writer = Some("display name".to_owned());
    let build = build(&fixture, &options);
    assert_eq!(decision(&build), DECISION_INVALID);
    assert_reason(&build, "ACTOR_IDENTITY_INVALID");
}


#[test]
fn plan2_006_writer_reviewer_collision() {
    let fixture = FixtureRepository::new();
    let mut options = complete_options(&fixture, "search-contracts");
    options.writer = Some("actor:service:same".to_owned());
    options.reviewer = Some("actor:service:same".to_owned());
    let build = build(&fixture, &options);
    assert_eq!(decision(&build), DECISION_CONFLICT);
    assert_reason(&build, "WRITER_REVIEWER_CONFLICT");
}


#[test]
fn plan2_007_abbreviated_base_commit() {
    let fixture = FixtureRepository::new();
    let mut options = options("search-contracts");
    options.base_commit = Some("deadbeef".to_owned());
    options.writer = Some("actor:service:writer".to_owned());
    options.reviewer = Some("actor:reviewer:reviewer".to_owned());
    let build = build(&fixture, &options);
    assert_eq!(decision(&build), DECISION_INVALID);
    assert_reason(&build, "BASE_COMMIT_INVALID");
}


#[test]
fn plan2_008_working_tree_is_ignored() {
    let fixture = FixtureRepository::new();
    fixture.replace_once(
        "swarm/ticket-drafts/p00/search-contracts.toml",
        "claimable = false",
        "claimable = true",
    );
    let build = build(&fixture, &options("search-contracts"));
    assert_eq!(decision(&build), DECISION_MISSING);
    assert!(!reasons(&build).contains("DRAFT_BECAME_CLAIMABLE"));
}


#[test]
fn plan2_009_claimable_committed_draft() {
    let fixture = FixtureRepository::new();
    fixture.replace_once(
        "swarm/ticket-drafts/p00/search-contracts.toml",
        "claimable = false",
        "claimable = true",
    );
    fixture.commit("claimable draft");
    let build = build(&fixture, &options("search-contracts"));
    assert_eq!(decision(&build), DECISION_INVALID);
    assert_reason(&build, "DRAFT_BECAME_CLAIMABLE");
}


#[test]
fn plan2_010_premature_ticket_identity() {
    let fixture = FixtureRepository::new();
    fixture.replace_once(
        "swarm/ticket-drafts/p00/search-contracts.toml",
        "ticket_id = \"UNASSIGNED\"",
        "ticket_id = \"ticket-1\"",
    );
    fixture.commit("premature identity");
    let build = build(&fixture, &options("search-contracts"));
    assert_reason(&build, "DRAFT_IDENTITY_PREMATURELY_RESOLVED");
}

