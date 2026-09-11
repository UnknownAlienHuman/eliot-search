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
pub fn collection_name(route: &CollectionRoute) -> Result<String, BridgeError> {
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

fn hex_from_32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('?'));
        out.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('?'));
    }
    out
}

const fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hex_to_32(text: &str) -> Result<[u8; 32], BridgeError> {
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return Err(BridgeError::MalformedResponse);
    }
    let mut out = [0_u8; 32];
    for (index, pair) in bytes.chunks(2).enumerate() {
        let high = hex_val(pair[0]).ok_or(BridgeError::MalformedResponse)?;
        let low = hex_val(pair[1]).ok_or(BridgeError::MalformedResponse)?;
        out[index] = (high << 4) | low;
    }
    Ok(out)
}

/// Provider-neutral 128-bit point ID rendered as a lowercase UUID string for
/// the vendor transport (the full 128 bits survive the round trip).
fn uuid_string(id: &QdrantPointId) -> String {
    let mut hex = String::with_capacity(32);
    for byte in id.0 {
        hex.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('?'));
        hex.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('?'));
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn parse_uuid(text: &str) -> Result<QdrantPointId, BridgeError> {
    let bytes = text.as_bytes();
    if bytes.len() != 36 {
        return Err(BridgeError::MalformedResponse);
    }
    for dash in [8, 13, 18, 23] {
        if bytes[dash] != b'-' {
            return Err(BridgeError::MalformedResponse);
        }
    }
    let mut compact = [0_u8; 32];
    let mut next = 0_usize;
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            continue;
        }
        compact[next] = *byte;
        next += 1;
    }
    let mut out = [0_u8; 16];
    for (index, pair) in compact.chunks(2).enumerate() {
        let high = hex_val(pair[0]).ok_or(BridgeError::MalformedResponse)?;
        let low = hex_val(pair[1]).ok_or(BridgeError::MalformedResponse)?;
        out[index] = (high << 4) | low;
    }
    Ok(QdrantPointId(out))
}

fn vendor_point_id(id: &QdrantPointId) -> PointId {
    PointId {
        point_id_options: Some(point_id::PointIdOptions::Uuid(uuid_string(id))),
    }
}

fn bridge_point_id(id: &PointId) -> Result<QdrantPointId, BridgeError> {
    match id.point_id_options.as_ref() {
        Some(point_id::PointIdOptions::Num(number)) => {
            let mut bytes = [0_u8; 16];
            bytes[8..16].copy_from_slice(&number.to_be_bytes());
            Ok(QdrantPointId(bytes))
        }
        Some(point_id::PointIdOptions::Uuid(text)) => parse_uuid(text),
        None => Err(BridgeError::MalformedResponse),
    }
}
