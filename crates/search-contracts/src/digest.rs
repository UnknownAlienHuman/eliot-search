//! Real algorithm execution over the sole canonical encoder or bounded raw bytes.
//!
//! Domain-bound preimages are exactly `domain || 0 || payload`. CBOR and raw-byte domains
//! are disjoint. These primitives do not issue operation receipts or authorize
//! caller data; each semantic owner must freeze its schema and domain together.
//! The explicitly named exact-byte primitives add no framing.

use crate::canonical::{
    check_canonical_array_length, stream_canonical_cbor, stream_canonical_cbor_array,
};
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

/// Finite ceiling for the complete preimage, including any domain and separator.
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

/// Maximum unframed BLAKE3 input for the baseline source qualification profile.
pub const MAX_BLAKE3_EXACT_BYTES: usize = 16 * 1024 * 1024;

/// Finite exact-byte BLAKE3 ceiling, separate from canonical metadata limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Blake3ExactInputLimit(usize);

impl Blake3ExactInputLimit {
    /// Accept a nonzero ceiling no larger than [`MAX_BLAKE3_EXACT_BYTES`].
    pub fn new(max_bytes: usize) -> Result<Self, ContractError> {
        if max_bytes == 0 {
            return Err(ContractError::new(
                ContractErrorKind::ZeroNotAllowed,
                "blake3_exact_limit",
            ));
        }
        if max_bytes > MAX_BLAKE3_EXACT_BYTES {
            return Err(ContractError::oversize(
                "blake3_exact_limit",
                MAX_BLAKE3_EXACT_BYTES,
                max_bytes,
            ));
        }
        Ok(Self(max_bytes))
    }

    /// Exact maximum input length; no domain or framing is included.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Caller-supplied finite computation budget for unframed BLAKE3 bytes.
///
/// This budget limits only the bytes consumed by one digest computation. It
/// does not own or allocate the input, bound input production or retention,
/// admit data, qualify a source, impose a deadline or total-work budget, or
/// grant authority to the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Blake3ByteBudget(usize);

impl Blake3ByteBudget {
    /// Accept a nonzero finite computation budget.
    pub fn new(max_bytes: usize) -> Result<Self, ContractError> {
        if max_bytes == 0 {
            return Err(ContractError::new(
                ContractErrorKind::ZeroNotAllowed,
                "blake3_byte_budget",
            ));
        }
        Ok(Self(max_bytes))
    }

    /// Maximum bytes this computation may consume.
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

/// One completed canonical BLAKE3 computation and its exact complete byte usage.
///
/// This is encoder usage only, not an operation, validation, completeness or
/// budget receipt. Fields are private; no supplied digest or count can mint it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalBlake3Output {
    digest: Blake3Digest32,
    complete_preimage_bytes: usize,
}

impl CanonicalBlake3Output {
    /// Internally computed equality digest.
    #[must_use]
    pub const fn digest(self) -> Blake3Digest32 {
        self.digest
    }

    /// Actual domain, NUL and CBOR bytes consumed by this successful computation.
    #[must_use]
    pub const fn complete_preimage_bytes(self) -> usize {
        self.complete_preimage_bytes
    }
}

/// Hash a finite ordered canonical array without collecting its complete input.
///
/// Uses the same length-first CBOR profile and domain framing as
/// `blake3_canonical` over `CanonicalValue::Array`. The declared count is at most
/// `MAX_COLLECTION_ITEMS`; at most count + 1 iterator reads check exact length.
/// Each row is validated with the enclosing array included in the depth ceiling.
/// Caller row errors, malformed length or byte/depth/canonical failures return no digest.
///
/// The helper does not retain a complete array or preimage. Callers own row
/// construction, prior allocation and deterministic schema order; this computes
/// equality only and grants no operation, validation or completeness authority.
///
/// # Errors
/// Rejects a raw domain, oversized count/preimage, mismatched row count, invalid
/// canonical row or any error returned by the row producer.
pub fn blake3_canonical_array(
    domain: &CanonicalDigestDomain,
    row_count: usize,
    rows: impl IntoIterator<Item = Result<CanonicalValue, ContractError>>,
    limit: DigestInputLimit,
) -> Result<Blake3Digest32, ContractError> {
    blake3_canonical_array_with_usage(domain, row_count, rows, limit)
        .map(CanonicalBlake3Output::digest)
}

/// Hash the same finite canonical array and return exact successful byte usage.
///
/// The count includes domain framing, the containing array and all row bytes.
/// One shared checked encoder feeds both the count and the private hasher; no
/// preimage, second encoding pass or local CBOR size calculator is retained.
/// Caller row construction and allocations before sink charging remain outside
/// this byte-usage value. A consumer owns any cumulative budget and its units.
///
/// # Errors
/// Has the same domain/count/row/encoding/ceiling refusals and precedence as
/// the digest-only array function. Any error returns neither digest nor byte usage.
pub fn blake3_canonical_array_with_usage(
    domain: &CanonicalDigestDomain,
    row_count: usize,
    rows: impl IntoIterator<Item = Result<CanonicalValue, ContractError>>,
    limit: DigestInputLimit,
) -> Result<CanonicalBlake3Output, ContractError> {
    check_canonical_array_length(row_count)?;
    let payload_limit = domain.payload_limit(Representation::Cbor, limit)?;
    let mut complete_preimage_bytes = limit.get() - payload_limit;
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain.name.as_bytes());
    hasher.update(&[0]);
    stream_canonical_cbor_array(row_count, rows, payload_limit, |bytes| {
        complete_preimage_bytes = complete_preimage_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| ContractError::oversize("digest_preimage", limit.get(), usize::MAX))?;
        hasher.update(bytes);
        Ok(())
    })?;
    Ok(CanonicalBlake3Output {
        digest: Blake3Digest32::from_computed_bytes(*hasher.finalize().as_bytes()),
        complete_preimage_bytes,
    })
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

/// Standard BLAKE3 of exactly the supplied bytes, with no domain or framing.
///
/// Checks the complete input before hashing. Empty bytes have their standard
/// digest; source admission and original-operation work remain caller-owned.
/// This computes equality only and provides no read, admission or authority proof.
///
/// # Errors
/// Returns a content-free size error if the input exceeds the explicit ceiling.
pub fn blake3_exact_bytes(
    bytes: &[u8],
    limit: Blake3ExactInputLimit,
) -> Result<Blake3Digest32, ContractError> {
    if bytes.len() > limit.get() {
        return Err(ContractError::oversize(
            "blake3_exact_payload",
            limit.get(),
            bytes.len(),
        ));
    }
    Ok(blake3_hash_exact(bytes))
}

/// Standard BLAKE3 of the supplied bytes under a caller-selected finite
/// computation budget, with no domain or framing.
///
/// The caller owns input allocation, retention, admission and source
/// qualification. This helper checks the complete input before hashing and
/// computes equality only; it supplies no authority, deadline or total-work
/// proof. Empty bytes retain the standard BLAKE3 result.
///
/// # Errors
/// Returns a content-free size error if the input exceeds the budget.
pub fn blake3_bounded_bytes(
    bytes: &[u8],
    budget: Blake3ByteBudget,
) -> Result<Blake3Digest32, ContractError> {
    if bytes.len() > budget.get() {
        return Err(ContractError::oversize(
            "blake3_bounded_payload",
            budget.get(),
            bytes.len(),
        ));
    }
    Ok(blake3_hash_exact(bytes))
}

/// Standard SHA-256 of the supplied bytes with no added domain or framing.
///
/// Compatibility owners must already specify the exact preimage. This checksum
/// supplies no validation or authority. Prefer the domain-bound helpers for new
/// semantic identities. The complete input is checked before hashing starts.
///
/// # Errors
/// Returns a content-free size error if the input exceeds the finite ceiling.
pub fn sha256_exact_bytes(
    bytes: &[u8],
    limit: DigestInputLimit,
) -> Result<Sha256Digest32, ContractError> {
    if bytes.len() > limit.get() {
        return Err(ContractError::oversize(
            "digest_exact_payload",
            limit.get(),
            bytes.len(),
        ));
    }
    Ok(Sha256Digest32::from_computed_bytes(
        Sha256::digest(bytes).into(),
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

fn blake3_hash_exact(bytes: &[u8]) -> Blake3Digest32 {
    Blake3Digest32::from_computed_bytes(*blake3::hash(bytes).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_blake3_matches_published_unframed_vectors() {
        // BLAKE3 1.8.2 test_vectors/test_vectors.json: input[i] = i % 251.
        // These published values cover empty, short, chunk and tree inputs.
        let limit = Blake3ExactInputLimit::new(102_400).unwrap();
        for (length, expected) in [
            (0, "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"),
            (1, "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"),
            (1024, "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7"),
            (102_400, "bc3e3d41a1146b069abffad3c0d44860cf664390afce4d9661f7902e7943e085"),
        ] {
            let input = (0..length)
                .map(|index| u8::try_from(index % 251).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                crate::hex_encode(blake3_exact_bytes(&input, limit).unwrap().as_bytes()),
                expected
            );
        }
    }

    #[test]
    fn exact_blake3_limit_is_inclusive_and_does_not_enlarge_canonical_limit() {
        assert_eq!(
            Blake3ExactInputLimit::new(0).unwrap_err().kind(),
            ContractErrorKind::ZeroNotAllowed
        );
        assert_eq!(
            Blake3ExactInputLimit::new(MAX_BLAKE3_EXACT_BYTES + 1)
                .unwrap_err().kind(),
            ContractErrorKind::OversizePayload
        );
        let limit = Blake3ExactInputLimit::new(MAX_BLAKE3_EXACT_BYTES).unwrap();
        assert_eq!(limit.get(), MAX_BLAKE3_EXACT_BYTES);
        let input = vec![0; MAX_BLAKE3_EXACT_BYTES + 1];
        assert!(blake3_exact_bytes(&input[..MAX_BLAKE3_EXACT_BYTES], limit).is_ok());
        let error = blake3_exact_bytes(&input, limit).unwrap_err();
        assert_eq!(error.kind(), ContractErrorKind::OversizePayload);
        assert_eq!(error.field(), "blake3_exact_payload");
        assert!(DigestInputLimit::new(MAX_CANONICAL_BYTES).is_ok());
        assert!(DigestInputLimit::new(MAX_CANONICAL_BYTES + 1).is_err());
    }

    #[test]
    fn bounded_blake3_matches_published_nonempty_vector() {
        let input = [0_u8];
        let digest = blake3_bounded_bytes(&input, Blake3ByteBudget::new(1).unwrap())
            .expect("bounded digest");
        assert_eq!(
            crate::hex_encode(digest.as_bytes()),
            "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"
        );
    }

    #[test]
    fn bounded_blake3_refuses_input_above_budget_before_hashing() {
        let error = blake3_bounded_bytes(b"abcd", Blake3ByteBudget::new(3).unwrap())
            .unwrap_err();
        assert_eq!(error.kind(), ContractErrorKind::OversizePayload);
        assert_eq!(error.field(), "blake3_bounded_payload");
        assert_eq!(error.limit(), Some(3));
        assert_eq!(error.actual(), Some(4));
    }

    #[test]
    fn bounded_blake3_preserves_custom_limits_above_source_profile_ceiling() {
        let input = vec![0_u8; MAX_BLAKE3_EXACT_BYTES + 1];
        let budget = Blake3ByteBudget::new(input.len()).unwrap();
        assert!(blake3_bounded_bytes(&input, budget).is_ok());

        let exact_limit = Blake3ExactInputLimit::new(MAX_BLAKE3_EXACT_BYTES).unwrap();
        let error = blake3_exact_bytes(&input, exact_limit).unwrap_err();
        assert_eq!(error.kind(), ContractErrorKind::OversizePayload);
        assert_eq!(error.field(), "blake3_exact_payload");
    }

    #[test]
    fn exact_blake3_does_not_replace_domain_bound_raw_hashing() {
        let domain = CanonicalDigestDomain::parse("eliot/raw/exact-byte-comparison/v1").unwrap();
        let exact = blake3_exact_bytes(&[0], Blake3ExactInputLimit::new(1).unwrap()).unwrap();
        assert_ne!(
            exact,
            blake3_raw(&domain, &[0], DigestInputLimit::new(128).unwrap()).unwrap()
        );
        assert_eq!(
            blake3_exact_bytes(&[0, 1], Blake3ExactInputLimit::new(1).unwrap())
                .unwrap_err().kind(),
            ContractErrorKind::OversizePayload
        );
    }

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
    // Counted-array cases are AUTHORED_NOT_RUN; execution remains Q/#215.
    #[test]
    fn canonical_array_usage_is_exact_and_boundary_inclusive() {
        let domain = CanonicalDigestDomain::parse("eliot/cbor/usage-fixture/v1").unwrap();
        assert_eq!(domain.as_str().len(), 27);
        let rows = vec![
            CanonicalValue::U64(1),
            CanonicalValue::U64(2),
            CanonicalValue::U64(3),
        ];
        let output = blake3_canonical_array_with_usage(
            &domain,
            rows.len(),
            rows.iter().cloned().map(Ok),
            DigestInputLimit::new(32).unwrap(),
        )
        .unwrap();
        // RFC 8949: array header83 plus three one-byte integers; domain27+NUL1.
        assert_eq!(output.complete_preimage_bytes(), 32);
        assert_eq!(
            output.digest(),
            blake3_canonical_array(
                &domain,
                rows.len(),
                rows.iter().cloned().map(Ok),
                DigestInputLimit::new(32).unwrap(),
            )
            .unwrap()
        );
        let root = CanonicalValue::Array(crate::BoundedList::new(rows.clone()).unwrap());
        assert_eq!(
            crate::to_canonical_cbor(&root).unwrap().as_slice(),
            &[0x83, 1, 2, 3]
        );
        assert_eq!(
            output.digest(),
            blake3_canonical(&domain, &root, DigestInputLimit::new(32).unwrap()).unwrap()
        );
        let below = blake3_canonical_array_with_usage(
            &domain,
            rows.len(),
            rows.into_iter().map(Ok),
            DigestInputLimit::new(31).unwrap(),
        )
        .unwrap_err();
        assert_eq!(below.kind(), ContractErrorKind::OversizePayload);
        assert_eq!(below.field(), "canonical_sink");
        let empty = blake3_canonical_array_with_usage(
            &domain,
            0,
            std::iter::empty(),
            DigestInputLimit::new(29).unwrap(),
        )
        .unwrap();
        assert_eq!(empty.complete_preimage_bytes(), 29);
        let varied = [
            CanonicalValue::U64(256),
            CanonicalValue::Bytes(crate::BoundedBytes::new(vec![0; 32]).unwrap()),
        ];
        // Outer array1 + unsigned256 three bytes + byte-string32 thirty-four bytes.
        let output = blake3_canonical_array_with_usage(
            &domain,
            varied.len(),
            varied.into_iter().map(Ok),
            DigestInputLimit::new(66).unwrap(),
        )
        .unwrap();
        assert_eq!(output.complete_preimage_bytes(), 66);
    }

    #[test]
    fn canonical_array_usage_returns_no_partial_result_on_refusal() {
        let domain = CanonicalDigestDomain::parse("eliot/cbor/usage-fixture/v1").unwrap();
        let limit = DigestInputLimit::new(64).unwrap();
        let row_error = ContractError::new(ContractErrorKind::InvalidRange, "usage_row");
        let rejected =
            blake3_canonical_array_with_usage(&domain, 1, [Err(row_error.clone())], limit)
                .unwrap_err();
        assert_eq!(rejected, row_error);
        for (declared, actual) in [(2, 1), (1, 2)] {
            let legacy = blake3_canonical_array(
                &domain,
                declared,
                (0..actual).map(|_| Ok(CanonicalValue::Null)),
                limit,
            )
            .unwrap_err();
            let counted = blake3_canonical_array_with_usage(
                &domain,
                declared,
                (0..actual).map(|_| Ok(CanonicalValue::Null)),
                limit,
            )
            .unwrap_err();
            assert_eq!(counted, legacy);
        }
        let mut visits = 0;
        let rows = std::iter::from_fn(|| {
            visits += 1;
            Some(Ok(CanonicalValue::Null))
        });
        assert!(
            blake3_canonical_array_with_usage(
                &domain,
                crate::MAX_COLLECTION_ITEMS + 1,
                rows,
                limit,
            )
            .is_err()
        );
        assert_eq!(visits, 0);
        let raw = CanonicalDigestDomain::parse("eliot/raw/usage-fixture/v1").unwrap();
        assert_eq!(
            blake3_canonical_array_with_usage(&raw, 0, std::iter::empty(), limit,)
                .unwrap_err()
                .kind(),
            ContractErrorKind::InvalidTaggedVariant
        );
    }
}
