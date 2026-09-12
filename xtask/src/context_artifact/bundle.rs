//! Canonical context-artifact bundle codec.

mod model;
mod parse;
mod render;

pub use model::{BundleBlock, expected_header};
pub use parse::parse_bundle;
pub use render::render_bundle;

pub(super) fn bundle_error(
    message: impl Into<String>,
) -> super::error::ContextArtifactError {
    super::error::ContextArtifactError::new("BUNDLE_FORMAT_INVALID", message)
}
