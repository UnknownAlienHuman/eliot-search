//! Coverage graph Cargo dependency and progressive re-entry closure.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::load::{
    CoverageInputs, boolean, integer, string,
};

pub(super) fn validate(
    root: &Path,
    inputs: &CoverageInputs,
    public_entries: &BTreeMap<String, String>,
    errors: &mut Vec<String>,
) {
    if integer(&inputs.dependency_document, "edge_count")
        != i64::try_from(inputs.dependency_rows.len()).ok()
    {
        errors.push("dependency edge count mismatch".to_owned());
    }

    let valid_modules: BTreeSet<&str> =
        inputs.module_rows.keys().map(String::as_str).collect();
    for (id, row) in &inputs.dependency_rows {
        validate_edge(root, inputs, public_entries, &valid_modules, id, row, errors);
    }
}

fn validate_edge(
    root: &Path,
    inputs: &CoverageInputs,
    public_entries: &BTreeMap<String, String>,
    valid_modules: &BTreeSet<&str>,
    id: &str,
    row: &toml::Value,
    errors: &mut Vec<String>,
) {
    let consumer = string(row, "consumer").unwrap_or_default();
    let consumer_module = string(row, "consumer_module").unwrap_or_default();
    let producer = string(row, "producer").unwrap_or_default();
    let producer_module = string(row, "producer_module").unwrap_or_default();
    let consumer_ref = format!("{consumer}:{consumer_module}");
    let producer_ref = format!("{producer}:{producer_module}");

    if id != format!("{consumer}->{producer}") {
        errors.push(format!("{id}: dependency identity mismatch"));
    }
    if !valid_modules.contains(consumer_ref.as_str()) {
        errors.push(format!("{id}: invalid consumer module {consumer_ref}"));
    }
    if !valid_modules.contains(producer_ref.as_str()) {
        errors.push(format!("{id}: invalid producer module {producer_ref}"));
    }
    if public_entries.get(producer).map(String::as_str) != Some(producer_module) {
        errors.push(format!(
            "{id}: dependency must enter producer public boundary"
        ));
    }

    let Some(consumer_wave) = integer(row, "consumer_earliest_wave") else {
        errors.push(format!("{id}: consumer wave missing"));
        return;
    };
    let Some(producer_wave) = integer(row, "producer_earliest_wave") else {
        errors.push(format!("{id}: producer wave missing"));
        return;
    };
    let requires_reentry = boolean(row, "requires_stage_reentry") == Some(true);
    if requires_reentry {
        let expected_stage = format!("W{producer_wave}");
        let expected_override = format!("{expected_stage}.{consumer}");
        if string(row, "reentry_stage") != Some(expected_stage.as_str()) {
            errors.push(format!("{id}: reentry stage mismatch"));
        }
        if string(row, "relationship") != Some("progressive_reentry_handoff") {
            errors.push(format!(
                "{id}: later-wave edge must be progressive reentry"
            ));
        }
        let Some(override_row) = inputs.override_rows.get(&expected_override) else {
            errors.push(format!("{id}: exact reentry override missing"));
            return;
        };
        if string(override_row, "package") != Some(consumer) {
            errors.push(format!("{id}: reentry override package mismatch"));
        }
        if integer(override_row, "wave") != Some(producer_wave) {
            errors.push(format!("{id}: reentry override wave mismatch"));
        }
        for (field, expected) in [
            ("replace_previous_stage_context", true),
            ("accepted_prior_stage_handoff_only", true),
            ("dependency_implementation_reads_allowed", false),
        ] {
            if boolean(override_row, field) != Some(expected) {
                errors.push(format!("{id}: reentry invariant mismatch: {field}"));
            }
        }
    } else {
        if string(row, "reentry_stage") != Some("NONE") {
            errors.push(format!("{id}: same/earlier-wave edge has spurious reentry"));
        }
        if producer_wave > consumer_wave {
            errors.push(format!("{id}: unmodelled later-wave dependency"));
        }
    }

    for (field, label) in [
        ("contract_source", "contract source"),
        ("cargo_manifest", "Cargo manifest"),
    ] {
        let relative = string(row, field).unwrap_or_default();
        if relative.is_empty() || !root.join(relative).is_file() {
            errors.push(format!("{id}: missing {label}"));
        }
    }
}
