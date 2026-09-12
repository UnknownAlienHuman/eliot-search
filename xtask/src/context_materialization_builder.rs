//! Executable non-authoritative `materialize_context_v1` plan compiler.
//!
//! Inputs are canonical local candidate/bundle/selection artifacts. Output is
//! restricted to ignored advisory files. The compiler never writes a context
//! manifest control record, creates authority or accepts a package/gate/wave.

mod assemble;
mod input;
mod manifest;
mod model;
mod write;

pub use assemble::build_plan;
pub use model::MaterializationBuild;
pub use write::write_plan;
