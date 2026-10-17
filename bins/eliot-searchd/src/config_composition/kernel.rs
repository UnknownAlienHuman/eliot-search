//! Effective configuration composition and truthful capability readiness.
//!
//! Daemon-specific orchestration is split by responsibility while all merge,
//! projection, validation, fingerprinting, diffing, planning and redaction
//! mechanics remain delegated to `search-config`.

mod activation;
mod capture;
mod process;
mod readiness;
mod registry;
mod snapshot;
mod spec;
mod root_locators;
mod native_qdrant_settings;
pub use native_qdrant_settings::NativeQdrantSettings;
pub use root_locators::RootLocatorHints;

pub use activation::*;
pub use capture::*;
pub use process::*;
pub use readiness::*;
pub use registry::*;
pub use snapshot::*;
pub use spec::*;

#[cfg(test)]
mod tests;
