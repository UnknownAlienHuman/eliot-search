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
    let (endpoint_name, binding, key, context) = resolved.into_parts();
    TypedProviderSession::connect_local(
        endpoint_name,
        &binding,
        key,
        limits,
        &context,
        connect,
    )
}
