//! Checkpointed retention and owner-governed reconstitution of one complete v2
//! admission policy.
//!
//! [`RetainedPolicyV2`] keeps complete normalized policy inputs and reconstructs
//! them through the real validator/compiler. [`OriginalAdmissionV2`] references
//! that shared policy and reconstructs the full observation/decision/receipt
//! through their existing owners, then compares exact complete archive bytes.
//!
//! These pure archives grant no currentness or native capability. The actual
//! registry/normal readback consumer remains unfinished under #569. No filesystem,
//! clock, deadline renewal, second evaluator or database exists in this package.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod admission;
mod codec;
mod fragments;
mod policy;

pub use admission::{
    MAX_RETAINED_ADMISSION_BYTES, ORIGINAL_ADMISSION_PROFILE_V2, OriginalAdmissionV2,
};
pub use fragments::{
    ORIGINAL_INPUT_FRAGMENT_BYTES_V2, ObservedOriginalInputPart, OriginalInputPartV2,
    OriginalInputsLayoutV2,
};
pub use policy::{MAX_RETAINED_POLICY_BYTES, RETAINED_POLICY_PROFILE_V2, RetainedPolicyV2};

#[cfg(test)]
mod admission_tests;
#[cfg(test)]
mod storage_proof;
