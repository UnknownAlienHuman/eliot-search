//! Mandatory pre-retrieval access gate, live barriers and grant composition.
//!
//! The public daemon composition surface is kept stable here. The access
//! kernel owns no provider transport, Qdrant SDK type, persistence or local
//! authority shortcut. Production entropy and wall-clock observations remain
//! confined to explicit adapters injected into the standalone-grant issuer.

mod gate;
mod grant;
mod grant_authority;
mod grant_command;
mod system_grant;
mod native_security;
mod native_grant_policy;
mod native_binding;
mod native_binding_listener;
mod pinned_continuation;

pub use gate::*;
pub use grant::*;
pub use grant_authority::*;
pub use grant_command::*;
pub use system_grant::*;
pub use native_security::*;
pub use native_grant_policy::*;
pub use native_binding::*;
pub use native_binding_listener::*;

use crate::provider_composition::{CanonicalServingError, CanonicalTcpGrantError};

/// Failure while performing the mandatory initial grant command and entering serving.
#[derive(Debug)]
pub enum StandaloneServingTransitionError {
    /// The authenticated first-command grant exchange failed.
    Grant(CanonicalTcpGrantError),
    /// The successfully granted transport could not enter the serving owner.
    Serving(CanonicalServingError),
}

impl From<CanonicalTcpGrantError> for StandaloneServingTransitionError {
    fn from(error: CanonicalTcpGrantError) -> Self {
        Self::Grant(error)
    }
}

impl From<CanonicalServingError> for StandaloneServingTransitionError {
    fn from(error: CanonicalServingError) -> Self {
        Self::Serving(error)
    }
}

#[cfg(test)]
mod tests;
