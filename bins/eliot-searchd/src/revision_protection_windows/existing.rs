//! Read-only existing credential binding for explicit migration.

use search_os_secrets::{
    derive_legacy_revision_dpapi_entropy,
    derive_legacy_revision_key_binding,
};

use super::super::envelope::DirectRevisionDigest;
use super::credential::load_existing_root_secret;

impl super::super::RevisionProtector {
    /// Resolve only the existing namespace credential for explicit migration.
    /// Missing keys stay missing: no RNG, credential write, revision scan or conversion.
    pub(crate) fn open_existing(
        namespace_id: [u8; 32],
    ) -> Result<Option<Self>, String> {
        let Some(secret) = load_existing_root_secret(namespace_id)? else {
            return Ok(None);
        };
        let key_binding_digest =
            derive_legacy_revision_key_binding::<DirectRevisionDigest>(
                &namespace_id,
                secret.expose_secret(),
            );
        let entropy =
            derive_legacy_revision_dpapi_entropy::<DirectRevisionDigest>(
                &namespace_id,
                secret.expose_secret(),
            );
        Ok(Some(Self {
            namespace_id,
            key_binding_digest,
            entropy,
        }))
    }
}
