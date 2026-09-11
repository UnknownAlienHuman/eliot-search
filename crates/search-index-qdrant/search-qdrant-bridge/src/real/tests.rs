#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_name_binds_physical_and_generation() {
        let left = CollectionRoute {
            generation: search_contracts::CollectionGenerationId::from_bytes([0x11; 16]),
            physical_name: OpaqueId::new("t24_unit").expect("name"),
        };
        let right = CollectionRoute {
            generation: search_contracts::CollectionGenerationId::from_bytes([0x12; 16]),
            physical_name: OpaqueId::new("t24_unit").expect("name"),
        };
        let renamed = CollectionRoute {
            generation: search_contracts::CollectionGenerationId::from_bytes([0x11; 16]),
            physical_name: OpaqueId::new("t24_other").expect("name"),
        };
        let left_name = collection_name(&left).expect("left");
        // Fixed short budget for Windows gridstore paths (see `collection_name`).
        assert_eq!(left_name.len(), 28);
        assert!(left_name.starts_with("t24c"));
        assert_eq!(collection_name(&left).expect("deterministic"), left_name);
        assert_ne!(
            collection_name(&right).expect("generation bound"),
            left_name
        );
        assert_ne!(
            collection_name(&renamed).expect("physical bound"),
            left_name
        );
    }

    #[test]
    fn uuid_round_trip_preserves_128_bits() {
        let id = QdrantPointId([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ]);
        let text = uuid_string(&id);
        assert_eq!(text, "00112233-4455-6677-8899-aabbccddeeff");
        assert_eq!(parse_uuid(&text).expect("round trip"), id);
        assert_eq!(
            parse_uuid("not-a-uuid").expect_err("rejects garbage"),
            BridgeError::MalformedResponse
        );
        assert_eq!(
            parse_uuid("00112233-4455-6677-8899-aabbccddeefg").expect_err("rejects non-hex"),
            BridgeError::MalformedResponse
        );
    }

    #[test]
    fn hex_32_round_trip() {
        let bytes = [0xABu8; 32];
        let text = hex_from_32(&bytes);
        assert_eq!(text.len(), 64);
        assert_eq!(hex_to_32(&text).expect("round trip"), bytes);
        assert_eq!(
            hex_to_32("short").expect_err("rejects short"),
            BridgeError::MalformedResponse
        );
    }

    #[test]
    fn empty_filter_and_inexact_epoch_rejected_before_dispatch() {
        let mut allowed = BTreeSet::new();
        allowed.insert(OpaqueId::new("member").expect("member"));
        let filter = EligibilityFilter {
            access_partition_digest: search_contracts::Blake3Digest32::from_bytes([0x01; 32]),
            allowed_source_memberships: allowed,
            visible_epoch: search_contracts::Epoch::new(42).expect("epoch"),
        };
        assert!(base_filter(&filter).is_ok());
        let mut empty = filter;
        empty.allowed_source_memberships.clear();
        assert_eq!(
            base_filter(&empty).expect_err("empty"),
            BridgeError::InvalidFilter
        );
        // 2^53 + 1 is not exactly representable as f64: the filter must fail,
        // never silently widen.
        assert_eq!(
            epoch_bound(9_007_199_254_740_993).expect_err("inexact"),
            BridgeError::InvalidFilter
        );
        assert!(epoch_bound(42).is_ok());
    }

    #[test]
    fn error_codes_are_stable_and_redacted() {
        for error in [
            BridgeError::Cancelled,
            BridgeError::TransportFailed,
            BridgeError::MalformedResponse,
            BridgeError::MutationOutcomeUnknown,
            BridgeError::CollectionNotFound,
            BridgeError::InvalidFilter,
        ] {
            let rendered = error.to_string();
            assert_eq!(rendered, error.code());
            assert!(!rendered.contains("127.0.0.1"));
            assert!(!rendered.contains("http"));
        }
        assert_eq!(BridgeError::Cancelled.code(), "QDRANT_OPERATION_CANCELLED");
        assert_eq!(
            BridgeError::TransportFailed.code(),
            "QDRANT_TRANSPORT_FAILED"
        );
        assert_eq!(
            BridgeError::MalformedResponse.code(),
            "QDRANT_MALFORMED_RESPONSE"
        );
    }

    #[test]
    fn cancelled_context_fails_before_dispatch() {
        let flag = Arc::new(AtomicBool::new(true));
        let context = OpContext::with_cancel(Duration::from_secs(5), Arc::clone(&flag));
        assert_eq!(
            context.check().expect_err("cancelled"),
            BridgeError::Cancelled
        );
        flag.store(false, Ordering::SeqCst);
        assert!(context.check().is_ok());
        assert_eq!(OpContext::default().deadline(), Duration::from_secs(10));
    }

    #[test]
    fn indexed_field_constants_match_filter_translation() {
        assert_eq!(
            EligibilityFilter::INDEXED_FIELDS,
            [
                "access_partition_digest",
                "source_membership_id",
                "valid_from_epoch",
                "valid_until_epoch_exclusive"
            ]
        );
    }
}
