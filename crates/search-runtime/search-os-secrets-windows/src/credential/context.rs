//! Caller-owned cooperative deadline/cancellation for the revision credential kernel.

use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;
use std::time::Instant;

use super::LegacyRevisionRootSecretError;
use super::{LegacyRevisionRootSecret, LegacyRevisionRootSecretRequirement};

/// Content-free failure from a context-aware legacy revision credential operation.
///
/// The legacy error enum remains unchanged for existing exhaustive consumers.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyRevisionRootSecretContextError {
    /// The original caller cancellation flag was set before any possible write.
    Cancelled,
    /// The original absolute deadline expired before any possible write.
    DeadlineExpired,
    /// Credential-owner failure; post-write uncertainty uses the legacy unknown variant.
    Credential(LegacyRevisionRootSecretError),
}

impl LegacyRevisionRootSecretContextError {
    /// Stable reason without native error numbers, target coordinates or secret bytes.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "WINDOWS_REVISION_KEY_CANCELLED",
            Self::DeadlineExpired => "WINDOWS_REVISION_KEY_DEADLINE_EXPIRED",
            Self::Credential(error) => error.code(),
        }
    }
}

impl From<LegacyRevisionRootSecretError> for LegacyRevisionRootSecretContextError {
    fn from(error: LegacyRevisionRootSecretError) -> Self {
        Self::Credential(error)
    }
}

impl fmt::Debug for LegacyRevisionRootSecretContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for LegacyRevisionRootSecretContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LegacyRevisionRootSecretContextError {}

pub(super) trait RootSecretCheckpoint {
    type Error: From<LegacyRevisionRootSecretError>;

    fn check(&self) -> Result<(), Self::Error>;
    fn limit_sleep(&self, duration: Duration) -> Duration;
}

pub(super) struct LegacyCheckpoint;

impl RootSecretCheckpoint for LegacyCheckpoint {
    type Error = LegacyRevisionRootSecretError;

    fn check(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn limit_sleep(&self, duration: Duration) -> Duration {
        duration
    }
}

pub(super) struct OperationContext<'a> {
    deadline: Instant,
    cancellation: &'a AtomicBool,
}

impl<'a> OperationContext<'a> {
    pub(super) const fn new(deadline: Instant, cancellation: &'a AtomicBool) -> Self {
        Self {
            deadline,
            cancellation,
        }
    }
}

impl RootSecretCheckpoint for OperationContext<'_> {
    type Error = LegacyRevisionRootSecretContextError;

    fn check(&self) -> Result<(), Self::Error> {
        if self.cancellation.load(Ordering::Acquire) {
            return Err(Self::Error::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(Self::Error::DeadlineExpired);
        }
        Ok(())
    }

    fn limit_sleep(&self, duration: Duration) -> Duration {
        duration.min(self.deadline.saturating_duration_since(Instant::now()))
    }
}

/// Reads an existing credential under the original absolute deadline and flag.
///
/// Absence never generates or publishes a key. Checks surround the native read
/// and terminal return. Native calls are cooperative, not forcibly preempted.
pub fn load_existing_legacy_revision_root_secret_with_context(
    namespace_id: &[u8; 32],
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<Option<LegacyRevisionRootSecret>, LegacyRevisionRootSecretContextError> {
    let context = OperationContext::new(deadline, cancellation);
    context.check()?;
    #[cfg(windows)]
    {
        let mut platform = super::windows::WindowsCredentialPlatform;
        let result = super::read_with_platform(
            &mut platform,
            &super::credential_target(namespace_id),
            &context,
        )?;
        context.check()?;
        Ok(result)
    }
    #[cfg(not(windows))]
    {
        let _ = namespace_id;
        Err(LegacyRevisionRootSecretError::UnsupportedPlatform.into())
    }
}

/// Loads or creates a credential with cooperative original-context checkpoints.
///
/// Creation still requires the caller-established missing-key requirement.
/// Acquisition retries are finite and consume the original deadline. Publication
/// attempts at most one write; every subsequent unverified outcome or context
/// refusal returns `CredentialWriteOutcomeUnknown`, with no replay or cleanup.
/// The native mutex wait is bounded at five seconds and cannot be preempted by
/// this API; expired/cancelled calls refuse further work when that seam returns.
pub fn load_or_create_legacy_revision_root_secret_with_context(
    namespace_id: &[u8; 32],
    requirement: LegacyRevisionRootSecretRequirement,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<LegacyRevisionRootSecret, LegacyRevisionRootSecretContextError> {
    let context = OperationContext::new(deadline, cancellation);
    context.check()?;
    #[cfg(windows)]
    {
        let mut platform = super::windows::WindowsCredentialPlatform;
        super::load_or_create_checked(&mut platform, namespace_id, requirement, &context)
    }
    #[cfg(not(windows))]
    {
        let _ = (namespace_id, requirement);
        Err(LegacyRevisionRootSecretError::UnsupportedPlatform.into())
    }
}
