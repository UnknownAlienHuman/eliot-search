//! Reusable synchronous typed provider client; no endpoint/credential discovery.
//! Candidate protocol values never grant authority by decoding or reachability.

mod typed;
pub use typed::{NativeClientExpectations, TypedClientError, TypedProviderSession};
#[cfg(feature = "legacy-loopback-compatibility")]
pub use typed::NativeClientBinding;

/// Conservative submission evidence for the call that returned an error.
/// Earlier outstanding operations may still have unknown outcomes.
/// Neither class proves rollback or authorizes replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutcomeClass {
    /// A local guard proves this failed call submitted no command.
    DefinitelyNotSubmitted,
    /// Submission cannot be determined from this error alone.
    Unknown,
}
