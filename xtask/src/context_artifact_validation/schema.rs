use std::collections::BTreeSet;

use toml::Value;

use crate::context_artifact::{
    ADDITIONAL_FAILURE_CODES, ARTIFACT_FORMAT, AUTHORITY_FIELDS, RECORD_KIND,
    SCHEMA_VERSION, STATUS, UNRESOLVED_MANIFEST_FIELDS,
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
        "candidate schema identity is exact",
    );
    validation.require(
        string(schema, "artifact_format") == Some(ARTIFACT_FORMAT),
        "schema-format",
        "schema artifact format is exact",
    );
    validation.require(
        string_array(schema.get("additional_failure_codes")).as_deref()
            == Some(ADDITIONAL_FAILURE_CODES.as_slice()),
        "schema-failures",
        "additional failure registry is exact",
    );
    validation.require(
        string_array(schema.get("required_unresolved_manifest_fields"))
            .as_deref()
            == Some(UNRESOLVED_MANIFEST_FIELDS.as_slice()),
        "schema-unresolved",
        "unresolved manifest field set/order is exact",
    );
    validate_authority(schema, validation);
    validate_invariants(schema, validation);
    validate_digest(digest, validation);
    validate_cases(cases, validation);
}

fn validate_authority(schema: &Value, validation: &mut Validation) {
    let table = schema.get("authority").and_then(Value::as_table);
    validation.require(
        table.is_some(),
        "schema-authority",
        "schema authority table exists",
    );
    let expected: BTreeSet<&str> = AUTHORITY_FIELDS.into_iter().collect();
    let actual: BTreeSet<&str> = table
        .map(|value| value.keys().map(String::as_str).collect())
        .unwrap_or_default();
    validation.require(
        actual == expected,
        "schema-authority-keys",
        "schema authority keys are closed",
    );
    validation.require(
        table.is_some_and(|value| {
            !value.is_empty()
                && value
                    .values()
                    .all(|entry| entry.as_bool() == Some(false))
        }),
        "schema-authority-false",
        "schema authority flags are false",
    );
}

fn validate_invariants(schema: &Value, validation: &mut Validation) {
    let table = schema.get("invariants").and_then(Value::as_table);
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
        integer(digest, "schema_version") == Some(1)
            && string(digest, "profile")
                == Some("context_artifact_candidate_digest_v1"),
        "digest-identity",
        "digest profile identity is exact",
    );
    for key in [
        "self_referential_digest_allowed",
        "placeholder_replacement_allowed",
        "parsed_reserialization_allowed",
        "candidate_id_is_context_id",
        "candidate_id_is_materialize_context_operation_id",
        "candidate_sha256_is_control_record_digest",
        "artifact_sha256_is_immutable_artifact_ref",
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
        integer(cases, "case_count") == Some(20)
            && rows.is_some_and(|value| value.len() == 20),
        "cases-count",
        "twenty cases",
    );
    let ids: Vec<Option<&str>> = rows
        .map(|value| {
            value
                .iter()
                .map(|row| row.get("id").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let exact = ids.len() == 20
        && ids.iter().enumerate().all(|(index, actual)| {
            let expected = format!("CAC1-{:03}", index + 1);
            *actual == Some(expected.as_str())
        });
    validation.require(
        exact,
        "cases-ids",
        "case IDs are exact and ordered",
    );
}
