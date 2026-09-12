//! Coverage graph text, identifier and deterministic JSON helpers.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// Typed failure for exact single-replacement operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageGraphError(String);

impl CoverageGraphError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for CoverageGraphError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.0)
    }
}

impl Error for CoverageGraphError {}

/// Lowercase slug with non-ASCII-alphanumeric runs collapsed to `-`.
#[must_use]
pub fn slug(value: &str) -> String {
    let lower = value.to_lowercase();
    let mut output = String::with_capacity(lower.len());
    let mut pending_dash = false;
    let mut has_output = false;
    for character in lower.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            if pending_dash && has_output {
                output.push('-');
            }
            pending_dash = false;
            has_output = true;
            output.push(character);
        } else {
            pending_dash = true;
        }
    }
    if output.len() > 96 {
        output.truncate(96);
    }
    if output.is_empty() {
        "node".to_owned()
    } else {
        output
    }
}

fn append_json_string(output: &mut Vec<u8>, value: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push(b'"');
    for character in value.chars() {
        match character {
            '"' => output.extend_from_slice(b"\\\""),
            '\\' => output.extend_from_slice(b"\\\\"),
            '\u{08}' => output.extend_from_slice(b"\\b"),
            '\u{09}' => output.extend_from_slice(b"\\t"),
            '\n' => output.extend_from_slice(b"\\n"),
            '\u{0c}' => output.extend_from_slice(b"\\f"),
            '\r' => output.extend_from_slice(b"\\r"),
            character if (character as u32) < 0x20 => {
                output.extend_from_slice(b"\\u00");
                let byte = character as u8;
                output.push(HEX[usize::from(byte >> 4)]);
                output.push(HEX[usize::from(byte & 0x0f)]);
            }
            character => {
                let mut buffer = [0_u8; 4];
                output.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    output.push(b'"');
}

/// Deterministic JSON string quoting with raw non-ASCII UTF-8.
#[must_use]
pub fn quote_json(value: &str) -> String {
    let mut output = Vec::with_capacity(value.len() + 2);
    append_json_string(&mut output, value);
    String::from_utf8(output).expect("JSON quoting emits valid UTF-8")
}

/// Deterministic JSON-style string array rendering.
#[must_use]
pub fn arr(values: &[&str]) -> String {
    let mut output = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        output.push_str(&quote_json(value));
    }
    output.push(']');
    output
}

/// Camel/snake/kebab tokenization plus the historical bounded stem rules.
#[must_use]
pub fn words(value: &str) -> BTreeSet<String> {
    let characters: Vec<char> = value.chars().collect();
    let mut spaced = String::with_capacity(value.len() + 4);
    for (index, character) in characters.iter().enumerate() {
        if index > 0
            && character.is_ascii_uppercase()
            && (characters[index - 1].is_ascii_lowercase()
                || characters[index - 1].is_ascii_digit())
        {
            spaced.push(' ');
        }
        spaced.push(*character);
    }
    let normalized: String = spaced
        .to_lowercase()
        .chars()
        .map(|character| {
            if character == '_' || character == '-' {
                ' '
            } else {
                character
            }
        })
        .collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in normalized.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            current.push(character);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    let mut output = BTreeSet::new();
    for token in tokens {
        if token.ends_with("ies") && token.len() > 4 {
            output.insert(format!("{}y", &token[..token.len() - 3]));
        } else if token.ends_with('s') && token.len() > 3 {
            output.insert(token[..token.len() - 1].to_owned());
        }
        if token.ends_with("ing") && token.len() > 5 {
            output.insert(token[..token.len() - 3].to_owned());
        }
        if token.ends_with("ed") && token.len() > 4 {
            output.insert(token[..token.len() - 2].to_owned());
        }
        output.insert(token);
    }
    output
}

/// Replaces exactly one occurrence.
///
/// # Errors
///
/// Returns [`CoverageGraphError`] when `old` occurs zero or multiple times.
pub fn replace_once(
    text: &str,
    old: &str,
    new: &str,
    label: &str,
) -> Result<String, CoverageGraphError> {
    let count = text.matches(old).count();
    if count != 1 {
        return Err(CoverageGraphError::new(format!(
            "{label}: expected one occurrence, found {count}"
        )));
    }
    Ok(text.replacen(old, new, 1))
}

/// Sorted unique package names from `package:module` references.
#[must_use]
pub fn module_refs_to_packages(refs: &[&str]) -> Vec<String> {
    refs.iter()
        .map(|reference| {
            reference
                .split_once(':')
                .map_or(*reference, |(package, _)| package)
        })
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
