//! The legacy coordinator import is the shared type itself, not a conversion.

use search_contracts::{Blake3Digest32, OwnerEpoch};
use search_publication::PublicationGuards;

const fn for_coordinator(value: search_contracts::PublicationGuards) -> PublicationGuards {
    value
}

const fn for_control(value: PublicationGuards) -> search_contracts::PublicationGuards {
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
    #[path = "support/publication.rs"]
    mod publication_support;
    use publication_support::{PhasePort, SubmitFixture, manifest_fixture, proposal};
    let mut coordinator = search_publication::PublicationCoordinator::new(32).unwrap();
    assert_eq!(coordinator.visible_epoch(), None);
    let case = proposal(None, manifest_fixture(&[3]), "publication-fixture");
    coordinator.submit_fixture(case).unwrap();
    let expected = guards();
    let mut port = PhasePort::default();
    let intent = publication_support::persist_intent(&mut coordinator, &mut port).unwrap();
    let observed: search_contracts::PublicationGuards = intent.guards();
    assert_eq!(observed, expected);
}
