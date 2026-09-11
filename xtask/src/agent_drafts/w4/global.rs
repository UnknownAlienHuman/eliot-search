use std::collections::{BTreeMap, BTreeSet};

use toml::Value;

use super::super::{
    boolean, child_string_list, expected_strings, integer, string,
    string_list,
};
use super::spec::{CENTRAL_PACKAGES, PACKAGES};

pub(super) fn validate_global(
    packet_doc: &Value,
    stages: &BTreeMap<String, Value>,
    readsets: &BTreeMap<String, Value>,
    launch: &Value,
    baseline: &Value,
    probes: &Value,
    errors: &mut Vec<String>,
) {
    validate_stage(packet_doc, stages, launch, errors);
    validate_execution(packet_doc, errors);
    validate_daemon_override(readsets, errors);
    validate_query_qualification(baseline, probes, errors);
}

fn validate_stage(
    packet_doc: &Value,
    stages: &BTreeMap<String, Value>,
    launch: &Value,
    errors: &mut Vec<String>,
) {
    if string(packet_doc, "status")
        != Some("BLOCKED_ON_G1_W3_AND_QUERY_QUALIFICATION")
    {
        errors.push("W4 packet registry is not fail-closed".to_owned());
    }
    if string_list(packet_doc, "requires_accepted_gates")
        != Some(expected_strings(&["G1"]))
        || string_list(packet_doc, "requires_accepted_receipts")
            != Some(expected_strings(&["W3"]))
    {
        errors.push("W4 prerequisite mismatch".to_owned());
    }
    if boolean(packet_doc, "one_writer_one_package") != Some(true)
        || boolean(packet_doc, "implementation_authorized_by_this_registry")
            != Some(false)
    {
        errors.push("W4 ownership or authority ceiling invalid".to_owned());
    }
    if boolean(packet_doc, "query_product_enabled") != Some(false) {
        errors.push("W4 registry enables query product".to_owned());
    }

    let empty = Value::Table(toml::map::Map::new());
    let stage = stages.get("W4").unwrap_or(&empty);
    if string(stage, "status") != Some("BLOCKED")
        || string_list(stage, "packages")
            != Some(expected_strings(&CENTRAL_PACKAGES))
    {
        errors.push("central W4 stage mismatch".to_owned());
    }
    if string_list(stage, "requires_accepted_gates")
        != Some(expected_strings(&["G1"]))
        || string_list(stage, "requires_accepted_receipts")
            != Some(expected_strings(&["W3"]))
    {
        errors.push("central W4 prerequisites mismatch".to_owned());
    }
    if string(launch, "active_stage") != Some("P00")
        || integer(launch, "active_wave") != Some(0)
        || string_list(launch, "authorized_packages")
            != Some(expected_strings(&["search-contracts"]))
    {
        errors.push("launch authority moved from P00/W0".to_owned());
    }
}

fn validate_execution(packet_doc: &Value, errors: &mut Vec<String>) {
    if child_string_list(packet_doc, "execution", "group_order")
        != Some(expected_strings(&["A", "B", "C", "D"]))
    {
        errors.push("execution group order mismatch".to_owned());
    }

    for group in ["A", "B", "C", "D"] {
        let key = format!("group_{group}_packages");
        let actual: BTreeSet<String> =
            child_string_list(packet_doc, "execution", &key)
                .unwrap_or_default()
                .into_iter()
                .collect();
        let expected: BTreeSet<String> = PACKAGES
            .iter()
            .filter(|spec| spec.group == group)
            .map(|spec| spec.name.to_owned())
            .collect();
        if actual != expected {
            errors.push(format!("group {group} mismatch"));
        }
    }

    if child_string_list(
        packet_doc,
        "execution",
        "query_planner_requires",
    ) != Some(expected_strings(&["search-access"]))
        || child_string_list(
            packet_doc,
            "execution",
            "candidate_validator_requires",
        ) != Some(expected_strings(&["search-access"]))
    {
        errors.push("group B predecessor mismatch".to_owned());
    }
    if child_string_list(
        packet_doc,
        "execution",
        "retrieval_executor_requires",
    ) != Some(expected_strings(&[
        "search-query-planner",
        "search-access",
        "search-lexical",
        "search-epoch-pins",
    ])) {
        errors.push("retrieval executor predecessor mismatch".to_owned());
    }
    if child_string_list(
        packet_doc,
        "execution",
        "result_projector_requires",
    ) != Some(expected_strings(&[
        "search-candidate-validator",
        "search-handles",
    ])) {
        errors.push("result projector predecessor mismatch".to_owned());
    }
    if child_string_list(
        packet_doc,
        "execution",
        "continuation_requires",
    ) != Some(expected_strings(&[
        "search-query-planner",
        "search-access",
        "search-epoch-pins",
    ])) {
        errors.push("continuation predecessor mismatch".to_owned());
    }
}

fn validate_daemon_override(
    readsets: &BTreeMap<String, Value>,
    errors: &mut Vec<String>,
) {
    let empty = Value::Table(toml::map::Map::new());
    let override_row = readsets.get("W4.eliot-searchd").unwrap_or(&empty);
    if boolean(override_row, "replace_previous_stage_context") != Some(true)
        || boolean(override_row, "accepted_prior_stage_handoff_only")
            != Some(true)
    {
        errors.push("W4 daemon replacement semantics missing".to_owned());
    }
    if string_list(override_row, "required_prior_handoffs")
        != Some(expected_strings(&[
            "accepted_eliot-searchd_W3_API",
            "accepted_W3_receipt",
        ]))
    {
        errors.push("W4 daemon prior handoffs mismatch".to_owned());
    }
    if string(override_row, "write_scope") != Some("bins/eliot-searchd/**")
        || boolean(
            override_row,
            "dependency_implementation_reads_allowed",
        ) != Some(false)
    {
        errors.push(
            "W4 daemon override scope/read boundary mismatch".to_owned(),
        );
    }
}

fn validate_query_qualification(
    baseline: &Value,
    probes: &Value,
    errors: &mut Vec<String>,
) {
    if string(baseline, "status") != Some("DESIGNED_NOT_EXECUTED")
        || boolean(baseline, "implementation_authorized") != Some(false)
        || boolean(baseline, "runtime_evidence_available") != Some(false)
    {
        errors.push(
            "query baseline has premature implementation/evidence state"
                .to_owned(),
        );
    }
    if string(baseline, "w4_status") != Some("BLOCKED")
        || string(baseline, "query_product_status") != Some("UNAVAILABLE")
    {
        errors.push(
            "query baseline verdict state is not blocked/unavailable"
                .to_owned(),
        );
    }

    let Some(rows) = probes.get("probe").and_then(Value::as_array) else {
        errors.push(
            "query probe registry has premature evidence".to_owned(),
        );
        return;
    };
    if string(probes, "status") != Some("NOT_EXECUTED")
        || rows.iter().any(|row| {
            row.as_table().is_none_or(|table| {
                table.get("mandatory").and_then(Value::as_bool) != Some(true)
                    || table.get("result").and_then(Value::as_str)
                        != Some("UNAVAILABLE")
            })
        })
    {
        errors.push(
            "query probe registry has premature evidence".to_owned(),
        );
    }
}
