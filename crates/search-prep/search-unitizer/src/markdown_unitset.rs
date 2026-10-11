//! Complete Markdown structural observations beside unchanged exact v3 coverage.
mod budget;
mod build;
mod codec;
mod decode;
mod model;
mod profile;

pub use budget::{MAX_MARKDOWN_UNITIZER_LIMITS, MarkdownUnitizationBudget, MarkdownUnitizerLimits};
pub use build::{MarkdownUnitizationSource, unitize_markdown, verify_markdown_unit_manifest};
pub use codec::canonicalize_markdown_unit_manifest;
pub use decode::decode_markdown_unit_manifest;
pub use model::{
    CanonicalMarkdownUnitManifestBytes, MarkdownInlineObservation, MarkdownMappedValue,
    MarkdownSourceBinding, MarkdownStructuralUnit, MarkdownUnitManifest, MarkdownUnitizationError,
    MarkdownUnitizationUsage, VerifiedMarkdownUnitSet,
};
pub use profile::MarkdownUnitizerProfile;

#[cfg(test)]
mod fixtures;
