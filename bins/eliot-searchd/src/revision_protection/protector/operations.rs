//! Cross-platform validation and composition around protected bytes.

use search_os_secrets::{
    LEGACY_REVISION_MAX_PLAINTEXT_BYTES,
    LEGACY_REVISION_MAX_PLAINTEXT_LENGTH,
    LEGACY_REVISION_MAX_PROTECTED_OBJECT_BYTES,
    LegacyRevisionExpected, decode_legacy_revision_outer,
};
#[cfg(windows)]
use search_os_secrets::decode_legacy_revision_inner;
#[cfg(windows)]
use zeroize::Zeroizing;

use super::RevisionProtector;
use super::super::envelope::{decode_digest, envelope_reason};
#[cfg(windows)]
use super::super::envelope::DirectRevisionDigest;
#[cfg(windows)]
use super::super::windows as native_windows;
use crate::sha256;

const MAX_PLAINTEXT_BYTES: usize = LEGACY_REVISION_MAX_PLAINTEXT_BYTES;
const MAX_PROTECTED_OBJECT_BYTES: usize =
    LEGACY_REVISION_MAX_PROTECTED_OBJECT_BYTES;

impl RevisionProtector {
    pub(crate) fn protect(
        &self,
        revision_id: &str,
        content_digest: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, String> {
        let revision_id =
            decode_digest(revision_id, "DIRECT_REVISION_ID_INVALID")?;
        let content_digest =
            decode_digest(content_digest, "DIRECT_CONTENT_DIGEST_INVALID")?;
        if plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err("DIRECT_REVISION_PLAINTEXT_TOO_LARGE".to_owned());
        }
        // Validate before invoking native encryption, including the explicit
        // development branch. A caller's digest is not evidence by itself.
        if sha256::digest(plaintext) != content_digest {
            return Err("DIRECT_REVISION_CONTENT_MISMATCH".to_owned());
        }
        #[cfg(windows)]
        {
            self.protect_windows(revision_id, content_digest, plaintext)
        }
        #[cfg(not(windows))]
        {
            let _ = revision_id;
            Ok(plaintext.to_vec())
        }
    }

    /// Opens a protected object only. There is deliberately no plaintext
    /// fallback, even if raw bytes have the expected digest and length.
    pub(crate) fn unprotect(
        &self,
        object: &[u8],
        revision_id: &str,
        content_digest: &str,
        expected_plaintext_len: u64,
    ) -> Result<Vec<u8>, String> {
        if object.len() > MAX_PROTECTED_OBJECT_BYTES {
            return Err("DIRECT_REVISION_OBJECT_TOO_LARGE".to_owned());
        }
        if expected_plaintext_len > LEGACY_REVISION_MAX_PLAINTEXT_LENGTH {
            return Err("DIRECT_REVISION_PLAINTEXT_TOO_LARGE".to_owned());
        }
        let revision_id =
            decode_digest(revision_id, "DIRECT_REVISION_ID_INVALID")?;
        let content_digest =
            decode_digest(content_digest, "DIRECT_CONTENT_DIGEST_INVALID")?;
        if !Self::is_protected_object(object) {
            return Err(
                "DIRECT_REVISION_PROTECTED_FORMAT_REQUIRED".to_owned(),
            );
        }
        #[cfg(windows)]
        let expected_key = Some(self.key_binding_digest);
        #[cfg(not(windows))]
        let expected_key = None;
        let expected = LegacyRevisionExpected::new(
            self.namespace_id,
            revision_id,
            content_digest,
            expected_plaintext_len,
        );
        let (binding, ciphertext) =
            decode_legacy_revision_outer(object, expected, expected_key)
                .map_err(envelope_reason)?;
        #[cfg(windows)]
        {
            let mut ciphertext = ciphertext.to_vec();
            let inner = Zeroizing::new(native_windows::unprotect_data(
                &mut ciphertext,
                &self.entropy,
            )?);
            // Validate the borrowed plaintext before allocating a return value;
            // every rejected inner envelope stays inside the zeroizing owner.
            Ok(
                decode_legacy_revision_inner::<DirectRevisionDigest>(
                    &inner, binding,
                )
                .map_err(envelope_reason)?
                .to_vec(),
            )
        }
        #[cfg(not(windows))]
        {
            let _ = (binding, ciphertext);
            Err("DIRECT_REVISION_ENCRYPTION_UNAVAILABLE".to_owned())
        }
    }
}
