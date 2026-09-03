//! Finite parser, registry, snapshot, and diagnostic limits.

use crate::ConfigError;

/// Finite limits applied before allocation or collection growth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigLimits {
    /// Maximum input document bytes.
    pub max_document_bytes: usize,
    /// Maximum logical input lines.
    pub max_lines: usize,
    /// Maximum registered sections.
    pub max_sections: usize,
    /// Maximum fields per section.
    pub max_fields_per_section: usize,
    /// Maximum entries in one document/layer.
    pub max_entries_per_layer: usize,
    /// Maximum canonical identifier bytes.
    pub max_identifier_bytes: usize,
    /// Maximum text or opaque reference bytes.
    pub max_text_bytes: usize,
    /// Maximum string-list entries.
    pub max_list_items: usize,
    /// Maximum canonical fingerprint preimage bytes.
    pub max_canonical_bytes: usize,
    /// Maximum redacted diagnostic entries.
    pub max_diagnostic_entries: usize,
}

impl ConfigLimits {
    /// Conservative W1 defaults.
    pub const W1: Self = Self {
        max_document_bytes: 1_048_576,
        max_lines: 16_384,
        max_sections: 256,
        max_fields_per_section: 512,
        max_entries_per_layer: 8_192,
        max_identifier_bytes: 128,
        max_text_bytes: 65_536,
        max_list_items: 4_096,
        max_canonical_bytes: 8_388_608,
        max_diagnostic_entries: 4_096,
    };

    /// Validates that every ceiling is non-zero and internally usable.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::InvalidLimits`] when any ceiling is zero.
    pub const fn validate(self) -> Result<Self, ConfigError> {
        if self.max_document_bytes == 0
            || self.max_lines == 0
            || self.max_sections == 0
            || self.max_fields_per_section == 0
            || self.max_entries_per_layer == 0
            || self.max_identifier_bytes == 0
            || self.max_text_bytes == 0
            || self.max_list_items == 0
            || self.max_canonical_bytes == 0
            || self.max_diagnostic_entries == 0
        {
            return Err(ConfigError::InvalidLimits);
        }
        Ok(self)
    }
}

impl Default for ConfigLimits {
    fn default() -> Self {
        Self::W1
    }
}
