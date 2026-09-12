//! Pure TOML contract checks for context materialization.

use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

pub(super) fn validate_registry(
    root: &Path,
    registry: &Value,
    errors: &mut Vec<String>,
) {
    if string(registry, "component")
        != Some("context_materialization_planner_v1")
        || string(registry, "status")
            != Some("EXECUTABLE_ADVISORY_NON_AUTHORITATIVE")
        || string(registry, "implementation")
            != Some("xtask/src/context_materialization_builder.rs")
        || string(registry, "powershell_wrapper")
            != Some("tools/plan-context-materialization.ps1")
        || string(registry, "qualification_tests")
            != Some("xtask/tests/context_materialization_builder.rs")
    {
        errors.push("planner implementation registry mismatch".to_owned());
    }
    let modules = string_array(registry.get("implementation_modules"));
    if modules.as_ref().is_none_or(|modules| {
        modules.len() != 5
            || modules.iter().any(|path| !root.join(*path).is_file())
            || modules.iter().copied().collect::<BTreeSet<_>>().len()
                != modules.len()
    }) {
        errors.push("planner implementation module closure mismatch".to_owned());
    }
    if string(registry, "operation_kind") != Some("materialize_context_v1") {
        errors.push("operation kind mismatch".to_owned());
    }
    if !all_false_table(registry.get("authority")) {
        errors.push("planner authority ceiling must be all false".to_owned());
    }
    if boolean(registry, "signature_refs_are_operation_id_inputs")
        != Some(false)
        || boolean(
            registry,
            "signature_refs_bind_post_operation_signed_payload",
        ) != Some(true)
    {
        errors.push("signature/operation binding mismatch".to_owned());
    }
}

pub(super) fn validate_contracts(
    schema: &Value,
    digest: &Value,
    instance: &Value,
    renderer: &Value,
    errors: &mut Vec<String>,
) {
    if string(schema, "record_kind")
        != Some("context_materialization_plan_v1")
    {
        errors.push("plan schema kind mismatch".to_owned());
    }
    let invariants = schema.get("invariants").and_then(Value::as_table);
    if invariants.is_none_or(|invariants| {
        invariants
            .get("control_record_mutations_must_be_empty")
            .and_then(Value::as_bool)
            != Some(true)
            || invariants
                .get("all_authority_fields_must_be_false")
                .and_then(Value::as_bool)
                != Some(true)
            || invariants
                .get("signature_refs_excluded_from_operation_id")
                .and_then(Value::as_bool)
                != Some(true)
    }) {
        errors.push("plan schema authority/operation invariants missing".to_owned());
    }
    if boolean(digest, "self_referential_digest_allowed") != Some(false) {
        errors.push("plan digest must remain non-self-referential".to_owned());
    }
    if string(instance, "instance_status") != Some("MATERIALIZED") {
        errors.push("context manifest instance status mismatch".to_owned());
    }
    if boolean(renderer, "signature_table_excluded_from_signed_payload")
        != Some(true)
    {
        errors.push("renderer signature boundary mismatch".to_owned());
    }
    let renderer_invariants = renderer.get("invariants").and_then(Value::as_table);
    if renderer_invariants.is_none_or(|invariants| {
        invariants
            .get("self_referential_complete_file_digest_allowed")
            .and_then(Value::as_bool)
            != Some(false)
    }) {
        errors.push("renderer allows complete-file self hash".to_owned());
    }
}

pub(super) fn validate_cases(
    cases: &Value,
    errors: &mut Vec<String>,
) -> usize {
    let rows = cases.get("case").and_then(Value::as_array);
    let count = rows.map_or(0, Vec::len);
    if integer(cases, "case_count") != Some(12) || count != 12 {
        errors.push("materialization corpus must contain exactly twelve cases".to_owned());
    }
    let ids: Vec<&str> = rows
        .into_iter()
        .flatten()
        .filter_map(|row| {
            row.as_table()
                .and_then(|table| table.get("id"))
                .and_then(Value::as_str)
        })
        .collect();
    if ids.len() != count
        || ids.iter().copied().collect::<BTreeSet<_>>().len() != count
    {
        errors.push("materialization case IDs are missing or non-unique".to_owned());
    }
    count
}

fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn boolean(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn integer(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_integer)
}

fn string_array(value: Option<&Value>) -> Option<Vec<&str>> {
    value?
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect()
}

fn all_false_table(value: Option<&Value>) -> bool {
    value.and_then(Value::as_table).is_some_and(|table| {
        !table.is_empty()
            && table
                .values()
                .all(|entry| entry.as_bool() == Some(false))
    })
}
