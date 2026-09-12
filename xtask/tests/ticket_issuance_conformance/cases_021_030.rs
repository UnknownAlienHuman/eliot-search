use crate::common::*;

#[test]
fn plan2_021_valid_conditional_handoff() {
    let fixture = FixtureRepository::new();
    let (handoff, head) = fixture.add_accepted_contracts_handoff(true);
    let mut options = options("search-domain");
    options.base_commit = Some(head);
    options.writer = Some("actor:service:domain-writer".to_owned());
    options.reviewer = Some("actor:reviewer:domain-reviewer".to_owned());
    options.accepted_handoffs = vec![handoff];
    let build = build(&fixture, &options);
    assert_eq!(decision(&build), DECISION_READY);
    assert!(reasons(&build).is_empty());
}


#[test]
fn plan2_022_invalid_handoff_signature() {
    let fixture = FixtureRepository::new();
    let (handoff, head) = fixture.add_accepted_contracts_handoff(false);
    let mut options = options("search-domain");
    options.base_commit = Some(head);
    options.writer = Some("actor:service:domain-writer".to_owned());
    options.reviewer = Some("actor:reviewer:domain-reviewer".to_owned());
    options.accepted_handoffs = vec![handoff];
    let build = build(&fixture, &options);
    assert_reason(&build, "HANDOFF_RECORD_INVALID");
}


#[test]
fn plan2_023_unexpected_handoff_for_contracts() {
    let fixture = FixtureRepository::new();
    let (handoff, head) = fixture.add_accepted_contracts_handoff(true);
    let mut options = options("search-contracts");
    options.base_commit = Some(head);
    options.writer = Some("actor:service:writer".to_owned());
    options.reviewer = Some("actor:reviewer:reviewer".to_owned());
    options.accepted_handoffs = vec![handoff];
    let build = build(&fixture, &options);
    assert_reason(&build, "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS");
}


#[test]
fn plan2_024_current_package_control_record() {
    let fixture = FixtureRepository::new();
    fixture.write_text(
        "swarm/tickets/search-domain/ticket-1.toml",
        "record_kind = \"assignment_ticket_v1\"\n",
    );
    fixture.commit("current package record");
    let build = build(&fixture, &options("search-domain"));
    assert_reason(&build, "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS");
}


#[test]
fn plan2_025_nested_readme_is_not_metadata() {
    let fixture = FixtureRepository::new();
    fixture.write_text(
        "swarm/tickets/search-domain/README.md",
        "# not metadata\n",
    );
    fixture.commit("nested README");
    let build = build(&fixture, &options("search-domain"));
    assert_reason(&build, "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS");
}


#[test]
fn plan2_026_w0_receipt_conflict() {
    let fixture = FixtureRepository::new();
    fixture.write_text(
        "swarm/wave-receipts/W0.toml",
        "record_kind = \"wave_receipt_v1\"\n",
    );
    fixture.commit("W0 receipt");
    let build = build(&fixture, &options("search-contracts"));
    assert_reason(&build, "W0_ALREADY_ACCEPTED");
}


#[test]
fn plan2_027_automatic_workflow_trigger() {
    let fixture = FixtureRepository::new();
    fixture.write_text(
        ".github/workflows/automatic.yml",
        r#"name: Automatic
on:
  push:
permissions:
  contents: read
jobs:
  test:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@0000000000000000000000000000000000000000
        with:
          persist-credentials: false
"#,
    );
    fixture.commit("automatic workflow");
    let build = build(&fixture, &options("search-contracts"));
    assert_reason(&build, "WORKFLOW_POLICY_VIOLATION");
}


#[test]
fn plan2_028_output_outside_artifact_root() {
    let fixture = FixtureRepository::new();
    let mut options = options("search-contracts");
    options.output = "swarm/tickets/plan.json".to_owned();
    let build = build(&fixture, &options);
    assert!(build.output_target().is_none());
    assert_reason(&build, "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT");
}


#[test]
fn plan2_029_output_inside_artifact_root() {
    let fixture = FixtureRepository::new();
    let mut options = options("search-contracts");
    options.output =
        "artifacts/ticket-issuance-plans/contracts.json".to_owned();
    let build = build(&fixture, &options);
    let target = build
        .output_target()
        .expect("validated advisory output")
        .to_owned();
    write_plan(&build).expect("write advisory plan");
    let parsed: Value = serde_json::from_slice(
        &std::fs::read(target).expect("read advisory plan"),
    )
    .expect("parse advisory plan");
    assert_eq!(parsed.get("plan_sha256"), build.plan().get("plan_sha256"));
}


#[test]
fn plan2_030_zero_authority_and_non_circular_digest() {
    let fixture = FixtureRepository::new();
    let build = build(
        &fixture,
        &complete_options(&fixture, "search-contracts"),
    );
    assert_non_authoritative(&build);
}
