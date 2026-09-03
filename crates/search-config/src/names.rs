//! Canonical configuration names and paths.

use std::fmt;

use crate::ConfigError;

fn validate_token(value: &str, max_bytes: usize, allow_hyphen: bool) -> Result<(), ConfigError> {
    if value.is_empty() || value.len() > max_bytes {
        return Err(ConfigError::InvalidIdentifier);
    }
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return Err(ConfigError::InvalidIdentifier);
    };
    if !first.is_ascii_lowercase() {
        return Err(ConfigError::InvalidIdentifier);
    }
    if characters.all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '_'
            || (allow_hyphen && character == '-')
    }) {
        Ok(())
    } else {
        Err(ConfigError::InvalidIdentifier)
    }
}

macro_rules! token_type {
    ($name:ident, $allow_hyphen:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Creates a canonical finite token.
            ///
            /// # Errors
            ///
            /// Rejects empty, oversize, uppercase, Unicode, whitespace, or punctuation
            /// outside the token's closed grammar.
            pub fn new(value: impl Into<String>, max_bytes: usize) -> Result<Self, ConfigError> {
                let value = value.into();
                validate_token(&value, max_bytes, $allow_hyphen)?;
                Ok(Self(value))
            }

            /// Canonical token text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

token_type!(
    ConfigSectionName,
    false,
    "Canonical lower-case configuration section name."
);
token_type!(
    ConfigKeyName,
    true,
    "Canonical lower-case configuration key name."
);
token_type!(
    ConfigOwner,
    true,
    "Canonical package or capability owner name."
);

/// Canonical `section.key` field path.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConfigKeyPath {
    section: ConfigSectionName,
    key: ConfigKeyName,
}

impl ConfigKeyPath {
    /// Creates a canonical field path.
    #[must_use]
    pub const fn new(section: ConfigSectionName, key: ConfigKeyName) -> Self {
        Self { section, key }
    }

    /// Section component.
    #[must_use]
    pub const fn section(&self) -> &ConfigSectionName {
        &self.section
    }

    /// Key component.
    #[must_use]
    pub const fn key(&self) -> &ConfigKeyName {
        &self.key
    }
}

impl fmt::Debug for ConfigKeyPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.section, self.key)
    }
}

impl fmt::Display for ConfigKeyPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.section, self.key)
    }
}

/// Opaque, finite source locator safe for diagnostics.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConfigSourceRef(String);

impl ConfigSourceRef {
    /// Creates a finite opaque source reference.
    ///
    /// # Errors
    ///
    /// Rejects empty strings, control characters, NUL, and oversize values.
    pub fn new(value: impl Into<String>, max_bytes: usize) -> Result<Self, ConfigError> {
        let value = value.into();
        if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
            return Err(ConfigError::InvalidIdentifier);
        }
        Ok(Self(value))
    }

    /// Opaque source reference.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ConfigSourceRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ConfigSourceRef")
            .field(&"<opaque>")
            .finish()
    }
}
