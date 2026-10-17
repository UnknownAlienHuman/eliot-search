//! Exact vector bytes must match the real C13 digest before stage proof issuance.
use search_contracts::indexed::VectorValue;
use search_ports::{CancellationProbe, OperationContext};
use search_publication::{
    PublicationCoordinator, PublicationEffectCommand, PublicationEffectReadback, PublicationError,
    PublicationPhase, PublicationReadbackPort,
};

#[path = "support/publication.rs"]
mod publication_support;
use publication_support::{
    PhaseReadback, SubmitFixture, operation_context, persist_intent, projection_fixture, proposal,
};

struct ChangedZeroSign(PhaseReadback);
impl<C: CancellationProbe> PublicationReadbackPort<C> for ChangedZeroSign {
    fn readback(
        &mut self,
        command: &PublicationEffectCommand,
        context: &OperationContext<C>,
    ) -> PublicationEffectReadback {
        let mut observed = self.0.readback(command, context);
        for point in &mut observed.points {
            for vector in point.vectors.values_mut() {
                let values = match vector {
                    VectorValue::Dense(values) | VectorValue::Sparse { values, .. } => values,
                };
                for value in values {
                    if value.to_bits() == (-0.0_f32).to_bits() {
                        *value = 0.0;
                    }
                }
            }
        }
        observed
    }
}

#[test]
fn stage_rejects_zero_sign_substitution_against_admitted_c13_vector_bytes() {
    let fixture = projection_fixture(&[1]).with_sparse_values_at(0, vec![-0.0, 2.0]);
    let mut machine = PublicationCoordinator::new(32).unwrap();
    machine
        .submit_fixture(proposal(None, fixture, "exact-zero-bits"))
        .unwrap();
    let mut port = publication_support::PhasePort::default();
    persist_intent(&mut machine, &mut port).unwrap();
    let mut readback = ChangedZeroSign(port.readback);
    assert_eq!(
        machine.stage_new_points(&mut port.effects, &mut readback, &operation_context()),
        Err(PublicationError::StageReadbackMismatch)
    );
    assert_eq!(
        machine.active().unwrap().phase(),
        PublicationPhase::PublicationBlocked
    );
    assert_eq!(
        port.effects.effect_kinds,
        vec![
            search_publication::PublicationEffectKind::PersistIntent,
            search_publication::PublicationEffectKind::StageNewPoints,
        ]
    );
}
