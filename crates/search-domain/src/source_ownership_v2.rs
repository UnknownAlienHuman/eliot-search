//! Permanent-v2 namespace-owner equality, separate from retained v1 generations.
//!
//! This computes identity from the six Architecture S7.2.1 fields. It performs
//! no namespace allocation, admission, persistence or live owner validation.

use search_contracts::{
    BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText, CanonicalValue,
    DigestInputLimit, NamespaceOwnershipStatus, SourceNamespaceOwnership, SourceOwnerGeneration,
    blake3_canonical,
};

use crate::DomainError;

/// Exact permanent-v2 generation preimage domain; never an alias for v1.
pub const SOURCE_OWNER_GENERATION_DOMAIN_V2: &str = "eliot/cbor/source-owner-generation/v2";
/// Inclusive domain, NUL and canonical-payload ceiling, including a maximal owner ID.
pub const MAX_SOURCE_OWNER_GENERATION_PREIMAGE_BYTES_V2: usize = 512;

/// Compute a permanent-v2 owner generation from the six bound identity axes.
///
/// The supplied `source_owner_generation`, policy revision and cutover reference
/// are excluded. Equality of this result grants no ownership or currentness.
/// Callers retain their original checkpoints before and after this finite pure
/// operation. Historical v1 values must not be rewritten with this profile.
///
/// # Errors
/// Returns a bounded contract error if canonical construction or the complete
/// preimage exceeds its frozen limit. No partial generation is returned.
pub fn source_owner_generation_v2(
    ownership: &SourceNamespaceOwnership,
) -> Result<SourceOwnerGeneration, DomainError> {
    let domain = CanonicalDigestDomain::parse(SOURCE_OWNER_GENERATION_DOMAIN_V2)?;
    let payload = generation_payload(ownership)?;
    let digest = blake3_canonical(
        &domain,
        &payload,
        DigestInputLimit::new(MAX_SOURCE_OWNER_GENERATION_PREIMAGE_BYTES_V2)?,
    )?;
    Ok(SourceOwnerGeneration::from_bytes(*digest.as_bytes()))
}

fn generation_payload(ownership: &SourceNamespaceOwnership) -> Result<CanonicalValue, DomainError> {
    let status = match ownership.status {
        NamespaceOwnershipStatus::Active => "ACTIVE",
        NamespaceOwnershipStatus::CutoverPrepared => "CUTOVER_PREPARED",
        NamespaceOwnershipStatus::Fenced => "FENCED",
        NamespaceOwnershipStatus::Retired => "RETIRED",
    };
    Ok(CanonicalValue::Array(BoundedList::new(vec![
        CanonicalValue::U64(2),
        CanonicalValue::Bytes(BoundedBytes::new(
            ownership.source_namespace_id.as_bytes().to_vec(),
        )?),
        CanonicalValue::Text(CanonicalText::new(ownership.owner_system_id.as_str())?),
        CanonicalValue::Bytes(BoundedBytes::new(
            ownership
                .owner_installation_incarnation_id
                .as_bytes()
                .to_vec(),
        )?),
        CanonicalValue::U64(ownership.owner_epoch.get()),
        CanonicalValue::U64(ownership.ownership_record_revision.get()),
        CanonicalValue::Text(CanonicalText::new(status)?),
    ])?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::{
        InstallationIncarnationId, NonZeroRevision, OpaqueId, OwnerEpoch, PolicyRevision,
        ReceiptRef, SourceNamespaceId, to_canonical_cbor,
    };

    fn ownership() -> SourceNamespaceOwnership {
        SourceNamespaceOwnership {
            source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
            owner_system_id: OpaqueId::new("search-provider").unwrap(),
            owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
            owner_epoch: OwnerEpoch::new(3).unwrap(),
            ownership_record_revision: NonZeroRevision::new(4).unwrap(),
            source_owner_generation: SourceOwnerGeneration::from_bytes([0; 32]),
            source_admission_policy_revision: PolicyRevision::new(7),
            status: NamespaceOwnershipStatus::Active,
            cutover_receipt_ref: None,
        }
    }

    #[test]
    fn namespace_generation_v2_matches_manually_frozen_bytes_and_digest() {
        // Manually encoded RFC8949 array, independently hashed with pinned BLAKE3
        // over domain + NUL + these bytes, without the Search encoder.
        let mut expected = vec![0x87, 0x02, 0x50];
        expected.extend_from_slice(&[1; 16]);
        expected.push(0x6f);
        expected.extend_from_slice(b"search-provider");
        expected.push(0x50);
        expected.extend_from_slice(&[2; 16]);
        expected.extend_from_slice(&[3, 4, 0x66]);
        expected.extend_from_slice(b"ACTIVE");
        let payload = to_canonical_cbor(&generation_payload(&ownership()).unwrap()).unwrap();
        assert_eq!(payload.as_slice(), expected);
        assert_eq!(expected.len(), 61);
        assert_eq!(
            source_owner_generation_v2(&ownership())
                .unwrap()
                .to_string(),
            "88af571a68fd22a2f870587ce1a8c3ec0d98bfab3238721430749ce115d3819b"
        );
    }

    #[test]
    fn namespace_generation_v2_binds_all_six_axes_and_keeps_policy_separate() {
        let original = ownership();
        let baseline = source_owner_generation_v2(&original).unwrap();
        let mut variants = Vec::new();
        for axis in 0..6 {
            let mut changed = original.clone();
            match axis {
                0 => changed.source_namespace_id = SourceNamespaceId::from_bytes([8; 16]),
                1 => changed.owner_system_id = OpaqueId::new("other-provider").unwrap(),
                2 => {
                    changed.owner_installation_incarnation_id =
                        InstallationIncarnationId::from_bytes([8; 16])
                }
                3 => changed.owner_epoch = OwnerEpoch::new(8).unwrap(),
                4 => changed.ownership_record_revision = NonZeroRevision::new(8).unwrap(),
                _ => changed.status = NamespaceOwnershipStatus::Fenced,
            }
            let digest = source_owner_generation_v2(&changed).unwrap();
            assert_ne!(digest, baseline, "identity axis {axis}");
            assert!(!variants.contains(&digest));
            variants.push(digest);
        }
        for status in [
            NamespaceOwnershipStatus::CutoverPrepared,
            NamespaceOwnershipStatus::Retired,
        ] {
            let mut changed = original.clone();
            changed.status = status;
            let digest = source_owner_generation_v2(&changed).unwrap();
            assert_ne!(digest, baseline);
            assert!(!variants.contains(&digest));
            variants.push(digest);
        }
        let mut independent = original;
        independent.source_owner_generation = SourceOwnerGeneration::from_bytes([99; 32]);
        independent.source_admission_policy_revision = PolicyRevision::new(99);
        independent.cutover_receipt_ref = Some(ReceiptRef::new("other-cutover").unwrap());
        assert_eq!(source_owner_generation_v2(&independent).unwrap(), baseline);
        independent.owner_system_id =
            OpaqueId::new("a".repeat(search_contracts::MAX_OPAQUE_ID_BYTES)).unwrap();
        independent.owner_epoch = OwnerEpoch::new(u64::MAX).unwrap();
        independent.ownership_record_revision = NonZeroRevision::new(u64::MAX).unwrap();
        independent.status = NamespaceOwnershipStatus::CutoverPrepared;
        let maximum = to_canonical_cbor(&generation_payload(&independent).unwrap()).unwrap();
        assert_eq!(
            maximum.len() + SOURCE_OWNER_GENERATION_DOMAIN_V2.len() + 1,
            368
        );
        assert!(
            source_owner_generation_v2(&independent).is_ok(),
            "full owner ID fits frozen preimage cap"
        );
    }
}
