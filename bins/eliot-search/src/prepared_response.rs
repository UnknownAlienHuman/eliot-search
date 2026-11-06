//! Complete bounded prepared-source response validation before terminal output.

use search_source_registry::{
    MAX_PREPARATION_RESPONSE_BYTES, MAX_SOURCE_OBJECT_BYTES, PreparationSettings,
};
use serde_json::Value;

fn keys(value: &Value, names: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == names.len() && names.iter().all(|name| o.contains_key(*name)))
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
fn number(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64()
}
fn positive(value: &Value, key: &str) -> bool {
    number(value, key).is_some_and(|n| n > 0)
}
fn object(value: &Value, kind: &str, maximum: u64) -> Option<u64> {
    if !keys(value, &["version", "kind", "digest", "bytes"])
        || number(value, "version")? != 1
        || value.get("kind")?.as_str()? != kind
        || !hex(value.get("digest")?.as_str()?, 64)
    {
        return None;
    }
    let bytes = number(value, "bytes")?;
    if bytes > maximum || (kind == "prepared" && bytes == 0) {
        None
    } else {
        Some(bytes)
    }
}
fn settings(value: &Value) -> Option<PreparationSettings> {
    if !keys(
        value,
        &[
            "utf8_version",
            "line_unitizer_version",
            "preferred_unit_bytes",
            "max_unit_bytes",
        ],
    ) {
        return None;
    }
    let settings = PreparationSettings {
        utf8_version: u16::try_from(number(value, "utf8_version")?).ok()?,
        line_unitizer_version: u16::try_from(number(value, "line_unitizer_version")?).ok()?,
        preferred_unit_bytes: u32::try_from(number(value, "preferred_unit_bytes")?).ok()?,
        max_unit_bytes: u32::try_from(number(value, "max_unit_bytes")?).ok()?,
    };
    settings.is_supported().then_some(settings)
}
fn unit(value: &Value, ordinal: u64, count: u64, representation: u64, hard: u32) -> bool {
    if !keys(
        value,
        &[
            "ordinal",
            "start",
            "end",
            "line_start",
            "line_end",
            "starts_at_line_boundary",
            "ends_at_line_boundary",
            "text",
        ],
    ) {
        return false;
    }
    let Some(start) = number(value, "start") else {
        return false;
    };
    let Some(end) = number(value, "end") else {
        return false;
    };
    let Some(first) = number(value, "line_start") else {
        return false;
    };
    let Some(last) = number(value, "line_end") else {
        return false;
    };
    let Some(text) = value.get("text").and_then(Value::as_str) else {
        return false;
    };
    if number(value, "ordinal") != Some(ordinal) {
        return false;
    }
    ordinal < count
        && start < end
        && end <= representation
        && end - start <= u64::from(hard)
        && u64::try_from(text.len()).ok() == Some(end - start)
        && first < last
        && value
            .get("starts_at_line_boundary")
            .is_some_and(Value::is_boolean)
        && value
            .get("ends_at_line_boundary")
            .is_some_and(Value::is_boolean)
}

fn bounded_line(bytes: &[u8]) -> Option<&str> {
    if bytes.len() > MAX_PREPARATION_RESPONSE_BYTES {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let line = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text);
    if line.is_empty() || line.contains(['\r', '\n']) {
        return None;
    }
    Some(line)
}

pub fn validate<'a>(bytes: &'a [u8], source_id: &str, ordinal: u64) -> Option<&'a str> {
    let line = bounded_line(bytes)?;
    let value: Value = serde_json::from_slice(line.as_bytes()).ok()?;
    if !keys(
        &value,
        &[
            "operation",
            "state",
            "source_namespace_id",
            "source_id",
            "source_row_revision",
            "registration_revision",
            "settings_revision",
            "outcome_revision",
            "settings",
            "input",
            "artifact",
            "representation_bytes",
            "unit_count",
            "unit",
            "gap",
        ],
    ) || value.get("operation")?.as_str()? != "prepare-source"
        || value.get("source_id")?.as_str()? != source_id
        || !uuid(source_id)
        || !uuid(value.get("source_namespace_id")?.as_str()?)
    {
        return None;
    }
    if ![
        "source_row_revision",
        "registration_revision",
        "settings_revision",
        "outcome_revision",
    ]
    .iter()
    .all(|key| positive(&value, key))
    {
        return None;
    }
    let selected = settings(value.get("settings")?)?;
    let input = object(value.get("input")?, "source", MAX_SOURCE_OBJECT_BYTES)?;
    match value.get("state")?.as_str()? {
        "gap" => {
            if !["artifact", "representation_bytes", "unit_count", "unit"]
                .iter()
                .all(|key| value.get(*key).is_some_and(Value::is_null))
                || !matches!(
                    value.get("gap")?.as_str()?,
                    "binary_content"
                        | "invalid_utf8"
                        | "input_too_large"
                        | "output_too_large"
                        | "too_many_lines"
                        | "too_many_units"
                        | "unit_too_large"
                        | "encoded_too_large"
                )
            {
                return None;
            }
        }
        "ready" | "empty" => {
            object(value.get("artifact")?, "prepared", 64 * 1024 * 1024)?;
            if !value.get("gap")?.is_null() {
                return None;
            }
            let representation = number(&value, "representation_bytes")?;
            let count = number(&value, "unit_count")?;
            if representation != input || count > 1_000_000 {
                return None;
            }
            if value.get("state")?.as_str()? == "empty" {
                if input != 0 || count != 0 || ordinal != 0 || !value.get("unit")?.is_null() {
                    return None;
                }
            } else if representation == 0
                || count == 0
                || !unit(
                    value.get("unit")?,
                    ordinal,
                    count,
                    representation,
                    selected.max_unit_bytes,
                )
            {
                return None;
            }
        }
        _ => return None,
    }
    Some(line)
}
