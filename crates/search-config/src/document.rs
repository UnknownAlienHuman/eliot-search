//! Bounded parsing and construction of captured configuration documents.

use std::collections::{BTreeMap, BTreeSet};
use std::str;

use search_contracts::ProfileId;

use crate::{
    ConfigError, ConfigKeyName, ConfigKeyPath, ConfigLimits, ConfigSectionName, ConfigSource,
    ConfigSourceKind, DocumentValue, LayerOperation,
};

/// One finite captured configuration layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigDocument {
    schema_version: u32,
    requested_profile: Option<ProfileId>,
    source: ConfigSource,
    entries: BTreeMap<ConfigKeyPath, LayerOperation>,
    declared_sections: BTreeSet<ConfigSectionName>,
}

impl ConfigDocument {
    /// Constructs a finite already-captured layer.
    ///
    /// This constructor is intended for the environment and CLI acquisition
    /// owners. It performs no environment or command-line reads.
    ///
    /// # Errors
    ///
    /// Rejects zero schema versions, default-source documents, duplicate paths,
    /// excessive entries, and a requested profile outside the file layer.
    pub fn from_entries(
        schema_version: u32,
        requested_profile: Option<ProfileId>,
        source: ConfigSource,
        entries: impl IntoIterator<Item = (ConfigKeyPath, LayerOperation)>,
        limits: ConfigLimits,
    ) -> Result<Self, ConfigError> {
        let limits = limits.validate()?;
        if schema_version == 0 {
            return Err(ConfigError::UnsupportedSchemaVersion);
        }
        if source.kind == ConfigSourceKind::Defaults {
            return Err(ConfigError::SourceKindMismatch);
        }
        if source.kind != ConfigSourceKind::File && requested_profile.is_some() {
            return Err(ConfigError::SourceKindMismatch);
        }

        let mut values = BTreeMap::new();
        let mut declared_sections = BTreeSet::new();
        for (path, operation) in entries {
            if values.len() >= limits.max_entries_per_layer {
                return Err(ConfigError::CapacityExceeded);
            }
            declared_sections.insert(path.section().clone());
            if values.insert(path, operation).is_some() {
                return Err(ConfigError::DuplicateKey);
            }
        }
        Ok(Self {
            schema_version,
            requested_profile,
            source,
            entries: values,
            declared_sections,
        })
    }

    /// Configuration schema version declared by the source.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Requested profile ceiling declared by the local file.
    #[must_use]
    pub const fn requested_profile(&self) -> Option<&ProfileId> {
        self.requested_profile.as_ref()
    }

    /// Exact captured source identity.
    #[must_use]
    pub const fn source(&self) -> &ConfigSource {
        &self.source
    }

    /// Canonical field operations.
    #[must_use]
    pub fn entries(&self) -> impl ExactSizeIterator<Item = (&ConfigKeyPath, &LayerOperation)> {
        self.entries.iter()
    }

    /// Canonical declared section names, including empty tables.
    #[must_use]
    pub fn declared_sections(&self) -> impl ExactSizeIterator<Item = &ConfigSectionName> {
        self.declared_sections.iter()
    }

    /// Number of field operations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the layer contains no field operation.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Parses one bounded UTF-8 TOML 1.0 configuration file.
///
/// The accepted subset intentionally excludes dotted/nested tables, inline
/// tables, floats, date/time values and multiline values. Capability fields are
/// scalar booleans, signed integers, strings or finite arrays of strings.
///
/// # Errors
///
/// Returns a typed deterministic error for framing, encoding, duplicate,
/// unsupported syntax, invalid profile, or finite-limit failures.
pub fn parse_document(
    bytes: &[u8],
    source: ConfigSource,
    limits: ConfigLimits,
) -> Result<ConfigDocument, ConfigError> {
    let limits = limits.validate()?;
    if source.kind != ConfigSourceKind::File {
        return Err(ConfigError::SourceKindMismatch);
    }
    if bytes.is_empty() {
        return Err(ConfigError::EmptyInput);
    }
    if bytes.len() > limits.max_document_bytes {
        return Err(ConfigError::CapacityExceeded);
    }
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(ConfigError::BomForbidden);
    }
    let text = str::from_utf8(bytes).map_err(|_| ConfigError::InvalidUtf8)?;
    let line_count = text.lines().count();
    if line_count > limits.max_lines {
        return Err(ConfigError::CapacityExceeded);
    }

    let mut schema_version = None;
    let mut requested_profile = None;
    let mut current_section = None;
    let mut declared_sections = BTreeSet::new();
    let mut entries = BTreeMap::new();
    let mut top_level_keys = BTreeSet::new();

    for raw_line in text.lines() {
        let line = strip_comment(raw_line)?.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            if !line.ends_with(']')
                || line.starts_with("[[")
                || line.ends_with("]]")
                || line.matches('[').count() != 1
                || line.matches(']').count() != 1
            {
                return Err(ConfigError::UnsupportedSyntax);
            }
            let name = line[1..line.len() - 1].trim();
            if name.contains('.') || name.contains(' ') || name.contains('\t') {
                return Err(ConfigError::UnsupportedSyntax);
            }
            let section = ConfigSectionName::new(name, limits.max_identifier_bytes)?;
            if !declared_sections.insert(section.clone()) {
                return Err(ConfigError::DuplicateTable);
            }
            current_section = Some(section);
            if declared_sections.len() > limits.max_sections {
                return Err(ConfigError::CapacityExceeded);
            }
            continue;
        }

        let (raw_key, raw_value) = split_assignment(line)?;
        if raw_key.contains('.') || raw_key.contains(' ') || raw_key.contains('\t') {
            return Err(ConfigError::UnsupportedSyntax);
        }
        if let Some(section) = &current_section {
            let key = ConfigKeyName::new(raw_key, limits.max_identifier_bytes)?;
            let path = ConfigKeyPath::new(section.clone(), key);
            let operation = LayerOperation::Set(parse_value(raw_value, limits)?);
            if entries.len() >= limits.max_entries_per_layer {
                return Err(ConfigError::CapacityExceeded);
            }
            if entries.insert(path, operation).is_some() {
                return Err(ConfigError::DuplicateKey);
            }
        } else {
            if !top_level_keys.insert(raw_key.to_owned()) {
                return Err(ConfigError::DuplicateKey);
            }
            match raw_key {
                "schema_version" => {
                    let value = parse_integer(raw_value)?;
                    let value =
                        u32::try_from(value).map_err(|_| ConfigError::UnsupportedSchemaVersion)?;
                    if value == 0 {
                        return Err(ConfigError::UnsupportedSchemaVersion);
                    }
                    schema_version = Some(value);
                }
                "profile" => {
                    let value = parse_string(raw_value, limits.max_text_bytes)?;
                    requested_profile =
                        Some(ProfileId::new(value).map_err(|_| ConfigError::InvalidIdentifier)?);
                }
                _ => return Err(ConfigError::UnknownKey),
            }
        }
    }

    let schema_version = schema_version.ok_or(ConfigError::UnsupportedSchemaVersion)?;
    let requested_profile = requested_profile.ok_or(ConfigError::ProfileNotAuthorized)?;
    Ok(ConfigDocument {
        schema_version,
        requested_profile: Some(requested_profile),
        source,
        entries,
        declared_sections,
    })
}

fn split_assignment(line: &str) -> Result<(&str, &str), ConfigError> {
    let mut quote = None;
    let mut escaped = false;
    let mut bracket_depth = 0_u8;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some('"') => match character {
                '\\' => escaped = true,
                '"' => quote = None,
                _ => {}
            },
            Some('\'') => {
                if character == '\'' {
                    quote = None;
                }
            }
            Some(_) => unreachable!("quote state is closed"),
            None => match character {
                '"' | '\'' => quote = Some(character),
                '[' => bracket_depth = bracket_depth.saturating_add(1),
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                '=' if bracket_depth == 0 => {
                    let key = line[..index].trim();
                    let value = line[index + 1..].trim();
                    if key.is_empty() || value.is_empty() {
                        return Err(ConfigError::UnsupportedSyntax);
                    }
                    return Ok((key, value));
                }
                _ => {}
            },
        }
    }
    Err(ConfigError::UnsupportedSyntax)
}

fn strip_comment(line: &str) -> Result<&str, ConfigError> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some('"') => match character {
                '\\' => escaped = true,
                '"' => quote = None,
                '\n' | '\r' => return Err(ConfigError::UnsupportedSyntax),
                _ => {}
            },
            Some('\'') => {
                if character == '\'' {
                    quote = None;
                }
            }
            Some(_) => unreachable!("quote state is closed"),
            None => match character {
                '"' | '\'' => quote = Some(character),
                '#' => return Ok(&line[..index]),
                _ => {}
            },
        }
    }
    if quote.is_some() || escaped {
        Err(ConfigError::UnsupportedSyntax)
    } else {
        Ok(line)
    }
}

fn parse_value(value: &str, limits: ConfigLimits) -> Result<DocumentValue, ConfigError> {
    match value {
        "true" => Ok(DocumentValue::Boolean(true)),
        "false" => Ok(DocumentValue::Boolean(false)),
        _ if value.starts_with('"') || value.starts_with('\'') => {
            parse_string(value, limits.max_text_bytes).map(DocumentValue::Text)
        }
        _ if value.starts_with('[') => {
            parse_string_array(value, limits).map(DocumentValue::StringList)
        }
        _ => parse_integer(value).map(DocumentValue::Integer),
    }
}

fn parse_integer(value: &str) -> Result<i64, ConfigError> {
    if value.is_empty()
        || value.starts_with('+')
        || value.contains("__")
        || value.starts_with('_')
        || value.ends_with('_')
    {
        return Err(ConfigError::UnsupportedSyntax);
    }
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || digits
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && byte != b'_')
    {
        return Err(ConfigError::UnsupportedSyntax);
    }
    let canonical = value.replace('_', "");
    canonical
        .parse::<i64>()
        .map_err(|_| ConfigError::ValueOutOfBounds)
}

fn parse_string(value: &str, max_bytes: usize) -> Result<String, ConfigError> {
    if value.len() < 2 {
        return Err(ConfigError::UnsupportedSyntax);
    }
    let quote = value.as_bytes()[0];
    if !matches!(quote, b'\'' | b'"') || value.as_bytes().last() != Some(&quote) {
        return Err(ConfigError::UnsupportedSyntax);
    }
    let body = &value[1..value.len() - 1];
    let parsed = if quote == b'\'' {
        if body.contains('\'') || body.contains('\n') || body.contains('\r') {
            return Err(ConfigError::UnsupportedSyntax);
        }
        body.to_owned()
    } else {
        parse_basic_string(body)?
    };
    if parsed.len() > max_bytes {
        Err(ConfigError::CapacityExceeded)
    } else {
        Ok(parsed)
    }
}

fn parse_basic_string(body: &str) -> Result<String, ConfigError> {
    let mut output = String::with_capacity(body.len());
    let mut characters = body.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            if character.is_control() {
                return Err(ConfigError::UnsupportedSyntax);
            }
            output.push(character);
            continue;
        }
        let escaped = characters.next().ok_or(ConfigError::UnsupportedSyntax)?;
        match escaped {
            'b' => output.push('\u{0008}'),
            't' => output.push('\t'),
            'n' => output.push('\n'),
            'f' => output.push('\u{000c}'),
            'r' => output.push('\r'),
            '"' => output.push('"'),
            '\\' => output.push('\\'),
            _ => return Err(ConfigError::UnsupportedSyntax),
        }
    }
    Ok(output)
}

fn parse_string_array(value: &str, limits: ConfigLimits) -> Result<Vec<String>, ConfigError> {
    if !value.ends_with(']') {
        return Err(ConfigError::UnsupportedSyntax);
    }
    let body = value[1..value.len() - 1].trim();
    if body.is_empty() {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    let mut start = 0_usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in body.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some('"') => match character {
                '\\' => escaped = true,
                '"' => quote = None,
                _ => {}
            },
            Some('\'') => {
                if character == '\'' {
                    quote = None;
                }
            }
            Some(_) => unreachable!("quote state is closed"),
            None => match character {
                '"' | '\'' => quote = Some(character),
                ',' => {
                    push_array_value(&body[start..index], &mut values, limits)?;
                    start = index + 1;
                }
                '[' | ']' | '{' | '}' => return Err(ConfigError::UnsupportedSyntax),
                _ => {}
            },
        }
    }
    if quote.is_some() || escaped {
        return Err(ConfigError::UnsupportedSyntax);
    }
    let tail = body[start..].trim();
    if !tail.is_empty() {
        push_array_value(tail, &mut values, limits)?;
    }
    if values.len() > limits.max_list_items {
        return Err(ConfigError::CapacityExceeded);
    }
    Ok(values)
}

fn push_array_value(
    value: &str,
    values: &mut Vec<String>,
    limits: ConfigLimits,
) -> Result<(), ConfigError> {
    if values.len() >= limits.max_list_items {
        return Err(ConfigError::CapacityExceeded);
    }
    let value = value.trim();
    if value.is_empty() {
        return Err(ConfigError::UnsupportedSyntax);
    }
    values.push(parse_string(value, limits.max_text_bytes)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use search_contracts::Blake3Digest32;

    use super::parse_document;
    use crate::{
        ConfigError, ConfigLimits, ConfigSource, ConfigSourceKind, ConfigSourceRef, DocumentValue,
        LayerOperation,
    };

    fn source() -> ConfigSource {
        ConfigSource {
            kind: ConfigSourceKind::File,
            source_ref: ConfigSourceRef::new("local-config", 64).expect("source"),
            source_digest: Blake3Digest32::from_bytes([7; 32]),
        }
    }

    #[test]
    fn parses_example_shaped_bounded_document() {
        let document = parse_document(
            br#"schema_version = 1
profile = "direct"
[instance]
mode = "standalone"
lock_timeout_ms = 1_000
[observability]
include_query_text = false
levels = ["warn", "error"]
"#,
            source(),
            ConfigLimits::W1,
        )
        .expect("document");
        assert_eq!(document.schema_version(), 1);
        assert_eq!(
            document.requested_profile().expect("profile").as_str(),
            "direct"
        );
        assert_eq!(document.len(), 4);
        assert!(document.entries().any(|(_, operation)| {
            operation == &LayerOperation::Set(DocumentValue::Integer(1_000))
        }));
    }

    #[test]
    fn duplicate_key_and_table_fail_closed() {
        assert_eq!(
            parse_document(
                b"schema_version=1\nprofile='direct'\n[a]\nx=1\nx=2\n",
                source(),
                ConfigLimits::W1,
            ),
            Err(ConfigError::DuplicateKey)
        );
        assert_eq!(
            parse_document(
                b"schema_version=1\nprofile='direct'\n[a]\nx=1\n[a]\ny=2\n",
                source(),
                ConfigLimits::W1,
            ),
            Err(ConfigError::DuplicateTable)
        );
    }

    #[test]
    fn bom_and_unsupported_nested_syntax_are_rejected() {
        assert_eq!(
            parse_document(&[0xef, 0xbb, 0xbf, b'x'], source(), ConfigLimits::W1),
            Err(ConfigError::BomForbidden)
        );
        assert_eq!(
            parse_document(
                b"schema_version=1\nprofile='direct'\n[a.b]\nx=1\n",
                source(),
                ConfigLimits::W1,
            ),
            Err(ConfigError::UnsupportedSyntax)
        );
    }
}
