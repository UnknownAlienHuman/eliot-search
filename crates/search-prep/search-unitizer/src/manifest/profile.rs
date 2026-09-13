//! Unitizer-profile model, validation, identity and transition classification.

use crate::{UnitizationError, UnitizationLimits};

use super::digest::digest32;
use super::spec::{MAX_UNITIZER_PROFILE_NAME_BYTES, PROFILE_DOMAIN};

/// Canonical unitizer-profile identity: the domain-separated digest over the
/// profile name, revision, finite limits and boundary-format identity.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnitizerProfileId([u8; 32]);

impl UnitizerProfileId {
    /// Rebuilds an identity from its 32 raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Raw identity bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl core::fmt::Debug for UnitizerProfileId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("UnitizerProfileId")
            .field(&self.to_string())
            .finish()
    }
}

impl core::fmt::Display for UnitizerProfileId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Unvalidated unitizer-profile descriptor: name, monotone revision and the
/// finite limits that own every boundary decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitizerProfileDescriptor {
    /// Human-readable profile name (ASCII alphanumeric plus `-_.:`).
    pub profile_name: String,
    /// Monotone profile revision; zero is rejected.
    pub profile_revision: u64,
    /// Finite profile-owned unitization limits.
    pub limits: UnitizationLimits,
}

/// Validated unitizer profile with a bound canonical identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedUnitizerProfile {
    name: String,
    revision: u64,
    limits: UnitizationLimits,
    id: UnitizerProfileId,
}

impl ValidatedUnitizerProfile {
    /// Canonical profile identity.
    #[must_use]
    pub const fn id(&self) -> UnitizerProfileId {
        self.id
    }

    /// Monotone profile revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Human-readable profile name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Finite profile-owned limits.
    #[must_use]
    pub const fn limits(&self) -> UnitizationLimits {
        self.limits
    }
}

/// Validates a unitizer-profile descriptor and binds its canonical identity.
///
/// Rejects empty, overlong or non-explicit names, zero revisions and invalid
/// limits. Implicit language or size defaults can never pass: every
/// load-bearing behavior is explicit in the descriptor.
pub fn validate_unitizer_profile(
    descriptor: &UnitizerProfileDescriptor,
) -> Result<ValidatedUnitizerProfile, UnitizationError> {
    if descriptor.profile_name.is_empty()
        || descriptor.profile_name.len() > MAX_UNITIZER_PROFILE_NAME_BYTES
        || !descriptor
            .profile_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(UnitizationError::UnitizerProfileInvalid);
    }
    if descriptor.profile_revision == 0 {
        return Err(UnitizationError::UnitizerProfileInvalid);
    }
    let limits = descriptor.limits.validate()?;
    let profile = ValidatedUnitizerProfile {
        name: descriptor.profile_name.clone(),
        revision: descriptor.profile_revision,
        limits,
        id: UnitizerProfileId::from_bytes([0; 32]),
    };
    let id = unitizer_profile_digest(&profile);
    Ok(ValidatedUnitizerProfile { id, ..profile })
}

/// Domain-separated canonical digest over the full profile identity.
///
/// Covers profile name, revision, finite limits and boundary-format identity.
/// Any load-bearing change yields a different identity, so existing manifests
/// are never reinterpreted under a changed profile.
#[must_use]
pub fn unitizer_profile_digest(profile: &ValidatedUnitizerProfile) -> UnitizerProfileId {
    let limits = profile.limits();
    UnitizerProfileId::from_bytes(digest32(
        PROFILE_DOMAIN,
        &[
            profile.name().as_bytes(),
            &profile.revision().to_le_bytes(),
            &limits.max_input_bytes.to_le_bytes(),
            &limits.preferred_unit_bytes.to_le_bytes(),
            &limits.max_unit_bytes.to_le_bytes(),
            &limits.max_lines.to_le_bytes(),
            &limits.max_units.to_le_bytes(),
            crate::UnitizationLimits::LAYOUT_FORMAT.as_bytes(),
        ],
    ))
}

/// Profile-change classification. Existing unit IDs and manifests are never
/// reinterpreted under a changed profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UnitizerProfileChange {
    /// Identical canonical identity: nothing to redo.
    Noop,
    /// Same profile name at a monotone revision: reunitize from the exact
    /// representation and reproject downstream coordinates.
    ReunitizeAndReproject,
    /// Renamed profile, decreased revision or identity change outside a
    /// monotone revision: refuse to reinterpret existing manifests.
    Reject,
}

/// Classifies a unitizer-profile transition without reinterpreting history.
#[must_use]
pub fn classify_unitizer_profile_change(
    old: &ValidatedUnitizerProfile,
    new: &ValidatedUnitizerProfile,
) -> UnitizerProfileChange {
    if old.id() == new.id() {
        UnitizerProfileChange::Noop
    } else if old.name() == new.name() && new.revision() > old.revision() {
        UnitizerProfileChange::ReunitizeAndReproject
    } else {
        UnitizerProfileChange::Reject
    }
}
