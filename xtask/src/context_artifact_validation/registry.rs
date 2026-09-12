use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use crate::context_artifact::{ARTIFACT_FORMAT, ARTIFACT_ROOT, AUTHORITY_FIELDS};

use super::spec::{
    EXECUTION_FALSE_KEYS, EXECUTION_TRUE_KEYS, EXPECTED_PATHS,
    IMPLEMENTATION_MODULES,
};
use super::{Validation, boolean, integer, string, string_array};

pub(super) fn validate_registry(
    root: &Path,
    registry: &Value,
    validation: &mut Validation,
) {
    validation.require(
        integer(registry, "schema_version") == Some(1)
            && string(registry, "component")
                == Some("context_artifact_builder_v1")
            && string(registry, "status")
                == Some("EXECUTABLE_CANDIDATE_BUILDER_ONLY"),
        "registry-identity",
        "builder registry identity is exact",
    );

    for (key, expected) in EXPECTED_PATHS {
        validation.require(
            string(registry, key) == Some(expected),
            &format!("registry-path:{key}"),
            &format!("{key} = {expected}"),
        );
        validation.require(
            root.join(expected).is_file(),
            &format!("registry-file:{key}"),
            &format!("registered file exists: {expected}"),
        );
    }
    validation.require(
        string(registry, "artifact_root") == Some(ARTIFACT_ROOT),
        "registry-root",
        "artifact root is exact",
    );
    validation.require(
        string(registry, "artifact_format") == Some(ARTIFACT_FORMAT),
        "registry-format",
        "artifact format is exact",
    );

    validation.require(
        string_array(registry.get("implementation_modules")).as_deref()
            == Some(IMPLEMENTATION_MODULES.as_slice()),
        "registry-modules",
        "implementation module set/order is exact",
    );
    for module in IMPLEMENTATION_MODULES {
        validation.require(
            root.join(module).is_file(),
            &format!("module:{module}"),
            &format!("module exists: {module}"),
        );
    }

    validate_authority(registry, validation);
    validate_execution(registry, validation);
    validate_current_disposition(registry, validation);
}

fn validate_authority(registry: &Value, validation: &mut Validation) {
    let table = registry.get("authority").and_then(Value::as_table);
    validation.require(
        table.is_some(),
        "registry-authority",
        "authority table exists",
    );
    let expected: BTreeSet<&str> = AUTHORITY_FIELDS.into_iter().collect();
    let actual: BTreeSet<&str> = table
        .map(|value| value.keys().map(String::as_str).collect())
        .unwrap_or_default();
    validation.require(
        actual == expected,
        "registry-authority-keys",
        "authority key set is closed",
    );
    validation.require(
        table.is_some_and(|value| {
            !value.is_empty()
                && value
                    .values()
                    .all(|entry| entry.as_bool() == Some(false))
        }),
        "registry-authority-false",
        "all authority flags are false",
    );
}

fn validate_execution(registry: &Value, validation: &mut Validation) {
    let table = registry.get("execution").and_then(Value::as_table);
    for key in EXECUTION_TRUE_KEYS {
        validation.require(
            table
                .and_then(|value| value.get(key))
                .and_then(Value::as_bool)
                == Some(true),
            &format!("execution:{key}"),
            &format!("{key} is true"),
        );
    }
    for key in EXECUTION_FALSE_KEYS {
        validation.require(
            table
                .and_then(|value| value.get(key))
                .and_then(Value::as_bool)
                == Some(false),
            &format!("execution:{key}"),
            &format!("{key} is false"),
        );
    }
    for (key, expected) in [
        ("source_materialization", "UTF8_LF"),
        (
            "registry_fragment_materialization",
            "CANONICAL_JSON_UTF8_LF",
        ),
        ("accepted_handoff_materialization", "EXACT_UTF8_LF"),
    ] {
        validation.require(
            table
                .and_then(|value| value.get(key))
                .and_then(Value::as_str)
                == Some(expected),
            &format!("execution:{key}"),
            &format!("{key} = {expected}"),
        );
    }
}

fn validate_current_disposition(
    registry: &Value,
    validation: &mut Validation,
) {
    let table = registry
        .get("current_disposition")
        .and_then(Value::as_table);
    let valid = table.is_some_and(|value| {
        value
            .get("search_contracts_candidate_buildable_at_exact_commit")
            .and_then(Value::as_bool)
            == Some(true)
            && [
                "materialized_contexts",
                "issued_tickets",
                "active_leases",
                "accepted_package_handoffs",
            ]
            .iter()
            .all(|key| {
                value.get(*key).and_then(Value::as_integer) == Some(0)
            })
            && value.get("accepted_g0").and_then(Value::as_bool) == Some(false)
            && value.get("accepted_w0").and_then(Value::as_bool) == Some(false)
            && value.get("active_phase").and_then(Value::as_str) == Some("P00")
            && value.get("active_wave").and_then(Value::as_integer) == Some(0)
    });
    validation.require(
        valid,
        "current-disposition",
        "current disposition remains buildable but zero-authority",
    );
}
