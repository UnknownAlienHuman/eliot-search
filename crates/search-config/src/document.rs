//! Bounded parsing and construction of captured configuration documents.

use std::collections::BTreeMap;
use std::str;

use search_contracts::ProfileId;

use crate::{
    ConfigError, ConfigKeyPath, ConfigLimits, ConfigSectionName, ConfigSource, ConfigSourceKind,
    DocumentValue, LayerOperation,
};

#[path = "builder.rs"]
mod builder;

use builder::{DocumentBuilder, TextPopulation};

/// One finite captured configuration layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigDocument {
    schema_version: u32,
    requested_profile: Option<ProfileId>,
    source: ConfigSource,
    entries: BTreeMap<ConfigKeyPath, LayerOperation>,
    declared_sections: BTreeMap<ConfigSectionName, usize>,
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
    /// excessive entries/sections/fields/decoded data, and a requested profile
    /// outside the file layer. Caller-owned inputs are admitted before this
    /// owner clones names or grows its collections.
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

        let mut builder = DocumentBuilder::new(limits);
        if let Some(profile) = &requested_profile {
            builder.charge_text(profile.as_str().len(), TextPopulation::Profile)?;
        }
        for (path, operation) in entries {
            builder.push_captured(path, operation)?;
        }
        Ok(builder.finish(schema_version, requested_profile, source))
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
        self.declared_sections.keys()
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
    let mut builder = DocumentBuilder::new(limits);

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
            builder.declare_table(name)?;
            // The input owns this locator; no current-table name is cloned.
            current_section = Some(name);
            continue;
        }

        let (raw_key, raw_value) = split_assignment(line)?;
        if raw_key.contains('.') || raw_key.contains(' ') || raw_key.contains('\t') {
            return Err(ConfigError::UnsupportedSyntax);
        }
        if let Some(section) = current_section {
            let path = builder.parsed_path(section, raw_key)?;
            let operation = LayerOperation::Set(parse_value(raw_value, &mut builder)?);
            builder.commit_entry(path, operation)?;
        } else {
            match raw_key {
                "schema_version" => {
                    if schema_version.is_some() {
                        return Err(ConfigError::DuplicateKey);
                    }
                    let value = parse_integer(raw_value)?;
                    let value =
                        u32::try_from(value).map_err(|_| ConfigError::UnsupportedSchemaVersion)?;
                    if value == 0 {
                        return Err(ConfigError::UnsupportedSchemaVersion);
                    }
                    schema_version = Some(value);
                }
                "profile" => {
                    if requested_profile.is_some() {
                        return Err(ConfigError::DuplicateKey);
                    }
                    let value = parse_string(raw_value, &mut builder, TextPopulation::Profile)?;
                    requested_profile =
                        Some(ProfileId::new(value).map_err(|_| ConfigError::InvalidIdentifier)?);
                }
                _ => return Err(ConfigError::UnknownKey),
            }
        }
    }

    let schema_version = schema_version.ok_or(ConfigError::UnsupportedSchemaVersion)?;
    let requested_profile = requested_profile.ok_or(ConfigError::ProfileNotAuthorized)?;
    Ok(builder.finish(schema_version, Some(requested_profile), source))
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

fn parse_value(value: &str, builder: &mut DocumentBuilder) -> Result<DocumentValue, ConfigError> {
    match value {
        "true" | "false" => {
            builder.charge_scalar(1)?;
            Ok(DocumentValue::Boolean(value == "true"))
        }
        _ if value.starts_with('"') || value.starts_with('\'') => {
            parse_string(value, builder, TextPopulation::Value).map(DocumentValue::Text)
        }
        _ if value.starts_with('[') => {
            parse_string_array(value, builder).map(DocumentValue::StringList)
        }
        _ => {
            let value = parse_integer(value)?;
            builder.charge_scalar(8)?;
            Ok(DocumentValue::Integer(value))
        }
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
    // Preserve the legacy underscore grammar without allocating a normalized copy.
    // Accumulating negatively admits i64::MIN without an intermediate overflow.
    let negative = value.starts_with('-');
    digits
        .bytes()
        .filter(|byte| *byte != b'_')
        .try_fold(0_i64, |total, byte| {
            let digit = i64::from(byte - b'0');
            total
                .checked_mul(10)
                .and_then(|total| {
                    if negative {
                        total.checked_sub(digit)
                    } else {
                        total.checked_add(digit)
                    }
                })
                .ok_or(ConfigError::ValueOutOfBounds)
        })
}

fn parse_string(
    value: &str,
    builder: &mut DocumentBuilder,
    population: TextPopulation,
) -> Result<String, ConfigError> {
    if value.len() < 2 {
        return Err(ConfigError::UnsupportedSyntax);
    }
    let quote = value.as_bytes()[0];
    if !matches!(quote, b'\'' | b'"') || value.as_bytes().last() != Some(&quote) {
        return Err(ConfigError::UnsupportedSyntax);
    }
    let body = &value[1..value.len() - 1];
    let mut decoded_bytes = 0_usize;
    visit_string(body, quote, |character| {
        decoded_bytes = decoded_bytes
            .checked_add(character.len_utf8())
            .ok_or(ConfigError::LengthOverflow)?;
        Ok(())
    })?;
    builder.charge_text(decoded_bytes, population)?;
    let mut output = String::with_capacity(decoded_bytes);
    visit_string(body, quote, |character| {
        output.push(character);
        Ok(())
    })?;
    Ok(output)
}

// The same legacy decoding rules drive counting and writing. Counting allocates
// nothing, and the exact decoded byte charge precedes all String growth.
fn visit_string(
    body: &str,
    quote: u8,
    mut emit: impl FnMut(char) -> Result<(), ConfigError>,
) -> Result<(), ConfigError> {
    if quote == b'\'' {
        if body.contains('\'') || body.contains('\n') || body.contains('\r') {
            return Err(ConfigError::UnsupportedSyntax);
        }
        for character in body.chars() {
            emit(character)?;
        }
        return Ok(());
    }
    let mut characters = body.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            if character.is_control() {
                return Err(ConfigError::UnsupportedSyntax);
            }
            emit(character)?;
            continue;
        }
        let escaped = characters.next().ok_or(ConfigError::UnsupportedSyntax)?;
        let decoded = match escaped {
            'b' => '\u{0008}',
            't' => '\t',
            'n' => '\n',
            'f' => '\u{000c}',
            'r' => '\r',
            '"' => '"',
            '\\' => '\\',
            _ => return Err(ConfigError::UnsupportedSyntax),
        };
        emit(decoded)?;
    }
    Ok(())
}

fn parse_string_array(
    value: &str,
    builder: &mut DocumentBuilder,
) -> Result<Vec<String>, ConfigError> {
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
                    push_array_value(&body[start..index], &mut values, builder)?;
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
        push_array_value(tail, &mut values, builder)?;
    }
    Ok(values)
}

fn push_array_value(
    value: &str,
    values: &mut Vec<String>,
    builder: &mut DocumentBuilder,
) -> Result<(), ConfigError> {
    builder.charge_list_item(values.len())?;
    let value = value.trim();
    if value.is_empty() {
        return Err(ConfigError::UnsupportedSyntax);
    }
    values.push(parse_string(value, builder, TextPopulation::Value)?);
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
