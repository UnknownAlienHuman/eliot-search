//! DIRECT compatibility translation around package-owned legacy secret contracts.

use search_os_secrets::LegacyRevisionEnvelopeError;
#[cfg(windows)]
use search_os_secrets::{
    LegacyRevisionContentDigest, LegacyRevisionKeyDigest,
};

use crate::sha256;

#[cfg(windows)]
pub(super) struct DirectRevisionDigest;

#[cfg(windows)]
impl LegacyRevisionContentDigest for DirectRevisionDigest {
    fn digest(bytes: &[u8]) -> [u8; 32] {
        sha256::digest(bytes)
    }
}

#[cfg(windows)]
impl LegacyRevisionKeyDigest for DirectRevisionDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        sha256::digest_parts(domain, parts)
    }
}

pub(super) fn decode_digest(
    value: &str,
    error: &'static str,
) -> Result<[u8; 32], String> {
    sha256::decode_digest(value).ok_or_else(|| error.to_owned())
}

pub(super) fn envelope_reason(error: LegacyRevisionEnvelopeError) -> String {
    match error {
        LegacyRevisionEnvelopeError::ProtectedPayloadInvalid => {
            "DIRECT_REVISION_OBJECT_TOO_LARGE"
        }
        LegacyRevisionEnvelopeError::PlaintextTooLarge => {
            "DIRECT_REVISION_PLAINTEXT_TOO_LARGE"
        }
        LegacyRevisionEnvelopeError::ProtectedFormatRequired => {
            "DIRECT_REVISION_PROTECTED_FORMAT_REQUIRED"
        }
        LegacyRevisionEnvelopeError::EnvelopeInvalid => {
            "DIRECT_REVISION_ENVELOPE_INVALID"
        }
        LegacyRevisionEnvelopeError::NamespaceMismatch => {
            "DIRECT_REVISION_NAMESPACE_MISMATCH"
        }
        LegacyRevisionEnvelopeError::KeyBindingMismatch => {
            "DIRECT_REVISION_KEY_BINDING_MISMATCH"
        }
        LegacyRevisionEnvelopeError::EnvelopeBindingMismatch => {
            "DIRECT_REVISION_ENVELOPE_BINDING_MISMATCH"
        }
        LegacyRevisionEnvelopeError::InnerEnvelopeInvalid => {
            "DIRECT_REVISION_INNER_ENVELOPE_INVALID"
        }
        LegacyRevisionEnvelopeError::InnerBindingMismatch => {
            "DIRECT_REVISION_INNER_BINDING_MISMATCH"
        }
        LegacyRevisionEnvelopeError::LengthMismatch => {
            "DIRECT_REVISION_LENGTH_MISMATCH"
        }
        LegacyRevisionEnvelopeError::ContentMismatch => {
            "DIRECT_REVISION_CONTENT_MISMATCH"
        }
    }
    .to_owned()
}
