//! Mandatory pre-retrieval access gate, live barriers and grant composition.
//!
//! The public daemon composition surface is kept stable here. The production
//! modules own no provider transport, Qdrant SDK type, persistence, local
//! authority shortcut, CSPRNG or clock. Exact grant identity/time material is
//! supplied only through the injected standalone-grant issuer boundary.

mod gate;
mod grant;

pub use gate::*;
pub use grant::*;

#[cfg(test)]
mod tests;
