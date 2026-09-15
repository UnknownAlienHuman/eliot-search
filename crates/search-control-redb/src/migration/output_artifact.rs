//! Package-owned lifecycle for one inactive source-mapping artifact.
//!
//! The caller still owns source replay and exact redb row verification. This
//! owner controls pending/final locators, opened-object identity, hard-link
//! publication, directory durability and crash-alias cleanup under the exact
//! per-artifact output lock.

mod io;
mod lifecycle;
mod model;

pub use lifecycle::SourceImportOutputArtifact;
pub use model::{
    SourceImportOutputArtifactError, SourceImportOutputArtifactPlatform,
    SourceImportPendingArtifact, SourceImportPublishedArtifact,
};

#[cfg(test)]
mod tests;
