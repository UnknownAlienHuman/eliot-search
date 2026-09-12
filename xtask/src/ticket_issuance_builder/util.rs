//! Small TOML/JSON helpers shared by ticket-issuance builder modules.

use std::collections::BTreeSet;

use serde_json::Value as JsonValue;
use toml::Value;

pub(super) fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(super) fn integer(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_integer)
}

pub(super) fn boolean(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

pub(super) fn unique_row(
    document: &Value,
    array_key: &str,
    identity_key: &str,
    expected: &str,
) -> Option<Value> {
    let rows = document.get(array_key)?.as_array()?;
    let mut matches = rows.iter().filter(|row| {
        row.get(identity_key).and_then(Value::as_str) == Some(expected)
    });
    let row = matches.next()?.clone();
    matches.next().is_none().then_some(row)
}

pub(super) fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    value?
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

pub(super) fn strings_unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

pub(super) fn count_string(value: Option<&Value>, target: &str) -> usize {
    value.and_then(Value::as_array).map_or(0, |items| {
        items
            .iter()
            .filter(|item| item.as_str() == Some(target))
            .count()
    })
}

pub(super) fn toml_to_json(value: &Value) -> JsonValue {
    serde_json::to_value(value).unwrap_or(JsonValue::Null)
}

pub(super) fn table_keys(value: &Value) -> Vec<&str> {
    value
        .as_table()
        .map_or_else(Vec::new, |table| table.keys().map(String::as_str).collect())
}

pub(super) fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
