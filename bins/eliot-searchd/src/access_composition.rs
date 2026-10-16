//! Mandatory pre-retrieval access gate, live barriers and grant composition.
//!
//! The public daemon composition surface is kept stable here. The access
//! kernel owns no provider transport, Qdrant SDK type, persistence or local
//! authority shortcut. Production entropy and wall-clock observations remain
//! confined to explicit adapters injected into the standalone-grant issuer.

#[cfg(feature = "wave4-query")]
mod gate;
#[cfg(feature = "wave4-query")]
mod grant;
#[cfg(feature = "wave4-query")]
mod grant_authority;
#[cfg(feature = "wave4-query")]
mod grant_command;
#[cfg(feature = "wave4-query")]
mod system_grant;
mod native_security;
#[cfg(feature = "wave4-query")]
mod native_grant_policy;
#[cfg(feature = "wave4-query")]
mod native_binding;
#[cfg(feature = "wave4-query")]
mod native_binding_listener;
#[cfg(feature = "wave4-query")]
mod pinned_continuation;

#[cfg(feature = "wave4-query")]
pub use gate::*;
#[cfg(feature = "wave4-query")]
pub use grant::*;
#[cfg(feature = "wave4-query")]
pub use grant_authority::*;
#[cfg(feature = "wave4-query")]
pub use grant_command::*;
#[cfg(feature = "wave4-query")]
pub use system_grant::*;
pub use native_security::*;
#[cfg(feature = "wave4-query")]
pub use native_grant_policy::*;
#[cfg(feature = "wave4-query")]
pub use native_binding::*;
#[cfg(feature = "wave4-query")]
pub use native_binding_listener::*;

#[cfg(feature = "wave4-query")]
use crate::provider_composition::{CanonicalServingError, CanonicalTcpGrantError};

/// Failure while performing the mandatory initial grant command and entering serving.
#[derive(Debug)]
#[cfg(feature = "wave4-query")]
pub enum StandaloneServingTransitionError {
    /// The authenticated first-command grant exchange failed.
    Grant(CanonicalTcpGrantError),
    /// The successfully granted transport could not enter the serving owner.
    Serving(CanonicalServingError),
}

#[cfg(feature = "wave4-query")]
impl From<CanonicalTcpGrantError> for StandaloneServingTransitionError {
    fn from(error: CanonicalTcpGrantError) -> Self {
        Self::Grant(error)
    }
}

#[cfg(feature = "wave4-query")]
impl From<CanonicalServingError> for StandaloneServingTransitionError {
    fn from(error: CanonicalServingError) -> Self {
        Self::Serving(error)
    }
}

#[cfg(all(test, feature = "wave4-query"))]
mod tests;
