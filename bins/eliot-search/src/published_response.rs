//! Closed bounded publication-state response validation.

use search_source_registry::MAX_PUBLICATION_RESPONSE_BYTES;
use serde_json::Value;

fn keys(value: &Value, expected: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k)))
}
fn hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn number(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64()
}
pub fn validate(bytes: &[u8]) -> Option<&str> {
    if bytes.len() > MAX_PUBLICATION_RESPONSE_BYTES {
        return None;
    }
    let line = std::str::from_utf8(bytes).ok()?.strip_suffix('\n')?;
    if line.contains(['\r', '\n']) {
        return None;
    }
    let value: Value = serde_json::from_str(line).ok()?;
    if !keys(
        &value,
        &[
            "operation",
            "state",
            "collection",
            "generation",
            "source_count",
            "gap_count",
            "point_count",
            "page_count",
        ],
    ) || value.get("operation")?.as_str()? != "publish-prepared"
        || value.get("state")?.as_str()? != "published"
    {
        return None;
    }
    let generation = number(&value, "generation")?;
    let collection = value.get("collection")?.as_str()?.strip_prefix("eliot_")?;
    let (namespace, sequence) = collection.split_once("_g")?;
    if generation == 0 || !hex(namespace, 32) || sequence != generation.to_string() {
        return None;
    }
    let sources = number(&value, "source_count")?;
    let gaps = number(&value, "gap_count")?;
    let points = number(&value, "point_count")?;
    let pages = number(&value, "page_count")?;
    if sources > 4096
        || gaps > sources
        || points > 1_000_000
        || pages > points
        || (points == 0) != (pages == 0)
    {
        return None;
    }
    Some(line)
}
