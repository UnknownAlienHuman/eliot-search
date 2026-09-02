//! Materialization, unitization, structural enrichment, and lexical encoding ports.

use search_contracts::Blake3Digest32;

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

/// Exact sparse lexical encoder boundary.
pub trait LexicalEncoderPort: Port {
    /// Exact analyzer/vector profile descriptor.
    type Profile: Send + Sync + 'static;
    /// Document encoding input.
    type DocumentInput: Send + Sync + 'static;
    /// Query encoding input.
    type QueryInput: Send + Sync + 'static;
    /// Deterministic bounded sparse vector.
    type SparseVector: Send + Sync + 'static;

    /// Returns the exact lexical profile.
    fn profile(&self) -> Result<Self::Profile, Self::Error>;

    /// Encodes one document using the exact document analyzer profile.
    fn encode_document(
        &self,
        input: &Self::DocumentInput,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::SparseVector, Self::Error>;

    /// Encodes one query using the exact query analyzer profile.
    fn encode_query(
        &self,
        input: &Self::QueryInput,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::SparseVector, Self::Error>;

    /// Returns the immutable golden-fixture digest for the active profile.
    fn fixture_digest(&self) -> Result<Blake3Digest32, Self::Error>;
}
