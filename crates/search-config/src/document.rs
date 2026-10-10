//! Bounded parsing and construction of captured configuration documents.

use std::collections::BTreeMap;
use std::fmt;
use std::str;

use search_contracts::ProfileId;

use crate::{
    ConfigError, ConfigKeyPath, ConfigLimits, ConfigSectionName, ConfigSource, ConfigSourceKind,
    LayerOperation,
};

#[path = "builder.rs"]
mod builder;
#[path = "decode.rs"]
mod decode;
#[path = "syntax.rs"]
mod syntax;

use builder::{DocumentBuilder, TextPopulation};

/// One finite captured configuration layer.
///
/// Debug exposes fixed metadata only; captured text is not yet authorized.
#[derive(Clone, Eq, PartialEq)]
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

impl fmt::Debug for ConfigDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigDocument")
            .field("schema_version", &self.schema_version)
            .field(
                "requested_profile_present",
                &self.requested_profile.is_some(),
            )
            .field(
                "requested_profile_bytes",
                &self
                    .requested_profile
                    .as_ref()
                    .map(|profile| profile.as_str().len()),
            )
            .field("source_kind", &self.source.kind)
            .field("entry_count", &self.entries.len())
            .field("declared_section_count", &self.declared_sections.len())
            .finish_non_exhaustive()
    }
}

/// Parses the bounded maintained FILE profile eliot-config-toml@2.
///
/// Schema2 and a composition profile are required before flat bare-name tables.
/// Boolean, decimal i64, single-line strings and flat string lists are supported.
/// The maintained donor owns all TOML grammar and decoding.
///
/// # Errors
///
/// Returns only closed content-free errors. Schema1 files require migration.
/// Byte/line/token and decoded collection bounds apply before owned growth;
/// no partial document, source contents or donor diagnostic escapes.
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
    framing(bytes, limits.max_lines)?;
    syntax::parse(text, source, limits)
}

// Framing only: LF and CRLF terminate one line; lone CR is unsupported.
// Final unterminated segment counts once; a trailing terminator adds no line.
fn framing(bytes: &[u8], max_lines: usize) -> Result<(), ConfigError> {
    let mut lines = 0_usize;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\r' && bytes.get(index + 1) != Some(&b'\n') {
            return Err(ConfigError::UnsupportedSyntax);
        }
        if *byte == b'\n' {
            lines = lines.checked_add(1).ok_or(ConfigError::LengthOverflow)?;
            if lines > max_lines {
                return Err(ConfigError::CapacityExceeded);
            }
        }
    }
    if !bytes.ends_with(b"\n") {
        lines = lines.checked_add(1).ok_or(ConfigError::LengthOverflow)?;
    }
    if lines > max_lines {
        return Err(ConfigError::CapacityExceeded);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // AUTHORED_NOT_RUN; integrated #238/#215 qualification under #352.
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
            br#"schema_version = 2
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
        assert_eq!(document.schema_version(), 2);
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
                b"schema_version=2\nprofile='direct'\n[a]\nx=1\nx=2\n",
                source(),
                ConfigLimits::W1,
            ),
            Err(ConfigError::DuplicateKey)
        );
        assert_eq!(
            parse_document(
                b"schema_version=2\nprofile='direct'\n[a]\nx=1\n[a]\ny=2\n",
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
                b"schema_version=2\nprofile='direct'\n[a.b]\nx=1\n",
                source(),
                ConfigLimits::W1,
            ),
            Err(ConfigError::UnsupportedSyntax)
        );
    }
}
