//! Typed values, bounds, redaction policy, and security floors.

use std::fmt;

use crate::ConfigError;

/// Closed configuration value kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConfigValueKind {
    /// Boolean value.
    Boolean,
    /// Signed 64-bit integer value.
    Integer,
    /// Finite UTF-8 text value.
    Text,
    /// Opaque secret reference; never secret plaintext.
    SecretReference,
    /// Finite ordered UTF-8 string list.
    StringList,
}

/// Opaque secret locator with an explicit non-plaintext grammar.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct SecretReference(String);

impl SecretReference {
    /// Creates an opaque secret reference.
    ///
    /// Accepted references start with `secret://`, contain no whitespace or
    /// control characters, and remain within the supplied byte ceiling.
    ///
    /// # Errors
    ///
    /// Any ordinary string is rejected as potential plaintext.
    pub fn new(value: impl Into<String>, max_bytes: usize) -> Result<Self, ConfigError> {
        let value = value.into();
        if !value.starts_with("secret://")
            || value.len() <= "secret://".len()
            || value.len() > max_bytes
            || value
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(ConfigError::SecretPlaintextForbidden);
        }
        Ok(Self(value))
    }

    /// Opaque reference text for the secret adapter only.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SecretReference")
            .field(&"<redacted>")
            .finish()
    }
}

/// Validated typed configuration value.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConfigValue {
    /// Field is intentionally absent under its capability-owned default.
    Absent,
    /// Boolean value.
    Boolean(bool),
    /// Signed integer value.
    Integer(i64),
    /// Public or metadata text value.
    Text(String),
    /// Opaque secret reference.
    SecretReference(SecretReference),
    /// Ordered finite string list.
    StringList(Vec<String>),
}

impl fmt::Debug for ConfigValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => formatter.write_str("Absent"),
            Self::Boolean(value) => formatter.debug_tuple("Boolean").field(value).finish(),
            Self::Integer(value) => formatter.debug_tuple("Integer").field(value).finish(),
            Self::Text(value) => formatter
                .debug_tuple("Text")
                .field(&format_args!("<{} bytes>", value.len()))
                .finish(),
            Self::SecretReference(_) => formatter
                .debug_tuple("SecretReference")
                .field(&"<redacted>")
                .finish(),
            Self::StringList(values) => formatter
                .debug_tuple("StringList")
                .field(&format_args!("<{} items>", values.len()))
                .finish(),
        }
    }
}

impl ConfigValue {
    /// Closed value kind.
    #[must_use]
    pub const fn kind(&self) -> Option<ConfigValueKind> {
        match self {
            Self::Absent => None,
            Self::Boolean(_) => Some(ConfigValueKind::Boolean),
            Self::Integer(_) => Some(ConfigValueKind::Integer),
            Self::Text(_) => Some(ConfigValueKind::Text),
            Self::SecretReference(_) => Some(ConfigValueKind::SecretReference),
            Self::StringList(_) => Some(ConfigValueKind::StringList),
        }
    }

    /// Returns whether the field is intentionally absent.
    #[must_use]
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
}

/// Parser-level value before a field descriptor assigns secret semantics.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum DocumentValue {
    /// Boolean value.
    Boolean(bool),
    /// Signed integer value.
    Integer(i64),
    /// TOML string value.
    Text(String),
    /// Ordered TOML string array.
    StringList(Vec<String>),
}

impl fmt::Debug for DocumentValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boolean(value) => formatter.debug_tuple("Boolean").field(value).finish(),
            Self::Integer(value) => formatter.debug_tuple("Integer").field(value).finish(),
            Self::Text(value) => formatter
                .debug_tuple("Text")
                .field(&format_args!("<{} bytes>", value.len()))
                .finish(),
            Self::StringList(values) => formatter
                .debug_tuple("StringList")
                .field(&format_args!("<{} items>", values.len()))
                .finish(),
        }
    }
}

/// Explicit set or reset operation in a layer.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LayerOperation {
    /// Set a parsed value.
    Set(DocumentValue),
    /// Restore the capability-owned compiled default.
    Reset,
}

/// Finite field value constraints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueBounds {
    /// Maximum UTF-8 bytes for text or secret references.
    pub max_text_bytes: usize,
    /// Maximum string-list entries.
    pub max_list_items: usize,
    /// Maximum UTF-8 bytes for one list entry.
    pub max_list_item_bytes: usize,
    /// Inclusive integer minimum.
    pub integer_min: i64,
    /// Inclusive integer maximum.
    pub integer_max: i64,
}

impl ValueBounds {
    /// Validates internal consistency.
    ///
    /// # Errors
    ///
    /// Rejects zero text/list ceilings or an inverted integer range.
    pub const fn validate(self) -> Result<Self, ConfigError> {
        if self.max_text_bytes == 0
            || self.max_list_items == 0
            || self.max_list_item_bytes == 0
            || self.integer_min > self.integer_max
        {
            return Err(ConfigError::InvalidDescriptor);
        }
        Ok(self)
    }
}

/// Field disclosure behavior for ordinary diagnostics.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RedactionPolicy {
    /// Value may be shown after finite truncation rules.
    Public,
    /// Text is represented only by location class and digest.
    PathDigest,
    /// Opaque secret reference is hidden completely.
    Secret,
    /// Only the value kind and size/count may be shown.
    MetadataOnly,
}

/// Directional security constraint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecurityFloor {
    /// No directional security floor.
    None,
    /// The field is fixed to its compiled default.
    Fixed,
    /// A boolean may move from `true` to `false`, but not from `false` to `true`.
    BooleanMayOnlyRestrict,
    /// Integers may not move below this minimum.
    IntegerMinimum(i64),
    /// Integers may not move above this maximum.
    IntegerMaximum(i64),
}
