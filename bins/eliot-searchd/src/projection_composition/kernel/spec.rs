//! Closed persisted-format and digest-domain constants.

/// Magic prefix of every persisted projection reference record.
pub(super) const REFERENCE_MAGIC: &[u8; 8] = b"ELSPRJ01";
/// Exact byte length of one serialized projection reference.
pub(super) const REFERENCE_BYTES: usize = 8 + 32 + 32 + 8 + 8;
/// File extension of immutable CAS manifest objects.
pub(super) const MANIFEST_EXTENSION: &str = "pman";
/// Domain tag framing the typed scope-binding key preimage.
pub(super) const SCOPE_KEY_DOMAIN: &[u8] = b"eliot-search/projection-scope/v2";
