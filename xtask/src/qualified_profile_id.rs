//! Syntax-only parsing for `namespace/local_opaque_id` profile references.
//!
//! This parser does not resolve a profile or establish namespace ownership,
//! qualification, trust, or authority. Its validation scope is
//! [`VALIDATION_SCOPE`].

use std::fmt;

/// Maximum UTF-8 byte length of a complete qualified opaque identifier.
pub const MAX_QUALIFIED_OPAQUE_ID_BYTES: usize = 225;
/// Maximum ASCII byte length of the namespace component.
pub const MAX_NAMESPACE_BYTES: usize = 96;
/// Maximum ASCII byte length of the local opaque identifier component.
pub const MAX_LOCAL_OPAQUE_ID_BYTES: usize = 128;

/// Authority scope of this syntax-only parser.
pub const VALIDATION_SCOPE: &str = "NON_AUTHORITATIVE";

/// A validated qualified identifier borrowing the exact input bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QualifiedOpaqueId<'a> {
    value: &'a str,
    namespace: &'a str,
    local: &'a str,
}

impl<'a> QualifiedOpaqueId<'a> {
    /// Parses `namespace/local_opaque_id` without normalizing or allocating.
    pub fn parse(value: &'a str) -> Result<Self, QualifiedOpaqueIdError> {
        let bytes = value.as_bytes();
        if bytes.len() > MAX_QUALIFIED_OPAQUE_ID_BYTES {
            return Err(QualifiedOpaqueIdError::ValueTooLong);
        }

        let mut separator = None;
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == b'/' {
                if separator.is_some() {
                    return Err(QualifiedOpaqueIdError::MultipleSeparators);
                }
                separator = Some(index);
            }
        }
        let separator = separator.ok_or(QualifiedOpaqueIdError::MissingSeparator)?;

        let namespace = &value[..separator];
        let local = &value[separator + 1..];
        if namespace.is_empty() {
            return Err(QualifiedOpaqueIdError::EmptyNamespace);
        }
        if local.is_empty() {
            return Err(QualifiedOpaqueIdError::EmptyLocalId);
        }
        if namespace.len() > MAX_NAMESPACE_BYTES {
            return Err(QualifiedOpaqueIdError::NamespaceTooLong);
        }
        if local.len() > MAX_LOCAL_OPAQUE_ID_BYTES {
            return Err(QualifiedOpaqueIdError::LocalIdTooLong);
        }

        if !valid_namespace(namespace.as_bytes()) {
            return Err(QualifiedOpaqueIdError::InvalidNamespace);
        }
        if !valid_local(local.as_bytes()) {
            return Err(QualifiedOpaqueIdError::InvalidLocalId);
        }

        Ok(Self {
            value,
            namespace,
            local,
        })
    }

    /// Returns the exact complete value supplied to [`parse`](Self::parse).
    pub const fn value(&self) -> &'a str {
        self.value
    }

    /// Returns the exact namespace component supplied to [`parse`](Self::parse).
    pub const fn namespace(&self) -> &'a str {
        self.namespace
    }

    /// Returns the exact local opaque identifier component supplied to [`parse`](Self::parse).
    pub const fn local(&self) -> &'a str {
        self.local
    }
}

/// A bounded, input-independent syntax error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualifiedOpaqueIdError {
    MissingSeparator,
    MultipleSeparators,
    EmptyNamespace,
    EmptyLocalId,
    ValueTooLong,
    NamespaceTooLong,
    LocalIdTooLong,
    InvalidNamespace,
    InvalidLocalId,
}

impl QualifiedOpaqueIdError {
    /// Returns a stable, bounded code without echoing any part of the input.
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingSeparator => "missing_separator",
            Self::MultipleSeparators => "multiple_separators",
            Self::EmptyNamespace => "empty_namespace",
            Self::EmptyLocalId => "empty_local_id",
            Self::ValueTooLong => "value_too_long",
            Self::NamespaceTooLong => "namespace_too_long",
            Self::LocalIdTooLong => "local_id_too_long",
            Self::InvalidNamespace => "invalid_namespace",
            Self::InvalidLocalId => "invalid_local_id",
        }
    }
}

impl fmt::Display for QualifiedOpaqueIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for QualifiedOpaqueIdError {}

fn valid_namespace(bytes: &[u8]) -> bool {
    let Some((&first, rest)) = bytes.split_first() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }

    let mut previous_was_hyphen = false;
    for byte in rest {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_was_hyphen = false;
        } else if *byte == b'-' && !previous_was_hyphen {
            previous_was_hyphen = true;
        } else {
            return false;
        }
    }
    !previous_was_hyphen
}

fn valid_local(bytes: &[u8]) -> bool {
    let Some((&first, rest)) = bytes.split_first() else {
        return false;
    };
    first.is_ascii_alphanumeric()
        && rest
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'.' | b'_' | b'-'))
}
