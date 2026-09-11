//! Compatibility identity for the legacy DIRECT journal.
//!
//! The legacy plaintext journal predates the structured filesystem identity
//! model. It stores one already-qualified stable-identity digest. This module
//! owns the pure resolution and deterministic identifier formulas for that
//! format while the daemon owns observation capture and filesystem I/O.

use core::fmt;
use std::collections::BTreeSet;

/// Canonical legacy DIRECT source identifier domain.
pub const LEGACY_DIRECT_SOURCE_ID_DOMAIN: &[u8] = b"eliot-search/direct-source-id/v1";
/// Canonical legacy DIRECT revision identifier domain.
pub const LEGACY_DIRECT_REVISION_ID_DOMAIN: &[u8] = b"eliot-search/direct-revision-id/v1";
/// Maximum candidates inspected by one compatibility resolution.
pub const MAX_LEGACY_DIGEST_CANDIDATES: usize = 100_000;

/// Qualified digest primitive supplied by the integration boundary.
///
/// The identity package owns framing and semantics but does not add another
/// cryptographic implementation or dependency.
pub trait LegacyIdentityDigest {
    /// Domain-separated digest over ordered, length-prefixed parts.
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32];
}

/// Prior durable source plus its exact legacy stable-identity digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyDigestPriorIdentity {
    /// Existing durable source identifier as lowercase SHA-256 text.
    pub source_id: String,
    /// Exact stable identity digest stored by the legacy journal.
    pub stable_identity_digest: String,
}

/// Exact compatibility resolution outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyDigestIdentityResolution {
    /// One exact prior stable identity matched.
    MatchExisting {
        /// Existing durable identifier to preserve byte-for-byte.
        source_id: String,
    },
    /// Exact stable evidence is unseen and may produce a new identifier.
    CreateNew,
    /// Stable evidence is unavailable; path evidence never substitutes.
    Ambiguous,
}

/// Closed compatibility failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyDigestIdentityError {
    /// Stable identity evidence is malformed, unavailable, or over budget.
    Ambiguous,
    /// Existing durable identity is malformed or inconsistent.
    Conflict,
    /// One stable identity maps to multiple durable source identifiers.
    Collision,
    /// Content digest input is malformed.
    ContentDigestInvalid,
    /// Durable source identifier input is malformed.
    SourceIdInvalid,
}

impl LegacyDigestIdentityError {
    /// Stable daemon-compatible reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ambiguous => "SOURCE_IDENTITY_AMBIGUOUS",
            Self::Conflict => "SOURCE_IDENTITY_CONFLICT",
            Self::Collision => "SOURCE_IDENTITY_COLLISION",
            Self::ContentDigestInvalid => "DIRECT_CONTENT_DIGEST_INVALID",
            Self::SourceIdInvalid => "DIRECT_SOURCE_ID_INVALID",
        }
    }
}

impl fmt::Display for LegacyDigestIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LegacyDigestIdentityError {}

/// Resolves one legacy stable digest against finite prior identities.
///
/// `identity_strength` must be exactly `native`; path-bound evidence remains
/// ambiguous and never creates or matches a durable source. The function does
/// not read files, generate randomness, or mutate registry state.
///
/// # Errors
///
/// Malformed evidence, excessive candidates, malformed prior identities, or
/// collisions return a closed [`LegacyDigestIdentityError`].
pub fn resolve_legacy_digest_identity(
    stable_identity_digest: &str,
    identity_strength: &str,
    prior: &[LegacyDigestPriorIdentity],
) -> Result<LegacyDigestIdentityResolution, LegacyDigestIdentityError> {
    if decode_digest(stable_identity_digest).is_none() {
        return Err(LegacyDigestIdentityError::Ambiguous);
    }
    if identity_strength != "native" {
        return Ok(LegacyDigestIdentityResolution::Ambiguous);
    }
    if prior.len() > MAX_LEGACY_DIGEST_CANDIDATES {
        return Err(LegacyDigestIdentityError::Ambiguous);
    }

    let mut matches = BTreeSet::new();
    for candidate in prior {
        if decode_digest(&candidate.source_id).is_none()
            || decode_digest(&candidate.stable_identity_digest).is_none()
        {
            return Err(LegacyDigestIdentityError::Conflict);
        }
        if candidate.stable_identity_digest == stable_identity_digest {
            matches.insert(candidate.source_id.clone());
        }
    }
    match matches.len() {
        0 => Ok(LegacyDigestIdentityResolution::CreateNew),
        1 => Ok(LegacyDigestIdentityResolution::MatchExisting {
            source_id: matches.into_iter().next().expect("one exact match"),
        }),
        _ => Err(LegacyDigestIdentityError::Collision),
    }
}

/// Derives or preserves the durable legacy source identifier.
///
/// # Errors
///
/// Malformed namespace/evidence, an invalid existing identifier, or an
/// ambiguous resolution fails closed.
pub fn derive_legacy_digest_source_id<D: LegacyIdentityDigest>(
    namespace_digest: &str,
    stable_identity_digest: &str,
    resolution: &LegacyDigestIdentityResolution,
) -> Result<String, LegacyDigestIdentityError> {
    match resolution {
        LegacyDigestIdentityResolution::MatchExisting { source_id } => {
            if decode_digest(source_id).is_none() {
                return Err(LegacyDigestIdentityError::Conflict);
            }
            Ok(source_id.clone())
        }
        LegacyDigestIdentityResolution::CreateNew => {
            let namespace = decode_digest(namespace_digest)
                .ok_or(LegacyDigestIdentityError::Ambiguous)?;
            let stable = decode_digest(stable_identity_digest)
                .ok_or(LegacyDigestIdentityError::Ambiguous)?;
            Ok(hex(&D::digest_parts(
                LEGACY_DIRECT_SOURCE_ID_DOMAIN,
                &[&namespace, &stable],
            )))
        }
        LegacyDigestIdentityResolution::Ambiguous => {
            Err(LegacyDigestIdentityError::Ambiguous)
        }
    }
}

/// Derives the immutable legacy revision identifier.
///
/// The source identifier is framed as canonical lowercase text to preserve the
/// deployed DIRECT wire formula. Content contributes decoded digest bytes and
/// length contributes big-endian `u64` bytes.
///
/// # Errors
///
/// Malformed source or content digest input fails closed.
pub fn derive_legacy_digest_revision_id<D: LegacyIdentityDigest>(
    source_id: &str,
    content_digest: &str,
    byte_length: u64,
) -> Result<String, LegacyDigestIdentityError> {
    if decode_digest(source_id).is_none() {
        return Err(LegacyDigestIdentityError::SourceIdInvalid);
    }
    let content = decode_digest(content_digest)
        .ok_or(LegacyDigestIdentityError::ContentDigestInvalid)?;
    Ok(hex(&D::digest_parts(
        LEGACY_DIRECT_REVISION_ID_DOMAIN,
        &[source_id.as_bytes(), &content, &byte_length.to_be_bytes()],
    )))
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for &byte in bytes {
        output.push(char::from(TABLE[usize::from(byte >> 4)]));
        output.push(char::from(TABLE[usize::from(byte & 0x0f)]));
    }
    output
}

fn decode_digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        output[index] = (high << 4) | low;
    }
    Some(output)
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ToyDigest;

    impl LegacyIdentityDigest for ToyDigest {
        fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
            let mut output = [0_u8; 32];
            for (index, byte) in domain
                .iter()
                .chain(parts.iter().flat_map(|part| part.iter()))
                .enumerate()
            {
                let slot = index % output.len();
                output[slot] = output[slot]
                    .wrapping_add(*byte)
                    .rotate_left(u32::try_from(index % 8).expect("rotation below eight"));
            }
            output
        }
    }

    fn prior(source_fill: &str, stable_fill: &str) -> LegacyDigestPriorIdentity {
        LegacyDigestPriorIdentity {
            source_id: source_fill.repeat(64),
            stable_identity_digest: stable_fill.repeat(64),
        }
    }

    #[test]
    fn stable_identity_matches_and_path_bound_never_matches() {
        let candidate = prior("1", "2");
        assert_eq!(
            resolve_legacy_digest_identity(&"2".repeat(64), "native", &[candidate.clone()]),
            Ok(LegacyDigestIdentityResolution::MatchExisting {
                source_id: candidate.source_id,
            })
        );
        assert_eq!(
            resolve_legacy_digest_identity(&"2".repeat(64), "path-bound", &[candidate]),
            Ok(LegacyDigestIdentityResolution::Ambiguous)
        );
    }

    #[test]
    fn collisions_fail_before_identifier_creation() {
        let candidates = vec![prior("1", "3"), prior("2", "3")];
        assert_eq!(
            resolve_legacy_digest_identity(&"3".repeat(64), "native", &candidates),
            Err(LegacyDigestIdentityError::Collision)
        );
    }

    #[test]
    fn new_identifiers_are_deterministic_and_domain_separated() {
        let resolution = LegacyDigestIdentityResolution::CreateNew;
        let source = derive_legacy_digest_source_id::<ToyDigest>(
            &"1".repeat(64),
            &"2".repeat(64),
            &resolution,
        )
        .expect("source id");
        let source_again = derive_legacy_digest_source_id::<ToyDigest>(
            &"1".repeat(64),
            &"2".repeat(64),
            &resolution,
        )
        .expect("source id");
        assert_eq!(source, source_again);
        let revision = derive_legacy_digest_revision_id::<ToyDigest>(
            &source,
            &"3".repeat(64),
            9,
        )
        .expect("revision id");
        assert_ne!(source, revision);
    }
}
