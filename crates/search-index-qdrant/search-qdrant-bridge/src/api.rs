use std::collections::BTreeMap;

use search_contracts::OpaqueId;

use crate::admin::CollectionState;
use crate::{
    AuthLeaseEvidence, BridgeEndpoint, BridgeError, BridgeLimits,
    CollectionRoute, MutationReceipt, QdrantCapabilityReceipt,
    SupervisorReceipt,
};

/// Deterministic reference bridge after capability admission.
#[derive(Clone, Debug)]
pub struct QdrantBridge {
    supervisor: SupervisorReceipt,
    capability: QdrantCapabilityReceipt,
    pub(crate) limits: BridgeLimits,
    pub(crate) collections: BTreeMap<CollectionRoute, CollectionState>,
    pub(crate) operations: BTreeMap<OpaqueId, MutationReceipt>,
}

impl QdrantBridge {
    /// Connects only to the exact authenticated loopback process.
    pub fn connect(
        endpoint: BridgeEndpoint,
        auth: AuthLeaseEvidence,
        supervisor: SupervisorReceipt,
        capability: QdrantCapabilityReceipt,
        limits: BridgeLimits,
    ) -> Result<Self, BridgeError> {
        if !endpoint.loopback_only {
            return Err(BridgeError::EndpointNotLoopback);
        }
        if !auth.valid {
            return Err(BridgeError::AuthenticationInvalid);
        }
        if endpoint.endpoint_digest != supervisor.endpoint_digest {
            return Err(BridgeError::SupervisorReceiptMismatch);
        }
        if capability.process_identity_digest != supervisor.process_identity_digest
            || capability.artifact_digest != supervisor.artifact_digest
            || !capability.results.all_required()
        {
            return Err(BridgeError::CapabilityReceiptMismatch);
        }
        Ok(Self {
            supervisor,
            capability,
            limits: limits.validate()?,
            collections: BTreeMap::new(),
            operations: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn capability_receipt(&self) -> QdrantCapabilityReceipt {
        self.capability
    }

    #[must_use]
    pub const fn supervisor_receipt(&self) -> SupervisorReceipt {
        self.supervisor
    }
}
