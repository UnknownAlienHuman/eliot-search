use std::collections::BTreeMap;

use toml::Value;

use super::super::{
    boolean, child_bool, child_string_list, expected_strings, integer, string,
    string_list,
};
use super::spec::CENTRAL_PACKAGES;

pub(super) fn validate_global(
    packet_doc: &Value,
    stages: &BTreeMap<String, Value>,
    overrides: &BTreeMap<String, Value>,
    launch: &Value,
    errors: &mut Vec<String>,
) {
    if string(packet_doc, "status") != Some("BLOCKED_ON_G0_AND_W1") {
        errors.push("W2 packet registry is not blocked on G0/W1".to_owned());
    }
    if string_list(packet_doc, "requires_accepted_gates")
        != Some(expected_strings(&["G0"]))
    {
        errors.push("W2 gate prerequisite mismatch".to_owned());
    }
    if string_list(packet_doc, "requires_accepted_receipts")
        != Some(expected_strings(&["W1"]))
    {
        errors.push("W2 receipt prerequisite mismatch".to_owned());
    }
    if boolean(packet_doc, "one_writer_one_package") != Some(true) {
        errors.push("one-writer-one-package invariant disabled".to_owned());
    }
    if boolean(packet_doc, "implementation_authorized_by_this_registry") != Some(false) {
        errors.push("W2 packet registry authorizes implementation".to_owned());
    }

    let empty = Value::Table(toml::map::Map::new());
    let stage = stages.get("W2").unwrap_or(&empty);
    if string(stage, "status") != Some("BLOCKED") {
        errors.push("central W2 stage is not BLOCKED".to_owned());
    }
    if string_list(stage, "packages") != Some(expected_strings(&CENTRAL_PACKAGES)) {
        errors.push("central W2 package order/set mismatch".to_owned());
    }
    if string_list(stage, "requires_accepted_gates") != Some(expected_strings(&["G0"])) {
        errors.push("central W2 gate prerequisite mismatch".to_owned());
    }
    if string_list(stage, "requires_accepted_receipts") != Some(expected_strings(&["W1"])) {
        errors.push("central W2 receipt prerequisite mismatch".to_owned());
    }

    if string(launch, "active_stage") != Some("P00")
        || integer(launch, "active_wave") != Some(0)
    {
        errors.push("launch authority moved from P00/W0".to_owned());
    }
    if string_list(launch, "authorized_packages")
        != Some(expected_strings(&["search-contracts"]))
    {
        errors.push("current authorized package set changed".to_owned());
    }

    validate_execution(packet_doc, errors);
    validate_daemon_override(overrides, errors);
}

fn validate_execution(packet_doc: &Value, errors: &mut Vec<String>) {
    if packet_doc.get("execution").and_then(Value::as_table).is_none() {
        errors.push("missing W2 execution table".to_owned());
        return;
    }
    if child_string_list(packet_doc, "execution", "group_order")
        != Some(expected_strings(&["A", "B", "C"]))
    {
        errors.push("W2 group order mismatch".to_owned());
    }
    if child_string_list(packet_doc, "execution", "group_A_packages")
        != Some(expected_strings(&[
            "search-source-admission",
            "search-source-identity",
            "search-safe-reader",
            "search-revision-store",
            "search-materializer",
            "search-unitizer",
        ]))
    {
        errors.push("W2 group A mismatch".to_owned());
    }
    if child_string_list(packet_doc, "execution", "group_B_packages")
        != Some(expected_strings(&["search-source-registry"]))
    {
        errors.push("W2 group B mismatch".to_owned());
    }
    if child_string_list(packet_doc, "execution", "group_C_packages")
        != Some(expected_strings(&["eliot-searchd"]))
    {
        errors.push("W2 group C mismatch".to_owned());
    }
    if child_string_list(
        packet_doc,
        "execution",
        "group_B_requires_accepted_handoffs",
    ) != Some(expected_strings(&[
        "search-source-admission",
        "search-source-identity",
    ]))
    {
        errors.push("source registry predecessor set mismatch".to_owned());
    }
    if child_bool(
        packet_doc,
        "execution",
        "group_C_requires_all_W2_library_handoffs",
    ) != Some(true)
    {
        errors.push("daemon does not require all W2 library handoffs".to_owned());
    }
    if child_string_list(
        packet_doc,
        "execution",
        "eliot_searchd_requires_prior_stage_handoffs",
    ) != Some(expected_strings(&["eliot-searchd", "W1"]))
    {
        errors.push("daemon prior-stage requirements mismatch".to_owned());
    }
}

fn validate_daemon_override(
    overrides: &BTreeMap<String, Value>,
    errors: &mut Vec<String>,
) {
    let empty = Value::Table(toml::map::Map::new());
    let daemon = overrides.get("W2.eliot-searchd").unwrap_or(&empty);
    if boolean(daemon, "replace_previous_stage_context") != Some(true) {
        errors.push("W2 daemon override does not replace previous context".to_owned());
    }
    if boolean(daemon, "accepted_prior_stage_handoff_only") != Some(true) {
        errors.push("W2 daemon override permits prior implementation context".to_owned());
    }
    if string_list(daemon, "required_prior_handoffs")
        != Some(expected_strings(&[
            "accepted_eliot-searchd_W1_API",
            "accepted_W1_receipt",
        ]))
    {
        errors.push("W2 daemon prior handoff list mismatch".to_owned());
    }
    if string_list(daemon, "forbidden_prior_stage_packets")
        != Some(expected_strings(&["docs/handoff/W1_IMPLEMENTATION_PACKET.md"]))
    {
        errors.push("W2 daemon does not forbid W1 packet replay".to_owned());
    }
}
