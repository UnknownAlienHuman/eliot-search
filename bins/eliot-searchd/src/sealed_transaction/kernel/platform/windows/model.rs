//! Exact internal Windows intent and receipt records.

use crate::sealed_digest::Sha256Digest;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Intent {
    pub(super) operation_id: String,
    pub(super) object_id: String,
    pub(super) plaintext_bytes: u64,
    pub(super) plaintext_sha256: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Receipt {
    pub(super) operation_id: String,
    pub(super) object_id: String,
    pub(super) plaintext_bytes: u64,
    pub(super) plaintext_sha256: Sha256Digest,
    pub(super) ciphertext_bytes: u64,
}
