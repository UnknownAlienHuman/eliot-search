//! Fresh identity allocation port. Source identity interpretation stays in its owner.

use search_contracts::{SourceId, SourceNamespaceId};
use search_ports::{OperationContext, Port};

/// Caller-bound fresh UUID-v4 allocation; implementations use a qualified entropy owner.
///
/// Allocation grants no source/native access and reserves no durable identity.
/// The registry checks collisions and commits the complete assignment atomically.
/// Replay must resolve the original operation before invoking this port.
pub trait SourceIdentityIdPortV2: Port {
    /// Allocate a fresh Source ID under the original caller context.
    fn fresh_source_id(
        &mut self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<SourceId, Self::Error>;

    /// Allocate a distinct fresh namespace ID under the original caller context.
    fn fresh_source_namespace_id(
        &mut self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<SourceNamespaceId, Self::Error>;
}
