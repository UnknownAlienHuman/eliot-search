//! Closed restore limits and content-free failure vocabulary.

/// Finite restore inventory limits.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestoreLimits {
    /// Maximum sources carried by one export.
    pub max_sources: usize,
    /// Maximum memberships carried by one export.
    pub max_memberships: usize,
    /// Maximum canonical manifest bytes hashed for the digest.
    pub max_manifest_bytes: usize,
}

impl RestoreLimits {
    /// Conservative local baseline.
    pub const BASELINE: Self = Self {
        max_sources: 1_024,
        max_memberships: 1_024,
        max_manifest_bytes: 64 * 1_024,
    };

    /// Validates finite non-zero limits.
    ///
    /// # Errors
    ///
    /// Returns [`RestoreCompositionError::CapacityExceeded`] when any limit is zero.
    pub const fn validate(self) -> Result<Self, RestoreCompositionError> {
        if self.max_sources == 0 || self.max_memberships == 0 || self.max_manifest_bytes == 0 {
            Err(RestoreCompositionError::CapacityExceeded)
        } else {
            Ok(self)
        }
    }
}

/// Closed restore-composition failure with a stable machine code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreCompositionError {
    /// Export manifest is partial, tampered or unpaired.
    ManifestInvalid,
    /// Destination names a different or relocated root.
    DestinationMismatch,
    /// Destination ACL or restrictive policy is not enforced.
    DestinationNotValidated,
    /// Key user, installation, incarnation or purpose binding differs.
    KeyBindingMismatch,
    /// Key bytes differ without an explicit migration.
    KeyMismatch,
    /// Cipher bytes would change without explicit authorization.
    CipherChangeNotAuthorized,
    /// Backup predates the live purge fence and must not resurrect.
    PurgeFenceStale,
    /// Ownership change requires a registry-verified cutover.
    OwnerCutoverRequired,
    /// Old owner attempted to serve after an accepted cutover.
    OldOwnerStillServing,
    /// Source deletion would destroy the sole valid copy.
    SoleCopyProtection,
    /// Restore has not completed exact readback.
    RevalidationIncomplete,
    /// External mutation outcome requires exact recovery.
    OutcomeUnknown,
    /// Contradictory state requires quarantine.
    Quarantined,
    /// Finite capacity was exhausted.
    CapacityExceeded,
}

impl RestoreCompositionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ManifestInvalid => "RESTORE_MANIFEST_INVALID",
            Self::DestinationMismatch => "RESTORE_DESTINATION_MISMATCH",
            Self::DestinationNotValidated => "RESTORE_DESTINATION_NOT_VALIDATED",
            Self::KeyBindingMismatch => "RESTORE_KEY_BINDING_MISMATCH",
            Self::KeyMismatch => "RESTORE_KEY_MISMATCH",
            Self::CipherChangeNotAuthorized => "RESTORE_CIPHER_CHANGE_NOT_AUTHORIZED",
            Self::PurgeFenceStale => "RESTORE_PURGE_FENCE_STALE",
            Self::OwnerCutoverRequired => "RESTORE_OWNER_CUTOVER_REQUIRED",
            Self::OldOwnerStillServing => "RESTORE_OLD_OWNER_STILL_SERVING",
            Self::SoleCopyProtection => "RESTORE_SOLE_COPY_PROTECTION",
            Self::RevalidationIncomplete => "RESTORE_REVALIDATION_INCOMPLETE",
            Self::OutcomeUnknown => "RESTORE_OUTCOME_UNKNOWN",
            Self::Quarantined => "RESTORE_QUARANTINED",
            Self::CapacityExceeded => "RESTORE_CAPACITY_EXCEEDED",
        }
    }
}

impl core::fmt::Display for RestoreCompositionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RestoreCompositionError {}
