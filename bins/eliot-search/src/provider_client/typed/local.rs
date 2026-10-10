//! One-shot handoff from trusted registration resolution to local transport.
//!
//! The resolved bundle owns one non-clonable pairing key and the diminishing
//! remainder of the caller's original context. This module consumes that bundle
//! exactly once, invokes one platform connector for the canonical endpoint name,
//! and hands the sole stream into the existing pairing/profile engine.

#![allow(dead_code)]

use std::time::Duration;

use search_ports::CancellationProbe;
use search_provider_protocol::{NativeEndpointNameV1, ProtocolLimits};

use crate::native_local_registered::ResolvedNativeLocal;

use super::io::LocalByteStream;
use super::{TypedClientError, TypedProviderSession};

/// Select the concrete Windows producer once, preserving the resolver origin.
/// Command cutover remains #332; this is the actual compiled attachment path.
#[cfg(windows)]
pub(super) fn open_resolved_windows<C: CancellationProbe>(
    resolved: ResolvedNativeLocal<C>,
    limits: ProtocolLimits,
) -> Result<TypedProviderSession, TypedClientError> {
    use search_provider_transport_windows::{NativeLocalConnection, NativeTransportError};

    let (endpoint_name, binding, key, context, deadline) = resolved.into_parts();
    let cancelled = || context.cancellation().is_cancelled();
    let budget = super::io::SetupBudget::from_deadline(deadline, &cancelled)?;
    let stream = NativeLocalConnection::open(endpoint_name, deadline).map_err(|error| {
        match error {
            NativeTransportError::DeadlineExpired => TypedClientError::DeadlineExpired,
            _ => TypedClientError::Io,
        }
    })?;
    budget.remaining()?;
    TypedProviderSession::open_connected_local_stream(
        stream, &binding, key, limits, &context, &budget,
    )
}

/// Connect and authenticate one already-resolved local registration.
///
/// The connector is called exactly once. It receives no binding, key, data-root,
/// host, port, path or alternate endpoint and cannot renew the original deadline.
/// Returning a stream is not authentication; canonical mutual pairing and current
/// daemon-side binding/policy validation remain mandatory.
pub(super) fn open_resolved_local<C, S>(
    resolved: ResolvedNativeLocal<C>,
    limits: ProtocolLimits,
    connect: impl FnOnce(NativeEndpointNameV1, Duration) -> std::io::Result<S>,
) -> Result<TypedProviderSession, TypedClientError>
where
    C: CancellationProbe,
    S: LocalByteStream + 'static,
{
    let (endpoint_name, binding, key, context, deadline) = resolved.into_parts();
    TypedProviderSession::connect_local(
        endpoint_name,
        &binding,
        key,
        limits,
        &context,
        deadline,
        connect,
    )
}
