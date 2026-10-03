//! Lease-bound secret material for the Qdrant API key.
//!
//! Plaintext exists only inside [`SecretMaterial`]: a non-clone, Debug-
//! redacted owner that zeroes its bytes on drop (best-effort memory hygiene
//! in safe Rust, not a secure-erasure claim). Material never enters argv,
//! config files, logs, or receipts; the only sink is the child process
//! environment block plus loopback HTTP probe headers, both provided by the
//! execution layer in this same package.

use core::fmt;
use core::num::NonZeroU64;

use search_contracts::{Blake3Digest32, InstallationIncarnationId};

use crate::SupervisorError;

/// Maximum API-key size accepted by this process boundary.
pub const MAX_SECRET_BYTES: usize = 4_096;

/// Vendor-neutral view of the already-issued process secret lease.
///
/// Composition implements this over the opaque lease it already owns. The
/// callback is the only plaintext access path; callers pass the same lease to
/// the bridge's authenticated client adapter so process and client use one
/// secret reference. This trait does not create, persist, or rotate secrets.
pub trait QdrantSecretLease {
    /// Digest of the exact opaque secret reference.
    fn secret_reference_digest(&self) -> Blake3Digest32;

    /// Installation incarnation bound into the lease.
    fn installation_incarnation_id(&self) -> InstallationIncarnationId;

    /// Purpose digest bound into the lease.
    fn purpose_digest(&self) -> Blake3Digest32;

    /// Expiry in the composition-owned monotonic clock domain.
    fn expires_at_tick(&self) -> NonZeroU64;

    /// Exposes the lease bytes only for the lifetime of `use_bytes`.
    fn with_secret_bytes<R>(&self, use_bytes: impl FnOnce(&[u8]) -> R) -> R;
}

/// Content-free binding copied from one concrete lease capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecretLeaseBinding {
    secret_reference_digest: Blake3Digest32,
    installation_incarnation_id: InstallationIncarnationId,
    purpose_digest: Blake3Digest32,
    expires_at_tick: NonZeroU64,
}

impl SecretLeaseBinding {
    pub(crate) fn from_lease(lease: &impl QdrantSecretLease) -> Self {
        Self {
            secret_reference_digest: lease.secret_reference_digest(),
            installation_incarnation_id: lease.installation_incarnation_id(),
            purpose_digest: lease.purpose_digest(),
            expires_at_tick: lease.expires_at_tick(),
        }
    }

    /// Exact opaque secret reference digest, suitable for equality binding.
    #[must_use]
    pub const fn secret_reference_digest(self) -> Blake3Digest32 {
        self.secret_reference_digest
    }

    /// Lease installation incarnation.
    #[must_use]
    pub const fn installation_incarnation_id(self) -> InstallationIncarnationId {
        self.installation_incarnation_id
    }

    /// Lease purpose digest.
    #[must_use]
    pub const fn purpose_digest(self) -> Blake3Digest32 {
        self.purpose_digest
    }

    /// Lease expiry in the issuing monotonic clock domain.
    #[must_use]
    pub const fn expires_at_tick(self) -> NonZeroU64 {
        self.expires_at_tick
    }
}

/// Owned plaintext bound to one launch. Never cloned, never logged.
pub struct SecretMaterial {
    bytes: Vec<u8>,
    binding: SecretLeaseBinding,
}

impl SecretMaterial {
    /// Takes a bounded snapshot of the bytes from the exact planned lease.
    pub(crate) fn from_lease(
        lease: &impl QdrantSecretLease,
        expected: SecretLeaseBinding,
        now_tick: NonZeroU64,
    ) -> Result<Self, SupervisorError> {
        let binding = SecretLeaseBinding::from_lease(lease);
        if binding != expected || binding.expires_at_tick <= now_tick {
            return Err(SupervisorError::SecretLeaseInvalid);
        }
        let bytes = lease.with_secret_bytes(|bytes| {
            if bytes.is_empty() || bytes.len() > MAX_SECRET_BYTES {
                return Err(SupervisorError::SecretLeaseInvalid);
            }
            let owned = bytes.to_vec();
            if !owned.iter().all(|byte| (0x21_u8..=0x7E_u8).contains(byte)) {
                let mut owned = owned;
                owned.fill(0);
                core::hint::black_box(owned.as_mut_ptr());
                return Err(SupervisorError::SecretLeaseInvalid);
            }
            Ok(owned)
        })?;
        Ok(Self { bytes, binding })
    }

    /// Internal fixture constructor for unit tests only.
    #[cfg(test)]
    pub(crate) fn for_tests(bytes: Vec<u8>) -> Result<Self, SupervisorError> {
        if bytes.is_empty() || bytes.len() > MAX_SECRET_BYTES {
            return Err(SupervisorError::SecretLeaseInvalid);
        }
        let binding = SecretLeaseBinding {
            secret_reference_digest: Blake3Digest32::from_bytes([0xF1; 32]),
            installation_incarnation_id: InstallationIncarnationId::from_bytes([0xF2; 16]),
            purpose_digest: Blake3Digest32::from_bytes([0xF3; 32]),
            expires_at_tick: NonZeroU64::new(u64::MAX).expect("nonzero"),
        };
        Ok(Self { bytes, binding })
    }

    /// Length of the held material, without exposing it.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Always false after construction; provided for API completeness.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Content-free lease binding for launch/readiness receipt equality.
    #[must_use]
    pub const fn binding(&self) -> SecretLeaseBinding {
        self.binding
    }

    /// Crate-internal one-time exposure for environment/header injection.
    pub(crate) fn secret_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Requires visible-ASCII bytes so the key can traverse the HTTP
    /// `api-key` header without mangling or smuggling control bytes.
    pub fn validate_header_safe(&self) -> Result<(), SupervisorError> {
        let safe = self
            .bytes
            .iter()
            .all(|byte| (0x21_u8..=0x7E_u8).contains(byte));
        if safe {
            Ok(())
        } else {
            Err(SupervisorError::SecretLeaseInvalid)
        }
    }

    /// Side-channel audit: does `haystack` contain these exact bytes?
    /// Used by tests to prove argv, config files, and debug output stay clean.
    #[must_use]
    pub fn appears_in(&self, haystack: &str) -> bool {
        if self.bytes.is_empty() {
            return false;
        }
        haystack
            .as_bytes()
            .windows(self.bytes.len())
            .any(|window| window == self.bytes.as_slice())
    }
}

impl fmt::Debug for SecretMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretMaterial")
            .field("len", &self.bytes.len())
            .field("redacted", &true)
            .finish()
    }
}

impl Drop for SecretMaterial {
    fn drop(&mut self) {
        self.bytes.fill(0);
        core::hint::black_box(self.bytes.as_mut_ptr());
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_SECRET_BYTES, SecretMaterial};
    use crate::SupervisorError;

    #[test]
    fn debug_output_stays_redacted() {
        let secret = SecretMaterial::for_tests(b"audit-key-123".to_vec()).unwrap();
        let rendered = format!("{secret:?}");
        assert!(!secret.appears_in(&rendered));
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn empty_and_oversized_material_is_refused() {
        assert_eq!(
            SecretMaterial::for_tests(Vec::new()).unwrap_err(),
            SupervisorError::SecretLeaseInvalid
        );
        assert_eq!(
            SecretMaterial::for_tests(vec![b'x'; MAX_SECRET_BYTES + 1]).unwrap_err(),
            SupervisorError::SecretLeaseInvalid
        );
    }

    #[test]
    fn header_safety_rejects_control_and_non_ascii_bytes() {
        let plain = SecretMaterial::for_tests(b"Qdrant-Key_9.z~".to_vec()).unwrap();
        assert!(plain.validate_header_safe().is_ok());
        for bad in [
            b"has space".to_vec(),
            b"tab\there".to_vec(),
            vec![0xC3, 0xA9],
        ] {
            let material = SecretMaterial::for_tests(bad).unwrap();
            assert_eq!(
                material.validate_header_safe().unwrap_err(),
                SupervisorError::SecretLeaseInvalid
            );
        }
    }
}
