//! Sealed recovery composition behind the stable facade.

mod api;
mod platform;
mod report;
mod spec;

pub use api::recover_all;
pub use report::{RecoveryIssue, SealedRecoveryReport};
pub use spec::{
    MAX_RECOVERY_ISSUES, MAX_RECOVERY_OPERATIONS, RecoveryIssueCode,
    SealedRecoveryError,
};

#[cfg(test)]
mod tests;
