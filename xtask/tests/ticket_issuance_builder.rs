use std::path::{Path, PathBuf};

use serde_json::Value;
use xtask::ticket_issuance_builder::{
    TicketIssuanceBuildOptions, build_plan,
};
use xtask::ticket_planner::{
    DECISION_CONFLICT, DECISION_INVALID, DECISION_MISSING, plan_digest,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under repository root")
        .to_owned()
}

#[test]
fn current_zero_selection_is_deterministic_and_non_authoritative() {
    let root = repository_root();
    let options = TicketIssuanceBuildOptions::new("search-contracts");
    let first = build_plan(&root, &options).expect("current advisory plan");
    let second = build_plan(&root, &options).expect("deterministic replay");
    assert_eq!(first.plan_bytes(), second.plan_bytes());
    assert!(first.output_target().is_none());
    assert_eq!(
        first.plan().get("decision").and_then(Value::as_str),
        Some(DECISION_MISSING)
    );
    assert!(
        first
            .plan()
            .get("reason_codes")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    );
    assert!(
        first
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
        assert_eq!(first.plan().get(field).and_then(Value::as_bool), Some(false));
    }
    assert_eq!(
        first
            .plan()
            .pointer("/repository/working_tree_used_as_input")
            .and_then(Value::as_bool),
        Some(false)
    );

    let mut payload = first
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
fn partial_selection_is_a_conflict_without_authority() {
    let root = repository_root();
    let mut options = TicketIssuanceBuildOptions::new("search-contracts");
    options.writer = Some("actor:service:writer-01".to_owned());
    let build = build_plan(&root, &options).expect("partial advisory plan");
    assert_eq!(
        build.plan().get("decision").and_then(Value::as_str),
        Some(DECISION_CONFLICT)
    );
    assert!(
        build
            .plan()
            .get("reason_codes")
            .and_then(Value::as_array)
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason.as_str() == Some("PARTIAL_ISSUANCE_SELECTION")
            }))
    );
    assert_eq!(
        build
            .plan()
            .get("authorizes_ticket_issuance")
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn output_outside_advisory_root_is_invalid_and_not_writable() {
    let root = repository_root();
    let mut options = TicketIssuanceBuildOptions::new("search-contracts");
    options.output = "swarm/tickets/plan.json".to_owned();
    let build = build_plan(&root, &options).expect("invalid-output advisory plan");
    assert!(build.output_target().is_none());
    assert_eq!(
        build.plan().get("decision").and_then(Value::as_str),
        Some(DECISION_INVALID)
    );
    assert!(
        build
            .plan()
            .get("reason_codes")
            .and_then(Value::as_array)
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason.as_str() == Some("OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT")
            }))
    );
}
