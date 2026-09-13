//! Materialization request validation, finite budgets and cancellation.
//!
//! Public request names remain stable while budget mechanics, cancellation,
//! request models, validation and tests have separate private owners.

mod budget;
mod cancellation;
mod model;
mod validate;

pub use budget::{DEFAULT_MATERIALIZATION_BUDGET, MaterializationBudget};
pub use cancellation::CancellationToken;
pub use model::{
    AcceptedProfiles, MaterializationRequest, ValidatedMaterializationRequest,
};
pub use validate::validate_materialization_request;

#[cfg(test)]
mod tests;
