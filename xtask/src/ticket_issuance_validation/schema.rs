use std::collections::BTreeSet;

use toml::Value;

use crate::ticket_planner::{
    CLOSED_REASON_CODES, DECISION_CONFLICT, DECISION_INVALID, DECISION_MISSING,
    DECISION_PREREQUISITE, DECISION_READY, PLAN_ARTIFACT_ROOT, RECORD_KIND,
    SCHEMA_VERSION, STATUS,
};

use super::spec::{FALSE_INVARIANTS, TRUE_INVARIANTS};
use super::{Validation, boolean, integer, string, string_array};

pub(super) fn validate_schema(
    schema: &Value,
    digest: &Value,
    cases: &Value,
    validation: &mut Validation,
) {
    validation.require(
        integer(schema, "schema_version") == Some(SCHEMA_VERSION)
            && string(schema, "record_kind") == Some(RECORD_KIND)
            && string(schema, "status") == Some(STATUS),
        "schema-identity",
        "plan schema identity matches Rust planner constants",
    );

    let expected_decisions = [
        DECISION_READY,
        DECISION_MISSING,
        DECISION_PREREQUISITE,
        DECISION_CONFLICT,
        DECISION_INVALID,
    ];
    validation.require(
        string_array(schema.get("closed_decisions")).as_deref()
            == Some(expected_decisions.as_slice()),
        "schema-decisions",
        "exact five-decision registry",
    );
    validation.require(
        string_array(schema.get("closed_reason_codes")).as_deref()
            == Some(CLOSED_REASON_CODES.as_slice()),
        "schema-reasons",
        "machine reason registry equals Rust planner constants",
    );
    validation.require(
        string(schema, "output_artifact_root") == Some(PLAN_ARTIFACT_ROOT)
            && boolean(schema, "all_other_output_paths_allowed") == Some(false)
            && boolean(schema, "working_tree_input_allowed") == Some(false),
        "schema-output-boundary",
        "artifact root and immutable-input boundary are exact",
    );

    validate_invariants(schema, validation);
    for key in [
        "output_is_control_record",
        "output_is_evidence_receipt",
        "output_is_claimable",
    ] {
        validation.require(
            boolean(schema, key) == Some(false),
            &format!("schema:{key}"),
            &format!("{key} is false"),
        );
    }

    validate_digest(digest, validation);
    validate_cases(cases, validation);
}

fn validate_invariants(schema: &Value, validation: &mut Validation) {
    let table = schema.get("invariants").and_then(Value::as_table);
    validation.require(
        table.is_some_and(|value| !value.is_empty()),
        "schema-invariants",
        "invariant table exists",
    );
    let expected: BTreeSet<&str> = TRUE_INVARIANTS
        .into_iter()
        .chain(FALSE_INVARIANTS)
        .collect();
    let actual: BTreeSet<&str> = table
        .map(|value| value.keys().map(String::as_str).collect())
        .unwrap_or_default();
    validation.require(
        actual == expected,
        "schema-invariant-keys",
        "schema invariant key set is closed",
    );
    for key in TRUE_INVARIANTS {
        validation.require(
            table
                .and_then(|value| value.get(key))
                .and_then(Value::as_bool)
                == Some(true),
            &format!("schema-invariant:{key}"),
            &format!("{key} is true"),
        );
    }
    for key in FALSE_INVARIANTS {
        validation.require(
            table
                .and_then(|value| value.get(key))
                .and_then(Value::as_bool)
                == Some(false),
            &format!("schema-invariant:{key}"),
            &format!("{key} is false"),
        );
    }
}

fn validate_digest(digest: &Value, validation: &mut Validation) {
    validation.require(
        integer(digest, "schema_version") == Some(2)
            && string(digest, "profile")
                == Some("ticket_issuance_plan_digest_v2")
            && string(digest, "domain_separator_ascii")
                == Some("eliot-search/ticket-issuance-plan/v2\\0")
            && string(digest, "canonical_payload")
                == Some(
                    "complete_canonical_plan_object_with_plan_sha256_field_omitted",
                ),
        "digest-identity",
        "digest profile and payload are exact",
    );
    for key in [
        "self_referential_digest_allowed",
        "placeholder_replacement_allowed",
        "parsed_reserialization_allowed",
    ] {
        validation.require(
            boolean(digest, key) == Some(false),
            &format!("digest:{key}"),
            &format!("{key} is false"),
        );
    }
}

fn validate_cases(cases: &Value, validation: &mut Validation) {
    let rows = cases.get("case").and_then(Value::as_array);
    validation.require(
        integer(cases, "schema_version") == Some(2)
            && integer(cases, "case_count") == Some(30)
            && rows.is_some_and(|value| value.len() == 30),
        "cases-count",
        "30 schema-v2 cases",
    );
    let ids: Vec<Option<&str>> = rows
        .map(|value| {
            value
                .iter()
                .map(|row| row.get("id").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let exact = ids.len() == 30
        && ids.iter().enumerate().all(|(index, actual)| {
            let expected = format!("PLAN2-{:03}", index + 1);
            *actual == Some(expected.as_str())
        });
    validation.require(
        exact,
        "cases-ids",
        "case IDs are exact and ordered",
    );
}
