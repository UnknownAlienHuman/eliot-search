//! Immutable-tree source, registry-fragment and handoff extraction.

use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue, json};
use toml::Value as TomlValue;

use crate::context_artifact::{
    BundleBlock, expected_header, normalize_utf8_lf, require_json_value,
};
use crate::ticket_planner::{canonical_json_bytes, exact_sha256_hex};

use super::model::{ContextArtifactBuildError, Preflight};

/// Fully extracted semantic records plus their ordered bundle blocks.
#[derive(Clone, Debug)]
pub(super) struct Extracted {
    pub(super) sources: Vec<JsonValue>,
    pub(super) fragments: Vec<JsonValue>,
    pub(super) handoffs: Vec<JsonValue>,
    pub(super) blocks: Vec<BundleBlock>,
}

/// Extracts every candidate input from the immutable Git view selected during
/// preflight. The working tree is never consulted.
pub(super) fn extract(
    preflight: &Preflight,
    package: &str,
) -> Result<Extracted, ContextArtifactBuildError> {
    let (sources, source_blocks) = source_records(preflight)?;
    let (fragments, fragment_blocks) = fragment_records(preflight, package)?;
    let (handoffs, handoff_blocks) = handoff_records(preflight)?;
    let mut blocks = Vec::with_capacity(
        source_blocks.len() + fragment_blocks.len() + handoff_blocks.len(),
    );
    blocks.extend(source_blocks);
    blocks.extend(fragment_blocks);
    blocks.extend(handoff_blocks);
    Ok(Extracted {
        sources,
        fragments,
        handoffs,
        blocks,
    })
}

fn source_records(
    preflight: &Preflight,
) -> Result<(Vec<JsonValue>, Vec<BundleBlock>), ContextArtifactBuildError> {
    let mut records = Vec::with_capacity(preflight.pair.sources.len());
    let mut blocks = Vec::with_capacity(preflight.pair.sources.len());
    for (order, path) in preflight.pair.sources.iter().enumerate() {
        let (raw, entry) = preflight
            .tree
            .read_bytes(path)
            .map_err(map_git)?;
        let normalized = normalize_utf8_lf(&raw).map_err(map_primitive)?;
        let record = json!({
            "order": order,
            "repository_path": path,
            "git_blob_id": preflight.tree.blob_identity(&entry),
            "exact_sha256": exact_sha256_hex(&raw),
            "exact_bytes": raw.len(),
            "materialization": "UTF8_LF",
            "materialized_sha256": exact_sha256_hex(&normalized),
            "materialized_bytes": normalized.len(),
        });
        let header = expected_header("source", &record).map_err(map_primitive)?;
        blocks.push(BundleBlock {
            kind: "source".to_owned(),
            header,
            metadata: record.clone(),
            content: normalized,
        });
        records.push(record);
    }
    Ok((records, blocks))
}

fn fragment_records(
    preflight: &Preflight,
    package: &str,
) -> Result<(Vec<JsonValue>, Vec<BundleBlock>), ContextArtifactBuildError> {
    let mut records = Vec::with_capacity(preflight.pair.selectors.len());
    let mut blocks = Vec::with_capacity(preflight.pair.selectors.len());
    for (order, selector) in preflight.pair.selectors.iter().enumerate() {
        let (path, expression) = selector.split_once("::").ok_or_else(|| {
            ContextArtifactBuildError::new(
                "CONTEXT_SELECTOR_INVALID",
                format!("invalid selector: {selector}"),
            )
        })?;
        let (document, entry) = preflight.tree.load_toml(path).map_err(map_git)?;
        let (source_raw, _) = preflight.tree.read_bytes(path).map_err(map_git)?;
        let value = resolve_selector(&document, path, expression, package)?;
        require_json_value(&value).map_err(map_primitive)?;
        let fragment = canonical_json_bytes(&json!({
            "registry_path": path,
            "selector": expression,
            "value": value,
        }));
        let record = json!({
            "order": order,
            "registry_path": path,
            "selector": expression,
            "source_git_blob_id": preflight.tree.blob_identity(&entry),
            "source_exact_sha256": exact_sha256_hex(&source_raw),
            "selector_match_count": 1,
            "fragment_sha256": exact_sha256_hex(&fragment),
            "fragment_bytes": fragment.len(),
        });
        let header = expected_header("registry_fragment", &record)
            .map_err(map_primitive)?;
        blocks.push(BundleBlock {
            kind: "registry_fragment".to_owned(),
            header,
            metadata: record.clone(),
            content: fragment,
        });
        records.push(record);
    }
    Ok((records, blocks))
}

fn handoff_records(
    preflight: &Preflight,
) -> Result<(Vec<JsonValue>, Vec<BundleBlock>), ContextArtifactBuildError> {
    let mut records = Vec::with_capacity(preflight.handoffs.len());
    let mut blocks = Vec::with_capacity(preflight.handoffs.len());
    for (order, handoff) in preflight.handoffs.iter().enumerate() {
        let text = std::str::from_utf8(&handoff.bytes).map_err(|_| {
            ContextArtifactBuildError::new(
                "HANDOFF_RECORD_INVALID",
                "handoff is not UTF-8",
            )
        })?;
        if text.contains('\r') || !handoff.bytes.ends_with(b"\n") {
            return Err(ContextArtifactBuildError::new(
                "HANDOFF_RECORD_INVALID",
                "handoff is not exact UTF-8/LF with terminal LF",
            ));
        }
        let mut record = handoff
            .summary
            .as_object()
            .cloned()
            .ok_or_else(|| {
                ContextArtifactBuildError::new(
                    "HANDOFF_RECORD_INVALID",
                    "handoff summary is not an object",
                )
            })?;
        record.insert(
            "order".to_owned(),
            JsonValue::Number(JsonNumber::from(
                u64::try_from(order).unwrap_or(u64::MAX),
            )),
        );
        record.insert(
            "materialization".to_owned(),
            JsonValue::String("EXACT_UTF8_LF".to_owned()),
        );
        record.insert(
            "materialized_sha256".to_owned(),
            JsonValue::String(exact_sha256_hex(&handoff.bytes)),
        );
        record.insert(
            "materialized_bytes".to_owned(),
            JsonValue::Number(JsonNumber::from(
                u64::try_from(handoff.bytes.len()).unwrap_or(u64::MAX),
            )),
        );
        let record = JsonValue::Object(record);
        let header = expected_header("accepted_handoff", &record)
            .map_err(map_primitive)?;
        blocks.push(BundleBlock {
            kind: "accepted_handoff".to_owned(),
            header,
            metadata: record.clone(),
            content: handoff.bytes.clone(),
        });
        records.push(record);
    }
    Ok((records, blocks))
}

fn resolve_selector(
    document: &TomlValue,
    path: &str,
    expression: &str,
    package: &str,
) -> Result<JsonValue, ContextArtifactBuildError> {
    if let Some(name) = bracket_value(expression, "package[name=") {
        if name == package
            && matches!(path, "swarm/crates.toml" | "swarm/modules/w0.toml")
        {
            return unique_toml_row(document, "package", "name", package)
                .and_then(toml_to_json);
        }
    }
    if let Some(name) = bracket_value(expression, "foundation[package=") {
        if name == package && path == "swarm/function-packets.toml" {
            return unique_toml_row(document, "foundation", "package", package)
                .and_then(toml_to_json);
        }
    }
    if let Some(stage) = bracket_value(expression, "stage[id=") {
        if stage == "W0" && path == "swarm/stages.toml" {
            return unique_toml_row(document, "stage", "id", "W0")
                .and_then(toml_to_json);
        }
    }
    for membership in ["authorized_packages", "conditional_packages"] {
        let prefix = format!("{membership}[");
        if expression
            .strip_prefix(&prefix)
            .and_then(|tail| tail.strip_suffix(']'))
            == Some(package)
            && path == "swarm/launch-state.toml"
        {
            let matches = document
                .get(membership)
                .and_then(TomlValue::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter(|value| value.as_str() == Some(package))
                        .count()
                })
                .unwrap_or_default();
            if matches == 1 {
                return Ok(json!({"membership": membership, "package": package}));
            }
        }
    }
    if expression == format!("conditional_activation.{package}")
        && path == "swarm/launch-state.toml"
    {
        if let Some(value) = document
            .get("conditional_activation")
            .and_then(TomlValue::as_table)
            .and_then(|table| table.get(package))
        {
            return toml_to_json(value.clone());
        }
    }
    Err(ContextArtifactBuildError::new(
        "CONTEXT_SELECTOR_NOT_UNIQUE",
        format!("selector did not resolve exactly once: {path}::{expression}"),
    ))
}

fn bracket_value<'a>(expression: &'a str, prefix: &str) -> Option<&'a str> {
    expression.strip_prefix(prefix)?.strip_suffix("]")
}

fn unique_toml_row(
    document: &TomlValue,
    array: &str,
    key: &str,
    expected: &str,
) -> Result<TomlValue, ContextArtifactBuildError> {
    let rows = document
        .get(array)
        .and_then(TomlValue::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| row.get(key).and_then(TomlValue::as_str) == Some(expected))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if rows.len() == 1 {
        Ok(rows.into_iter().next().expect("one row"))
    } else {
        Err(ContextArtifactBuildError::new(
            "CONTEXT_SELECTOR_NOT_UNIQUE",
            format!("{array}[{key}={expected}] did not resolve exactly once"),
        ))
    }
}

fn toml_to_json(value: TomlValue) -> Result<JsonValue, ContextArtifactBuildError> {
    match value {
        TomlValue::String(value) => Ok(JsonValue::String(value)),
        TomlValue::Integer(value) => Ok(JsonValue::Number(JsonNumber::from(value))),
        TomlValue::Boolean(value) => Ok(JsonValue::Bool(value)),
        TomlValue::Array(values) => values
            .into_iter()
            .map(toml_to_json)
            .collect::<Result<Vec<_>, _>>()
            .map(JsonValue::Array),
        TomlValue::Table(values) => {
            let mut map = JsonMap::new();
            for (key, value) in values {
                map.insert(key, toml_to_json(value)?);
            }
            Ok(JsonValue::Object(map))
        }
        TomlValue::Float(_) | TomlValue::Datetime(_) => {
            Err(ContextArtifactBuildError::new(
                "REGISTRY_FRAGMENT_NONCANONICAL",
                "selector contains a forbidden float or datetime value",
            ))
        }
    }
}

fn map_git(error: crate::git_tree::GitTreeError) -> ContextArtifactBuildError {
    ContextArtifactBuildError::new(error.reason(), error.message())
}

fn map_primitive(
    error: crate::context_artifact::ContextArtifactError,
) -> ContextArtifactBuildError {
    ContextArtifactBuildError::new(error.reason(), error.message())
}
