//! Read-only structural validation for the first four immutable control-record
//! instance-status profiles. A passing report is never issuance authority.
//!
//! The exact input scope is one control-plane registry (16 KiB maximum), four
//! bound record-schema descriptors (64 KiB each), and four bound instance
//! profiles (4 KiB each): 288 KiB maximum total input. This checks profile and
//! binding structure only; it does not validate record signatures, actor
//! authority, issuance state, or full record-schema semantics.

use std::{io::Read, path::Path};

use toml::Value;
use toml::map::Map;

const REGISTRY_PATH: &str = "swarm/control-plane-schema.toml";
const TEMPLATE_STATUS: &str = "SCHEMA_ONLY_NOT_AN_INSTANCE";
const MAX_REGISTRY_BYTES: usize = 16 * 1024;
const MAX_RECORD_SCHEMA_BYTES: usize = 64 * 1024;
const MAX_PROFILE_BYTES: usize = 4 * 1024;
pub const MAX_TOTAL_INPUT_BYTES: usize =
    MAX_REGISTRY_BYTES + (4 * MAX_RECORD_SCHEMA_BYTES) + (4 * MAX_PROFILE_BYTES);
pub const VALIDATION_SCOPE: &str = "FIRST_FOUR_INSTANCE_PROFILE_STRUCTURE_ONLY";

const REQUIRED_SCHEMA_FILES: [&str; 9] = [
    "swarm/schemas/types-v1.toml",
    "swarm/schemas/context-manifest-v1.toml",
    "swarm/schemas/assignment-ticket-v1.toml",
    "swarm/schemas/writer-lease-v1.toml",
    "swarm/schemas/lease-event-v1.toml",
    "swarm/schemas/package-submission-v1.toml",
    "swarm/schemas/independent-review-v1.toml",
    "swarm/schemas/package-handoff-v1.toml",
    "swarm/schemas/supersession-receipt-v1.toml",
];

const FIRST_FOUR: [ProfileSpec; 4] = [
    ProfileSpec {
        record_kind: "context_manifest_v1",
        schema_path: "swarm/schemas/context-manifest-v1.toml",
        profile_path: "swarm/context-manifest-instance-v1.toml",
        profile_id: "context_manifest_instance_v1",
        instance_status: "MATERIALIZED",
    },
    ProfileSpec {
        record_kind: "assignment_ticket_v1",
        schema_path: "swarm/schemas/assignment-ticket-v1.toml",
        profile_path: "swarm/assignment-ticket-instance-v1.toml",
        profile_id: "assignment_ticket_instance_v1",
        instance_status: "ISSUED",
    },
    ProfileSpec {
        record_kind: "writer_lease_v1",
        schema_path: "swarm/schemas/writer-lease-v1.toml",
        profile_path: "swarm/writer-lease-instance-v1.toml",
        profile_id: "writer_lease_instance_v1",
        instance_status: "LEASED",
    },
    ProfileSpec {
        record_kind: "lease_event_v1",
        schema_path: "swarm/schemas/lease-event-v1.toml",
        profile_path: "swarm/lease-event-instance-v1.toml",
        profile_id: "lease_event_instance_v1",
        instance_status: "RECORDED",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProfileSpec {
    record_kind: &'static str,
    schema_path: &'static str,
    profile_path: &'static str,
    profile_id: &'static str,
    instance_status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlRecordInstanceProfileReport {
    pub authority: &'static str,
    pub scope: &'static str,
    pub max_total_input_bytes: usize,
    pub record_kinds: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRecordInstanceProfileError {
    RegistryInputInvalid,
    RegistryDefinitionInvalid,
    RegistryBindingInvalid,
    ProfileInputInvalid,
    ProfileDefinitionInvalid,
    RecordSchemaInputInvalid,
    RecordSchemaDefinitionInvalid,
}

impl ControlRecordInstanceProfileError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RegistryInputInvalid => "INSTANCE_PROFILE_REGISTRY_INPUT_INVALID",
            Self::RegistryDefinitionInvalid => "INSTANCE_PROFILE_REGISTRY_INVALID",
            Self::RegistryBindingInvalid => "INSTANCE_PROFILE_BINDING_INVALID",
            Self::ProfileInputInvalid => "INSTANCE_PROFILE_INPUT_INVALID",
            Self::ProfileDefinitionInvalid => "INSTANCE_PROFILE_DEFINITION_INVALID",
            Self::RecordSchemaInputInvalid => "INSTANCE_PROFILE_RECORD_SCHEMA_INPUT_INVALID",
            Self::RecordSchemaDefinitionInvalid => "INSTANCE_PROFILE_RECORD_SCHEMA_INVALID",
        }
    }
}

/// Validate the exact working-tree definitions for the first four immutable
/// control-record kinds. The function reads only the registry, these four
/// schema descriptors, and the four bound profiles.
pub fn validate_control_record_instance_profiles(
    root: &Path,
) -> Result<ControlRecordInstanceProfileReport, ControlRecordInstanceProfileError> {
    let registry = read_toml(
        root,
        REGISTRY_PATH,
        MAX_REGISTRY_BYTES,
        ControlRecordInstanceProfileError::RegistryInputInvalid,
        ControlRecordInstanceProfileError::RegistryDefinitionInvalid,
    )?;
    let registry_table = registry
        .as_table()
        .ok_or(ControlRecordInstanceProfileError::RegistryDefinitionInvalid)?;
    validate_registry_identity(registry_table)?;
    validate_current_non_authority_disposition(registry_table)?;
    validate_profile_path_list(registry_table)?;
    validate_record_bindings(root, registry_table)?;

    Ok(ControlRecordInstanceProfileReport {
        authority: "NON_AUTHORITATIVE",
        scope: VALIDATION_SCOPE,
        max_total_input_bytes: MAX_TOTAL_INPUT_BYTES,
        record_kinds: FIRST_FOUR.map(|profile| profile.record_kind).to_vec(),
    })
}

fn validate_registry_identity(
    registry: &Map<String, Value>,
) -> Result<(), ControlRecordInstanceProfileError> {
    if integer(registry, "schema_version") != Some(4)
        || string(registry, "project") != Some("eliot-search")
        || string(registry, "status") != Some("SCHEMA_ONLY_NOT_IMPLEMENTED")
        || string(registry, "unknown_load_bearing_fields") != Some("reject")
        || !array_matches(registry, "required_schema_files", &REQUIRED_SCHEMA_FILES)
    {
        return Err(ControlRecordInstanceProfileError::RegistryDefinitionInvalid);
    }
    Ok(())
}

fn validate_current_non_authority_disposition(
    registry: &Map<String, Value>,
) -> Result<(), ControlRecordInstanceProfileError> {
    let Some(disposition) = registry
        .get("current_disposition")
        .and_then(Value::as_table)
    else {
        return Err(ControlRecordInstanceProfileError::RegistryDefinitionInvalid);
    };
    if integer(disposition, "schema_files") != Some(9)
        || integer(disposition, "type_registry_files") != Some(1)
        || integer(disposition, "record_schema_files") != Some(8)
        || integer(disposition, "registered_types") != Some(52)
        || string(disposition, "implementation") != Some("ABSENT")
        || string(disposition, "executed_schema_conformance") != Some("UNAVAILABLE")
        || integer(disposition, "issued_records") != Some(0)
        || integer(disposition, "accepted_handoffs") != Some(0)
    {
        return Err(ControlRecordInstanceProfileError::RegistryDefinitionInvalid);
    }
    Ok(())
}

fn validate_profile_path_list(
    registry: &Map<String, Value>,
) -> Result<(), ControlRecordInstanceProfileError> {
    let expected = FIRST_FOUR.map(|profile| profile.profile_path);
    if !array_matches(registry, "required_instance_profile_files", &expected) {
        return Err(ControlRecordInstanceProfileError::RegistryBindingInvalid);
    }
    Ok(())
}

fn validate_record_bindings(
    root: &Path,
    registry: &Map<String, Value>,
) -> Result<(), ControlRecordInstanceProfileError> {
    let Some(records) = registry.get("record").and_then(Value::as_array) else {
        return Err(ControlRecordInstanceProfileError::RegistryBindingInvalid);
    };
    let mut counts = [0_u8; 4];

    for record in records {
        let Some(record) = record.as_table() else {
            return Err(ControlRecordInstanceProfileError::RegistryBindingInvalid);
        };
        let Some(record_kind) = string(record, "kind") else {
            return Err(ControlRecordInstanceProfileError::RegistryBindingInvalid);
        };
        let Some(index) = FIRST_FOUR
            .iter()
            .position(|profile| profile.record_kind == record_kind)
        else {
            if record.contains_key("instance_profile") {
                return Err(ControlRecordInstanceProfileError::RegistryBindingInvalid);
            }
            continue;
        };

        let expected = FIRST_FOUR[index];
        if !has_exact_keys(
            record,
            &[
                "kind",
                "path",
                "canonical_layout",
                "producer",
                "consumer",
                "instance_profile",
            ],
        ) {
            return Err(ControlRecordInstanceProfileError::RegistryBindingInvalid);
        }
        counts[index] = counts[index].saturating_add(1);
        let bound_schema_path = string(record, "path")
            .ok_or(ControlRecordInstanceProfileError::RegistryBindingInvalid)?;
        let bound_profile_path = string(record, "instance_profile")
            .ok_or(ControlRecordInstanceProfileError::RegistryBindingInvalid)?;
        if counts[index] != 1
            || bound_schema_path != expected.schema_path
            || bound_profile_path != expected.profile_path
        {
            return Err(ControlRecordInstanceProfileError::RegistryBindingInvalid);
        }

        validate_profile(root, expected, bound_profile_path)?;
        validate_record_schema(root, expected, bound_schema_path)?;
    }

    if counts != [1, 1, 1, 1] {
        return Err(ControlRecordInstanceProfileError::RegistryBindingInvalid);
    }
    Ok(())
}

fn validate_profile(
    root: &Path,
    expected: ProfileSpec,
    bound_path: &str,
) -> Result<(), ControlRecordInstanceProfileError> {
    let profile = read_toml(
        root,
        bound_path,
        MAX_PROFILE_BYTES,
        ControlRecordInstanceProfileError::ProfileInputInvalid,
        ControlRecordInstanceProfileError::ProfileDefinitionInvalid,
    )?;
    let Some(profile) = profile.as_table() else {
        return Err(ControlRecordInstanceProfileError::ProfileDefinitionInvalid);
    };

    let expected_keys: &[&str] = if expected.record_kind == "context_manifest_v1" {
        &[
            "schema_version",
            "profile",
            "record_kind",
            "instance_status",
            "renderer_profile",
            "renderer_profile_path",
            "status_values",
            "unknown_instance_status",
            "schema_template_status_is_instance_status",
            "invariants",
        ]
    } else {
        &[
            "schema_version",
            "profile",
            "record_kind",
            "instance_status",
            "status_values",
            "unknown_instance_status",
            "schema_template_status_is_instance_status",
            "invariants",
        ]
    };

    if !has_exact_keys(profile, expected_keys)
        || integer(profile, "schema_version") != Some(1)
        || string(profile, "profile") != Some(expected.profile_id)
        || string(profile, "record_kind") != Some(expected.record_kind)
        || string(profile, "instance_status") != Some(expected.instance_status)
        || expected.instance_status == TEMPLATE_STATUS
        || !array_matches(profile, "status_values", &[expected.instance_status])
        || string(profile, "unknown_instance_status") != Some("reject")
        || boolean(profile, "schema_template_status_is_instance_status") != Some(false)
        || !validate_profile_invariants(profile, expected.record_kind)
    {
        return Err(ControlRecordInstanceProfileError::ProfileDefinitionInvalid);
    }

    if expected.record_kind == "context_manifest_v1"
        && (string(profile, "renderer_profile") != Some("context_manifest_renderer_v1")
            || string(profile, "renderer_profile_path")
                != Some("swarm/context-manifest-renderer-v1.toml"))
    {
        return Err(ControlRecordInstanceProfileError::ProfileDefinitionInvalid);
    }
    Ok(())
}

fn validate_profile_invariants(profile: &Map<String, Value>, record_kind: &str) -> bool {
    let Some(invariants) = profile.get("invariants").and_then(Value::as_table) else {
        return false;
    };
    let expected: &[&str] = if record_kind == "context_manifest_v1" {
        &[
            "record_is_append_only",
            "complete_file_digest_is_external",
            "signed_payload_digest_is_embedded",
            "artifact_readback_required",
            "materializer_and_reviewer_distinct",
            "one_writer_visible_artifact",
        ]
    } else {
        &[
            "record_is_append_only",
            "complete_file_digest_is_external",
            "signed_payload_digest_is_embedded",
        ]
    };
    has_exact_keys(invariants, expected)
        && expected
            .iter()
            .all(|key| boolean(invariants, key) == Some(true))
}

fn validate_record_schema(
    root: &Path,
    expected: ProfileSpec,
    bound_path: &str,
) -> Result<(), ControlRecordInstanceProfileError> {
    let schema = read_toml(
        root,
        bound_path,
        MAX_RECORD_SCHEMA_BYTES,
        ControlRecordInstanceProfileError::RecordSchemaInputInvalid,
        ControlRecordInstanceProfileError::RecordSchemaDefinitionInvalid,
    )?;
    let Some(schema) = schema.as_table() else {
        return Err(ControlRecordInstanceProfileError::RecordSchemaDefinitionInvalid);
    };
    if integer(schema, "schema_version") != Some(1)
        || string(schema, "record_kind") != Some(expected.record_kind)
        || string(schema, "status") != Some(TEMPLATE_STATUS)
        || boolean(schema, "immutable") != Some(true)
        || string(schema, "unknown_fields") != Some("reject")
        || !serialized_status_is_listed_once(schema)
    {
        return Err(ControlRecordInstanceProfileError::RecordSchemaDefinitionInvalid);
    }
    Ok(())
}

fn serialized_status_is_listed_once(schema: &Map<String, Value>) -> bool {
    let Some(field_order) = schema
        .get("canonical_field_order")
        .and_then(Value::as_array)
    else {
        return false;
    };
    field_order.iter().all(Value::is_str)
        && field_order
            .iter()
            .filter(|field| field.as_str() == Some("status"))
            .count()
            == 1
}

fn read_toml(
    root: &Path,
    relative_path: &str,
    max_bytes: usize,
    input_error: ControlRecordInstanceProfileError,
    parse_error: ControlRecordInstanceProfileError,
) -> Result<Value, ControlRecordInstanceProfileError> {
    let path = root.join(relative_path);
    let metadata = std::fs::metadata(&path).map_err(|_| input_error)?;
    let max_bytes_u64 = u64::try_from(max_bytes).map_err(|_| input_error)?;
    if !metadata.is_file() || metadata.len() > max_bytes_u64 {
        return Err(input_error);
    }
    let file = std::fs::File::open(path).map_err(|_| input_error)?;
    let mut bytes = Vec::new();
    file.take(max_bytes_u64.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| input_error)?;
    if bytes.len() > max_bytes {
        return Err(input_error);
    }
    let contents = std::str::from_utf8(&bytes).map_err(|_| parse_error)?;
    toml::from_str(contents).map_err(|_| parse_error)
}

fn has_exact_keys(table: &Map<String, Value>, expected: &[&str]) -> bool {
    table.len() == expected.len() && expected.iter().all(|key| table.contains_key(*key))
}

fn array_matches(table: &Map<String, Value>, key: &str, expected: &[&str]) -> bool {
    let Some(actual) = table.get(key).and_then(Value::as_array) else {
        return false;
    };
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.as_str() == Some(*expected))
}

fn string<'a>(table: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    table.get(key).and_then(Value::as_str)
}

fn integer(table: &Map<String, Value>, key: &str) -> Option<i64> {
    table.get(key).and_then(Value::as_integer)
}

fn boolean(table: &Map<String, Value>, key: &str) -> Option<bool> {
    table.get(key).and_then(Value::as_bool)
}
