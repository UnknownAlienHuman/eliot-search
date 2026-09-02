//! Search-index administration, exact mutation, query, and epoch-pin ports.

use search_contracts::{Epoch, OpaqueId};

use crate::{BoundedStream, MutationIdentity, OperationContext, PackageOpaque, Port};

/// Exact search-index data-plane boundary.
pub trait SearchIndexPort: Port {
    /// Capability/schema probe receipt.
    type CapabilityReceipt: Send + Sync + 'static;
    /// Exact desired schema descriptor.
    type Schema: Send + Sync + 'static;
    /// Verified schema receipt.
    type SchemaReceipt: Send + Sync + 'static;
    /// Finite exact upsert batch.
    type UpsertBatch: Send + Sync + 'static;
    /// Finite exact point-identity set.
    type PointIds: Send + Sync + 'static;
    /// Mutation write policy.
    type WritePolicy: Send + Sync + 'static;
    /// Content-free exact mutation receipt.
    type MutationReceipt: Send + Sync + 'static;
    /// Exact point readback.
    type PointReadback: Send + Sync + 'static;
    /// One already-authorized safe query leg.
    type QueryLeg: Send + Sync + 'static;
    /// One bounded index candidate nomination.
    type IndexCandidate: Send + Sync + 'static;
    /// Process-local candidate stream capability.
    type CandidateStreamRef: PackageOpaque;
    /// Exact filtered-count predicate.
    type CountFilter: Send + Sync + 'static;

    /// Probes exact runtime capabilities without inferring them from version text.
    fn probe_capabilities(
        &self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::CapabilityReceipt, Self::Error>;

    /// Ensures the exact accepted schema under one mutation identity.
    fn ensure_schema(
        &mut self,
        schema: &Self::Schema,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::SchemaReceipt, Self::Error>;

    /// Upserts only an exact finite batch.
    fn upsert_exact(
        &mut self,
        batch: &Self::UpsertBatch,
        write_policy: &Self::WritePolicy,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::MutationReceipt, Self::Error>;

    /// Closes only exact point identities at one epoch.
    fn close_exact(
        &mut self,
        ids: &Self::PointIds,
        epoch: Epoch,
        write_policy: &Self::WritePolicy,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::MutationReceipt, Self::Error>;

    /// Reads back only exact point identities.
    fn readback_exact(
        &self,
        ids: &Self::PointIds,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::PointReadback, Self::Error>;

    /// Executes one authorized safe leg and returns a finite nomination stream.
    fn query(
        &self,
        leg: &Self::QueryLeg,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<BoundedStream<Self::IndexCandidate, Self::CandidateStreamRef>, Self::Error>;

    /// Counts the exact already-authorized filter population.
    fn exact_count(
        &self,
        filter: &Self::CountFilter,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<u64, Self::Error>;
}

/// Exact-ID search-index administrative boundary.
pub trait SearchIndexAdminPort: Port {
    /// Finite exact point-identity set.
    type PointIds: Send + Sync + 'static;
    /// Mutation write policy.
    type WritePolicy: Send + Sync + 'static;
    /// Content-free exact deletion receipt.
    type MutationReceipt: Send + Sync + 'static;
    /// Route descriptor.
    type Route: Send + Sync + 'static;
    /// Exact route validation receipt.
    type RouteValidationReceipt: Send + Sync + 'static;

    /// Deletes only exact point identities; broad correctness-path delete is absent.
    fn delete_exact(
        &mut self,
        ids: &Self::PointIds,
        write_policy: &Self::WritePolicy,
        context: &OperationContext<Self::Cancellation>,
        mutation: &MutationIdentity,
    ) -> Result<Self::MutationReceipt, Self::Error>;

    /// Validates exact route identity and readiness.
    fn validate_route(
        &self,
        route: &Self::Route,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::RouteValidationReceipt, Self::Error>;
}

/// Process-local route and epoch pin boundary.
pub trait EpochPinPort: Port {
    /// Exact collection route.
    type Route: Send + Sync + 'static;
    /// Opaque request/handle/continuation owner identity.
    type PinOwner: Send + Sync + 'static;
    /// Process-local epoch guard.
    type EpochPinGuard: PackageOpaque;
    /// Process-local route guard.
    type RoutePinGuard: PackageOpaque;
    /// Reclamation watermark derived from all active guards.
    type ReclamationWatermark: Send + Sync + 'static;
    /// Content-free release receipt.
    type ReleaseReceipt: Send + Sync + 'static;

    /// Acquires a finite guard for one exact route and epoch.
    fn acquire_epoch_pin(
        &mut self,
        route: &Self::Route,
        epoch: Epoch,
        owner: &Self::PinOwner,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::EpochPinGuard, Self::Error>;

    /// Acquires a finite guard for one exact route generation.
    fn acquire_route_pin(
        &mut self,
        route: &Self::Route,
        owner: &Self::PinOwner,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::RoutePinGuard, Self::Error>;

    /// Reads the exact safe reclamation watermark for a route.
    fn reclamation_watermark(
        &self,
        route: &Self::Route,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::ReclamationWatermark, Self::Error>;

    /// Releases every process-local guard owned by the exact owner identity.
    fn release_owner(
        &mut self,
        owner: &Self::PinOwner,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::ReleaseReceipt, Self::Error>;
}

/// Stable helper for packages that use an opaque owner identifier directly.
pub type DefaultPinOwner = OpaqueId;
