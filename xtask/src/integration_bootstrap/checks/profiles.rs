use std::path::Path;

use toml::Value;

use super::super::{EXPECTED_PROFILES, Finding, load_toml};

pub(super) fn validate_build_profiles(
    root: &Path,
    findings: &mut Vec<Finding>,
) {
    let path = root.join("config/build-profiles-v1.toml");
    let document = match load_toml(&path) {
        Ok(document) => document,
        Err(detail) => {
            findings.push(Finding::new(
                "BUILD_PROFILE_REGISTRY_INVALID",
                &path,
                detail,
            ));
            return;
        }
    };
    if document.get("status").and_then(Value::as_str)
        != Some("FROZEN_BOOTSTRAP_NOT_PRODUCT_ACCEPTED")
    {
        findings.push(Finding::new(
            "BUILD_PROFILE_STATUS_INVALID",
            &path,
            "bootstrap status must retain the non-acceptance sentinel",
        ));
    }
    if document.get("default_profile").and_then(Value::as_str)
        != Some("P00_FOUNDATION")
    {
        findings.push(Finding::new(
            "DEFAULT_PROFILE_INVALID",
            &path,
            "P00_FOUNDATION must be the sole bootstrap default",
        ));
    }
    if document
        .get("automatic_profile_upgrade")
        .and_then(Value::as_bool)
        != Some(false)
    {
        findings.push(Finding::new(
            "AUTOMATIC_PROFILE_UPGRADE_ENABLED",
            &path,
            "automatic profile upgrade must fail closed",
        ));
    }

    let Some(profiles) = document.get("profile").and_then(Value::as_array) else {
        findings.push(Finding::new(
            "BUILD_PROFILES_MISSING",
            &path,
            "[[profile]] records are required",
        ));
        return;
    };
    let ids: Option<Vec<&str>> = profiles
        .iter()
        .map(|profile| {
            profile
                .as_table()
                .and_then(|table| table.get("id"))
                .and_then(Value::as_str)
        })
        .collect();
    if ids.as_deref() != Some(EXPECTED_PROFILES.as_slice()) {
        findings.push(Finding::new(
            "BUILD_PROFILE_SET_NOT_EXACT",
            &path,
            "build profile IDs/order differ from the frozen bootstrap set",
        ));
    }
    let defaults: Vec<&str> = profiles
        .iter()
        .filter_map(|profile| {
            let table = profile.as_table()?;
            if table.get("default").and_then(Value::as_bool) == Some(true) {
                table.get("id").and_then(Value::as_str)
            } else {
                None
            }
        })
        .collect();
    if defaults.as_slice() != ["P00_FOUNDATION"] {
        findings.push(Finding::new(
            "BUILD_PROFILE_DEFAULT_NOT_UNIQUE",
            &path,
            format!("unexpected defaults: {defaults:?}"),
        ));
    }
}
