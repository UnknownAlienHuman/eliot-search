use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use super::spec::{
    EXECUTION_FALSE_KEYS, EXECUTION_TRUE_KEYS, EXPECTED_PATHS,
    IMPLEMENTATION_MODULES, REGISTRY_AUTHORITY_KEYS,
};
use super::{Validation, string, string_array};

pub(super) fn validate_registry(
    root: &Path,
    registry: &Value,
    validation: &mut Validation,
) {
    validation.require(
        integer(registry, "schema_version") == Some(2)
            && string(registry, "component") == Some("ticket_issuance_planner_v2")
            && string(registry, "status") == Some("ADVISORY_DRY_RUN_ONLY"),
        "registry-identity",
        "planner registry is schema-v2 advisory-only",
    );

    for (key, expected) in EXPECTED_PATHS {
        validation.require(
            string(registry, key) == Some(expected),
            &format!("registry-path:{key}"),
            &format!("{key} = {expected}"),
        );
        if key != "artifact_root" {
            validation.require(
                root.join(expected).is_file(),
                &format!("registry-file:{key}"),
                &format!("registered file exists: {expected}"),
            );
        }
    }

    let modules = string_array(registry.get("implementation_modules"));
    validation.require(
        modules.as_deref() == Some(IMPLEMENTATION_MODULES.as_slice()),
        "registry-modules",
        "implementation module set and order are exact",
    );
    for relative in IMPLEMENTATION_MODULES {
        validation.require(
            root.join(relative).is_file(),
            &format!("registry-module:{relative}"),
            &format!("registered module exists: {relative}"),
        );
    }

    validate_authority(registry, validation);
    validate_execution(registry, validation);
    validate_current_disposition(registry, validation);
}

fn validate_authority(registry: &Value, validation: &mut Validation) {
    let table = registry.get("authority").and_then(Value::as_table);
    validation.require(
        table.is_some_and(|value| !value.is_empty()),
        "registry-authority-table",
        "authority table exists",
    );
    let expected: BTreeSet<&str> = REGISTRY_AUTHORITY_KEYS.into_iter().collect();
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
    validation.require(
        table.is_some(),
        "registry-execution",
        "execution table exists",
    );
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
}

fn validate_current_disposition(
    registry: &Value,
    validation: &mut Validation,
) {
    let table = registry
        .get("current_disposition")
        .and_then(Value::as_table);
    let zero_counts = [
        "materialized_contexts",
        "issued_tickets",
        "active_leases",
        "accepted_package_handoffs",
    ];
    let zero = table.is_some_and(|value| {
        zero_counts.iter().all(|key| {
            value.get(*key).and_then(Value::as_integer) == Some(0)
        }) && value.get("accepted_g0").and_then(Value::as_bool) == Some(false)
            && value.get("accepted_w0").and_then(Value::as_bool) == Some(false)
            && value.get("active_phase").and_then(Value::as_str) == Some("P00")
            && value.get("active_wave").and_then(Value::as_integer) == Some(0)
    });
    validation.require(
        zero,
        "current-disposition",
        "current disposition remains zero-state P00/W0",
    );
}

fn integer(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_integer)
}
