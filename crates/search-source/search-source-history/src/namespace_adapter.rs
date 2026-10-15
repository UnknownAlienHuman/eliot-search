//! Process-local observed namespace adapter, without current or durable authority.
use search_contracts::{SourceNamespaceId, SourceNamespaceOwnership, SourceOwnerCutoverReceipt};
use search_ports::{
    CancellationProbe, MutationIdentity, OperationContext, Port, PortErrorKind, PortRetryability,
    SourceOwnershipPort,
};
use search_source_records::error::{RegistryError, RegistryPortError, registry_port_error};
use search_source_records::namespace::{
    NamespaceCutoverState, NamespaceOwnershipCommand, VerifiedCutoverReceipt,
};
use std::collections::{BTreeMap, BTreeSet};
/// Process-local observed ownership adapter; it provides no durable or live authority.
pub struct RegistryOwnershipAdapter {
    states: BTreeMap<SourceNamespaceId, NamespaceCutoverState>,
}

impl RegistryOwnershipAdapter {
    /// Creates an adapter over durable ownership states.
    #[must_use]
    pub fn new(states: BTreeMap<SourceNamespaceId, NamespaceCutoverState>) -> Self {
        Self { states }
    }

    /// Exact ownership states.
    #[must_use]
    pub const fn states(&self) -> &BTreeMap<SourceNamespaceId, NamespaceCutoverState> {
        &self.states
    }
}

impl Port for RegistryOwnershipAdapter {
    type Error = RegistryPortError;
    type Cancellation = search_ports::FakeCancellation;
}

impl SourceOwnershipPort for RegistryOwnershipAdapter {
    type Namespace = SourceNamespaceId;
    type TransitionCommand = NamespaceOwnershipCommand;
    type CutoverVerificationReceipt = VerifiedCutoverReceipt;

    fn read_namespace_owner(
        &self,
        namespace: &Self::Namespace,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<SourceNamespaceOwnership, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                RegistryError::CancelledBeforeCommit,
                None,
            ));
        }
        self.states
            .get(namespace)
            .map(|state| state.ownership.clone())
            .ok_or_else(|| {
                registry_port_error(
                    PortErrorKind::InvalidInput,
                    PortRetryability::AfterRefresh,
                    RegistryError::NamespaceOwnershipConflict,
                    None,
                )
            })
    }

    fn transition_namespace_owner(
        &mut self,
        command: &Self::TransitionCommand,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<SourceNamespaceOwnership, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                RegistryError::CancelledBeforeCommit,
                Some(mutation.operation_id.clone()),
            ));
        }
        let _ = command;
        Err(registry_port_error(
            PortErrorKind::InvalidInput,
            PortRetryability::AfterRefresh,
            RegistryError::CutoverRequired,
            Some(mutation.operation_id.clone()),
        ))
    }

    fn verify_cutover_receipt(
        &self,
        receipt: &SourceOwnerCutoverReceipt,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::CutoverVerificationReceipt, Self::Error> {
        if context.cancellation().is_cancelled() {
            return Err(registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                RegistryError::CancelledBeforeCommit,
                None,
            ));
        }
        receipt.validate().map_err(|_| {
            registry_port_error(
                PortErrorKind::InvalidInput,
                PortRetryability::Never,
                RegistryError::CutoverReceiptMismatch,
                None,
            )
        })?;
        let old_state = self
            .states
            .get(&receipt.cutover.source_namespace_id)
            .map(|state| state.ownership.clone())
            .ok_or_else(|| {
                registry_port_error(
                    PortErrorKind::InvalidInput,
                    PortRetryability::AfterRefresh,
                    RegistryError::CutoverReceiptMismatch,
                    None,
                )
            })?;
        let covered: BTreeSet<SourceNamespaceId> =
            BTreeSet::from([receipt.cutover.source_namespace_id]);
        let _ = covered;
        Ok(VerifiedCutoverReceipt {
            cutover_id: receipt.cutover.cutover_id,
            namespace_id: receipt.cutover.source_namespace_id,
            old_generation: old_state.source_owner_generation,
            new_generation: receipt.new_owner.source_owner_generation_after_activation,
            covered_sources: 0,
            covered_memberships: 0,
        })
    }
}
