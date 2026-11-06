//! Materialization, unitization and structural enrichment ports.

use crate::{OperationContext, Port};

/// Deterministic revision materialization boundary.
pub trait MaterializerPort: Port {
    /// Exact materializer profile descriptor.
    type Profile: Send + Sync + 'static;
    /// Verified immutable revision input.
    type Revision: Send + Sync + 'static;
    /// Materialization product with maps, loss, and assurance.
    type MaterializationProduct: Send + Sync + 'static;

    /// Returns the exact immutable materializer profile.
    fn profile(&self) -> Result<Self::Profile, Self::Error>;

    /// Materializes one exact retained revision under finite context limits.
    fn materialize(
        &self,
        revision: &Self::Revision,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::MaterializationProduct, Self::Error>;
}

/// Deterministic unit-manifest boundary.
pub trait UnitizerPort: Port {
    /// Exact unitizer profile descriptor.
    type Profile: Send + Sync + 'static;
    /// Accepted materialization input.
    type Materialization: Send + Sync + 'static;
    /// Deterministic bounded unit manifest.
    type UnitManifest: Send + Sync + 'static;

    /// Returns the exact immutable unitizer profile.
    fn profile(&self) -> Result<Self::Profile, Self::Error>;

    /// Unitizes one materialization without owning semantic identity or ranking.
    fn unitize(
        &self,
        materialization: &Self::Materialization,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::UnitManifest, Self::Error>;
}

/// Qualified no-execute structural enrichment boundary.
pub trait CodeEnricherPort: Port {
    /// Exact qualified parser/enricher profile descriptor.
    type Profile: Send + Sync + 'static;
    /// Accepted representation input.
    type Representation: Send + Sync + 'static;
    /// Bounded source-anchored structural fact set.
    type StructuralFactSet: Send + Sync + 'static;

    /// Returns the exact qualified enrichment profile.
    fn profile(&self) -> Result<Self::Profile, Self::Error>;

    /// Produces tolerant syntax facts without toolchain, macro, shell, or network execution.
    fn enrich(
        &self,
        representation: &Self::Representation,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::StructuralFactSet, Self::Error>;
}
