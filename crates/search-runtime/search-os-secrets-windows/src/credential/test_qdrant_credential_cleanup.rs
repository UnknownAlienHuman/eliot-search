//! Feature-gated cleanup for one freshly observed Qdrant test credential.

use core::{fmt, time::Duration};

use crate::{LegacyRevisionDpapiContext, LegacyRevisionDpapiWork};

use super::{QdrantApiSecretObservation, QdrantApiSecretStoreError};

/// Redacted failure from exact Qdrant Credential Manager cleanup.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum QdrantApiSecretCleanupError {
    /// Credential Manager cleanup is available only on Windows.
    UnsupportedPlatform,
    /// The original operation context refused before delete dispatch.
    ContextRefused,
    /// The exact per-reference Credential Manager mutex was unavailable.
    CredentialLockUnavailable,
    /// The current full Root/user/reference/version/frame observation differed.
    ObservationMismatch,
    /// The bounded selected credential could not be read before delete dispatch.
    CredentialUnavailable,
    /// Delete may have dispatched, but absence was not proved by exact readback.
    OutcomeUnknown,
}

impl QdrantApiSecretCleanupError {
    /// Stable content-free reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "QDRANT_TEST_CLEANUP_UNSUPPORTED_PLATFORM",
            Self::ContextRefused => "QDRANT_TEST_CLEANUP_CONTEXT_REFUSED",
            Self::CredentialLockUnavailable => "QDRANT_TEST_CLEANUP_LOCK_UNAVAILABLE",
            Self::ObservationMismatch => "QDRANT_TEST_CLEANUP_OBSERVATION_MISMATCH",
            Self::CredentialUnavailable => "QDRANT_TEST_CLEANUP_CREDENTIAL_UNAVAILABLE",
            Self::OutcomeUnknown => "QDRANT_TEST_CLEANUP_OUTCOME_UNKNOWN",
        }
    }
}

impl fmt::Display for QdrantApiSecretCleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for QdrantApiSecretCleanupError {}

/// Deletes one exact Qdrant credential from a fresh public selected-secret observation.
///
/// Call only with an observation returned by
/// [`crate::QdrantApiSecretProvisioner::observe_selected_with_context`] for the freshly
/// Root-selected reference created by the current test scene. The owner rechecks
/// the complete selected binding, record version and frame digest while holding
/// the same reference-scoped mutex used by normal Qdrant credential operations.
/// It performs one exact delete and returns success only after readback proves
/// absence; a post-dispatch refusal or unresolved readback remains unknown.
///
/// This API exists only with the `test-credential-cleanup` feature. It never
/// enumerates Credential Manager entries, retries a delete, or selects another
/// target.
///
/// # Errors
/// Returns `ObservationMismatch` without dispatch when the stored frame no
/// longer matches the supplied observation, and `OutcomeUnknown` if possible
/// deletion cannot be resolved by absence readback.
pub fn delete_observed_qdrant_api_secret_for_test<C: LegacyRevisionDpapiContext>(
    observation: &QdrantApiSecretObservation,
    context: &mut C,
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<(), QdrantApiSecretCleanupError> {
    #[cfg(windows)]
    {
        delete_observed_qdrant_api_secret_windows(observation, context, remaining)
    }
    #[cfg(not(windows))]
    {
        let _ = (observation, context, remaining);
        Err(QdrantApiSecretCleanupError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
fn delete_observed_qdrant_api_secret_windows<C: LegacyRevisionDpapiContext>(
    expected: &QdrantApiSecretObservation,
    context: &mut C,
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<(), QdrantApiSecretCleanupError> {
    use super::windows::{WindowsCredentialPlatform, WindowsVaultLock};

    let selected = expected.selected();
    checkpoint(context)?;
    remaining().ok_or(QdrantApiSecretCleanupError::ContextRefused)?;
    let coordinates =
        super::qdrant::credential_cleanup_coordinates(selected, context).map_err(before_effect)?;
    let wait_ms = lock_wait_ms(remaining)?;
    let _lock = WindowsVaultLock::acquire_named(&coordinates.mutex_target, wait_ms)
        .map_err(|()| QdrantApiSecretCleanupError::CredentialLockUnavailable)?
        .ok_or(QdrantApiSecretCleanupError::CredentialLockUnavailable)?;
    remaining().ok_or(QdrantApiSecretCleanupError::ContextRefused)?;
    checkpoint(context)?;

    let frame = WindowsCredentialPlatform::read_provider_credential_with_context(
        &coordinates.target,
        context,
    )
    .map_err(|error| preflight_read(&error))?;
    remaining().ok_or(QdrantApiSecretCleanupError::ContextRefused)?;
    checkpoint(context)?;
    let Some(frame) = frame else {
        return Err(QdrantApiSecretCleanupError::ObservationMismatch);
    };
    let current =
        super::qdrant::observe_credential_cleanup_frame(selected, &frame, context, remaining)
            .map_err(before_effect)?;
    if current != *expected {
        return Err(QdrantApiSecretCleanupError::ObservationMismatch);
    }

    remaining().ok_or(QdrantApiSecretCleanupError::ContextRefused)?;
    charge_native_call(context)?;
    remaining().ok_or(QdrantApiSecretCleanupError::ContextRefused)?;
    checkpoint(context)?;

    // The native return is not proof either way; the same-lock exact readback
    // below is the only success authority. Never replay this delete.
    let _native_status = WindowsCredentialPlatform::delete_credential(&coordinates.target);
    if remaining().is_none() || context.checkpoint().is_err() {
        return Err(QdrantApiSecretCleanupError::OutcomeUnknown);
    }
    let readback = WindowsCredentialPlatform::read_provider_credential_with_context(
        &coordinates.target,
        context,
    )
    .map_err(|_| QdrantApiSecretCleanupError::OutcomeUnknown)?;
    if remaining().is_none() || context.checkpoint().is_err() {
        return Err(QdrantApiSecretCleanupError::OutcomeUnknown);
    }
    if readback.is_none() {
        Ok(())
    } else {
        Err(QdrantApiSecretCleanupError::OutcomeUnknown)
    }
}

#[cfg(windows)]
fn checkpoint<C: LegacyRevisionDpapiContext>(
    context: &mut C,
) -> Result<(), QdrantApiSecretCleanupError> {
    context
        .checkpoint()
        .map_err(|_| QdrantApiSecretCleanupError::ContextRefused)
}

#[cfg(windows)]
fn charge_native_call<C: LegacyRevisionDpapiContext>(
    context: &mut C,
) -> Result<(), QdrantApiSecretCleanupError> {
    checkpoint(context)?;
    context
        .charge(LegacyRevisionDpapiWork::NativeCall)
        .map_err(|_| QdrantApiSecretCleanupError::ContextRefused)?;
    checkpoint(context)
}

#[cfg(windows)]
fn lock_wait_ms(
    remaining: &mut dyn FnMut() -> Option<Duration>,
) -> Result<u32, QdrantApiSecretCleanupError> {
    let left = remaining().ok_or(QdrantApiSecretCleanupError::ContextRefused)?;
    Ok(u32::try_from(left.as_millis().min(25)).unwrap_or(25))
}

#[cfg(windows)]
fn before_effect(error: QdrantApiSecretStoreError) -> QdrantApiSecretCleanupError {
    match error {
        QdrantApiSecretStoreError::UnsupportedPlatform => {
            QdrantApiSecretCleanupError::UnsupportedPlatform
        }
        QdrantApiSecretStoreError::ContextRefused => QdrantApiSecretCleanupError::ContextRefused,
        QdrantApiSecretStoreError::CredentialLockUnavailable => {
            QdrantApiSecretCleanupError::CredentialLockUnavailable
        }
        QdrantApiSecretStoreError::SecretNotFound
        | QdrantApiSecretStoreError::InvalidReference
        | QdrantApiSecretStoreError::CredentialRecordInvalid
        | QdrantApiSecretStoreError::ProtectionFailed
        | QdrantApiSecretStoreError::CredentialFrameMismatch => {
            QdrantApiSecretCleanupError::ObservationMismatch
        }
        QdrantApiSecretStoreError::CredentialReadFailed
        | QdrantApiSecretStoreError::LeaseRejected(_)
        | QdrantApiSecretStoreError::MutationUnsupported
        | QdrantApiSecretStoreError::InvalidLeasePolicy
        | QdrantApiSecretStoreError::CredentialAlreadyExists
        | QdrantApiSecretStoreError::CredentialGenerationFailed
        | QdrantApiSecretStoreError::CredentialWriteOutcomeUnknown => {
            QdrantApiSecretCleanupError::CredentialUnavailable
        }
    }
}

#[cfg(windows)]
fn preflight_read<E>(
    error: &super::windows::CredentialWorkError<E>,
) -> QdrantApiSecretCleanupError {
    match error {
        super::windows::CredentialWorkError::Context(_) => {
            QdrantApiSecretCleanupError::ContextRefused
        }
        super::windows::CredentialWorkError::Credential
        | super::windows::CredentialWorkError::KeyGeneration => {
            QdrantApiSecretCleanupError::CredentialUnavailable
        }
    }
}
