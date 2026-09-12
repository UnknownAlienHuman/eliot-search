//! Accepted-handoff topology, identity and supersession checks.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value as JsonValue, json};
use toml::Value;

use crate::git_tree::{GitTree, GitTreeEntry};
use crate::ticket_planner::{
    exact_sha256_hex, opaque_id_valid, package_name_valid, safe_path,
    sha256_hex_valid, signed_payload_digest, under,
};

use super::model::{Checks, DraftPair};
use super::util::{string_array, text};

struct SuppliedHandoff {
    path: String,
    record: Value,
    raw: Vec<u8>,
    entry: GitTreeEntry,
}

pub(super) fn validate_handoffs(
    tree: &GitTree,
    pair: &DraftPair,
    paths: &[String],
    checks: &mut Checks,
) -> Vec<JsonValue> {
    let mut expected_from_slots: Vec<String> = pair
        .handoff_slots
        .iter()
        .map(|slot| slot.split_once("::").map_or(slot.as_str(), |value| value.0).to_owned())
        .collect();
    expected_from_slots.sort();
    let mut expected_from_ticket = pair
        .ticket
        .get("dependencies")
        .and_then(Value::as_table)
        .and_then(|table| string_array(table.get("required_handoff_packages")))
        .unwrap_or_default();
    expected_from_ticket.sort();
    if expected_from_slots == expected_from_ticket {
        checks.pass(
            "handoff-topology",
            "ticket dependencies and context handoff slots agree",
        );
    } else {
        checks.fail(
            "handoff-topology",
            "DRAFT_PAIR_MISMATCH",
            "ticket dependencies and context handoff slots disagree",
        );
    }

    let mut supplied: BTreeMap<String, SuppliedHandoff> = BTreeMap::new();
    let mut invalid_or_duplicate = false;
    for (index, path) in paths.iter().enumerate() {
        let check_id = format!("handoff-input-{index:02}");
        if !safe_path(path) || !under(path, "swarm/handoffs") {
            checks.fail(
                check_id,
                "HANDOFF_RECORD_INVALID",
                format!("unsafe handoff path: {path}"),
            );
            invalid_or_duplicate = true;
            continue;
        }
        let (raw, entry) = match tree.read_bytes(path) {
            Ok(value) => value,
            Err(error) => {
                checks.fail(
                    check_id,
                    "HANDOFF_RECORD_INVALID",
                    error.message(),
                );
                invalid_or_duplicate = true;
                continue;
            }
        };
        let record = match parse_toml(&raw) {
            Ok(value) => value,
            Err(detail) => {
                checks.fail(check_id, "HANDOFF_RECORD_INVALID", detail);
                invalid_or_duplicate = true;
                continue;
            }
        };
        let identity = record.get("identity").and_then(Value::as_table);
        let accepted = record.get("accepted_code").and_then(Value::as_table);
        let public = record.get("public_surface").and_then(Value::as_table);
        let signature = record.get("signature").and_then(Value::as_table);
        let package = identity
            .and_then(|table| table.get("package"))
            .and_then(Value::as_str);
        let handoff_id = identity
            .and_then(|table| table.get("handoff_id"))
            .and_then(Value::as_str);
        let final_commit = accepted
            .and_then(|table| table.get("final_commit"))
            .and_then(Value::as_str);
        let api_digest = public
            .and_then(|table| table.get("api_schema_digest"))
            .and_then(Value::as_str);
        let error_digest = public
            .and_then(|table| table.get("error_reason_digest"))
            .and_then(Value::as_str);
        let record_digest = signature
            .and_then(|table| table.get("record_sha256"))
            .and_then(Value::as_str);
        let valid = package.is_some_and(package_name_valid)
            && handoff_id.is_some_and(opaque_id_valid)
            && package.zip(handoff_id).is_some_and(|(package, handoff_id)| {
                path == &format!("swarm/handoffs/{package}/{handoff_id}.toml")
            })
            && record.get("schema_version").and_then(Value::as_integer) == Some(1)
            && text(&record, "record_kind") == Some("package_handoff_v1")
            && text(&record, "status") == Some("ACCEPTED")
            && identity
                .and_then(|table| table.get("stage"))
                .and_then(Value::as_str)
                == Some("W0")
            && final_commit.is_some_and(|commit| tree.commit_exists(commit))
            && api_digest.is_some_and(sha256_hex_valid)
            && error_digest.is_some_and(sha256_hex_valid)
            && record_digest
                .zip(signed_payload_digest(&raw).as_deref())
                .is_some_and(|(actual, expected)| actual == expected);
        let Some(package) = package.map(str::to_owned) else {
            checks.fail(
                check_id,
                "HANDOFF_RECORD_INVALID",
                format!("handoff record failed canonical identity/readback checks: {path}"),
            );
            invalid_or_duplicate = true;
            continue;
        };
        if !valid || supplied.contains_key(&package) {
            checks.fail(
                check_id,
                "HANDOFF_RECORD_INVALID",
                format!("handoff record failed canonical identity/readback checks: {path}"),
            );
            invalid_or_duplicate = true;
            continue;
        }
        supplied.insert(
            package,
            SuppliedHandoff {
                path: path.clone(),
                record,
                raw,
                entry,
            },
        );
        checks.pass(check_id, format!("canonical accepted handoff: {path}"));
    }

    let actual: Vec<String> = supplied.keys().cloned().collect();
    if actual != expected_from_slots || invalid_or_duplicate {
        let expected_set: BTreeSet<&str> =
            expected_from_slots.iter().map(String::as_str).collect();
        let actual_set: BTreeSet<&str> = actual.iter().map(String::as_str).collect();
        if !expected_set.is_subset(&actual_set) {
            checks.fail(
                "handoff-set-missing",
                "HANDOFF_SLOT_UNSATISFIED",
                "required accepted handoff is missing",
            );
        }
        if !actual_set.is_subset(&expected_set)
            || invalid_or_duplicate
            || paths.len() != supplied.len()
        {
            checks.fail(
                "handoff-set-extra",
                "HANDOFF_SET_UNEXPECTED",
                "unexpected, invalid or duplicate handoff supplied",
            );
        }
    } else {
        checks.pass(
            "handoff-set",
            "accepted handoff package set exactly matches draft slots",
        );
    }

    let superseded = superseded_handoff_paths(tree);
    let mut result = Vec::new();
    for (package, supplied) in supplied {
        if superseded.contains(&supplied.path) {
            checks.fail(
                format!("handoff-current-{package}"),
                "HANDOFF_RECORD_SUPERSEDED",
                format!("handoff is superseded: {}", supplied.path),
            );
            continue;
        }
        checks.pass(
            format!("handoff-current-{package}"),
            "handoff is not superseded",
        );
        let identity = supplied
            .record
            .get("identity")
            .and_then(Value::as_table)
            .expect("validated handoff identity");
        let accepted = supplied
            .record
            .get("accepted_code")
            .and_then(Value::as_table)
            .expect("validated accepted code");
        let public = supplied
            .record
            .get("public_surface")
            .and_then(Value::as_table)
            .expect("validated public surface");
        result.push(json!({
            "package": package,
            "path": supplied.path,
            "handoff_id": identity["handoff_id"].as_str().unwrap_or_default(),
            "git_blob_id": tree.blob_identity(&supplied.entry),
            "exact_record_file_sha256": exact_sha256_hex(&supplied.raw),
            "accepted_commit": accepted["final_commit"].as_str().unwrap_or_default(),
            "api_schema_digest": public["api_schema_digest"].as_str().unwrap_or_default(),
            "error_reason_digest": public["error_reason_digest"].as_str().unwrap_or_default(),
        }));
    }
    result
}

fn parse_toml(raw: &[u8]) -> Result<Value, String> {
    let text = std::str::from_utf8(raw)
        .map_err(|error| format!("handoff is not strict UTF-8: {error}"))?;
    toml::from_str(text).map_err(|error| format!("invalid handoff TOML: {error}"))
}

fn superseded_handoff_paths(tree: &GitTree) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let Ok(paths) = tree.list_files("swarm/supersessions") else {
        return result;
    };
    for path in paths {
        if matches!(
            path.as_str(),
            "swarm/supersessions/README.md" | "swarm/supersessions/.gitkeep"
        ) {
            continue;
        }
        let Ok((record, _)) = tree.load_toml(&path) else {
            continue;
        };
        let old_path = record
            .get("old_record")
            .and_then(Value::as_table)
            .and_then(|table| table.get("ref"))
            .and_then(Value::as_table)
            .and_then(|table| table.get("path"))
            .and_then(Value::as_str);
        if let Some(old_path) = old_path {
            result.insert(old_path.to_owned());
        }
    }
    result
}
