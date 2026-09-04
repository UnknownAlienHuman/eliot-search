//! Registry-facing current source binding and observation view.
//!
//! Stable source identity, path-binding history, and content observations have
//! distinct authority. This module provides the finite aggregate consumed by
//! the registry and reconciler without making a path or content digest part of
//! [`SourceIdentity`] itself.

use search_contracts::{
    Blake3Digest32, NonZeroRevision, ReceiptRef, RootBindingId, SourceIdentity,
};

use crate::IdentityError;

/// Conservative limits for registry-facing identity values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityLimits {
    /// Maximum UTF-8 bytes in one canonical root-relative path.
    pub max_relative_path_bytes: usize,
}

impl IdentityLimits {
    /// Validates every finite dimension.
    pub const fn validate(self) -> Result<Self, IdentityError> {
        if self.max_relative_path_bytes == 0 {
            Err(IdentityError::IdentityCapacityExceeded)
        } else {
            Ok(self)
        }
    }
}

/// Default finite limits for registry-facing identity values.
pub const DEFAULT_IDENTITY_LIMITS: IdentityLimits = IdentityLimits {
    max_relative_path_bytes: 4_096,
};

/// Canonical path relative to one admitted logical root.
///
/// The value uses `/` separators, contains no empty, `.` or `..` component,
/// and is never interpreted as stable source identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalRelativePath(String);

impl CanonicalRelativePath {
    /// Validates and owns an already-canonical root-relative path.
    pub fn new(
        value: impl Into<String>,
        limits: IdentityLimits,
    ) -> Result<Self, IdentityError> {
        let limits = limits.validate()?;
        let value = value.into();
        if value.is_empty()
            || value.len() > limits.max_relative_path_bytes
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\\')
            || value.chars().any(char::is_control)
            || value
                .split('/')
                .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(IdentityError::PathEscapesAdmittedRoot);
        }
        Ok(Self(value))
    }

    /// Exact canonical relative path text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CanonicalRelativePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Exact current content/path observation retained outside stable identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceObservation {
    /// Admitted logical root containing the path.
    pub root_binding_id: RootBindingId,
    /// Canonical path relative to the admitted root.
    pub relative_path: CanonicalRelativePath,
    /// Reuse-resistant final-file identity when the platform supports it.
    pub stable_file_identity_digest: Option<Blake3Digest32>,
    /// Digest of exact observed source bytes.
    pub content_digest: Blake3Digest32,
    /// Exact observed source byte length.
    pub content_bytes: u64,
    /// Content-free authoritative observation receipt.
    pub observation_receipt: ReceiptRef,
}

/// Stable source identity plus independently revisioned current binding and
/// content observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBinding {
    identity: SourceIdentity,
    observation: SourceObservation,
    binding_revision: NonZeroRevision,
    observation_revision: NonZeroRevision,
}

impl SourceBinding {
    /// Creates a registry-facing current binding from already-validated values.
    #[must_use]
    pub const fn new(
        identity: SourceIdentity,
        observation: SourceObservation,
        binding_revision: NonZeroRevision,
        observation_revision: NonZeroRevision,
    ) -> Self {
        Self {
            identity,
            observation,
            binding_revision,
            observation_revision,
        }
    }

    /// Stable source identity, independent from current path and bytes.
    #[must_use]
    pub const fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    /// Complete current observation.
    #[must_use]
    pub const fn observation(&self) -> &SourceObservation {
        &self.observation
    }

    /// Admitted logical root binding.
    #[must_use]
    pub const fn root_binding_id(&self) -> RootBindingId {
        self.observation.root_binding_id
    }

    /// Current canonical root-relative path.
    #[must_use]
    pub const fn relative_path(&self) -> &CanonicalRelativePath {
        &self.observation.relative_path
    }

    /// Current stable final-file identity when supported.
    #[must_use]
    pub const fn stable_file_identity_digest(&self) -> Option<Blake3Digest32> {
        self.observation.stable_file_identity_digest
    }

    /// Digest of exact current source bytes.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 {
        self.observation.content_digest
    }

    /// Exact current source byte length.
    #[must_use]
    pub const fn content_bytes(&self) -> u64 {
        self.observation.content_bytes
    }

    /// Current path/root binding revision.
    #[must_use]
    pub const fn binding_revision(&self) -> NonZeroRevision {
        self.binding_revision
    }

    /// Current content-observation revision.
    #[must_use]
    pub const fn observation_revision(&self) -> NonZeroRevision {
        self.observation_revision
    }

    /// Content-free authoritative observation receipt.
    #[must_use]
    pub const fn observation_receipt(&self) -> &ReceiptRef {
        &self.observation.observation_receipt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_relative_path_rejects_escape_and_ambiguous_separators() {
        for value in ["", "/rooted", "trailing/", "a//b", "a/../b", "a\\b"] {
            assert!(CanonicalRelativePath::new(value, DEFAULT_IDENTITY_LIMITS).is_err());
        }
        assert_eq!(
            CanonicalRelativePath::new("src/lib.rs", DEFAULT_IDENTITY_LIMITS)
                .expect("canonical path")
                .as_str(),
            "src/lib.rs"
        );
    }
}
