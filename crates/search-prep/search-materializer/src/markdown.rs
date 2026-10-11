//! Closed inert `CommonMark` observations over exact retained raw source.
//!
//! The product preserves original bytes and maps, observes bounded structural
//! ranges and selected mapped values, and replays the complete inventory before
//! verification. It is not a `UnitSet` or a document qualification receipt.

mod identity;
mod model;
mod parse;
mod pipeline;
mod profile;

pub use model::{
    InlineKind as MarkdownInlineKind, InlineMetadataStatus as MarkdownInlineMetadataStatus,
    InlineRecord as MarkdownInlineRecord, MappedText as MarkdownMappedText, MarkdownError,
    MarkdownKind, MarkdownLimits, MarkdownNode, MetadataAssurance as MarkdownMetadataAssurance,
    ObservedLinkKind as MarkdownObservedLinkKind, RawSpan as MarkdownRawSpan,
    ReferenceMetadataStatus as MarkdownReferenceMetadataStatus,
};
pub use pipeline::{
    MarkdownProduct, MarkdownVerificationReceipt, materialize_markdown, verify_markdown,
};
pub use profile::{
    MAX_MARKDOWN_LIMITS, MAX_MARKDOWN_PREIMAGE_BYTES, MAX_MARKDOWN_TOTAL_CANONICAL_BYTES,
    MarkdownOperationLimits, MarkdownProfile,
};

#[cfg(test)]
mod fixtures;
