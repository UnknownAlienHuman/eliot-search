//! Coverage graph module, operation and documentation closure.

use std::collections::{BTreeMap, BTreeSet};

use toml::Value;

use super::load::{
    CoverageInputs, boolean, integer, ref_package, string, strings,
};

const IMPLEMENTATION_KINDS: [&str; 12] = [
    "implementation_contract_root",
    "implementation_operation",
    "implementation_error_contract",
    "principle_or_invariant",
    "implementation_contract",
    "architecture_root",
    "architecture_section",
    "architecture_node",
    "delivery_contract",
    "configuration_contract",
    "qualification_contract",
    "implementation_handoff",
];

pub(super) struct ContentCounts {
    pub(super) documentation_files: usize,
    pub(super) principles: usize,
    pub(super) governance: usize,
    pub(super) public_facades: usize,
    pub(super) semantic_low: usize,
    pub(super) weak_modules: Vec<String>,
    pub(super) public_entries: BTreeMap<String, String>,
}

pub(super) fn validate(
    inputs: &CoverageInputs,
    errors: &mut Vec<String>,
) -> ContentCounts {
    let (public_entries, weak_modules) = validate_modules(inputs, errors);
    let (public_facades, semantic_low) =
        validate_operations(inputs, &public_entries, errors);
    let (documentation_files, principles, governance) =
        validate_documentation(inputs, errors);

    ContentCounts {
        documentation_files,
        principles,
        governance,
        public_facades,
        semantic_low,
        weak_modules,
        public_entries,
    }
}

fn validate_modules(
    inputs: &CoverageInputs,
    errors: &mut Vec<String>,
) -> (BTreeMap<String, String>, Vec<String>) {
    if integer(&inputs.module_document, "module_count")
        != i64::try_from(inputs.module_rows.len()).ok()
    {
        errors.push("module coverage count mismatch".to_owned());
    }

    let mut public_entries = BTreeMap::new();
    let mut weak_modules = Vec::new();
    for (id, row) in &inputs.module_rows {
        let package = string(row, "package").unwrap_or_default();
        let module = string(row, "module").unwrap_or_default();
        if !inputs.package_rows.contains_key(package) {
            errors.push(format!("{id}: unknown module package {package}"));
        }
        if id != &format!("{package}:{module}") {
            errors.push(format!("{id}: module identity mismatch"));
        }

        let role = string(row, "role").unwrap_or_default();
        if matches!(
            role,
            "public_entry" | "structural_boundary" | "structural_support"
        ) && string(row, "structural_rationale")
            .is_none_or(|value| value.trim().is_empty())
        {
            errors.push(format!("{id}: structural module rationale missing"));
        }
        if role == "public_entry"
            && public_entries
                .insert(package.to_owned(), module.to_owned())
                .is_some()
        {
            errors.push(format!("{package}: multiple public entry modules"));
        }
        if boolean(row, "weakly_covered") == Some(true) {
            weak_modules.push(id.clone());
        }
    }
    weak_modules.sort();

    for package in inputs.package_rows.keys() {
        if !public_entries.contains_key(package) {
            errors.push(format!("{package}: public entry module missing"));
        }
    }
    if integer(&inputs.module_document, "weak_module_count")
        != i64::try_from(weak_modules.len()).ok()
    {
        errors.push("module weak count mismatch".to_owned());
    }
    if !weak_modules.is_empty() {
        errors.push(format!(
            "implementation modules without specific relation: {weak_modules:?}"
        ));
    }
    (public_entries, weak_modules)
}

fn validate_operations(
    inputs: &CoverageInputs,
    public_entries: &BTreeMap<String, String>,
    errors: &mut Vec<String>,
) -> (usize, usize) {
    if integer(&inputs.operation_document, "schema_version") != Some(2) {
        errors.push("operation-module registry must be schema v2".to_owned());
    }
    if integer(&inputs.operation_document, "operation_count")
        != i64::try_from(inputs.operation_rows.len()).ok()
    {
        errors.push("operation registry count mismatch".to_owned());
    }

    let valid_modules: BTreeSet<&str> =
        inputs.module_rows.keys().map(String::as_str).collect();
    let mut public_facades = 0_usize;
    let mut semantic_low = 0_usize;
    for (id, row) in &inputs.operation_rows {
        let package = string(row, "package").unwrap_or_default();
        let module = string(row, "module").unwrap_or_default();
        let module_ref = format!("{package}:{module}");
        if !valid_modules.contains(module_ref.as_str()) {
            errors.push(format!("{id}: invalid routed module {module_ref}"));
        }
        if !id.starts_with(&format!("{package}::")) {
            errors.push(format!("{id}: package identity mismatch"));
        }
        if string(row, "public_entry_module")
            != public_entries.get(package).map(String::as_str)
        {
            errors.push(format!("{id}: public entry module mismatch"));
        }
        if strings(row, "sources").is_empty()
            || strings(row, "source_contexts").is_empty()
        {
            errors.push(format!("{id}: operation source binding missing"));
        }
        match string(row, "route_kind") {
            Some("public_facade") => {
                public_facades = public_facades.saturating_add(1);
                if public_entries.get(package).map(String::as_str) != Some(module) {
                    errors.push(format!(
                        "{id}: facade route does not use public entry"
                    ));
                }
            }
            Some("semantic_low") => {
                semantic_low = semantic_low.saturating_add(1);
            }
            Some("semantic" | "package_rule") => {}
            Some(other) => errors.push(format!("{id}: unknown route kind {other}")),
            None => errors.push(format!("{id}: route kind missing")),
        }
    }
    if public_facades != 0 {
        errors.push(format!(
            "unreviewed public-entry operation routes remain: {public_facades}"
        ));
    }
    if semantic_low != 0 {
        errors.push(format!(
            "low-confidence operation routes remain: {semantic_low}"
        ));
    }
    (public_facades, semantic_low)
}

fn validate_documentation(
    inputs: &CoverageInputs,
    errors: &mut Vec<String>,
) -> (usize, usize, usize) {
    if integer(&inputs.documentation_document, "node_count")
        != i64::try_from(inputs.documentation_rows.len()).ok()
    {
        errors.push("documentation registry node count mismatch".to_owned());
    }

    let valid_modules: BTreeSet<&str> =
        inputs.module_rows.keys().map(String::as_str).collect();
    let mut source_files = BTreeSet::new();
    let mut principles = 0_usize;
    let mut governance = 0_usize;

    for (id, row) in &inputs.documentation_rows {
        let path = string(row, "path").unwrap_or_default();
        if path.is_empty() {
            errors.push(format!("{id}: documentation path missing"));
        } else {
            source_files.insert(path.to_owned());
        }
        if integer(row, "line").is_none_or(|line| line <= 0)
            || integer(row, "level").is_none_or(|level| !(1..=4).contains(&level))
        {
            errors.push(format!("{id}: documentation location invalid"));
        }

        let kind = string(row, "kind").unwrap_or_default();
        let modules = strings(row, "modules");
        let packages = strings(row, "packages");
        if IMPLEMENTATION_KINDS.contains(&kind) {
            if modules.is_empty() {
                errors.push(format!(
                    "{id}: implementation-bearing node lacks module route"
                ));
            }
            let mut routed_packages = BTreeSet::new();
            for module in &modules {
                if !valid_modules.contains(module.as_str()) {
                    errors.push(format!(
                        "{id}: invalid documentation module {module}"
                    ));
                }
                routed_packages.insert(ref_package(module).to_owned());
            }
            let declared_packages: BTreeSet<String> =
                packages.into_iter().collect();
            if routed_packages != declared_packages {
                errors.push(format!(
                    "{id}: documentation package/module mismatch"
                ));
            }
        } else {
            governance = governance.saturating_add(1);
            if !matches!(kind, "governance" | "navigation") {
                errors.push(format!(
                    "{id}: unknown nonimplementation node kind {kind}"
                ));
            }
            if !modules.is_empty() || !packages.is_empty() {
                errors.push(format!(
                    "{id}: governance/navigation node claims product module"
                ));
            }
            if string(row, "rationale")
                .is_none_or(|value| value.trim().is_empty())
            {
                errors.push(format!("{id}: non-crate rationale missing"));
            }
        }
        if kind == "principle_or_invariant" {
            principles = principles.saturating_add(1);
        }
    }

    if principles == 0 {
        errors.push("no principles or invariants were classified".to_owned());
    }
    if integer(&inputs.documentation_document, "source_file_count")
        != i64::try_from(source_files.len()).ok()
    {
        errors.push("documentation registry source count mismatch".to_owned());
    }
    (source_files.len(), principles, governance)
}
