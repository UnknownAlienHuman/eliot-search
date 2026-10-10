//! One-shot composition of trusted registration and concrete platform connection.
#![allow(dead_code)]
#[cfg(windows)]
pub(super) fn open_resolved_windows<C: search_ports::CancellationProbe>(
    resolved: crate::native_local_registered::ResolvedNativeLocal<C>,
    limits: search_provider_protocol::ProtocolLimits,
) -> Result<super::TypedProviderSession, super::TypedClientError> {
    use search_provider_transport_windows::{NativeLocalConnection, NativeTransportError};
    use super::{TypedProviderSession, TypedClientError};
    let (endpoint_name, expectations, key, context, deadline) = resolved.into_parts();
    if context.cancellation().is_cancelled() {
        return Err(TypedClientError::Cancelled);
    }
    let stream = NativeLocalConnection::open(endpoint_name, deadline).map_err(|error| {
        match error {
            NativeTransportError::DeadlineExpired => TypedClientError::DeadlineExpired,
            _ => TypedClientError::Io,
        }
    })?;
    TypedProviderSession::open_native(stream, &expectations, key, limits, &context, deadline)
}
