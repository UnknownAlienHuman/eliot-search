//! Read-only existing credential binding for explicit migration.

use search_os_secrets::{
    derive_legacy_revision_dpapi_entropy,
    derive_legacy_revision_key_binding,
};

use crate::sha256;

use super::super::envelope::DirectRevisionDigest;
use super::credential::read_credential;
use super::ffi::wide;

impl super::super::RevisionProtector {
    /// Resolve only the existing namespace credential for explicit migration.
    /// Missing keys stay missing: no RNG, credential write, revision scan or conversion.
    pub(crate) fn open_existing(
        namespace_id: [u8; 32],
    ) -> Result<Option<Self>, String> {
        let target = wide(&format!(
            "ELIOT Search/revision-key/{}",
            sha256::hex(&namespace_id),
        ));
        let Some(secret) = read_credential(&target)? else {
            return Ok(None);
        };
        let secret = zeroize::Zeroizing::new(secret);
        let key_binding_digest =
            derive_legacy_revision_key_binding::<DirectRevisionDigest>(
                &namespace_id,
                &secret,
            );
        let entropy =
            derive_legacy_revision_dpapi_entropy::<DirectRevisionDigest>(
                &namespace_id,
                &secret,
            );
        Ok(Some(Self {
            namespace_id,
            key_binding_digest,
            entropy,
        }))
    }
}
