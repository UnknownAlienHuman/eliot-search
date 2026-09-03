//! Fail-closed runtime-owner policy and state machine.
//!
//! This crate owns only pure, explicit ownership decisions. Filesystem locks,
//! durable owner records, process observation, clocks, termination, and other
//! effects are executed by qualified adapters from the returned effect plans.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

pub mod error;
pub mod identity;
pub mod lease;
pub mod state;
pub mod supervisor;
pub mod transition;

pub use error::OwnerError;
pub use identity::{
    DataRootIdentity, DataRootLocationClass, ExecutableIdentity, OwnerBinding, OwnerIdentity,
    OwnerOperation, ProcessCreationIdentity, RuntimeMode,
};
pub use lease::{
    DependencyComponent, DependencyShutdownReceipt, DrainReason, DrainToken, LeaseWindow,
    OwnerGuard, OwnerHealth, OwnerHealthReason, OwnerHealthState, OwnerLifecycle, OwnerRecord,
    OwnerShutdownReceipt, OwnerVerificationReceipt, ReleasePermit,
};
pub use state::{DrainFence, OwnerSnapshot, OwnerState, PendingAcquire, ReleaseFence};
pub use supervisor::OwnerSupervisor;
pub use transition::{
    AcquireCommitObservation, AcquirePlan, AcquireRecovery, AcquireRequest, AcquireResolution,
    LiveOwnerStatus, ModeChangeDecision, OwnerEffect, OwnerObservation, RecoveryDecision,
    RecoveryEvidence, RecoveryPolicy, ReleaseCommitObservation, ReleasePlan, ReleaseResolution,
    RenewalReceipt, classify_abandoned_owner, classify_owner_mutation_boundary, complete_acquire,
    complete_release, owner_health, plan_mode_or_root_change, prepare_acquire, prepare_release,
    recover_acquisition, recover_release, renew_verified, verify_owner_guard,
    verify_release_preconditions,
};
