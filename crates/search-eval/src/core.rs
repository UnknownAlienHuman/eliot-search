//! Control corpus, metric registry, preregistered policy, and frozen runs.
//!
//! The public surface stays crate-root compatible through this bounded facade.
//! Each private module owns one evaluation responsibility and no module performs
//! filesystem, process, network, Qdrant, or authority-state mutation.

mod baseline;
mod corpus;
mod limits;
mod policy;
mod registry;
mod run;

pub use baseline::*;
pub use corpus::*;
pub use limits::*;
pub use policy::*;
pub use registry::*;
pub use run::*;
