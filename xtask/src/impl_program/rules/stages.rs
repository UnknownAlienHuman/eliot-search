use std::collections::{BTreeMap, BTreeSet};

use super::super::model::{
    EXPECTED_BASELINE_REQUIRES, EXPECTED_GATE_IDS, EXPECTED_STAGE_IDS,
    EXPECTED_TARGETS,
};
use super::super::parse::{
    child, get, is_bool, is_non_blank_str, is_str, is_str_list,
    python_str_list, require, scalar_text,
};

pub(super) fn check_stage_order(
    program_stages: &[(String, &toml::Value)],
    stages: &[(String, &toml::Value)],
    errors: &mut Vec<String>,
) {
    let expected: Vec<String> = EXPECTED_STAGE_IDS
        .iter()
        .map(|id| (*id).to_owned())
        .collect();
    let program_ids: Vec<String> = program_stages.iter().map(|(id, _)| id.clone()).collect();
    let central_ids: Vec<String> = stages.iter().map(|(id, _)| id.clone()).collect();
    require(
        errors,
        central_ids == expected,
        "central stage order is not W0-W10".to_owned(),
    );
    require(
        errors,
        program_ids == expected,
        "program stage order is not W0-W10".to_owned(),
    );
    require(
        errors,
        program_ids.iter().collect::<BTreeSet<_>>() == central_ids.iter().collect::<BTreeSet<_>>(),
        "program/central stage set mismatch".to_owned(),
    );
}

fn expected_closes(source: &toml::Value) -> Vec<String> {
    let mut closes = Vec::new();
    if let Some(completion) = source
        .get("completion_receipt")
        .and_then(toml::Value::as_str)
        && !completion.is_empty()
    {
        closes.push(completion.to_owned());
    }
    if source.get("closes_gate").and_then(toml::Value::as_bool) == Some(true)
        && let Some(contributes) = source
            .get("contributes_to_gate")
            .and_then(toml::Value::as_str)
    {
        closes.push(contributes.to_owned());
    }
    closes
}

fn check_one_stage(
    stage_id: &str,
    row: &toml::Value,
    source: &toml::Value,
    package_names: &BTreeSet<String>,
    covered: &mut BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    require(
        errors,
        row.get("name") == source.get("name"),
        format!("{stage_id}: name mismatch"),
    );
    require(
        errors,
        row.get("required_gates") == source.get("requires_accepted_gates"),
        format!("{stage_id}: gate prerequisite mismatch"),
    );
    require(
        errors,
        row.get("required_receipts") == source.get("requires_accepted_receipts"),
        format!("{stage_id}: receipt prerequisite mismatch"),
    );
    require(
        errors,
        row.get("packages") == source.get("packages"),
        format!("{stage_id}: package set/order mismatch"),
    );
    let have_closes: Option<Vec<String>> = row
        .get("closes")
        .and_then(toml::Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect::<Option<Vec<String>>>()
        });
    require(
        errors,
        have_closes == Some(expected_closes(source)),
        format!("{stage_id}: completion/gate closure mismatch"),
    );
    match row.get("packages") {
        None => {}
        Some(toml::Value::Array(items)) => {
            for package in items {
                match package.as_str() {
                    Some(name) if package_names.contains(name) => {
                        covered.insert(name.to_owned());
                    }
                    _ => errors.push(format!(
                        "{stage_id}: unknown package {}",
                        scalar_text(package)
                    )),
                }
            }
        }
        Some(_) => errors.push(format!("{stage_id}: packages is not an array")),
    }
    require(
        errors,
        is_non_blank_str(row.get("required_product_result")),
        format!("{stage_id}: required product result missing"),
    );
}

pub(super) fn check_stages(
    program_stages: &[(String, &toml::Value)],
    stages: &[(String, &toml::Value)],
    packages: &[(String, &toml::Value)],
    stages_doc: &toml::Value,
    gates: &[(String, &toml::Value)],
    errors: &mut Vec<String>,
) {
    let program_map: BTreeMap<&str, &toml::Value> = program_stages
        .iter()
        .map(|(id, row)| (id.as_str(), *row))
        .collect();
    let central_map: BTreeMap<&str, &toml::Value> =
        stages.iter().map(|(id, row)| (id.as_str(), *row)).collect();
    let package_names: BTreeSet<String> = packages.iter().map(|(name, _)| name.clone()).collect();
    let empty = toml::Value::Table(toml::map::Map::new());
    let mut covered = BTreeSet::new();
    for stage_id in EXPECTED_STAGE_IDS {
        let row = program_map.get(stage_id).copied().unwrap_or(&empty);
        let source = central_map.get(stage_id).copied().unwrap_or(&empty);
        check_one_stage(stage_id, row, source, &package_names, &mut covered, errors);
    }
    if covered != package_names {
        let mut diff: BTreeSet<String> = package_names.difference(&covered).cloned().collect();
        diff.extend(covered.difference(&package_names).cloned());
        errors.push(format!(
            "program package closure mismatch: {}",
            python_str_list(&diff)
        ));
    }
    require(
        errors,
        stages.len() == EXPECTED_STAGE_IDS.len()
            && get(Some(stages_doc), "stage_count").and_then(toml::Value::as_integer) == Some(11),
        "central stage count mismatch".to_owned(),
    );
    require(
        errors,
        gates
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<BTreeSet<_>>()
            == EXPECTED_GATE_IDS.iter().copied().collect::<BTreeSet<_>>(),
        "gate registry is not G0-G6".to_owned(),
    );
}

pub(super) fn check_release_boundary(program: &toml::Value, errors: &mut Vec<String>) {
    let boundary = child(Some(program), "release_boundary", errors);
    for (key, expected) in [
        ("first_bootable_stage", "W1"),
        ("first_direct_source_stage", "W2"),
        ("first_useful_search_stage", "W4"),
        ("release_candidate_stage", "W9"),
        ("optional_depth_stage", "W10"),
    ] {
        let message = match key {
            "first_bootable_stage" => "bootable stage mismatch",
            "first_direct_source_stage" => "DIRECT stage mismatch",
            "first_useful_search_stage" => "useful baseline stage mismatch",
            "release_candidate_stage" => "release-candidate stage mismatch",
            _ => "optional-depth stage mismatch",
        };
        require(errors, is_str(boundary, key, expected), message.to_owned());
    }
    require(
        errors,
        is_str_list(
            boundary,
            "baseline_release_requires",
            &EXPECTED_BASELINE_REQUIRES,
        ),
        "baseline release gate/receipt sequence mismatch".to_owned(),
    );
    require(
        errors,
        is_bool(boundary, "baseline_release_requires_g6", false),
        "G6 became a baseline requirement".to_owned(),
    );
    require(
        errors,
        is_bool(boundary, "baseline_release_requires_w10", false),
        "W10 became a baseline requirement".to_owned(),
    );
}

pub(super) fn check_targets(targets: &[(String, &toml::Value)], errors: &mut Vec<String>) {
    let map: BTreeMap<&str, &toml::Value> = targets
        .iter()
        .map(|(id, row)| (id.as_str(), *row))
        .collect();
    require(
        errors,
        map.keys().copied().collect::<BTreeSet<_>>()
            == EXPECTED_TARGETS
                .iter()
                .map(|(id, _)| *id)
                .collect::<BTreeSet<_>>(),
        "target state set mismatch".to_owned(),
    );
    let empty = toml::Value::Table(toml::map::Map::new());
    for (target_id, stage_id) in EXPECTED_TARGETS {
        let row = map.get(target_id).copied().unwrap_or(&empty);
        require(
            errors,
            row.get("required_stage").and_then(toml::Value::as_str) == Some(stage_id),
            format!("{target_id}: required stage mismatch"),
        );
        require(
            errors,
            is_non_blank_str(row.get("claim")),
            format!("{target_id}: claim missing"),
        );
    }
    let optional = map.get("optional_depth").copied().unwrap_or(&empty);
    require(
        errors,
        optional
            .get("baseline_release_dependency")
            .and_then(toml::Value::as_bool)
            == Some(false),
        "optional depth became a baseline dependency".to_owned(),
    );
}
