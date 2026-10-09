//! Real algorithm execution over the sole canonical encoder or bounded raw bytes.
//!
//! The preimage is exactly `domain || 0 || payload`. CBOR and raw-byte domains
//! are disjoint. These primitives do not issue operation receipts or authorize
//! caller data; each semantic owner must freeze its schema and domain together.

use crate::canonical::stream_canonical_cbor;
use crate::{
    Blake3Digest32, CanonicalValue, ContractError, ContractErrorKind, MAX_CANONICAL_BYTES,
    Sha256Digest32,
};
use sha2::{Digest, Sha256};

/// Existing RFC 8949 section 4.2.3 length-first deterministic CBOR profile.
pub const CANONICAL_CBOR_PROFILE: &str = "eliot.cbor.length-first.v1";
pub const MAX_DIGEST_DOMAIN_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Representation {
    Cbor,
    Raw,
}

/// Bounded schema/revision name: `eliot/{cbor|raw}/<schema segments>/vN`.
///
/// Segments contain lowercase ASCII letters/digits and internal single hyphens,
/// start with a letter and end with a letter/digit. `N` is a nonzero decimal
/// revision without leading zeroes. Schema owners freeze meanings in their
/// golden fixtures; parsing a name does not grant semantic authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDigestDomain {
    name: String,
    representation: Representation,
}

impl CanonicalDigestDomain {
    pub fn parse(name: &str) -> Result<Self, ContractError> {
        if name.len() > MAX_DIGEST_DOMAIN_BYTES {
            return Err(ContractError::oversize(
                "digest_domain",
                MAX_DIGEST_DOMAIN_BYTES,
                name.len(),
            ));
        }
        let mut segments = name.split('/');
        if segments.next() != Some("eliot") {
            return Err(invalid_domain());
        }
        let representation = match segments.next() {
            Some("cbor") => Representation::Cbor,
            Some("raw") => Representation::Raw,
            _ => return Err(invalid_domain()),
        };
        let mut tail = segments.peekable();
        let mut schema_segments = 0_usize;
        while let Some(segment) = tail.next() {
            if tail.peek().is_none() {
                let Some(revision) = segment.strip_prefix('v') else {
                    return Err(invalid_domain());
                };
                if schema_segments == 0
                    || revision.is_empty()
                    || revision.starts_with('0')
                    || !revision.bytes().all(|byte| byte.is_ascii_digit())
                    || revision.parse::<u32>().is_err()
                {
                    return Err(invalid_domain());
                }
            } else {
                let bytes = segment.as_bytes();
                if !bytes.first().is_some_and(u8::is_ascii_lowercase)
                    || !bytes.last().is_some_and(u8::is_ascii_alphanumeric)
                    || segment.contains("--")
                    || !bytes.iter().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-'
                    })
                {
                    return Err(invalid_domain());
                }
                schema_segments += 1;
            }
        }
        if schema_segments == 0 {
            return Err(invalid_domain());
        }
        Ok(Self {
            name: name.into(),
            representation,
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.name
    }

    fn payload_limit(
        &self,
        representation: Representation,
        limit: DigestInputLimit,
    ) -> Result<usize, ContractError> {
        if self.representation != representation {
            return Err(ContractError::new(
                ContractErrorKind::InvalidTaggedVariant,
                "digest_representation",
            ));
        }
        limit
            .0
            .checked_sub(self.name.len() + 1)
            .ok_or_else(|| ContractError::oversize("digest_preimage", limit.0, self.name.len() + 1))
    }
}

fn invalid_domain() -> ContractError {
    ContractError::new(ContractErrorKind::InvalidCharacter, "digest_domain")
}

/// Finite ceiling for the complete preimage, including the domain and separator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DigestInputLimit(usize);

impl DigestInputLimit {
    pub fn new(max_bytes: usize) -> Result<Self, ContractError> {
        if max_bytes == 0 {
            return Err(ContractError::new(
                ContractErrorKind::ZeroNotAllowed,
                "digest_limit",
            ));
        }
        if max_bytes > MAX_CANONICAL_BYTES {
            return Err(ContractError::oversize(
                "digest_limit",
                MAX_CANONICAL_BYTES,
                max_bytes,
            ));
        }
        Ok(Self(max_bytes))
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Hash canonical CBOR without retaining another payload or complete preimage.
pub fn blake3_canonical(
    domain: &CanonicalDigestDomain,
    value: &CanonicalValue,
    limit: DigestInputLimit,
) -> Result<Blake3Digest32, ContractError> {
    let payload_limit = domain.payload_limit(Representation::Cbor, limit)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain.name.as_bytes());
    hasher.update(&[0]);
    stream_canonical_cbor(value, payload_limit, |bytes| {
        hasher.update(bytes);
        Ok(())
    })?;
    Ok(Blake3Digest32::from_computed_bytes(
        *hasher.finalize().as_bytes(),
    ))
}

/// SHA-256 over the same canonical CBOR profile and complete-preimage ceiling.
pub fn sha256_canonical(
    domain: &CanonicalDigestDomain,
    value: &CanonicalValue,
    limit: DigestInputLimit,
) -> Result<Sha256Digest32, ContractError> {
    let payload_limit = domain.payload_limit(Representation::Cbor, limit)?;
    let mut hasher = Sha256::new();
    hasher.update(domain.name.as_bytes());
    hasher.update([0]);
    stream_canonical_cbor(value, payload_limit, |bytes| {
        hasher.update(bytes);
        Ok(())
    })?;
    Ok(Sha256Digest32::from_computed_bytes(
        hasher.finalize().into(),
    ))
}

/// Hash exact raw bytes under a separate versioned raw-byte schema domain.
pub fn blake3_raw(
    domain: &CanonicalDigestDomain,
    bytes: &[u8],
    limit: DigestInputLimit,
) -> Result<Blake3Digest32, ContractError> {
    check_raw(domain, bytes, limit)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain.name.as_bytes());
    hasher.update(&[0]);
    hasher.update(bytes);
    Ok(Blake3Digest32::from_computed_bytes(
        *hasher.finalize().as_bytes(),
    ))
}

/// SHA-256 of exact raw bytes; input is checked before any hashing begins.
pub fn sha256_raw(
    domain: &CanonicalDigestDomain,
    bytes: &[u8],
    limit: DigestInputLimit,
) -> Result<Sha256Digest32, ContractError> {
    check_raw(domain, bytes, limit)?;
    let mut hasher = Sha256::new();
    hasher.update(domain.name.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    Ok(Sha256Digest32::from_computed_bytes(
        hasher.finalize().into(),
    ))
}

fn check_raw(
    domain: &CanonicalDigestDomain,
    bytes: &[u8],
    limit: DigestInputLimit,
) -> Result<(), ContractError> {
    let payload_limit = domain.payload_limit(Representation::Raw, limit)?;
    if bytes.len() > payload_limit {
        return Err(ContractError::oversize(
            "digest_raw_payload",
            payload_limit,
            bytes.len(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algorithm_standard_vectors_are_real() {
        // Official BLAKE3 empty-input and SHA-256 empty/abc vectors. These are
        // algorithm oracles, deliberately separate from domain-bound product APIs.
        assert_eq!(
            blake3::hash(b"").to_hex().as_str(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        assert_eq!(
            crate::hex_encode(&Sha256::digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            crate::hex_encode(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
