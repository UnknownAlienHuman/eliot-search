//! Source selectors, public tokens and native-coordinate schemas.

use search_contracts::{
    ArchiveMemberAnchor, BufferRangeAnchor, ContinuationHandle, CorpusOrPortfolioId,
    ExactScanPlanRef, GitBlobBytesAnchor, GitCommitSourceView, NativeAnchor, PdfRegionAnchor,
    SearchSourceHandle, SourceView, TextBytesAnchor, WorkspaceViewSource,
};

use crate::error::ProtocolError;
use super::wire::{Decoder, Encoder, Result, Schema, record, tagged};

record!(WorkspaceViewSource { workspace_instance_id, workspace_view_revision_ref });
record!(GitCommitSourceView { workspace_instance_id, git_commit_oid });
impl Schema for SourceView {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        match self {
            Self::WorkingTreeCurrent(value) => {
                output.tag("working_tree_current")?;
                value.put(output)?;
            }
            Self::GitIndex(value) => {
                output.tag("git_index")?;
                value.put(output)?;
            }
            Self::GitCommit(value) => {
                output.tag("git_commit")?;
                value.put(output)?;
            }
            Self::ImportedSnapshot(value) => {
                output.tag("imported_snapshot")?;
                put_named("imported_snapshot_id", value, output)?;
            }
            Self::RetainedRevision(value) => {
                output.tag("retained_revision")?;
                put_named("retained_revision_id", value, output)?;
            }
        }
        output.close(b'}')
    }

    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let value = match input.tag()?.as_str() {
            "working_tree_current" => Self::WorkingTreeCurrent(Schema::get(input)?),
            "git_index" => Self::GitIndex(Schema::get(input)?),
            "git_commit" => Self::GitCommit(Schema::get(input)?),
            "imported_snapshot" => Self::ImportedSnapshot(get_named("imported_snapshot_id", input)?),
            "retained_revision" => Self::RetainedRevision(get_named("retained_revision_id", input)?),
            _ => return Err(ProtocolError::InvalidBody),
        };
        input.close(b'}')?;
        Ok(value)
    }
}

fn put_named<T: Schema>(name: &str, value: &T, output: &mut Encoder) -> Result<()> {
    output.open(b'{')?;
    let mut first = true;
    output.field(&mut first, name)?;
    value.put(output)?;
    output.close(b'}')
}

fn get_named<T: Schema>(name: &str, input: &mut Decoder<'_>) -> Result<T> {
    input.open(b'{')?;
    let mut first = true;
    input.field(&mut first, name)?;
    let value = T::get(input)?;
    input.close(b'}')?;
    Ok(value)
}
tagged!(CorpusOrPortfolioId { Corpus => "corpus", Portfolio => "portfolio" });
record!(SearchSourceHandle { handle_id, handle_revision, handle_class, expires_at, opaque_token });
record!(ContinuationHandle { continuation_id, expires_at, opaque_token });
record!(ExactScanPlanRef { plan_id, plan_fingerprint });
record!(TextBytesAnchor { content_digest, byte_start_0, byte_end_exclusive_0 });
record!(GitBlobBytesAnchor {
    repository_lineage_id, commit_oid, path_bytes, byte_start_0, byte_end_exclusive_0,
});
record!(BufferRangeAnchor {
    buffer_snapshot_id, buffer_version, position_encoding,
    start_line_0, start_character_0, end_line_0, end_character_0,
});
// P00 has a fixed coordinate-space discriminator which is not stored as a
// mutable Rust field. Keep it explicit on the wire and reject other spaces.
impl Schema for PdfRegionAnchor {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        output.open(b'{')?;
        let mut first = true;
        output.field(&mut first, "source_revision_id")?;
        self.source_revision_id.put(output)?;
        output.field(&mut first, "page_1")?;
        self.page_1.put(output)?;
        output.field(&mut first, "coordinate_space")?;
        output.text("crop_box_points_after_rotation")?;
        output.field(&mut first, "x0")?;
        self.x0.put(output)?;
        output.field(&mut first, "y0")?;
        self.y0.put(output)?;
        output.field(&mut first, "x1")?;
        self.x1.put(output)?;
        output.field(&mut first, "y1")?;
        self.y1.put(output)?;
        output.close(b'}')
    }

    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        input.open(b'{')?;
        let mut first = true;
        input.field(&mut first, "source_revision_id")?;
        let source_revision_id = Schema::get(input)?;
        input.field(&mut first, "page_1")?;
        let page_1 = Schema::get(input)?;
        input.field(&mut first, "coordinate_space")?;
        input.literal(b"\"crop_box_points_after_rotation\"")?;
        input.field(&mut first, "x0")?;
        let x0 = Schema::get(input)?;
        input.field(&mut first, "y0")?;
        let y0 = Schema::get(input)?;
        input.field(&mut first, "x1")?;
        let x1 = Schema::get(input)?;
        input.field(&mut first, "y1")?;
        let y1 = Schema::get(input)?;
        input.close(b'}')?;
        Ok(Self { source_revision_id, page_1, x0, y0, x1, y1 })
    }
}
record!(ArchiveMemberAnchor { archive_revision_id, member_path_bytes, nested_anchor });

impl Schema for NativeAnchor {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        self.validate().map_err(|_| ProtocolError::InvalidBody)?;
        match self {
            Self::TextBytes(value) => { output.tag("text_bytes")?; value.put(output)?; }
            Self::GitBlobBytes(value) => { output.tag("git_blob_bytes")?; value.put(output)?; }
            Self::BufferRange(value) => { output.tag("buffer_range")?; value.put(output)?; }
            Self::PdfRegion(value) => { output.tag("pdf_region")?; value.put(output)?; }
            Self::ArchiveMember(value) => { output.tag("archive_member")?; value.put(output)?; }
        }
        output.close(b'}')
    }

    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let value = match input.tag()?.as_str() {
            "text_bytes" => Self::TextBytes(Schema::get(input)?),
            "git_blob_bytes" => Self::GitBlobBytes(Schema::get(input)?),
            "buffer_range" => Self::BufferRange(Schema::get(input)?),
            "pdf_region" => Self::PdfRegion(Schema::get(input)?),
            "archive_member" => Self::ArchiveMember(Schema::get(input)?),
            _ => return Err(ProtocolError::InvalidBody),
        };
        input.close(b'}')?;
        value.validate().map_err(|_| ProtocolError::InvalidBody)?;
        Ok(value)
    }
}
