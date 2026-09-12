//! Live DIRECT source composition over canonical source-owner kernels.
//!
//! The daemon owns path classification, safe-read observations and orchestration.
//! Admission policy/receipts are owned by `search-source-admission`; durable
//! identity formulas and matching are owned by `search-source-identity`.

mod admission;
mod classifier;
mod identity;
mod registry;

pub(crate) use admission::{
    AdmissionPolicy, SOURCE_ADMISSION_DENIED, SourceAdmissionConfig,
};
pub(crate) use registry::{PriorSourceView, RegistryView, plan_snapshot};
