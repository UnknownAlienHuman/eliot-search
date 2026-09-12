use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) use crate::fixture::FixtureRepository;
pub(crate) use serde_json::Value;
pub(crate) use xtask::ticket_issuance_builder::{
    TicketIssuanceBuild, TicketIssuanceBuildOptions, build_plan, write_plan,
};
pub(crate) use xtask::ticket_planner::{
    DECISION_CONFLICT, DECISION_INVALID, DECISION_MISSING,
    DECISION_PREREQUISITE, DECISION_READY, plan_digest,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under repository root")
        .to_owned()
}

pub(crate) fn build(
    fixture: &FixtureRepository,
    options: &TicketIssuanceBuildOptions,
) -> TicketIssuanceBuild {
    build_plan(fixture.root(), options).expect("build advisory plan")
}

pub(crate) fn options(package: &str) -> TicketIssuanceBuildOptions {
    TicketIssuanceBuildOptions::new(package)
}

pub(crate) fn complete_options(
    fixture: &FixtureRepository,
    package: &str,
) -> TicketIssuanceBuildOptions {
    let mut result = options(package);
    result.base_commit = Some(fixture.tagged_head());
    result.writer = Some("actor:service:writer-01".to_owned());
    result.reviewer = Some("actor:reviewer:reviewer-01".to_owned());
    result
}

pub(crate) fn decision(build: &TicketIssuanceBuild) -> &str {
    build
        .plan()
        .get("decision")
        .and_then(Value::as_str)
        .expect("plan decision")
}

pub(crate) fn reasons(build: &TicketIssuanceBuild) -> BTreeSet<String> {
    build
        .plan()
        .get("reason_codes")
        .and_then(Value::as_array)
        .expect("reason array")
        .iter()
        .map(|value| value.as_str().expect("reason string").to_owned())
        .collect()
}

pub(crate) fn assert_reason(build: &TicketIssuanceBuild, reason: &str) {
    assert!(
        reasons(build).contains(reason),
        "expected {reason}; got {:?}",
        reasons(build)
    );
}

pub(crate) fn assert_non_authoritative(build: &TicketIssuanceBuild) {
    assert!(
        build
            .plan()
            .get("mutations")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    );
    for field in [
        "authorizes_context_materialization",
        "authorizes_ticket_issuance",
        "creates_writer_lease",
        "authorizes_implementation",
        "publishes_package_handoff",
        "advances_launch_state",
    ] {
        assert_eq!(build.plan().get(field).and_then(Value::as_bool), Some(false));
    }
    let mut payload = build
        .plan()
        .as_object()
        .expect("plan object")
        .clone();
    let digest = payload
        .remove("plan_sha256")
        .and_then(|value| value.as_str().map(str::to_owned))
        .expect("plan digest");
    assert_eq!(digest, plan_digest(&Value::Object(payload)));
}


#[test]
fn case_inventory_is_exact_and_rust_owned() {
    let text = std::fs::read_to_string(
        repository_root().join("qualification/ticket-issuance/cases-v2.toml"),
    )
    .expect("case inventory");
    let document: toml::Value = toml::from_str(&text).expect("case inventory TOML");
    assert_eq!(
        document.get("planner").and_then(toml::Value::as_str),
        Some("xtask/src/ticket_issuance_builder.rs")
    );
    assert_eq!(
        document.get("case_count").and_then(toml::Value::as_integer),
        Some(30)
    );
    let rows = document
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("case rows");
    assert_eq!(rows.len(), 30);
    for (index, row) in rows.iter().enumerate() {
        let expected = format!("PLAN2-{:03}", index + 1);
        assert_eq!(
            row.get("id").and_then(toml::Value::as_str),
            Some(expected.as_str())
        );
    }
}

