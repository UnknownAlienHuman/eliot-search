//! Effective configuration composition and truthful capability readiness.
//!
//! Daemon-specific orchestration is split by responsibility while all merge,
//! projection, validation, fingerprinting, diffing, planning and redaction
//! mechanics remain delegated to `search-config`.

mod activation;
mod bootstrap_settings;
mod capture;
mod process;
mod readiness;
mod registry;
mod root_locators;
mod snapshot;
mod spec;
pub use root_locators::RootLocatorHints;

pub use activation::*;
pub use bootstrap_settings::{
    BootstrapInvocation, BootstrapSettings, load_bootstrap_settings, looks_like_root_command,
};
pub use capture::*;
pub use process::*;
pub use readiness::*;
pub use registry::*;
pub use snapshot::*;
pub use spec::*;

#[cfg(test)]
mod tests;
