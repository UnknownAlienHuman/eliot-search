//! Live DIRECT source composition over canonical source-owner kernels.
//!
//! The daemon owns path classification, safe-read observations and orchestration.
//! Admission policy/receipts are owned by `search-source-admission`; durable
//! identity formulas and matching are owned by `search-source-identity`.

#[path = "composition/admission.rs"]
mod admission;
#[path = "composition/classifier.rs"]
mod classifier;
#[path = "composition/identity.rs"]
mod identity;
#[path = "composition/registry.rs"]
mod registry;

pub(crate) use admission::{
    AdmissionPolicy, SOURCE_ADMISSION_DENIED, SourceAdmissionConfig,
};
pub(crate) use identity::derive_git_stable_digest;
pub(crate) use registry::{PriorSourceView, RegistryView, plan_snapshot};
