//! The legacy coordinator import is the shared type itself, not a conversion.

use search_contracts::{Blake3Digest32, OpaqueId, OwnerEpoch, ReceiptRef};
use search_publication::{DurableIntent, PublicationGuards};

fn for_coordinator(value: search_contracts::PublicationGuards) -> PublicationGuards {
    value
}

fn for_control(value: PublicationGuards) -> search_contracts::PublicationGuards {
    value
}

fn guards() -> PublicationGuards {
    PublicationGuards {
        owner_epoch: OwnerEpoch::new(7).unwrap(),
        source_catalog_generation: 11,
        membership_generation: 13,
        access_generation: 17,
        shadow_generation: 19,
        purge_generation: 23,
        profile_digest: Blake3Digest32::from_bytes([29; 32]),
    }
}

#[test]
fn old_and_new_imports_are_identical_without_conversion() {
    let original = guards();
    assert_eq!(for_control(for_coordinator(original)), original);
    assert_eq!(
        std::any::TypeId::of::<PublicationGuards>(),
        std::any::TypeId::of::<search_contracts::PublicationGuards>(),
    );
}

#[test]
fn coordinator_intent_exposes_the_same_complete_shared_value() {
    // These synthetic values test the shape only; no durable write is claimed.
    let expected = guards();
    let intent = DurableIntent {
        transaction_id: OpaqueId::new("publication-fixture").unwrap(),
        target_epoch: search_contracts::Epoch::new(37).unwrap(),
        old_manifest_digest: None,
        new_manifest_digest: Blake3Digest32::from_bytes([41; 32]),
        guards: for_coordinator(expected),
        persist_operation_id: OpaqueId::new("persist-fixture").unwrap(),
        intent_receipt: ReceiptRef::new("receipt:intent-fixture").unwrap(),
    };
    let observed: search_contracts::PublicationGuards = intent.guards;
    assert_eq!(observed, expected);
}
