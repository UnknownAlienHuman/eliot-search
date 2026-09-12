//! Delivery, manifest, launch and manual-workflow closure.

use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use super::{
    load::{Inputs, boolean, integer, read_text, require, string, string_list, validate_module_ref},
    markdown::delivery_ids,
    schemas::SchemaSummary,
    topology::{TopologySummary, rows_or_empty},
};

pub(super) fn validate(
    root: &Path,
    inputs: &Inputs,
    topology: &TopologySummary,
    schemas: &SchemaSummary,
    errors: &mut Vec<String>,
) -> usize {
    let delivery_count = validate_delivery(inputs, topology, errors);
    validate_manifest(inputs, topology, schemas, delivery_count, errors);
    validate_launch(inputs, errors);
    validate_qualification(root, errors);
    validate_workflow(root, errors);
    delivery_count
}

fn validate_delivery(
    inputs: &Inputs,
    topology: &TopologySummary,
    errors: &mut Vec<String>,
) -> usize {
    let source = delivery_ids(&inputs.architecture);
    let rows = rows_or_empty(&inputs.delivery_doc, "slice", "id", errors);
    let expected: BTreeSet<String> =
        (0..19).map(|index| format!("P{index:02}")).collect();
    require(
        errors,
        source == expected,
        "architecture source must contain P00-P18",
    );
    require(
        errors,
        rows.keys().cloned().collect::<BTreeSet<_>>() == source,
        "delivery slice registry mismatch",
    );

    let mut covered_packages = BTreeSet::new();
    for (slice, row) in &rows {
        let owners = string_list(row, "primary_packages");
        let refs = string_list(row, "modules");
        let outputs = string_list(row, "required_outputs");
        let evidence = string_list(row, "exit_evidence");
        require(
            errors,
            owners.as_ref().is_some_and(|items| !items.is_empty()),
            format!("{slice}: package owners missing"),
        );
        require(
            errors,
            refs.as_ref().is_some_and(|items| !items.is_empty()),
            format!("{slice}: module refs missing"),
        );
        require(
            errors,
            outputs.as_ref().is_some_and(|items| !items.is_empty()),
            format!("{slice}: required outputs missing"),
        );
        require(
            errors,
            evidence.as_ref().is_some_and(|items| !items.is_empty()),
            format!("{slice}: exit evidence missing"),
        );
        for package in owners.unwrap_or_default() {
            require(
                errors,
                topology.packages.contains(&package),
                format!("{slice}: unknown package {package}"),
            );
            covered_packages.insert(package);
        }
        for reference in refs.unwrap_or_default() {
            validate_module_ref(errors, &reference, &topology.modules, slice);
        }
    }
    require(
        errors,
        covered_packages == topology.packages,
        format!(
            "packages absent from delivery slices: {:?}",
            topology
                .packages
                .difference(&covered_packages)
                .cloned()
                .collect::<Vec<_>>()
        ),
    );
    rows.len()
}

fn validate_manifest(
    inputs: &Inputs,
    topology: &TopologySummary,
    schemas: &SchemaSummary,
    delivery_count: usize,
    errors: &mut Vec<String>,
) {
    for (key, actual) in [
        ("architecture_section_count", topology.section_count),
        ("architecture_invariant_count", topology.invariant_count),
        ("capability_cell_count", topology.capability_count),
        ("shared_port_count", topology.port_count),
        ("configuration_section_count", topology.config_count),
        ("p00_schema_or_registry_count", schemas.schema_total),
        ("recipe_count", schemas.recipe_count),
        ("delivery_slice_count", delivery_count),
        ("module_packet_count", topology.module_rows.len()),
        ("package_assignment_task_count", topology.assignment_paths.len()),
    ] {
        require(
            errors,
            integer(&inputs.manifest, key) == i64::try_from(actual).ok(),
            format!(
                "coverage manifest count mismatch for {key}: {:?} != {actual}",
                integer(&inputs.manifest, key)
            ),
        );
    }
    require(
        errors,
        integer(&inputs.manifest, "type_registry_named_symbol_count")
            == i64::try_from(schemas.type_registry_symbols).ok(),
        "coverage manifest TYPE_REGISTRY count mismatch",
    );
    require(
        errors,
        integer(&inputs.manifest, "named_type_completion_count")
            == i64::try_from(schemas.completion_symbols).ok(),
        "coverage manifest completion count mismatch",
    );
    require(
        errors,
        integer(&inputs.manifest, "canonical_primitive_family_count")
            == i64::try_from(schemas.primitive_families).ok(),
        "coverage manifest primitive family count mismatch",
    );
    require(
        errors,
        string(&inputs.manifest, "architecture_section_sha256")
            == string(&inputs.p00_manifest, "architecture_sha256"),
        "coverage/P00 architecture digest mismatch",
    );
    require(
        errors,
        string(&inputs.manifest, "architecture_section_sha256")
            == string(&inputs.package_doc, "architecture_sha256"),
        "coverage/package registry architecture digest mismatch",
    );
    for key in [
        "implementation_authorized_by_this_manifest",
        "package_acceptance_claimed",
        "gate_or_wave_acceptance_claimed",
        "runtime_evidence_available",
        "product_acceptance_claimed",
    ] {
        require(
            errors,
            boolean(&inputs.manifest, key) == Some(false),
            format!("coverage manifest authority flag {key}"),
        );
    }
}

fn validate_launch(inputs: &Inputs, errors: &mut Vec<String>) {
    require(
        errors,
        string(&inputs.launch, "active_stage") == Some("P00")
            && integer(&inputs.launch, "active_wave") == Some(0),
        "launch must remain P00/W0",
    );
    require(
        errors,
        string_list(&inputs.launch, "authorized_packages")
            == Some(vec!["search-contracts".to_owned()]),
        "only search-contracts may be authorized",
    );
}

fn validate_qualification(root: &Path, errors: &mut Vec<String>) {
    let document = match super::load::load_toml(
        root,
        "qualification/architecture-coverage/cases-v1.toml",
    ) {
        Ok(document) => document,
        Err(error) => {
            errors.push(error);
            return;
        }
    };
    let rows = rows_or_empty(&document, "case", "id", errors);
    require(
        errors,
        integer(&document, "case_count") == Some(40) && rows.len() == 40,
        "architecture coverage case count must be 40",
    );
    for (case, row) in rows {
        require(
            errors,
            boolean(&row, "mandatory") == Some(true),
            format!("coverage case {case} must be mandatory"),
        );
        require(
            errors,
            string(&row, "result") == Some("UNAVAILABLE"),
            format!("coverage case {case} has premature evidence"),
        );
    }
}

fn validate_workflow(root: &Path, errors: &mut Vec<String>) {
    let relative = ".github/workflows/architecture-coverage.yml";
    let text = match read_text(root, relative) {
        Ok(text) => text,
        Err(_) => {
            errors.push("architecture coverage workflow missing".to_owned());
            return;
        }
    };
    for token in [
        "workflow_dispatch:",
        "contents: read",
        "persist-credentials: false",
        "validate-architecture-coverage",
    ] {
        require(
            errors,
            text.contains(token),
            format!("coverage workflow missing {token}"),
        );
    }
    for trigger in [
        "\n  push:",
        "\n  pull_request:",
        "\n  pull_request_target:",
        "\n  schedule:",
        "\n  workflow_run:",
        "\n  repository_dispatch:",
        "\n  workflow_call:",
    ] {
        require(
            errors,
            !text.contains(trigger),
            format!("automatic coverage workflow trigger {}", trigger.trim()),
        );
    }
}
