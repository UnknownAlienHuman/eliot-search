//! Bounded, non-authoritative validation of the adopted control-record enum bindings.
//!
//! This reads only the control-plane registry and type registry, with a combined
//! input ceiling of 80 KiB. It validates four exact enum vocabularies and nine
//! type-to-field rule bindings. It does not validate record instances, signatures,
//! actors, fixture qualification, execution evidence, store readback, or issuance.

use std::{fs::File, io::Read, path::Path};

use toml::{Value, map::Map};

const CONTROL_REGISTRY_PATH: &str = "swarm/control-plane-schema.toml";
const TYPE_REGISTRY_PATH: &str = "swarm/schemas/types-v1.toml";
const CONTROL_REGISTRY_SCHEMA_VERSION: i64 = 4;
const TYPE_REGISTRY_SCHEMA_VERSION: i64 = 2;
const DECLARED_TYPE_COUNT: usize = 51;
const REGISTERED_TYPE_COUNT: i64 = 51;
const MAX_CONTROL_REGISTRY_BYTES: usize = 16 * 1024;
const MAX_TYPE_REGISTRY_BYTES: usize = 64 * 1024;

/// Maximum combined bytes read from the exact control and type registries.
pub const MAX_TOTAL_INPUT_BYTES: usize = MAX_CONTROL_REGISTRY_BYTES + MAX_TYPE_REGISTRY_BYTES;
/// Declared validation scope; it conveys no implementation or issuance authority.
pub const VALIDATION_SCOPE: &str = "CONTROL_RECORD_ENUM_BINDINGS_ONLY";

const ENUM_SPECS: [EnumSpec; 4] = [
    EnumSpec {
        name: "FixtureQualificationStatus",
        allowed: &["FAILED", "QUALIFIED", "UNAVAILABLE"],
    },
    EnumSpec {
        name: "NormalProcessExitClass",
        allowed: &["EXIT_NONZERO", "EXIT_ZERO"],
    },
    EnumSpec {
        name: "EvidenceArtifactClass",
        allowed: &[
            "PACKAGE_HANDOFF_CANDIDATE",
            "PUBLIC_API_SCHEMA_DIGEST",
            "QUALIFICATION_PROBE_RESULT",
            "RESIDUAL_RISK_RECORD",
            "TEST_RESULT",
        ],
    },
    EnumSpec {
        name: "ExpectedBehaviorClass",
        allowed: &["FAILURE", "POLICY", "RECOVERY", "SUCCESS"],
    },
];

const BINDING_SPECS: [BindingSpec; 9] = [
    BindingSpec {
        owner_type: "OrderedFixtureRef",
        field_name: "qualification_status",
        target_type: "FixtureQualificationStatus",
    },
    BindingSpec {
        owner_type: "BoundedCommandSpec",
        field_name: "expected_exit_class",
        target_type: "NormalProcessExitClass",
    },
    BindingSpec {
        owner_type: "OrderedRawCommandOutcomeRef",
        field_name: "exit_class",
        target_type: "NormalProcessExitClass",
    },
    BindingSpec {
        owner_type: "BoundedCommandSpec",
        field_name: "evidence_class",
        target_type: "EvidenceArtifactClass",
    },
    BindingSpec {
        owner_type: "EvidenceRequirement",
        field_name: "evidence_class",
        target_type: "EvidenceArtifactClass",
    },
    BindingSpec {
        owner_type: "OrderedEvidenceRef",
        field_name: "evidence_class",
        target_type: "EvidenceArtifactClass",
    },
    BindingSpec {
        owner_type: "OrderedAcceptedEvidenceRef",
        field_name: "evidence_class",
        target_type: "EvidenceArtifactClass",
    },
    BindingSpec {
        owner_type: "EvidenceRequirement",
        field_name: "acceptance_class",
        target_type: "ExpectedBehaviorClass",
    },
    BindingSpec {
        owner_type: "OrderedEvidenceRef",
        field_name: "acceptance_class",
        target_type: "ExpectedBehaviorClass",
    },
];

#[derive(Debug, Clone, Copy)]
struct EnumSpec {
    name: &'static str,
    allowed: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
struct BindingSpec {
    owner_type: &'static str,
    field_name: &'static str,
    target_type: &'static str,
}

impl BindingSpec {
    fn rule(self) -> String {
        format!("{}_is_{}", self.field_name, self.target_type)
    }

    fn legacy_rule(self) -> String {
        format!("{}_is_ClosedEnum", self.field_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Structural validation summary that explicitly remains non-authoritative.
pub struct ControlRecordEnumBindingsReport {
    /// Authority label for consumers.
    pub authority: &'static str,
    /// Finite scope validated by this report.
    pub scope: &'static str,
    /// Maximum combined bytes this validator may read.
    pub max_total_input_bytes: usize,
    /// Declared type count pinned by the control registry.
    pub declared_type_count: usize,
    /// Number of exact adopted enum definitions checked.
    pub enum_count: usize,
    /// Number of exact adopted field bindings checked.
    pub binding_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Content-minimized failures from this bounded structural validator.
pub enum ControlRecordEnumBindingsError {
    /// The exact control registry is missing, unreadable, or over its byte ceiling.
    ControlRegistryInputInvalid,
    /// The exact control registry identity or adopted type-registry pin is wrong.
    ControlRegistryDefinitionInvalid,
    /// The exact type registry is missing, unreadable, or over its byte ceiling.
    TypeRegistryInputInvalid,
    /// The type registry identity or declared type count is wrong.
    TypeRegistryDefinitionInvalid,
    /// An adopted enum definition is missing, duplicated, or not exact.
    EnumDefinitionInvalid,
    /// An adopted field binding is missing, duplicated, wrong, or cross-bound.
    BindingInvalid,
}

impl ControlRecordEnumBindingsError {
    /// Stable, content-minimized diagnostic code for CLI and test consumers.
    pub const fn code(self) -> &'static str {
        match self {
            Self::ControlRegistryInputInvalid => "CONTROL_ENUM_REGISTRY_INPUT_INVALID",
            Self::ControlRegistryDefinitionInvalid => "CONTROL_ENUM_REGISTRY_INVALID",
            Self::TypeRegistryInputInvalid => "CONTROL_ENUM_TYPE_INPUT_INVALID",
            Self::TypeRegistryDefinitionInvalid => "CONTROL_ENUM_TYPE_REGISTRY_INVALID",
            Self::EnumDefinitionInvalid => "CONTROL_ENUM_DEFINITION_INVALID",
            Self::BindingInvalid => "CONTROL_ENUM_BINDING_INVALID",
        }
    }
}

/// Validate the exact adopted enum definitions and bindings from a working tree.
pub fn validate_control_record_enum_bindings(
    root: &Path,
) -> Result<ControlRecordEnumBindingsReport, ControlRecordEnumBindingsError> {
    let control_registry = read_toml(
        root,
        CONTROL_REGISTRY_PATH,
        MAX_CONTROL_REGISTRY_BYTES,
        ControlRecordEnumBindingsError::ControlRegistryInputInvalid,
        ControlRecordEnumBindingsError::ControlRegistryDefinitionInvalid,
    )?;
    let control_registry = control_registry
        .as_table()
        .ok_or(ControlRecordEnumBindingsError::ControlRegistryDefinitionInvalid)?;
    validate_control_registry(control_registry)?;

    let type_registry = read_toml(
        root,
        TYPE_REGISTRY_PATH,
        MAX_TYPE_REGISTRY_BYTES,
        ControlRecordEnumBindingsError::TypeRegistryInputInvalid,
        ControlRecordEnumBindingsError::TypeRegistryDefinitionInvalid,
    )?;
    let type_registry = type_registry
        .as_table()
        .ok_or(ControlRecordEnumBindingsError::TypeRegistryDefinitionInvalid)?;
    validate_type_registry_identity(type_registry)?;

    let types = type_registry
        .get("type")
        .and_then(Value::as_array)
        .ok_or(ControlRecordEnumBindingsError::TypeRegistryDefinitionInvalid)?;
    if types.len() != DECLARED_TYPE_COUNT {
        return Err(ControlRecordEnumBindingsError::TypeRegistryDefinitionInvalid);
    }

    validate_enum_definitions(types)?;
    validate_field_bindings(types)?;

    Ok(ControlRecordEnumBindingsReport {
        authority: "NON_AUTHORITATIVE",
        scope: VALIDATION_SCOPE,
        max_total_input_bytes: MAX_TOTAL_INPUT_BYTES,
        declared_type_count: types.len(),
        enum_count: ENUM_SPECS.len(),
        binding_count: BINDING_SPECS.len(),
    })
}

fn validate_control_registry(
    registry: &Map<String, Value>,
) -> Result<(), ControlRecordEnumBindingsError> {
    if integer(registry, "schema_version") != Some(CONTROL_REGISTRY_SCHEMA_VERSION)
        || string(registry, "project") != Some("eliot-search")
        || string(registry, "status") != Some("SCHEMA_ONLY_NOT_IMPLEMENTED")
        || string(registry, "type_registry") != Some(TYPE_REGISTRY_PATH)
        || integer(registry, "type_registry_schema_version") != Some(TYPE_REGISTRY_SCHEMA_VERSION)
        || integer(registry, "registered_types") != Some(REGISTERED_TYPE_COUNT)
        || string(registry, "unknown_load_bearing_fields") != Some("reject")
    {
        return Err(ControlRecordEnumBindingsError::ControlRegistryDefinitionInvalid);
    }
    Ok(())
}

fn validate_type_registry_identity(
    registry: &Map<String, Value>,
) -> Result<(), ControlRecordEnumBindingsError> {
    if integer(registry, "schema_version") != Some(TYPE_REGISTRY_SCHEMA_VERSION)
        || string(registry, "registry_kind") != Some("control_plane_types_v1")
        || string(registry, "status") != Some("SCHEMA_ONLY_NOT_IMPLEMENTED")
        || string(registry, "unknown_types") != Some("reject")
        || boolean(registry, "implicit_string_coercion_allowed") != Some(false)
    {
        return Err(ControlRecordEnumBindingsError::TypeRegistryDefinitionInvalid);
    }
    Ok(())
}

fn validate_enum_definitions(types: &[Value]) -> Result<(), ControlRecordEnumBindingsError> {
    for expected in ENUM_SPECS {
        let mut matching = types.iter().filter(|ty| {
            ty.as_table()
                .and_then(|table| string(table, "name"))
                == Some(expected.name)
        });
        let Some(definition) = matching.next() else {
            return Err(ControlRecordEnumBindingsError::EnumDefinitionInvalid);
        };
        if matching.next().is_some() {
            return Err(ControlRecordEnumBindingsError::EnumDefinitionInvalid);
        }
        let Some(definition) = definition.as_table() else {
            return Err(ControlRecordEnumBindingsError::EnumDefinitionInvalid);
        };
        if !has_exact_keys(
            definition,
            &["name", "representation", "allowed", "canonical"],
        ) || string(definition, "representation") != Some("string")
            || boolean(definition, "canonical") != Some(true)
            || !array_matches(definition, "allowed", expected.allowed)
        {
            return Err(ControlRecordEnumBindingsError::EnumDefinitionInvalid);
        }
    }
    Ok(())
}

fn validate_field_bindings(types: &[Value]) -> Result<(), ControlRecordEnumBindingsError> {
    for expected in BINDING_SPECS {
        let mut matching_owner = types.iter().filter(|ty| {
            ty.as_table()
                .and_then(|table| string(table, "name"))
                == Some(expected.owner_type)
        });
        let Some(owner) = matching_owner.next() else {
            return Err(ControlRecordEnumBindingsError::BindingInvalid);
        };
        if matching_owner.next().is_some() {
            return Err(ControlRecordEnumBindingsError::BindingInvalid);
        }
        let Some(owner) = owner.as_table() else {
            return Err(ControlRecordEnumBindingsError::BindingInvalid);
        };
        if !has_exact_keys(owner, &["name", "representation", "canonical_fields", "rules"])
            || string(owner, "representation") != Some("ordered_record")
        {
            return Err(ControlRecordEnumBindingsError::BindingInvalid);
        }

        let fields = string_array(owner, "canonical_fields")
            .ok_or(ControlRecordEnumBindingsError::BindingInvalid)?;
        let rules = string_array(owner, "rules")
            .ok_or(ControlRecordEnumBindingsError::BindingInvalid)?;
        let expected_rule = expected.rule();
        let legacy_rule = expected.legacy_rule();
        if fields
            .iter()
            .filter(|field| **field == expected.field_name)
            .count()
            != 1
            || rules
                .iter()
                .filter(|rule| **rule == expected_rule.as_str())
                .count()
                != 1
            || rules
                .iter()
                .any(|rule| *rule == legacy_rule.as_str())
        {
            return Err(ControlRecordEnumBindingsError::BindingInvalid);
        }

        let mut global_rule_count = 0;
        for ty in types {
            let Some(table) = ty.as_table() else {
                return Err(ControlRecordEnumBindingsError::BindingInvalid);
            };
            let Some(type_name) = string(table, "name") else {
                return Err(ControlRecordEnumBindingsError::BindingInvalid);
            };
            let Some(type_rules) = table.get("rules").and_then(Value::as_array) else {
                continue;
            };
            if type_rules.iter().any(|rule| !rule.is_str()) {
                return Err(ControlRecordEnumBindingsError::BindingInvalid);
            }
            let occurrences = type_rules
                .iter()
                .filter(|rule| rule.as_str() == Some(expected_rule.as_str()))
                .count();
            if occurrences > 0 && type_name != expected.owner_type {
                return Err(ControlRecordEnumBindingsError::BindingInvalid);
            }
            global_rule_count += occurrences;
        }
        if global_rule_count != 1 {
            return Err(ControlRecordEnumBindingsError::BindingInvalid);
        }
    }

    for ty in types {
        let Some(table) = ty.as_table() else {
            return Err(ControlRecordEnumBindingsError::BindingInvalid);
        };
        let Some(rules) = table.get("rules").and_then(Value::as_array) else {
            continue;
        };
        for rule in rules.iter().filter_map(Value::as_str) {
            for adopted_type in ENUM_SPECS.map(|spec| spec.name) {
                let suffix = format!("_is_{adopted_type}");
                if rule.ends_with(suffix.as_str())
                    && !BINDING_SPECS
                        .iter()
                        .any(|binding| binding.rule().as_str() == rule)
                {
                    return Err(ControlRecordEnumBindingsError::BindingInvalid);
                }
            }
        }
    }

    Ok(())
}

fn read_toml(
    root: &Path,
    relative_path: &str,
    max_bytes: usize,
    input_error: ControlRecordEnumBindingsError,
    parse_error: ControlRecordEnumBindingsError,
) -> Result<Value, ControlRecordEnumBindingsError> {
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
