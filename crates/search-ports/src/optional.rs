//! Explicitly gated optional model-provider port.

use crate::{OperationContext, Port};

/// Optional local model encoding and finite rerank boundary.
///
/// Implementations are called only after an exact accepted profile binding.
/// Output may nominate or reorder a finite input set; it never creates source
/// evidence, authority, exact identity, or complete-negative proof.
pub trait ModelProviderPort: Port {
    /// Exact immutable model/runtime/tokenizer profile descriptor.
    type Profile: Send + Sync + 'static;
    /// Bounded model encoding input.
    type EncodeInput: Send + Sync + 'static;
    /// Validated bounded vector product.
    type VectorProduct: Send + Sync + 'static;
    /// Finite authorized rerank request.
    type RerankRequest: Send + Sync + 'static;
    /// Rerank result constrained to the finite request candidate set.
    type RerankResult: Send + Sync + 'static;

    /// Returns the exact admitted optional profile.
    fn profile(&self) -> Result<Self::Profile, Self::Error>;

    /// Encodes one bounded minimized input.
    fn encode(
        &self,
        input: &Self::EncodeInput,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::VectorProduct, Self::Error>;

    /// Reorders or removes only members of the finite input candidate set.
    fn rerank(
        &self,
        request: &Self::RerankRequest,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Self::RerankResult, Self::Error>;
}
