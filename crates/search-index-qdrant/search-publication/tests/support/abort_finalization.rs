//! Synthetic owner ports for the abort-finalization model only.

use search_publication::{PublicationCoordinator, PublicationError};

use super::publication_support::PhasePort;

pub fn commit(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<(), PublicationError> {
    machine.acknowledge_abort_commit(
        &mut port.effects,
        &mut port.readback,
        &super::publication_support::operation_context(),
    )
}

pub fn snapshot(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<(), PublicationError> {
    machine.publish_abort_snapshot(
        &mut port.effects,
        &mut port.readback,
        &super::publication_support::operation_context(),
    )
}

pub fn acknowledge(machine: &mut PublicationCoordinator) {
    let active = machine.active().expect("active abort fixture");
    let current_guards = active.durable_intent().expect("durable intent").guards();
    let target = active.target_epoch().get();
    let operation = search_contracts::OpaqueId::new(format!("fixture:abort-finalize-{target}"))
        .expect("abort operation id");
    let generation = 100 + u64::try_from(target).unwrap();
    machine
        .prepare_abort_finalization(operation, generation, current_guards)
        .expect("latched exact abort finalization");
    let mut port = PhasePort::default();
    commit(machine, &mut port).expect("synthetic typed control readback");
    snapshot(machine, &mut port).expect("synthetic current-process snapshot readback");
}
