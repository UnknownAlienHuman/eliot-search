use std::collections::{BTreeMap, BTreeSet};

use toml::Value;

use super::super::{
    boolean, child, child_bool, child_string, child_string_list,
    expected_strings, integer, string, string_list,
};
use super::spec::{CENTRAL_PACKAGES, PACKAGES};

pub(super) fn validate_global(
    packet_doc: &Value,
    stages: &BTreeMap<String, Value>,
    readsets: &BTreeMap<String, Value>,
    launch: &Value,
    artifact: &Value,
    collection: &Value,
    probes: &Value,
    errors: &mut Vec<String>,
) {
    validate_stage(packet_doc, stages, launch, errors);
    validate_execution(packet_doc, errors);
    validate_daemon_override(readsets, errors);
    validate_artifact(artifact, errors);
    validate_collection(collection, errors);
    validate_probe_progress(probes, errors);
}

fn validate_stage(
    packet_doc: &Value,
    stages: &BTreeMap<String, Value>,
    launch: &Value,
    errors: &mut Vec<String>,
) {
    if string(packet_doc, "status")
        != Some("BLOCKED_ON_G1_W2_G1_AND_QDRANT_QUALIFICATION")
    {
        errors.push("W3 packet registry is not fail-closed".to_owned());
    }
    if string_list(packet_doc, "requires_accepted_gates")
        != Some(expected_strings(&["G1"]))
        || string_list(packet_doc, "requires_accepted_receipts")
            != Some(expected_strings(&["W2_G1"]))
    {
        errors.push("W3 prerequisite mismatch".to_owned());
    }
    if boolean(packet_doc, "one_writer_one_package") != Some(true)
        || boolean(packet_doc, "implementation_authorized_by_this_registry") != Some(false)
    {
        errors.push("W3 ownership or authority ceiling invalid".to_owned());
    }
    if boolean(packet_doc, "indexed_mode_enabled") != Some(false) {
        errors.push("W3 registry enables indexed mode".to_owned());
    }

    let empty = Value::Table(toml::map::Map::new());
    let stage = stages.get("W3").unwrap_or(&empty);
    if string(stage, "status") != Some("BLOCKED")
        || string_list(stage, "packages") != Some(expected_strings(&CENTRAL_PACKAGES))
    {
        errors.push("central W3 stage mismatch".to_owned());
    }
    if string_list(stage, "requires_accepted_gates") != Some(expected_strings(&["G1"]))
        || string_list(stage, "requires_accepted_receipts")
            != Some(expected_strings(&["W2_G1"]))
    {
        errors.push("central W3 prerequisites mismatch".to_owned());
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
    for group in ["A", "B"] {
        let key = format!("group_{group}_packages");
        let actual: BTreeSet<String> = child_string_list(packet_doc, "execution", &key)
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
    if child_string_list(packet_doc, "execution", "group_C_packages")
        != Some(expected_strings(&["search-publication"]))
        || child_string_list(packet_doc, "execution", "group_D_packages")
            != Some(expected_strings(&["eliot-searchd"]))
    {
        errors.push("group C/D mismatch".to_owned());
    }
    if child_string_list(
        packet_doc,
        "execution",
        "projection_planner_requires",
    ) != Some(expected_strings(&["search-point-identity"]))
        || child_string_list(
            packet_doc,
            "execution",
            "index_reclaimer_requires",
        ) != Some(expected_strings(&["search-epoch-pins"]))
    {
        errors.push("group B predecessor mismatch".to_owned());
    }
    if child_string_list(packet_doc, "execution", "publication_requires")
        != Some(expected_strings(&[
            "search-projection-planner",
            "search-point-identity",
        ]))
    {
        errors.push("publication predecessor mismatch".to_owned());
    }
}

fn validate_daemon_override(
    readsets: &BTreeMap<String, Value>,
    errors: &mut Vec<String>,
) {
    let empty = Value::Table(toml::map::Map::new());
    let override_row = readsets.get("W3.eliot-searchd").unwrap_or(&empty);
    if boolean(override_row, "replace_previous_stage_context") != Some(true)
        || boolean(override_row, "accepted_prior_stage_handoff_only") != Some(true)
    {
        errors.push("W3 daemon replacement semantics missing".to_owned());
    }
    if string_list(override_row, "required_prior_handoffs")
        != Some(expected_strings(&[
            "accepted_eliot-searchd_W2_API",
            "accepted_W2_G1_receipt",
        ]))
    {
        errors.push("W3 daemon prior handoffs mismatch".to_owned());
    }
    if string(override_row, "write_scope") != Some("bins/eliot-searchd/**")
        || boolean(override_row, "dependency_implementation_reads_allowed") != Some(false)
    {
        errors.push("W3 daemon override scope/read boundary mismatch".to_owned());
    }
}

fn validate_artifact(artifact: &Value, errors: &mut Vec<String>) {
    if string(artifact, "status") != Some("UNQUALIFIED") {
        errors.push("Qdrant artifact must remain UNQUALIFIED until full W3 acceptance".to_owned());
    }
    if child_bool(artifact, "server", "automatic_download") != Some(false)
        || child_bool(artifact, "server", "automatic_upgrade") != Some(false)
    {
        errors.push("Qdrant automatic download/upgrade is enabled".to_owned());
    }
    let server_version = child_string(artifact, "server", "version");
    let client_version = child_string(artifact, "client", "version");
    if server_version.is_none_or(str::is_empty)
        || client_version.is_none_or(str::is_empty)
        || server_version != client_version
    {
        errors.push("Qdrant exact server/client version selection mismatch".to_owned());
    }
    if child_string(artifact, "client", "crate_name") != Some("qdrant-client") {
        errors.push("Qdrant client crate identity mismatch".to_owned());
    }
    if child_string(artifact, "server", "build_identity").is_none_or(str::is_empty)
        || child_string(artifact, "server", "artifact_sha256").is_none_or(str::is_empty)
        || child(artifact, "server", "artifact_bytes")
            .and_then(Value::as_integer)
            .is_none_or(|bytes| bytes <= 0)
    {
        errors.push("Qdrant selected artifact identity is incomplete".to_owned());
    }
    for key in [
        "latest_allowed",
        "version_range_allowed",
        "floating_git_revision_allowed",
        "documentation_only_acceptance_allowed",
        "health_only_acceptance_allowed",
    ] {
        if child_bool(artifact, "selection", key) != Some(false) {
            errors.push(format!("Qdrant selection policy permits {key}"));
        }
    }
}

fn validate_collection(collection: &Value, errors: &mut Vec<String>) {
    if string(collection, "status") != Some("DESIGNED_NOT_EXECUTED") {
        errors.push("collection schema acceptance state changed prematurely".to_owned());
    }
    let vectors = collection
        .get("sparse_vector")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if vectors.is_empty()
        || vectors.iter().any(|row| {
            row.get("profile_status").and_then(Value::as_str) != Some("UNQUALIFIED")
        })
    {
        errors.push("collection sparse profile state is not UNQUALIFIED".to_owned());
    }
}

fn validate_probe_progress(probes: &Value, errors: &mut Vec<String>) {
    if string(probes, "status") != Some("PARTIALLY_EXECUTED") {
        errors.push("Qdrant probe registry must declare PARTIALLY_EXECUTED".to_owned());
    }
    let Some(rows) = probes.get("probe").and_then(Value::as_array) else {
        errors.push("Qdrant probe registry is not an array".to_owned());
        return;
    };
    let mut passed = 0_usize;
    let mut unavailable = 0_usize;
    for row in rows {
        let Some(table) = row.as_table() else {
            errors.push("invalid Qdrant probe row".to_owned());
            continue;
        };
        if table.get("mandatory").and_then(Value::as_bool) != Some(true) {
            errors.push("non-mandatory probe entered mandatory W3 registry".to_owned());
        }
        match table.get("result").and_then(Value::as_str) {
            Some("PASS") => {
                passed += 1;
                if table
                    .get("raw_output_ref")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                    || table
                        .get("receipt_ref")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                {
                    errors.push("PASS Qdrant probe lacks raw output or receipt".to_owned());
                }
            }
            Some("UNAVAILABLE") => unavailable += 1,
            Some("FAIL") => errors.push("mandatory Qdrant probe is FAIL".to_owned()),
            _ => errors.push("Qdrant probe has an invalid result".to_owned()),
        }
    }
    if passed == 0 || unavailable == 0 {
        errors.push("Qdrant partial probe state must contain PASS and UNAVAILABLE rows".to_owned());
    }
}
