//! Windows-only composition of the pure envelope with native DPAPI.

use search_os_secrets::{
    LegacyRevisionBinding, LegacyRevisionExpected,
    encode_legacy_revision_inner, encode_legacy_revision_outer,
};
use zeroize::Zeroizing;

use super::RevisionProtector;
use super::super::envelope::envelope_reason;
use super::super::windows as native_windows;

impl RevisionProtector {
    pub(super) fn protect_windows(
        &self,
        revision_id: [u8; 32],
        content_digest: [u8; 32],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, String> {
        let plaintext_len = u64::try_from(plaintext.len())
            .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH".to_owned())?;
        let binding = LegacyRevisionBinding::new(
            LegacyRevisionExpected::new(
                self.namespace_id,
                revision_id,
                content_digest,
                plaintext_len,
            ),
            self.key_binding_digest,
        );
        let mut inner = Zeroizing::new(
            encode_legacy_revision_inner(binding, plaintext)
                .map_err(envelope_reason)?,
        );
        let protected =
            native_windows::protect_data(&mut inner, &self.entropy)?;
        encode_legacy_revision_outer(binding, &protected)
            .map_err(envelope_reason)
    }
}
