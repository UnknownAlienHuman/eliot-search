//! Mandatory pre-retrieval access gate and live-barrier path (T20).
//!
//! The public daemon composition surface is kept stable here. The production
//! gate owns no provider transport, Qdrant SDK type, persistence, or local
//! authority shortcut; the regression corpus is isolated from production code.

mod gate;
pub use gate::*;

#[cfg(test)]
mod tests;
