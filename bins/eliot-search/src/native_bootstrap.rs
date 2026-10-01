//! End-to-end authenticated native bootstrap for the standalone client.
//!
//! This composition preserves one caller deadline across bounded descriptor
//! resolution, the platform Credential Manager read, descriptor authentication,
//! loopback connect, mutual pairing and typed-profile setup. Registration
//! expectation remains caller-owned trusted configuration and is never derived
//! from the descriptor being authenticated.

#![allow(dead_code)]

use core::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::ProtocolLimits;

use crate::native_credentials::{
    PlatformNativePairingKeyError, PlatformNativePairingKeySource,
};
use crate::native_registered::{
    NativeDescriptorError, NativeEndpointExpectation, RegisteredNativeError,
    open_registered_native, read_native_endpoint_descriptor,
};
use crate::provider_client::TypedProviderSession;

/// Failure while resolving and opening one authenticated native provider.
#[derive(Debug)]
pub(crate) enum NativeBootstrapError {
    /// The original operation was cancelled before the registered opener began.
    Cancelled,
    /// The original deadline elapsed or could not be represented.
    DeadlineExpired,
    /// The bounded descriptor could not be read or decoded safely.
    Descriptor(NativeDescriptorError),
    /// Credential resolution, proof verification or native setup failed.
    Registered(RegisteredNativeError<PlatformNativePairingKeyError>),
}

impl NativeBootstrapError {
    /// Stable content-free reason code.
    #[must_use]
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "NATIVE_CLIENT_BOOTSTRAP_CANCELLED",
            Self::DeadlineExpired => "NATIVE_CLIENT_BOOTSTRAP_DEADLINE_EXPIRED",
            Self::Descriptor(error) => error.code(),
            Self::Registered(RegisteredNativeError::Protocol(error)) => error.code(),
            Self::Registered(RegisteredNativeError::Cancelled) => {
                "NATIVE_ENDPOINT_CANCELLED"
            }
            Self::Registered(RegisteredNativeError::DeadlineExpired) => {
                "NATIVE_ENDPOINT_DEADLINE_EXPIRED"
            }
            Self::Registered(RegisteredNativeError::Credential(error)) => error.code(),
            Self::Registered(RegisteredNativeError::CredentialMissing) => {
                "NATIVE_ENDPOINT_CREDENTIAL_MISSING"
            }
            Self::Registered(RegisteredNativeError::DescriptorMismatch) => {
                "NATIVE_ENDPOINT_DESCRIPTOR_MISMATCH"
            }
            Self::Registered(RegisteredNativeError::DescriptorProofInvalid) => {
                "NATIVE_ENDPOINT_DESCRIPTOR_PROOF_INVALID"
            }
            Self::Registered(RegisteredNativeError::Client(error)) => error.code(),
        }
    }
}

impl fmt::Display for NativeBootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for NativeBootstrapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Descriptor(error) => Some(error),
            Self::Registered(error) => Some(error),
            Self::Cancelled | Self::DeadlineExpired => None,
        }
    }
}

impl From<NativeDescriptorError> for NativeBootstrapError {
    fn from(error: NativeDescriptorError) -> Self {
        Self::Descriptor(error)
    }
}

impl From<RegisteredNativeError<PlatformNativePairingKeyError>> for NativeBootstrapError {
    fn from(error: RegisteredNativeError<PlatformNativePairingKeyError>) -> Self {
        Self::Registered(error)
    }
}

/// Read, authenticate and open the registered native provider under one budget.
///
/// The data root selects only the bounded descriptor locator. `expected` must be
/// supplied by independently trusted client configuration; descriptor fields,
/// loopback locality, same-user access and Credential Manager possession cannot
/// manufacture or widen it.
pub(crate) fn open_platform_registered_native<C>(
    data_root: &Path,
    expected: &NativeEndpointExpectation,
    limits: ProtocolLimits,
    context: &OperationContext<C>,
) -> Result<TypedProviderSession, NativeBootstrapError>
where
    C: CancellationProbe + Clone,
{
    let budget = BootstrapBudget::new(context)?;
    budget.check(context)?;
    let descriptor = read_native_endpoint_descriptor(data_root)?;
    let remaining = budget.remaining_context(context)?;
    let mut key_source = PlatformNativePairingKeySource;
    open_registered_native(
        &descriptor,
        expected,
        &mut key_source,
        limits,
        &remaining,
    )
    .map_err(Into::into)
}

struct BootstrapBudget {
    deadline: Instant,
}

impl BootstrapBudget {
    fn new<C: CancellationProbe>(
        context: &OperationContext<C>,
    ) -> Result<Self, NativeBootstrapError> {
        if context.cancellation().is_cancelled() {
            return Err(NativeBootstrapError::Cancelled);
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(context.relative_deadline_ms().get()))
            .ok_or(NativeBootstrapError::DeadlineExpired)?;
        Ok(Self { deadline })
    }

    fn check<C: CancellationProbe>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<Duration, NativeBootstrapError> {
        if context.cancellation().is_cancelled() {
            return Err(NativeBootstrapError::Cancelled);
        }
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(NativeBootstrapError::DeadlineExpired)
    }

    fn remaining_context<C: CancellationProbe + Clone>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<OperationContext<C>, NativeBootstrapError> {
        let remaining = self.check(context)?;
        let millis = u64::try_from(remaining.as_millis())
            .ok()
            .filter(|millis| *millis > 0)
            .ok_or(NativeBootstrapError::DeadlineExpired)?;
        OperationContext::new(
            context.request_id(),
            millis,
            context.cancellation().clone(),
            context.budget_ref().clone(),
        )
        .map_err(|_| NativeBootstrapError::DeadlineExpired)
    }
}
