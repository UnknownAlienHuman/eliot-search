//! Synthetic C02 generation observations; no journal or storage I/O occurs.

use search_publication::{PublicationCoordinator, PublicationEffectKind, PublicationError};

#[path = "support/publication.rs"]
mod publication_support;
use publication_support::{
    PhasePort, SubmitFixture, close_old_points, commit_visible_epoch, persist_intent,
    projection_fixture, proposal, stage_new_points, verify_readback,
};

fn ready() -> (PublicationCoordinator, PhasePort) {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    machine
        .submit_fixture(proposal(
            None,
            projection_fixture(&[2]),
            "visibility-generation",
        ))
        .unwrap();
    let mut port = PhasePort::default();
    persist_intent(&mut machine, &mut port).unwrap();
    stage_new_points(&mut machine, &mut port).unwrap();
    close_old_points(&mut machine, &mut port).unwrap();
    verify_readback(&mut machine, &mut port).unwrap();
    (machine, port)
}

#[test]
fn visibility_cas_validates_its_generation_pair_after_intervening_checkpoints() {
    let (mut machine, mut port) = ready();
    assert_eq!(
        machine
            .active()
            .unwrap()
            .durable_intent()
            .unwrap()
            .control_generation(),
        8
    );
    // The journal advanced after intent through phase checkpoints and other
    // technical writes. The visibility CAS itself is still one transaction.
    port.readback.control_before_generation_override = Some(12);
    port.readback.control_generation_override = Some(13);
    let receipt = commit_visible_epoch(&mut machine, &mut port).unwrap();
    assert_eq!(receipt.control_generation(), 13);

    for (before, after) in [(7, 8), (12, 14)] {
        let (mut machine, mut port) = ready();
        port.readback.control_before_generation_override = Some(before);
        port.readback.control_generation_override = Some(after);
        assert_eq!(
            commit_visible_epoch(&mut machine, &mut port),
            Err(PublicationError::ControlConflict)
        );
    }

    let (mut machine, mut port) = ready();
    port.readback.binding_request_id_override = Some((
        PublicationEffectKind::CommitVisibleEpoch,
        search_contracts::RequestId::from_bytes([88; 16]),
    ));
    assert_eq!(
        commit_visible_epoch(&mut machine, &mut port),
        Err(PublicationError::OperationMismatch)
    );
}
