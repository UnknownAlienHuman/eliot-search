//! Shared shape regressions, not proof that live guards were observed.

use search_contracts::{
    Blake3Digest32, Epoch, OwnerEpoch, PublicationGuards, PublicationIntent, PublicationIntentId,
    PublicationIntentState, ReceiptRef,
};

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
fn every_guard_axis_participates_in_exact_equality() {
    let original = guards();
    let changed = [
        PublicationGuards {
            owner_epoch: OwnerEpoch::new(8).unwrap(),
            ..original
        },
        PublicationGuards {
            source_catalog_generation: 12,
            ..original
        },
        PublicationGuards {
            membership_generation: 14,
            ..original
        },
        PublicationGuards {
            access_generation: 18,
            ..original
        },
        PublicationGuards {
            shadow_generation: 20,
            ..original
        },
        PublicationGuards {
            purge_generation: 24,
            ..original
        },
        PublicationGuards {
            profile_digest: Blake3Digest32::from_bytes([30; 32]),
            ..original
        },
    ];
    for value in changed {
        assert_ne!(value, original);
    }
    assert_eq!(guards(), original);
}

#[test]
fn intent_carries_all_guards_without_a_lossy_intermediate_list() {
    let expected = guards();
    let intent = PublicationIntent {
        publication_intent_id: PublicationIntentId::from_bytes([31; 16]),
        target_epoch: Epoch::new(37).unwrap(),
        prepared_manifest_ref: ReceiptRef::new("receipt:prepared-fixture").unwrap(),
        owner_source_membership_access_guards: expected,
        state: PublicationIntentState::Prepared,
    };
    assert_eq!(intent.owner_source_membership_access_guards, expected);
    assert_eq!(intent.clone(), intent);
}

#[test]
fn shape_preserves_actual_zero_and_large_catalog_generations_without_casting() {
    let value = PublicationGuards {
        source_catalog_generation: 0,
        membership_generation: u64::MAX,
        access_generation: 1_u64 << 63,
        ..guards()
    };
    assert_eq!(value.source_catalog_generation, 0);
    assert_eq!(value.membership_generation, u64::MAX);
    assert_eq!(value.access_generation, 1_u64 << 63);
    // These are not epoch values. The correction does not narrow existing u64
    // fields or supply implicit observations. Counter exhaustion is handled by
    // the owning mutation, not by a guessed value in this immutable record.
}

#[test]
fn shared_guard_value_retains_coordinator_copy_and_thread_bounds() {
    fn bounds<T: Copy + Eq + Send + Sync + 'static>() {}
    bounds::<PublicationGuards>();
}
