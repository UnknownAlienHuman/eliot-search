/// Validates a Qdrant collection name without touching the network.
///
/// Exactly `1..=128` ASCII characters from `[A-Za-z0-9_.-]`. Anything else is
/// rejected pre-dispatch so an opaque physical name can never become a
/// vendor-side surprise.
pub fn validate_collection_name(name: &str) -> Result<(), BridgeError> {
    if name.is_empty() || name.len() > 128 {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    if name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        Ok(())
    } else {
        Err(BridgeError::CollectionSchemaMismatch)
    }
}

/// Deterministic vendor collection name for one exact route.
///
/// Fixed 28-byte form `t24c` + 24 lowercase hex digits (96 bits of SHA-256
/// over a domain-separated physical-name/generation preimage). Both route
/// halves are bound in: the same physical name at a different generation
/// addresses a different collection, so a wrong generation reads as
/// [`BridgeError::CollectionNotFound`], never as another generation's data.
///
/// Fixed length is load-bearing on Windows: the native server stores payload
/// indexes under deep per-collection gridstore paths, and names around 40
/// bytes already fail index creation on disposable temp storage with a
/// server-side `Internal` error. Any operator can recompute this name from
/// the route with this function; it carries no secrets.
pub fn collection_name(
    route: &CollectionRoute,
) -> Result<String, BridgeError> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"eliot-qdrant-collection/v1\x00");
    hasher.update(route.physical_name.as_str().as_bytes());
    hasher.update(b"\x00");
    hasher.update(route.generation.as_bytes());
    let digest = hasher.finalize();
    let mut name = String::with_capacity(28);
    name.push_str("t24c");
    for byte in digest.iter().take(12) {
        name.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('?'));
        name.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('?'));
    }
    validate_collection_name(&name)?;
    Ok(name)
}
