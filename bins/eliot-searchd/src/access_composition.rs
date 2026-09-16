//! Mandatory pre-retrieval access gate, live barriers and grant composition.
//!
//! The public daemon composition surface is kept stable here. The access
//! kernel owns no provider transport, Qdrant SDK type, persistence or local
//! authority shortcut. Production entropy and wall-clock observations remain
//! confined to explicit adapters injected into the standalone-grant issuer.

mod gate;
mod grant;
mod system_grant;

pub use gate::*;
pub use grant::*;
pub use system_grant::*;

#[cfg(test)]
mod tests;
