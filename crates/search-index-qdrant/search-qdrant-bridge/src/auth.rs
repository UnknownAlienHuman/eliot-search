//! Bounded authenticated Qdrant connection capability.
//!
//! Composition adapts its existing purpose-bound secret lease and current
//! supervisor/process guard to these content-free bindings. The bridge never
//! owns the plaintext key: it is borrowed only inside the provider callback.
//! The pinned Qdrant SDK copies the supplied key into its interceptor, so that
//! SDK-owned copy lives only as long as the client and is not zeroized here.

use core::fmt;
use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};

use search_contracts::{
    ArtifactDigest, Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch,
};

use crate::{BridgeError, live::LiveEndpoint};

/// Explicit IPv4 or IPv6 loopback host. DNS names are not accepted as an
/// identity because they can resolve to different addresses over time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QdrantLoopbackHost {
    Ipv4,
    Ipv6,
}

/// Supervisor-issued endpoint identity. The digest is opaque equality
/// metadata; the normalized host and both ports are compared to the actual
/// client endpoint separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QdrantEndpointIdentity {
    host: QdrantLoopbackHost,
    http_port: NonZeroU16,
    grpc_port: NonZeroU16,
    endpoint_digest: Blake3Digest32,
}

impl QdrantEndpointIdentity {
    /// Constructs an endpoint identity from the validated supervisor plan.
    #[must_use]
    pub const fn new(
        host: QdrantLoopbackHost,
        http_port: NonZeroU16,
        grpc_port: NonZeroU16,
        endpoint_digest: Blake3Digest32,
    ) -> Self {
        Self {
            host,
            http_port,
            grpc_port,
            endpoint_digest,
        }
    }

    /// Explicit loopback host selected by the validated process plan.
    #[must_use]
    pub const fn host(self) -> QdrantLoopbackHost {
        self.host
    }

    /// HTTP port owned by the validated process plan.
    #[must_use]
    pub const fn http_port(self) -> NonZeroU16 {
        self.http_port
    }

    /// gRPC port owned by the validated process plan.
    #[must_use]
    pub const fn grpc_port(self) -> NonZeroU16 {
        self.grpc_port
    }

    /// Existing opaque supervisor endpoint digest, used only for equality.
    #[must_use]
    pub const fn endpoint_digest(self) -> Blake3Digest32 {
        self.endpoint_digest
    }

    pub(crate) fn matches_live_endpoint(self, endpoint: &LiveEndpoint) -> bool {
        let expected_host = match self.host {
            QdrantLoopbackHost::Ipv4 => "127.0.0.1",
            QdrantLoopbackHost::Ipv6 => "::1",
        };
        endpoint.host() == expected_host
            && endpoint.http_port() == self.http_port.get()
            && endpoint.grpc_port() == self.grpc_port.get()
    }
}

/// Exact current supervisor identity and its purpose-bound API-key lease
/// metadata. The values are equality bindings supplied by trusted daemon
/// composition; they are not signatures or independently verified process
/// evidence. The provider must compare them against its current OS process
/// guard, owner fence, endpoint plan and secret lease on every dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QdrantConnectionBinding {
    process_id: NonZeroU32,
    creation_marker: NonZeroU64,
    data_root_id: DataRootId,
    installation_incarnation_id: InstallationIncarnationId,
    owner_epoch: OwnerEpoch,
    artifact_digest: ArtifactDigest,
    endpoint: QdrantEndpointIdentity,
    secret_reference_digest: Blake3Digest32,
    secret_purpose_digest: Blake3Digest32,
    secret_expires_at_tick: NonZeroU64,
}

impl QdrantConnectionBinding {
    /// Creates a content-free binding from the exact supervisor identity,
    /// endpoint plan and API-key lease fields.
    ///
    /// The secret lease must belong to the same installation incarnation as
    /// the process owner fence. This constructor only enforces that equality;
    /// it does not verify the caller's process, endpoint, artifact or lease.
    pub fn new(
        process_id: NonZeroU32,
        creation_marker: NonZeroU64,
        data_root_id: DataRootId,
        installation_incarnation_id: InstallationIncarnationId,
        owner_epoch: OwnerEpoch,
        artifact_digest: ArtifactDigest,
        endpoint: QdrantEndpointIdentity,
        secret_reference_digest: Blake3Digest32,
        secret_installation_incarnation_id: InstallationIncarnationId,
        secret_purpose_digest: Blake3Digest32,
        secret_expires_at_tick: NonZeroU64,
    ) -> Result<Self, BridgeError> {
        if installation_incarnation_id != secret_installation_incarnation_id {
            return Err(BridgeError::SupervisorReceiptMismatch);
        }
        Ok(Self {
            process_id,
            creation_marker,
            data_root_id,
            installation_incarnation_id,
            owner_epoch,
            artifact_digest,
            endpoint,
            secret_reference_digest,
            secret_purpose_digest,
            secret_expires_at_tick,
        })
    }

    /// OS-derived process identifier from the current supervisor guard.
    #[must_use]
    pub const fn process_id(self) -> NonZeroU32 {
        self.process_id
    }

    /// OS-derived process creation marker from the current supervisor guard.
    #[must_use]
    pub const fn creation_marker(self) -> NonZeroU64 {
        self.creation_marker
    }

    /// Exact owner data-root identity.
    #[must_use]
    pub const fn data_root_id(self) -> DataRootId {
        self.data_root_id
    }

    /// Exact installation incarnation from the owner fence.
    #[must_use]
    pub const fn installation_incarnation_id(self) -> InstallationIncarnationId {
        self.installation_incarnation_id
    }

    /// Exact owner epoch from the owner fence.
    #[must_use]
    pub const fn owner_epoch(self) -> OwnerEpoch {
        self.owner_epoch
    }

    /// Typed artifact identity from the qualified process guard.
    #[must_use]
    pub const fn artifact_digest(self) -> ArtifactDigest {
        self.artifact_digest
    }

    /// Exact supervisor-owned loopback endpoint tuple and equality digest.
    #[must_use]
    pub const fn endpoint(self) -> QdrantEndpointIdentity {
        self.endpoint
    }

    /// Content-free identity of the leased secret reference.
    #[must_use]
    pub const fn secret_reference_digest(self) -> Blake3Digest32 {
        self.secret_reference_digest
    }

    /// Content-free digest of the fixed Qdrant API-key purpose.
    #[must_use]
    pub const fn secret_purpose_digest(self) -> Blake3Digest32 {
        self.secret_purpose_digest
    }

    /// Monotonic expiry tick from the exact purpose-bound secret lease.
    #[must_use]
    pub const fn secret_expires_at_tick(self) -> NonZeroU64 {
        self.secret_expires_at_tick
    }

    pub(crate) fn matches_live_endpoint(self, endpoint: &LiveEndpoint) -> bool {
        self.endpoint.matches_live_endpoint(endpoint)
    }
}

/// Callback-only provider for a purpose-bound API-key lease.
///
/// `with_secret` must revalidate the supplied binding against the current OS
/// process guard, owner fence, endpoint plan and underlying lease before
/// invoking the callback. Implementations should adapt the existing
/// secret-store lease and monotonic clock; the bridge has no store or
/// supervisor dependency.
pub trait QdrantApiKeyLeaseProvider: Send + Sync + 'static {
    /// Returns a tick from the same process-local monotonic clock as the lease.
    fn monotonic_now_ticks(&self) -> Result<u64, BridgeError>;

    /// Exposes the leased key only inside one bounded callback.
    fn with_secret(
        &self,
        binding: &QdrantConnectionBinding,
        now_tick: u64,
        callback: &mut dyn FnMut(&[u8]) -> Result<(), BridgeError>,
    ) -> Result<(), BridgeError>;
}

/// Redacted capability for one exact Qdrant API-key lease and supervisor
/// incarnation. The key is borrowed only while the provider callback runs.
pub struct QdrantApiKeyLease {
    binding: QdrantConnectionBinding,
    provider: Box<dyn QdrantApiKeyLeaseProvider>,
}

impl QdrantApiKeyLease {
    /// Builds a lease capability from the exact binding and its callback
    /// adapter. The adapter must use trusted daemon composition to revalidate
    /// the current OS process guard, owner fence, endpoint plan and secret
    /// lease each time it invokes the callback. The bridge owns no plaintext
    /// key representation.
    #[must_use]
    pub fn new(binding: QdrantConnectionBinding, provider: impl QdrantApiKeyLeaseProvider) -> Self {
        Self {
            binding,
            provider: Box::new(provider),
        }
    }

    /// Content-free binding captured by this capability.
    #[must_use]
    pub const fn binding(&self) -> QdrantConnectionBinding {
        self.binding
    }

    /// Confirms this lease belongs to the exact supervisor snapshot supplied
    /// for the connection or operation.
    pub(crate) fn validate_binding(
        &self,
        current: QdrantConnectionBinding,
    ) -> Result<(), BridgeError> {
        if self.binding == current {
            Ok(())
        } else {
            Err(BridgeError::SupervisorReceiptMismatch)
        }
    }

    /// Checks that the exact lease is current without retaining its key.
    pub(crate) fn validate(&self) -> Result<(), BridgeError> {
        self.with_secret(|_| Ok(()))
    }

    /// Invokes an operation inside a single secret callback after checking
    /// process binding and monotonic expiry.
    pub(crate) fn with_secret(
        &self,
        mut callback: impl FnMut(&str) -> Result<(), BridgeError>,
    ) -> Result<(), BridgeError> {
        let now_tick = self
            .provider
            .monotonic_now_ticks()
            .map_err(normalize_provider_error)?;
        if now_tick >= self.binding.secret_expires_at_tick.get() {
            return Err(BridgeError::AuthenticationLeaseExpired);
        }

        let mut callback_count = 0_u8;
        let mut callback_error = None;
        let result = self
            .provider
            .with_secret(&self.binding, now_tick, &mut |bytes| {
                callback_count = callback_count.saturating_add(1);
                if callback_count != 1 {
                    callback_error = Some(BridgeError::AuthenticationInvalid);
                    return Err(BridgeError::AuthenticationInvalid);
                }
                if bytes.is_empty()
                    || bytes.len() > MAX_API_KEY_BYTES
                    || !bytes.is_ascii()
                    || bytes.iter().any(|byte| *byte < 0x20 || *byte == 0x7f)
                {
                    callback_error = Some(BridgeError::AuthenticationInvalid);
                    return Err(BridgeError::AuthenticationInvalid);
                }
                let Ok(key) = core::str::from_utf8(bytes) else {
                    callback_error = Some(BridgeError::AuthenticationInvalid);
                    return Err(BridgeError::AuthenticationInvalid);
                };
                let result = callback(key);
                if let Err(error) = result {
                    callback_error = Some(error);
                    return Err(error);
                }
                Ok(())
            });

        if let Some(error) = callback_error {
            return Err(error);
        }
        result.map_err(normalize_provider_error)?;
        if callback_count != 1 {
            return Err(BridgeError::AuthenticationInvalid);
        }
        Ok(())
    }
}

impl fmt::Debug for QdrantApiKeyLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QdrantApiKeyLease")
            .field("binding", &self.binding)
            .field("key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

const MAX_API_KEY_BYTES: usize = 4096;

fn normalize_provider_error(error: BridgeError) -> BridgeError {
    match error {
        BridgeError::AuthenticationLeaseExpired
        | BridgeError::AuthenticationInvalid
        | BridgeError::SupervisorReceiptMismatch => error,
        _ => BridgeError::AuthenticationInvalid,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    struct FixtureProvider {
        binding: QdrantConnectionBinding,
        now_tick: u64,
        calls: Arc<AtomicUsize>,
    }

    impl QdrantApiKeyLeaseProvider for FixtureProvider {
        fn monotonic_now_ticks(&self) -> Result<u64, BridgeError> {
            Ok(self.now_tick)
        }

        fn with_secret(
            &self,
            binding: &QdrantConnectionBinding,
            now_tick: u64,
            callback: &mut dyn FnMut(&[u8]) -> Result<(), BridgeError>,
        ) -> Result<(), BridgeError> {
            if binding != &self.binding {
                return Err(BridgeError::SupervisorReceiptMismatch);
            }
            if now_tick >= binding.secret_expires_at_tick().get() {
                return Err(BridgeError::AuthenticationLeaseExpired);
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
            callback(b"fixture-api-key-not-for-production")
        }
    }

    fn binding(endpoint_digest: u8, expires_at: u64) -> QdrantConnectionBinding {
        let incarnation = InstallationIncarnationId::from_bytes([0x31; 16]);
        let endpoint = QdrantEndpointIdentity::new(
            QdrantLoopbackHost::Ipv4,
            NonZeroU16::new(6333).expect("nonzero port"),
            NonZeroU16::new(6334).expect("nonzero port"),
            Blake3Digest32::from_bytes([endpoint_digest; 32]),
        );
        QdrantConnectionBinding::new(
            NonZeroU32::new(7).expect("nonzero pid"),
            NonZeroU64::new(11).expect("nonzero creation marker"),
            DataRootId::from_bytes([0x32; 16]),
            incarnation,
            OwnerEpoch::new(3).expect("nonzero owner epoch"),
            ArtifactDigest::from_bytes([0x33; 32]),
            endpoint,
            Blake3Digest32::from_bytes([0x34; 32]),
            incarnation,
            Blake3Digest32::from_bytes([0x35; 32]),
            NonZeroU64::new(expires_at).expect("nonzero expiry"),
        )
        .expect("matching lease incarnation")
    }

    fn lease(
        binding: QdrantConnectionBinding,
        now_tick: u64,
    ) -> (QdrantApiKeyLease, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (
            QdrantApiKeyLease::new(
                binding,
                FixtureProvider {
                    binding,
                    now_tick,
                    calls: Arc::clone(&calls),
                },
            ),
            calls,
        )
    }

    #[test]
    fn expired_lease_is_rejected_before_secret_or_dispatch_callback() {
        let binding = binding(0x36, 10);
        let (lease, calls) = lease(binding, 10);

        assert_eq!(
            lease.validate(),
            Err(BridgeError::AuthenticationLeaseExpired)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn mismatched_supervisor_binding_is_rejected_before_secret_callback() {
        let expected = binding(0x37, 100);
        let stale = binding(0x38, 100);
        let (lease, calls) = lease(stale, 1);

        assert_eq!(
            lease.validate_binding(expected),
            Err(BridgeError::SupervisorReceiptMismatch)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn debug_output_never_contains_the_callback_secret() {
        let binding = binding(0x39, 100);
        let (lease, _) = lease(binding, 1);

        let rendered = format!("{lease:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("fixture-api-key-not-for-production"));
    }
}
