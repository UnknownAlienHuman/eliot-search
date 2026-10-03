//! Closed digest-text validation shared by DIRECT store owners.

use crate::sha256;

pub(super) fn validate_digest_text(value: &str, error: &'static str) -> Result<(), String> {
    if sha256::decode_digest(value).is_some() {
        Ok(())
    } else {
        Err(error.to_owned())
    }
}
