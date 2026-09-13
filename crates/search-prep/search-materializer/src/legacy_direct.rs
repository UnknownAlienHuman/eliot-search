//! Legacy DIRECT preparation framing and representation identity.
//!
//! This compatibility owner is pure and I/O-free. It owns the exact persisted
//! outcome tags and the domain-separated representation preimage used by the
//! daemon adapter. Cryptographic implementation selection remains at the
//! composition boundary through [`LegacyDirectRepresentationDigest`].

use core::fmt;

/// Domain bound into every legacy DIRECT preparation representation identity.
pub const LEGACY_DIRECT_REPRESENTATION_DOMAIN: &[u8] =
    b"eliot-searchd/preparation-representation/v1\x00";

/// BLAKE3-256 wire tag matching `search-contracts::DigestAlgorithm`.
pub const DIGEST_ALGORITHM_BLAKE3_256: u8 = 1;
/// SHA-256 wire tag matching `search-contracts::DigestAlgorithm`.
pub const DIGEST_ALGORITHM_SHA256: u8 = 2;
/// Legacy DIRECT source content uses SHA-256 identities.
pub const CONTENT_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_SHA256;
/// Legacy DIRECT representation identities use BLAKE3-256.
pub const REPRESENTATION_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_BLAKE3_256;
/// Legacy DIRECT preparation manifests use SHA-256 envelope digests.
pub const MANIFEST_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_SHA256;

/// Closed legacy preparation framing failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyDirectPreparationError {
    /// The frame tag, cardinality or layout body is invalid.
    InvalidFrame,
}

impl LegacyDirectPreparationError {
    /// Stable daemon-compatible reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidFrame => "DIRECT_PREPARATION_INVALID",
        }
    }
}

impl fmt::Display for LegacyDirectPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LegacyDirectPreparationError {}

/// Exact closed gap encoded by the legacy DIRECT preparation frame.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegacyDirectPreparationGap {
    /// Retained bytes are not UTF-8.
    RevisionNotUtf8,
    /// Retained bytes violate the text/binary policy.
    BinaryContent,
    /// Exact materialization exceeded its line ceiling.
    TooManyLines,
    /// Exact unitization exceeded its unit ceiling.
    TooManyUnits,
    /// Encoded layout exceeded the retained-object ceiling.
    LayoutTooLarge,
    /// A leading BOM would shift exact DIRECT coordinates.
    RevisionHasBom,
}

impl LegacyDirectPreparationGap {
    /// Persisted singleton tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::RevisionNotUtf8 => 1,
            Self::BinaryContent => 2,
            Self::TooManyLines => 3,
            Self::TooManyUnits => 4,
            Self::LayoutTooLarge => 5,
            Self::RevisionHasBom => 6,
        }
    }

    /// Stable daemon-compatible gap reason.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::RevisionNotUtf8 => "DIRECT_REVISION_NOT_UTF8",
            Self::BinaryContent => "MATERIALIZATION_BINARY_CONTENT",
            Self::TooManyLines => "MATERIALIZATION_TOO_MANY_LINES",
            Self::TooManyUnits => "UNITIZATION_TOO_MANY_UNITS",
            Self::LayoutTooLarge => "DIRECT_PREPARATION_LAYOUT_TOO_LARGE",
            Self::RevisionHasBom => "DIRECT_REVISION_HAS_BOM",
        }
    }
}

/// Borrowed decoded legacy DIRECT preparation frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyDirectPreparationFrame<'a> {
    /// Non-empty exact unit layout bytes.
    Layout(&'a [u8]),
    /// Closed deterministic preparation gap.
    Gap(LegacyDirectPreparationGap),
}

impl<'a> LegacyDirectPreparationFrame<'a> {
    /// Identity marker used by representation derivation.
    ///
    /// Layouts bind their exact bytes; gaps bind their stable reason text.
    #[must_use]
    pub const fn identity_marker(self) -> &'a [u8] {
        match self {
            Self::Layout(layout) => layout,
            Self::Gap(gap) => gap.reason().as_bytes(),
        }
    }

    /// Gap reason, or `None` for a searchable layout.
    #[must_use]
    pub const fn gap_reason(self) -> Option<&'static str> {
        match self {
            Self::Layout(_) => None,
            Self::Gap(gap) => Some(gap.reason()),
        }
    }
}

/// Decodes the exact persisted legacy DIRECT preparation frame.
///
/// Layout tag `0` requires a non-empty body. Gap tags are fixed singletons;
/// trailing bytes and unknown tags fail closed.
pub const fn decode_legacy_direct_preparation(
    encoded: &[u8],
) -> Result<LegacyDirectPreparationFrame<'_>, LegacyDirectPreparationError> {
    match encoded {
        [0, layout @ ..] if !layout.is_empty() => {
            Ok(LegacyDirectPreparationFrame::Layout(layout))
        }
        [1] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::RevisionNotUtf8,
        )),
        [2] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::BinaryContent,
        )),
        [3] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::TooManyLines,
        )),
        [4] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::TooManyUnits,
        )),
        [5] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::LayoutTooLarge,
        )),
        [6] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::RevisionHasBom,
        )),
        _ => Err(LegacyDirectPreparationError::InvalidFrame),
    }
}

/// Encodes a non-empty exact layout under the version-one layout tag.
pub fn encode_legacy_direct_layout(
    layout: &[u8],
) -> Result<Vec<u8>, LegacyDirectPreparationError> {
    if layout.is_empty() {
        return Err(LegacyDirectPreparationError::InvalidFrame);
    }
    let mut encoded = Vec::with_capacity(layout.len().saturating_add(1));
    encoded.push(0);
    encoded.extend_from_slice(layout);
    Ok(encoded)
}

/// Encodes one closed gap as its exact singleton frame.
#[must_use]
pub fn encode_legacy_direct_gap(gap: LegacyDirectPreparationGap) -> Vec<u8> {
    vec![gap.tag()]
}

/// Exact source, revision, content and profile inputs to representation identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyDirectPreparationBinding {
    /// Data-root namespace digest.
    pub namespace: [u8; 32],
    /// Stable source identifier bytes.
    pub source_id: [u8; 32],
    /// Immutable revision identifier bytes.
    pub revision_id: [u8; 32],
    /// Exact source-content digest bytes.
    pub content_digest: [u8; 32],
    /// Exact retained source byte length.
    pub byte_length: u64,
    /// Canonical materializer profile digest.
    pub materializer_digest: [u8; 32],
    /// Canonical unitizer profile digest.
    pub unitizer_digest: [u8; 32],
}

/// Composition-supplied BLAKE3-compatible digest primitive.
///
/// Implementations absorb the domain followed by every ordered part exactly as
/// supplied, without adding implicit separators or changing algorithms.
pub trait LegacyDirectRepresentationDigest {
    /// Computes one digest over a domain and ordered raw parts.
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32];
}

/// Derives the exact version-one legacy DIRECT representation identity.
#[must_use]
pub fn derive_legacy_direct_representation_id<D: LegacyDirectRepresentationDigest>(
    binding: &LegacyDirectPreparationBinding,
    canonical_or_gap: &[u8],
) -> [u8; 32] {
    let byte_length = binding.byte_length.to_be_bytes();
    let marker_length = u64::try_from(canonical_or_gap.len())
        .unwrap_or(u64::MAX)
        .to_be_bytes();
    D::digest_parts(
        LEGACY_DIRECT_REPRESENTATION_DOMAIN,
        &[
            &binding.namespace,
            &binding.source_id,
            &binding.revision_id,
            &binding.content_digest,
            &byte_length,
            &binding.materializer_digest,
            &binding.unitizer_digest,
            &marker_length,
            canonical_or_gap,
        ],
    )
}

/// Verifies an expected representation identity by exact recomputation.
pub fn verify_legacy_direct_representation<D: LegacyDirectRepresentationDigest>(
    expected: &[u8; 32],
    binding: &LegacyDirectPreparationBinding,
    canonical_or_gap: &[u8],
) -> Result<(), LegacyDirectPreparationError> {
    if derive_legacy_direct_representation_id::<D>(binding, canonical_or_gap) == *expected {
        Ok(())
    } else {
        Err(LegacyDirectPreparationError::InvalidFrame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ToyDigest;

    impl LegacyDirectRepresentationDigest for ToyDigest {
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

    fn binding() -> LegacyDirectPreparationBinding {
        LegacyDirectPreparationBinding {
            namespace: [1; 32],
            source_id: [2; 32],
            revision_id: [3; 32],
            content_digest: [4; 32],
            byte_length: 5,
            materializer_digest: [6; 32],
            unitizer_digest: [7; 32],
        }
    }

    #[test]
    fn layout_and_gap_frames_round_trip_exactly() {
        let layout = encode_legacy_direct_layout(b"layout").expect("layout");
        let frame = decode_legacy_direct_preparation(&layout).expect("decode");
        assert_eq!(frame, LegacyDirectPreparationFrame::Layout(b"layout"));
        assert_eq!(frame.identity_marker(), b"layout");
        assert_eq!(frame.gap_reason(), None);

        for gap in [
            LegacyDirectPreparationGap::RevisionNotUtf8,
            LegacyDirectPreparationGap::BinaryContent,
            LegacyDirectPreparationGap::TooManyLines,
            LegacyDirectPreparationGap::TooManyUnits,
            LegacyDirectPreparationGap::LayoutTooLarge,
            LegacyDirectPreparationGap::RevisionHasBom,
        ] {
            let encoded = encode_legacy_direct_gap(gap);
            let decoded = decode_legacy_direct_preparation(&encoded).expect("gap");
            assert_eq!(decoded, LegacyDirectPreparationFrame::Gap(gap));
            assert_eq!(decoded.identity_marker(), gap.reason().as_bytes());
            assert_eq!(decoded.gap_reason(), Some(gap.reason()));
        }
    }

    #[test]
    fn malformed_frames_fail_closed() {
        for encoded in [
            Vec::new(),
            vec![0],
            vec![7],
            vec![1, 0],
            vec![6, 6],
        ] {
            assert_eq!(
                decode_legacy_direct_preparation(&encoded),
                Err(LegacyDirectPreparationError::InvalidFrame)
            );
        }
        assert_eq!(
            encode_legacy_direct_layout(&[]),
            Err(LegacyDirectPreparationError::InvalidFrame)
        );
    }

    #[test]
    fn representation_identity_binds_every_field_and_marker() {
        let base = binding();
        let first = derive_legacy_direct_representation_id::<ToyDigest>(&base, b"layout");
        assert_eq!(
            first,
            derive_legacy_direct_representation_id::<ToyDigest>(&base, b"layout")
        );
        let mut changed = base;
        changed.revision_id = [9; 32];
        assert_ne!(
            first,
            derive_legacy_direct_representation_id::<ToyDigest>(&changed, b"layout")
        );
        assert_ne!(
            first,
            derive_legacy_direct_representation_id::<ToyDigest>(&base, b"other")
        );
        assert!(verify_legacy_direct_representation::<ToyDigest>(&first, &base, b"layout").is_ok());
        assert_eq!(
            verify_legacy_direct_representation::<ToyDigest>(&first, &base, b"other"),
            Err(LegacyDirectPreparationError::InvalidFrame)
        );
    }

    #[test]
    fn digest_algorithm_tags_remain_explicit_and_distinct() {
        assert_eq!(CONTENT_DIGEST_ALGORITHM, DIGEST_ALGORITHM_SHA256);
        assert_eq!(REPRESENTATION_DIGEST_ALGORITHM, DIGEST_ALGORITHM_BLAKE3_256);
        assert_eq!(MANIFEST_DIGEST_ALGORITHM, DIGEST_ALGORITHM_SHA256);
        assert_ne!(REPRESENTATION_DIGEST_ALGORITHM, CONTENT_DIGEST_ALGORITHM);
    }
}
