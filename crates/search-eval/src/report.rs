//! Product Pulse assembly, independent review, acceptance verdict, and receipt.
//!
//! The public surface remains crate-root compatible through this bounded facade.
//! Coverage, report assembly, independent review, verdict and receipt issuance
//! remain separate owners; none performs repository or external-system I/O.

mod assemble;
mod coverage;
mod model;
mod receipt;
mod review;
mod support;
mod verdict;

pub use assemble::*;
pub use coverage::*;
pub use model::*;
pub use receipt::*;
pub use review::*;
pub use verdict::*;
