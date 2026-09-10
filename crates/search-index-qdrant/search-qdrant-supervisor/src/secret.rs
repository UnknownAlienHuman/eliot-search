//! Lease-bound secret material for the Qdrant API key.
//!
//! Plaintext exists only inside [`SecretMaterial`]: a non-clone, Debug-
//! redacted owner that zeroes its bytes on drop (best-effort memory hygiene
//! in safe Rust, not a secure-erasure claim). Material never enters argv,
//! config files, logs, or receipts; the only sink is the child process
//! environment block plus loopback HTTP probe headers, both provided by the
//! execution layer in this same package.

use core::fmt;

use crate::SupervisorError;

/// Maximum accepted secret size, aligned with the secret catalog limit.
pub const MAX_SECRET_BYTES: usize = 1_048_576;

/// Owned plaintext bound to one launch. Never cloned, never logged.
pub struct SecretMaterial {
    bytes: Vec<u8>,
}

impl SecretMaterial {
    /// Takes ownership of lease plaintext after finite-size validation.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, SupervisorError> {
        if bytes.is_empty() || bytes.len() > MAX_SECRET_BYTES {
            return Err(SupervisorError::SecretLeaseInvalid);
        }
        Ok(Self { bytes })
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
        let secret = SecretMaterial::from_bytes(b"audit-key-123".to_vec()).unwrap();
        let rendered = format!("{secret:?}");
        assert!(!secret.appears_in(&rendered));
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn empty_and_oversized_material_is_refused() {
        assert_eq!(
            SecretMaterial::from_bytes(Vec::new()).unwrap_err(),
            SupervisorError::SecretLeaseInvalid
        );
        assert_eq!(
            SecretMaterial::from_bytes(vec![b'x'; MAX_SECRET_BYTES + 1]).unwrap_err(),
            SupervisorError::SecretLeaseInvalid
        );
    }

    #[test]
    fn header_safety_rejects_control_and_non_ascii_bytes() {
        let plain = SecretMaterial::from_bytes(b"Qdrant-Key_9.z~".to_vec()).unwrap();
        assert!(plain.validate_header_safe().is_ok());
        for bad in [
            b"has space".to_vec(),
            b"tab\there".to_vec(),
            vec![0xC3, 0xA9],
        ] {
            let material = SecretMaterial::from_bytes(bad).unwrap();
            assert_eq!(
                material.validate_header_safe().unwrap_err(),
                SupervisorError::SecretLeaseInvalid
            );
        }
    }
}
