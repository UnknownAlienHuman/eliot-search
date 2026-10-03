//! Bounded, non-authoritative validation of the adopted qualified opaque-id type and field bindings.
//!
//! This checks the exact named syntax type and two existing field rules. It does
//! not consult namespace/profile registrations, qualification, trust, or issuance.

use std::{fs::File, io::Read, path::Path};

use toml::{Value, map::Map};

use crate::qualified_profile_id::QualifiedOpaqueId;

const CONTROL_REGISTRY_PATH: &str = "swarm/control-plane-schema.toml";
const TYPE_REGISTRY_PATH: &str = "swarm/schemas/types-v1.toml";
const CONTROL_REGISTRY_SCHEMA_VERSION: i64 = 4;
const TYPE_REGISTRY_SCHEMA_VERSION: i64 = 2;
const DECLARED_TYPE_COUNT: usize = 52;
const REGISTERED_TYPE_COUNT: i64 = 52;
const MAX_CONTROL_REGISTRY_BYTES: usize = 16 * 1024;
const MAX_TYPE_REGISTRY_BYTES: usize = 64 * 1024;
const MAX_VALUE_CHECK_INPUT_BYTES: usize = 512;

const QUALIFIED_TYPE_PATTERN: &str =
    "^[a-z][a-z0-9]*(?:-[a-z0-9]+)*/[A-Za-z0-9][A-Za-z0-9._-]{0,127}$";
const QUALIFIED_TYPE_RULES: [&str; 4] = [
    "exactly_one_ascii_forward_slash",
    "namespace_uses_lowercase_ascii_hyphen_segments",
    "local_component_uses_OpaqueId_pattern",
    "preserve_exact_bytes_and_case_without_normalization",
];

/// Maximum combined bytes read from the exact control and type registries.
pub const MAX_TOTAL_INPUT_BYTES: usize = MAX_CONTROL_REGISTRY_BYTES + MAX_TYPE_REGISTRY_BYTES;
/// Scope explicitly excludes profile resolution, qualification, trust, and authority.
pub const VALIDATION_SCOPE: &str = "QUALIFIED_OPAQUE_ID_SYNTAX_BINDINGS_ONLY";

const FIELD_BINDINGS: [FieldBinding; 2] = [
    FieldBinding {
        owner_type: "ImmutableArtifactRef",
        field_name: "store_profile_ref",
        rule: "store_profile_ref_is_qualified_opaque_id",
        canonical_fields: &["store_profile_ref", "artifact_id", "bytes", "sha256"],
    },
    FieldBinding {
        owner_type: "ImmutableSignatureRef",
        field_name: "approval_profile_ref",
        rule: "approval_profile_ref_is_qualified_opaque_id",
        canonical_fields: &[
            "approval_profile_ref",
            "approval_artifact_ref",
            "signed_payload_sha256",
            "actor_identity",
        ],
    },
];

#[derive(Debug, Clone, Copy)]
struct FieldBinding {
    owner_type: &'static str,
    field_name: &'static str,
    rule: &'static str,
    canonical_fields: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Structural summary; NON_AUTHORITATIVE does not mean profiles are qualified.
pub struct QualifiedProfileIdRegistryReport {
    /// Authority label for consumers.
    pub authority: &'static str,
    /// Finite validation scope.
    pub scope: &'static str,
    /// Maximum combined bytes read from the two registry inputs.
    pub max_total_input_bytes: usize,
    /// Registered type count pinned by the accepted control registry.
    pub declared_type_count: usize,
    /// Number of exact qualified-identifier field bindings checked.
    pub binding_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Content-minimized failures from this bounded validator.
pub enum QualifiedProfileIdRegistryError {
    /// A required registry is missing, unreadable, or exceeds its byte ceiling.
    RegistryInputInvalid,
    /// Registry TOML is malformed or does not pin the accepted schema/count.
    RegistryDefinitionInvalid,
    /// The qualified type is missing, duplicated, aliased, or not exact.
    QualifiedTypeDefinitionInvalid,
    /// An adopted owner/field/rule binding is missing, duplicated, or cross-bound.
    FieldBindingInvalid,
    /// A value is not an exact accepted qualified-identifier token.
    QualifiedValueInvalid,
}

impl QualifiedProfileIdRegistryError {
    /// Stable, content-minimized diagnostic code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::RegistryInputInvalid => "QUALIFIED_PROFILE_REGISTRY_INPUT_INVALID",
            Self::RegistryDefinitionInvalid => "QUALIFIED_PROFILE_REGISTRY_INVALID",
            Self::QualifiedTypeDefinitionInvalid => "QUALIFIED_OPAQUE_ID_DEFINITION_INVALID",
            Self::FieldBindingInvalid => "QUALIFIED_PROFILE_FIELD_BINDING_INVALID",
            Self::QualifiedValueInvalid => "QUALIFIED_OPAQUE_ID_VALUE_INVALID",
        }
    }
}

/// Validate the exact syntax type and two accepted owner/field rule bindings.
///
/// This structural check is NON_AUTHORITATIVE; it does not resolve profiles,
/// consult namespaces, establish qualification or trust, validate a full record
/// instance, or authorize issuance.
pub fn validate_qualified_profile_id_registry(
    root: &Path,
) -> Result<QualifiedProfileIdRegistryReport, QualifiedProfileIdRegistryError> {
    let control_registry = read_toml(
        root,
        CONTROL_REGISTRY_PATH,
        MAX_CONTROL_REGISTRY_BYTES,
        QualifiedProfileIdRegistryError::RegistryInputInvalid,
        QualifiedProfileIdRegistryError::RegistryDefinitionInvalid,
    )?;
    let control_registry = control_registry
        .as_table()
        .ok_or(QualifiedProfileIdRegistryError::RegistryDefinitionInvalid)?;
    validate_control_registry(control_registry)?;

    let type_registry = read_toml(
        root,
        TYPE_REGISTRY_PATH,
        MAX_TYPE_REGISTRY_BYTES,
        QualifiedProfileIdRegistryError::RegistryInputInvalid,
        QualifiedProfileIdRegistryError::RegistryDefinitionInvalid,
    )?;
    let type_registry = type_registry
        .as_table()
        .ok_or(QualifiedProfileIdRegistryError::RegistryDefinitionInvalid)?;
    validate_type_registry_identity(type_registry)?;

    let types = type_registry
        .get("type")
        .and_then(Value::as_array)
        .ok_or(QualifiedProfileIdRegistryError::RegistryDefinitionInvalid)?;
    if types.len() != DECLARED_TYPE_COUNT {
        return Err(QualifiedProfileIdRegistryError::RegistryDefinitionInvalid);
    }

    validate_qualified_type_definition(types)?;
    validate_field_bindings(types)?;

    Ok(QualifiedProfileIdRegistryReport {
        authority: "NON_AUTHORITATIVE",
        scope: VALIDATION_SCOPE,
        max_total_input_bytes: MAX_TOTAL_INPUT_BYTES,
        declared_type_count: types.len(),
        binding_count: FIELD_BINDINGS.len(),
    })
}

/// Pure NON_AUTHORITATIVE syntax checking for the two accepted field bindings.
///
/// Only exact owner/field pairs are accepted, and the value is checked by the
/// pure QualifiedOpaqueId parser. This performs no namespace/profile lookup,
/// execution qualification, trust check, full record-instance validation, or
/// authority decision.
pub fn validate_adopted_qualified_profile_id_field_value(
    owner_type: &str,
    field_name: &str,
    value: &str,
) -> Result<(), QualifiedProfileIdRegistryError> {
    if !FIELD_BINDINGS
        .iter()
        .any(|binding| binding.owner_type == owner_type && binding.field_name == field_name)
    {
        return Err(QualifiedProfileIdRegistryError::FieldBindingInvalid);
    }

    if owner_type
        .len()
        .saturating_add(field_name.len())
        .saturating_add(value.len())
        > MAX_VALUE_CHECK_INPUT_BYTES
        || QualifiedOpaqueId::parse(value).is_err()
    {
        return Err(QualifiedProfileIdRegistryError::QualifiedValueInvalid);
    }

    Ok(())
}

fn validate_control_registry(
    registry: &Map<String, Value>,
) -> Result<(), QualifiedProfileIdRegistryError> {
    let current_disposition = registry
        .get("current_disposition")
        .and_then(Value::as_table)
        .ok_or(QualifiedProfileIdRegistryError::RegistryDefinitionInvalid)?;
    if integer(registry, "schema_version") != Some(CONTROL_REGISTRY_SCHEMA_VERSION)
        || string(registry, "project") != Some("eliot-search")
        || string(registry, "status") != Some("SCHEMA_ONLY_NOT_IMPLEMENTED")
        || string(registry, "type_registry") != Some(TYPE_REGISTRY_PATH)
        || integer(registry, "type_registry_schema_version") != Some(TYPE_REGISTRY_SCHEMA_VERSION)
        || integer(current_disposition, "registered_types") != Some(REGISTERED_TYPE_COUNT)
        || string(registry, "unknown_load_bearing_fields") != Some("reject")
    {
        return Err(QualifiedProfileIdRegistryError::RegistryDefinitionInvalid);
    }
    Ok(())
}

fn validate_type_registry_identity(
    registry: &Map<String, Value>,
) -> Result<(), QualifiedProfileIdRegistryError> {
    if integer(registry, "schema_version") != Some(TYPE_REGISTRY_SCHEMA_VERSION)
        || string(registry, "registry_kind") != Some("control_plane_types_v1")
        || string(registry, "status") != Some("SCHEMA_ONLY_NOT_IMPLEMENTED")
        || string(registry, "unknown_types") != Some("reject")
        || boolean(registry, "implicit_string_coercion_allowed") != Some(false)
    {
        return Err(QualifiedProfileIdRegistryError::RegistryDefinitionInvalid);
    }
    Ok(())
}

fn validate_qualified_type_definition(
    types: &[Value],
) -> Result<(), QualifiedProfileIdRegistryError> {
    let definition = unique_named_type(types, "QualifiedOpaqueId")
        .ok_or(QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid)?;

    if !has_exact_keys(
        definition,
        &[
            "name",
            "representation",
            "pattern",
            "max_bytes",
            "namespace_max_bytes",
            "local_max_bytes",
            "canonical",
            "rules",
        ],
    ) || string(definition, "name") != Some("QualifiedOpaqueId")
        || string(definition, "representation") != Some("string")
        || string(definition, "pattern") != Some(QUALIFIED_TYPE_PATTERN)
        || integer(definition, "max_bytes") != Some(225)
        || integer(definition, "namespace_max_bytes") != Some(96)
        || integer(definition, "local_max_bytes") != Some(128)
        || boolean(definition, "canonical") != Some(true)
        || !array_matches(definition, "rules", &QUALIFIED_TYPE_RULES)
    {
        return Err(QualifiedProfileIdRegistryError::QualifiedTypeDefinitionInvalid);
    }
    Ok(())
}

fn validate_field_bindings(types: &[Value]) -> Result<(), QualifiedProfileIdRegistryError> {
    for binding in FIELD_BINDINGS {
        let owner = unique_named_type(types, binding.owner_type)
            .ok_or(QualifiedProfileIdRegistryError::FieldBindingInvalid)?;
        if !has_exact_keys(owner, &["name", "representation", "canonical_fields", "rules"])
            || string(owner, "representation") != Some("ordered_record")
            || !array_matches(owner, "canonical_fields", binding.canonical_fields)
        {
            return Err(QualifiedProfileIdRegistryError::FieldBindingInvalid);
        }

        let rules = string_array(owner, "rules")
            .ok_or(QualifiedProfileIdRegistryError::FieldBindingInvalid)?;
        let fields = string_array(owner, "canonical_fields")
            .ok_or(QualifiedProfileIdRegistryError::FieldBindingInvalid)?;
        if fields
            .iter()
            .filter(|field| **field == binding.field_name)
            .count()
            != 1
            || rules
                .iter()
                .filter(|rule| **rule == binding.rule)
                .count()
                != 1
        {
            return Err(QualifiedProfileIdRegistryError::FieldBindingInvalid);
        }
    }

    for ty in types {
        let Some(table) = ty.as_table() else {
            continue;
        };
        let Some(owner_name) = string(table, "name") else {
            continue;
        };
        let Some(rules) = table.get("rules").and_then(Value::as_array) else {
            continue;
        };
        for rule in rules.iter().filter_map(Value::as_str) {
            if let Some(binding) = FIELD_BINDINGS.iter().find(|binding| binding.rule == rule) {
                if owner_name != binding.owner_type {
                    return Err(QualifiedProfileIdRegistryError::FieldBindingInvalid);
                }
            } else if rule.ends_with("_is_qualified_opaque_id") {
                return Err(QualifiedProfileIdRegistryError::FieldBindingInvalid);
            }
        }
    }

    Ok(())
}

fn unique_named_type<'a>(types: &'a [Value], name: &str) -> Option<&'a Map<String, Value>> {
    let mut matches = types.iter().filter_map(|ty| {
        let table = ty.as_table()?;
        (string(table, "name") == Some(name)).then_some(table)
    });
    let definition = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(definition)
}

fn read_toml(
    root: &Path,
    relative_path: &str,
    max_bytes: usize,
    input_error: QualifiedProfileIdRegistryError,
    parse_error: QualifiedProfileIdRegistryError,
) -> Result<Value, QualifiedProfileIdRegistryError> {
    let path = root.join(relative_path);
    let metadata = std::fs::metadata(&path).map_err(|_| input_error)?;
    let max_bytes_u64 = u64::try_from(max_bytes).map_err(|_| input_error)?;
    if !metadata.is_file() || metadata.len() > max_bytes_u64 {
        return Err(input_error);
    }
    let file = File::open(path).map_err(|_| input_error)?;
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

fn string_array<'a>(table: &'a Map<String, Value>, key: &str) -> Option<Vec<&'a str>> {
    table
        .get(key)?
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect()
}

fn string<'a>(table: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    table.get(key)?.as_str()
}

fn integer(table: &Map<String, Value>, key: &str) -> Option<i64> {
    table.get(key)?.as_integer()
}

fn boolean(table: &Map<String, Value>, key: &str) -> Option<bool> {
    table.get(key)?.as_bool()
}