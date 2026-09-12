//! Architecture sections, capability cells and invariant ownership.

use std::collections::BTreeSet;

use super::rows_or_empty;
use super::super::load::{
    Inputs, ModuleMap, require, string, string_list, validate_module_ref,
};
use super::super::markdown::{
    architecture_sections, capability_cells, invariant_ids,
};

pub(super) fn validate(
    inputs: &Inputs,
    packages: &BTreeSet<String>,
    modules: &ModuleMap,
    errors: &mut Vec<String>,
) -> (usize, usize, usize) {
    let section_count = validate_sections(inputs, packages, modules, errors);
    let capability_count = validate_capabilities(inputs, packages, modules, errors);
    let invariant_count = validate_invariants(inputs, packages, modules, errors);
    (section_count, capability_count, invariant_count)
}

fn validate_sections(
    inputs: &Inputs,
    packages: &BTreeSet<String>,
    modules: &ModuleMap,
    errors: &mut Vec<String>,
) -> usize {
    let source = architecture_sections(&inputs.architecture);
    let rows = rows_or_empty(&inputs.section_doc, "section", "id", errors);
    let expected: BTreeSet<String> = (0..40).map(|index| format!("S{index}")).collect();
    require(
        errors,
        source.keys().cloned().collect::<BTreeSet<_>>() == expected,
        "architecture source must contain S0-S39",
    );
    require(
        errors,
        rows.keys().cloned().collect::<BTreeSet<_>>()
            == source.keys().cloned().collect(),
        "architecture section registry mismatch",
    );
    for (id, row) in &rows {
        require(
            errors,
            string(row, "heading") == source.get(id).map(String::as_str),
            format!("{id}: heading mismatch"),
        );
        let owners = string_list(row, "primary_packages");
        let refs = string_list(row, "modules");
        require(
            errors,
            owners.as_ref().is_some_and(|values| !values.is_empty()),
            format!("{id}: owner packages missing"),
        );
        require(
            errors,
            refs.as_ref().is_some_and(|values| !values.is_empty()),
            format!("{id}: module refs missing"),
        );
        for package in owners.unwrap_or_default() {
            require(
                errors,
                packages.contains(&package),
                format!("{id}: unknown owner package {package}"),
            );
        }
        for reference in refs.unwrap_or_default() {
            validate_module_ref(errors, &reference, modules, id);
        }
    }
    rows.len()
}

fn validate_capabilities(
    inputs: &Inputs,
    packages: &BTreeSet<String>,
    modules: &ModuleMap,
    errors: &mut Vec<String>,
) -> usize {
    let source = capability_cells(&inputs.architecture);
    let rows = rows_or_empty(&inputs.capability_doc, "cell", "id", errors);
    let expected: BTreeSet<String> =
        (0..31).map(|index| format!("C{index:02}")).collect();
    require(
        errors,
        source.keys().cloned().collect::<BTreeSet<_>>() == expected,
        "architecture source must contain C00-C30",
    );
    require(
        errors,
        rows.keys().cloned().collect::<BTreeSet<_>>()
            == source.keys().cloned().collect(),
        "capability registry mismatch",
    );
    for (id, row) in &rows {
        require(
            errors,
            string(row, "name") == source.get(id).map(String::as_str),
            format!("{id}: capability name mismatch"),
        );
        for key in ["primary_packages", "supporting_packages", "state_owner_packages"] {
            let values = string_list(row, key);
            require(
                errors,
                values.is_some(),
                format!("{id}: {key} must be an array"),
            );
            if key == "primary_packages" {
                require(
                    errors,
                    values.as_ref().is_some_and(|items| !items.is_empty()),
                    format!("{id}: primary owner missing"),
                );
            }
            for package in values.unwrap_or_default() {
                require(
                    errors,
                    packages.contains(&package),
                    format!("{id}: unknown package {package}"),
                );
            }
        }
        let refs = string_list(row, "modules");
        require(
            errors,
            refs.as_ref().is_some_and(|items| !items.is_empty()),
            format!("{id}: module refs missing"),
        );
        for reference in refs.unwrap_or_default() {
            validate_module_ref(errors, &reference, modules, id);
        }
    }
    rows.len()
}

fn validate_invariants(
    inputs: &Inputs,
    packages: &BTreeSet<String>,
    modules: &ModuleMap,
    errors: &mut Vec<String>,
) -> usize {
    let source = invariant_ids(&inputs.architecture);
    let rows = rows_or_empty(&inputs.invariant_doc, "invariant", "id", errors);
    let expected: BTreeSet<String> =
        (1..=30).map(|index| format!("INV-{index:02}")).collect();
    require(
        errors,
        source == expected,
        "architecture source must contain INV-01..INV-30",
    );
    require(
        errors,
        rows.keys().cloned().collect::<BTreeSet<_>>() == source,
        "invariant registry mismatch",
    );
    for (id, row) in &rows {
        let owners = string_list(row, "enforcement_packages");
        let refs = string_list(row, "modules");
        require(
            errors,
            owners.as_ref().is_some_and(|items| !items.is_empty()),
            format!("{id}: enforcement owner missing"),
        );
        require(
            errors,
            refs.as_ref().is_some_and(|items| !items.is_empty()),
            format!("{id}: module refs missing"),
        );
        for package in owners.unwrap_or_default() {
            require(
                errors,
                packages.contains(&package),
                format!("{id}: unknown package {package}"),
            );
        }
        for reference in refs.unwrap_or_default() {
            validate_module_ref(errors, &reference, modules, id);
        }
    }
    rows.len()
}
