use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use super::super::{Finding, load_toml};

pub(super) fn validate_workspace(root: &Path, findings: &mut Vec<Finding>) {
    let path = root.join("Cargo.toml");
    let document = match load_toml(&path) {
        Ok(document) => document,
        Err(detail) => {
            findings.push(Finding::new(
                "WORKSPACE_MANIFEST_INVALID",
                &path,
                detail,
            ));
            return;
        }
    };
    let Some(workspace) = document.get("workspace").and_then(Value::as_table)
    else {
        findings.push(Finding::new(
            "WORKSPACE_MANIFEST_INVALID",
            &path,
            "[workspace] table is required",
        ));
        return;
    };
    if workspace.get("resolver").and_then(Value::as_str) != Some("3") {
        findings.push(Finding::new(
            "WORKSPACE_RESOLVER_INVALID",
            &path,
            "resolver must be 3",
        ));
    }

    let members = workspace.get("members").and_then(Value::as_array);
    let member_names: Option<Vec<&str>> = members.map(|entries| {
        entries
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
    });
    let exact_members = members.is_some_and(|entries| {
        let names = member_names.as_deref().unwrap_or_default();
        names.len() == entries.len()
            && names.len() == 45
            && names.iter().copied().collect::<BTreeSet<_>>().len() == 45
    });
    if !exact_members {
        findings.push(Finding::new(
            "WORKSPACE_MEMBER_SET_INVALID",
            &path,
            "workspace must contain exactly 45 unique packages",
        ));
    }

    if workspace
        .get("package")
        .and_then(Value::as_table)
        .and_then(|package| package.get("edition"))
        .and_then(Value::as_str)
        != Some("2024")
    {
        findings.push(Finding::new(
            "WORKSPACE_EDITION_INVALID",
            &path,
            "workspace edition must be 2024",
        ));
    }
}
