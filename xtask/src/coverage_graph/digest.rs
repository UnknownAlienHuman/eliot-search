//! Coverage graph digest helper.

/// SHA-256 hex of UTF-8 text bytes.
#[must_use]
pub fn digest_text(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}
