//! Shared v3 digest helpers and the frozen legacy DIRECT profile envelope.

/// Computes BLAKE3 over an explicitly framed domain and ordered byte chunks.
///
/// Every length and count uses an unsigned 64-bit little-endian encoding. This
/// framing distinguishes different domain/chunk boundaries before the bytes
/// are passed to BLAKE3. The result is an identity digest, not a raw-content
/// digest; raw source bytes are hashed directly by their owning producer.
pub(super) fn digest32(domain: &[u8], chunks: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(
        &u64::try_from(domain.len())
            .expect("domain length fits the supported 64-bit wire format")
            .to_le_bytes(),
    );
    hasher.update(domain);
    hasher.update(
        &u64::try_from(chunks.len())
            .expect("chunk count fits the supported 64-bit wire format")
            .to_le_bytes(),
    );
    for chunk in chunks {
        hasher.update(
            &u64::try_from(chunk.len())
                .expect("chunk length fits the supported 64-bit wire format")
                .to_le_bytes(),
        );
        hasher.update(chunk);
    }
    *hasher.finalize().as_bytes()
}

// v3 uses only the shared canonical and algorithm owner below.
use crate::UnitizationError;
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, BoundedMap, CanonicalKey, CanonicalText,
    CanonicalValue,
};

pub(super) fn object(
    fields: Vec<(&'static str, CanonicalValue)>,
) -> Result<CanonicalValue, UnitizationError> {
    let fields = fields
        .into_iter()
        .map(|(key, value)| CanonicalKey::new_non_empty(key).map(|key| (key, value)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    BoundedMap::from_entries(fields)
        .map(CanonicalValue::Object)
        .map_err(|_| UnitizationError::InputTooLarge)
}

pub(super) fn text(value: &str) -> Result<CanonicalValue, UnitizationError> {
    CanonicalText::new(value)
        .map(CanonicalValue::Text)
        .map_err(|_| UnitizationError::InputTooLarge)
}

pub(super) fn bytes(value: &[u8]) -> Result<CanonicalValue, UnitizationError> {
    BoundedBytes::new(value.to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| UnitizationError::InputTooLarge)
}

pub(super) fn array(value: Vec<CanonicalValue>) -> Result<CanonicalValue, UnitizationError> {
    BoundedList::new(value)
        .map(CanonicalValue::Array)
        .map_err(|_| UnitizationError::InputTooLarge)
}

pub(super) fn hash_cbor(
    domain: &str,
    value: &CanonicalValue,
    limit: usize,
) -> Result<Blake3Digest32, UnitizationError> {
    let domain = search_contracts::CanonicalDigestDomain::parse(domain)
        .map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let limit = search_contracts::DigestInputLimit::new(limit)
        .map_err(|_| UnitizationError::InvalidLimits)?;
    search_contracts::blake3_canonical(&domain, value, limit)
        .map_err(|_| UnitizationError::InputTooLarge)
}

pub(super) fn hash_raw(
    domain: &str,
    value: &[u8],
    limit: usize,
) -> Result<Blake3Digest32, UnitizationError> {
    let domain = search_contracts::CanonicalDigestDomain::parse(domain)
        .map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let limit = search_contracts::DigestInputLimit::new(limit)
        .map_err(|_| UnitizationError::InvalidLimits)?;
    search_contracts::blake3_raw(&domain, value, limit).map_err(|_| UnitizationError::InputTooLarge)
}

#[cfg(test)]
mod tests {
    use super::digest32;

    #[test]
    fn blake3_dependency_matches_the_published_empty_input_vector() {
        assert_eq!(
            blake3::hash(b"").to_hex().as_str(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

    #[test]
    fn chunk_boundaries_are_part_of_the_v2_hash_input() {
        let domain = b"eliot-search/unitizer/test/v2";
        assert_ne!(
            digest32(domain, &[b"ab", b"c"]),
            digest32(domain, &[b"a", b"bc"])
        );
    }
}
