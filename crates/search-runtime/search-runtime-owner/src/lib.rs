//! Plain local root owner plus legacy pure policy retained for unconverted
//! direct callers. The plain implementation lives in plain_root and owns
//! one retained lock, one final manifest, the current object directory and
//! one control database validated by search-control-redb. Legacy lease/epoch/receipt modules are
//! not used by the plain root and remain only until their last direct
//! caller is converted.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

pub mod admission;
pub mod error;
pub mod identity;
pub mod lease;
pub mod plain_root;
pub mod state;
pub mod supervisor;
pub mod transition;

pub use plain_root::{
    CONTROL_DB_FILE, CONTROL_DIR, INIT_MARKER, INCOMPLETE_IMPORT_MARKER, LOCK_FILE,
    MANIFEST_FILE, OBJECT_DIR, PlainRootError, PlainRootGuard, PlainRootInspection,
    PlainRootResult, PlainRootStatus, discard_incomplete_plain, initialize_plain, inspect_plain,
    open_plain,
};

pub use admission::{DataRootOpenMode, RootOpenDecision, RootOpenState, classify_root_open};
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
    RenewalReceipt, begin_drain, classify_abandoned_owner, classify_owner_mutation_boundary,
    complete_acquire, complete_release, owner_health, plan_mode_or_root_change, prepare_acquire,
    prepare_release, recover_acquisition, recover_release, renew_verified, verify_owner_guard,
    verify_release_preconditions,
};
