//! Namespace-bound protector model and platform credential opening.

use core::fmt;
use std::path::Path;

use search_os_secrets::{
    LEGACY_REVISION_PROTECTED_OBJECT_EXTENSION,
    legacy_revision_is_protected_object,
};
#[cfg(windows)]
use search_os_secrets::{
    derive_legacy_revision_dpapi_entropy,
    derive_legacy_revision_key_binding,
};
#[cfg(windows)]
use zeroize::Zeroize;

#[cfg(windows)]
use super::super::envelope::DirectRevisionDigest;
#[cfg(windows)]
use super::super::windows;
use crate::sha256;

/// Legacy protected revision filename extension retained for call-site parity.
pub const PROTECTED_OBJECT_EXTENSION: &str =
    LEGACY_REVISION_PROTECTED_OBJECT_EXTENSION;

/// Per-namespace revision protection capability.
pub struct RevisionProtector {
    pub(in crate::revision_protection) namespace_id: [u8; 32],
    #[cfg(windows)]
    pub(in crate::revision_protection) key_binding_digest: [u8; 32],
    #[cfg(windows)]
    pub(in crate::revision_protection) entropy: [u8; 32],
}

impl fmt::Debug for RevisionProtector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("RevisionProtector");
        debug
            .field("namespace_id", &sha256::hex(&self.namespace_id))
            .field("backend", &self.backend_name())
            .field("secret", &"<redacted>");
        #[cfg(windows)]
        {
            debug
                .field("key_binding_digest", &"<redacted>")
                .field("entropy", &"<redacted>");
        }
        debug.finish()
    }
}

#[cfg(windows)]
impl Drop for RevisionProtector {
    fn drop(&mut self) {
        self.entropy.zeroize();
    }
}

impl RevisionProtector {
    /// Opens the platform protector. Existing protected objects require the
    /// original credential; a missing credential is not silently replaced.
    pub(crate) fn open(
        namespace_id: [u8; 32],
        revision_root: &Path,
    ) -> Result<Self, String> {
        #[cfg(windows)]
        {
            let root_secret =
                windows::load_or_create_root_secret(namespace_id, revision_root)?;
            let key_binding_digest =
                derive_legacy_revision_key_binding::<DirectRevisionDigest>(
                    &namespace_id,
                    root_secret.expose_secret(),
                );
            let entropy =
                derive_legacy_revision_dpapi_entropy::<DirectRevisionDigest>(
                    &namespace_id,
                    root_secret.expose_secret(),
                );
            Ok(Self {
                namespace_id,
                key_binding_digest,
                entropy,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = revision_root;
            Ok(Self { namespace_id })
        }
    }

    #[must_use]
    pub(crate) const fn backend_name(&self) -> &'static str {
        #[cfg(windows)]
        {
            "windows-dpapi-credential-manager-v1"
        }
        #[cfg(not(windows))]
        {
            "plaintext-development-v1"
        }
    }

    #[must_use]
    pub(crate) const fn encrypts_new_objects(&self) -> bool {
        cfg!(windows)
    }

    /// Format marker only, not proof that an object authenticates or decrypts.
    #[must_use]
    pub(crate) fn is_protected_object(bytes: &[u8]) -> bool {
        legacy_revision_is_protected_object(bytes)
    }
}
