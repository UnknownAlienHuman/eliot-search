//! Closed bounded generation/point-evidence response validation.

use search_source_registry::{MAX_PUBLICATION_RESPONSE_BYTES, MAX_SOURCE_OBJECT_BYTES};
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
fn uuid(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(i, b)| {
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
fn object(value: &Value, kind: &str, maximum: u64) -> Option<u64> {
    if !keys(value, &["version", "kind", "digest", "bytes"])
        || number(value, "version")? != 1
        || value.get("kind")?.as_str()? != kind
        || !hex(value.get("digest")?.as_str()?, 64)
    {
        return None;
    }
    let length = number(value, "bytes")?;
    (length <= maximum && (kind != "prepared" || length > 0)).then_some(length)
}
fn hit(value: &Value, generation: u64) -> Option<()> {
    if !keys(
        value,
        &[
            "point_id",
            "source_namespace_id",
            "source_id",
            "score",
            "input",
            "prepared",
            "unit",
            "start",
            "end",
            "line_start",
            "line_end",
            "starts_at_line_boundary",
            "ends_at_line_boundary",
        ],
    ) {
        return None;
    }
    let point = value.get("point_id")?.as_str()?;
    if !hex(point, 32)
        || !point.starts_with(&format!("{generation:016x}"))
        || !uuid(value.get("source_namespace_id")?.as_str()?)
        || !uuid(value.get("source_id")?.as_str()?)
    {
        return None;
    }
    let score = value.get("score")?.as_f64()?;
    if !score.is_finite() || score.abs() > f64::from(f32::MAX) {
        return None;
    }
    let input = object(value.get("input")?, "source", MAX_SOURCE_OBJECT_BYTES)?;
    object(value.get("prepared")?, "prepared", 64 * 1024 * 1024)?;
    number(value, "unit")?;
    let start = number(value, "start")?;
    let end = number(value, "end")?;
    if start >= end
        || end > input
        || end - start > 65_536
        || number(value, "line_start")? >= number(value, "line_end")?
    {
        return None;
    }
    value.get("starts_at_line_boundary")?.as_bool()?;
    value.get("ends_at_line_boundary")?.as_bool()?;
    Some(())
}
pub fn validate(bytes: &[u8], query_requested: bool, limit: usize) -> Option<&str> {
    if bytes.len() > MAX_PUBLICATION_RESPONSE_BYTES || limit == 0 || limit > 64 {
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
            "query_performed",
            "hits",
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
    let performed = value.get("query_performed")?.as_bool()?;
    if performed != (query_requested && points != 0) {
        return None;
    }
    let hits = value.get("hits")?.as_array()?;
    if hits.len() > limit || (!performed && !hits.is_empty()) {
        return None;
    }
    let mut ids = std::collections::BTreeSet::new();
    for value in hits {
        hit(value, generation)?;
        if !ids.insert(value.get("point_id")?.as_str()?) {
            return None;
        }
    }
    Some(line)
}
