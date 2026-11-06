//! Bounded actual source/page/evidence storage encodings.

use search_contracts::{Blake3Digest32, SourceId, SourceNamespaceId};
use search_source_registry::{MAX_SOURCE_OBJECT_BYTES, ObjectKind, ObjectRef};

use super::{
    CapturedSource, PointEvidence, PublicationError as E, Result, SourceEntry, UploadPage,
    UploadPoint,
};

const MAX_PAGE_BYTES: usize = 1_048_576;
const MAX_PAGE_POINTS: usize = 128;
const MAX_TEXT_BYTES: usize = 65_536;
const PAGE_MAGIC: &[u8; 8] = b"ELSUPG01";
const EVIDENCE_BYTES: usize = 160;

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, size: usize) -> Result<&'a [u8]> {
        let end = self.offset.checked_add(size).ok_or(E::Corrupt)?;
        let value = self.bytes.get(self.offset..end).ok_or(E::Corrupt)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| E::Corrupt)
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn boolean(&mut self) -> Result<bool> {
        match self.take(1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(E::Corrupt),
        }
    }
    const fn done(&self) -> Result<()> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(E::Corrupt)
        }
    }
}
fn put_object(out: &mut Vec<u8>, object: ObjectRef) {
    out.extend_from_slice(&object.format_version.to_le_bytes());
    out.push(object.kind.tag());
    out.extend_from_slice(object.digest.as_bytes());
    out.extend_from_slice(&object.byte_length.to_le_bytes());
}
fn object(cursor: &mut Cursor<'_>, kind: ObjectKind) -> Result<ObjectRef> {
    let version = cursor.u16()?;
    let actual_kind = ObjectKind::from_tag(cursor.take(1)?[0]).ok_or(E::Corrupt)?;
    let digest = Blake3Digest32::from_bytes(cursor.array()?);
    let byte_length = cursor.u64()?;
    let bound = if kind == ObjectKind::Source {
        MAX_SOURCE_OBJECT_BYTES
    } else {
        64 * 1024 * 1024
    };
    if version != 1
        || actual_kind != kind
        || byte_length > bound
        || (kind == ObjectKind::Prepared && byte_length == 0)
    {
        return Err(E::Corrupt);
    }
    Ok(ObjectRef {
        format_version: version,
        kind,
        digest,
        byte_length,
    })
}
fn put_evidence(out: &mut Vec<u8>, e: &PointEvidence) {
    out.extend_from_slice(e.source_namespace_id.as_bytes());
    out.extend_from_slice(e.source_id.as_bytes());
    put_object(out, e.input);
    put_object(out, e.prepared);
    for value in [e.unit, e.start, e.end, e.line_start, e.line_end] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.push(u8::from(e.starts_at_line_boundary));
    out.push(u8::from(e.ends_at_line_boundary));
}
fn evidence(cursor: &mut Cursor<'_>) -> Result<PointEvidence> {
    let e = PointEvidence {
        source_namespace_id: SourceNamespaceId::from_bytes(cursor.array()?),
        source_id: SourceId::from_bytes(cursor.array()?),
        input: object(cursor, ObjectKind::Source)?,
        prepared: object(cursor, ObjectKind::Prepared)?,
        unit: cursor.u64()?,
        start: cursor.u64()?,
        end: cursor.u64()?,
        line_start: cursor.u64()?,
        line_end: cursor.u64()?,
        starts_at_line_boundary: cursor.boolean()?,
        ends_at_line_boundary: cursor.boolean()?,
    };
    validate_evidence(&e)?;
    Ok(e)
}
fn validate_evidence(e: &PointEvidence) -> Result<()> {
    if e.input.format_version != 1
        || e.input.kind != ObjectKind::Source
        || e.input.byte_length > MAX_SOURCE_OBJECT_BYTES
        || e.prepared.format_version != 1
        || e.prepared.kind != ObjectKind::Prepared
        || e.prepared.byte_length == 0
        || e.prepared.byte_length > 64 * 1024 * 1024
        || e.start >= e.end
        || e.end > e.input.byte_length
        || e.end - e.start > MAX_TEXT_BYTES as u64
        || e.line_start >= e.line_end
    {
        return Err(E::Corrupt);
    }
    Ok(())
}
pub(super) fn encode_evidence(e: &PointEvidence) -> Vec<u8> {
    let mut out = Vec::with_capacity(EVIDENCE_BYTES);
    put_evidence(&mut out, e);
    out
}
pub(super) fn decode_evidence(bytes: &[u8]) -> Result<PointEvidence> {
    if bytes.len() != EVIDENCE_BYTES {
        return Err(E::Corrupt);
    }
    let mut cursor = Cursor::new(bytes);
    let e = evidence(&mut cursor)?;
    cursor.done()?;
    Ok(e)
}
pub(super) fn page_size(page: &UploadPage) -> Result<usize> {
    if page.points.is_empty() || page.points.len() > MAX_PAGE_POINTS {
        return Err(E::Limit);
    }
    let mut size = 12_usize;
    for point in &page.points {
        let e = &point.evidence;
        if point.retrieval_text.is_empty()
            || point.retrieval_text.len() > MAX_TEXT_BYTES
            || u64::try_from(point.retrieval_text.len()).ok() != e.end.checked_sub(e.start)
        {
            return Err(E::Corrupt);
        }
        validate_evidence(e)?;
        size = size
            .checked_add(16 + EVIDENCE_BYTES + 4)
            .and_then(|n| n.checked_add(point.retrieval_text.len()))
            .ok_or(E::Limit)?;
        if size > MAX_PAGE_BYTES {
            return Err(E::Limit);
        }
    }
    Ok(size)
}
pub(super) fn encode_page(page: &UploadPage) -> Result<Vec<u8>> {
    let size = page_size(page)?;
    let mut out = Vec::new();
    out.try_reserve_exact(size).map_err(|_| E::Allocation)?;
    out.extend_from_slice(PAGE_MAGIC);
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(
        &u16::try_from(page.points.len())
            .map_err(|_| E::Limit)?
            .to_le_bytes(),
    );
    for point in &page.points {
        out.extend_from_slice(&point.id);
        put_evidence(&mut out, &point.evidence);
        out.extend_from_slice(
            &u32::try_from(point.retrieval_text.len())
                .map_err(|_| E::Limit)?
                .to_le_bytes(),
        );
        out.extend_from_slice(point.retrieval_text.as_bytes());
    }
    Ok(out)
}
pub(super) fn decode_page(bytes: &[u8]) -> Result<UploadPage> {
    if bytes.len() > MAX_PAGE_BYTES {
        return Err(E::Corrupt);
    }
    let mut cursor = Cursor::new(bytes);
    if cursor.take(8)? != PAGE_MAGIC || cursor.u16()? != 1 {
        return Err(E::Corrupt);
    }
    let count = usize::from(cursor.u16()?);
    if count == 0 || count > MAX_PAGE_POINTS {
        return Err(E::Corrupt);
    }
    let mut points = Vec::new();
    points.try_reserve_exact(count).map_err(|_| E::Allocation)?;
    for _ in 0..count {
        let id = cursor.array()?;
        let evidence = evidence(&mut cursor)?;
        let size = usize::try_from(cursor.u32()?).map_err(|_| E::Corrupt)?;
        if size > MAX_TEXT_BYTES {
            return Err(E::Corrupt);
        }
        let text = std::str::from_utf8(cursor.take(size)?).map_err(|_| E::Corrupt)?;
        let mut retrieval_text = String::new();
        retrieval_text
            .try_reserve_exact(size)
            .map_err(|_| E::Allocation)?;
        retrieval_text.push_str(text);
        points.push(UploadPoint {
            id,
            evidence,
            retrieval_text,
        });
    }
    cursor.done()?;
    let page = UploadPage { points };
    if page_size(&page)? != bytes.len() {
        return Err(E::Corrupt);
    }
    Ok(page)
}
pub(super) fn encode_source(entry: &SourceEntry) -> Result<Vec<u8>> {
    let source = crate::current_source::encode_record(&entry.captured.source)
        .map_err(super::source_error)?;
    let prep =
        crate::current_preparation::encode(&entry.captured.preparation).map_err(|_| E::Corrupt)?;
    let size = 8_usize
        .checked_add(source.len())
        .and_then(|n| n.checked_add(prep.len()))
        .and_then(|n| n.checked_add(9))
        .ok_or(E::Limit)?;
    if size > 64 * 1024 {
        return Err(E::Limit);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(size).map_err(|_| E::Allocation)?;
    out.extend_from_slice(
        &u32::try_from(source.len())
            .map_err(|_| E::Limit)?
            .to_le_bytes(),
    );
    out.extend_from_slice(&source);
    out.extend_from_slice(
        &u32::try_from(prep.len())
            .map_err(|_| E::Limit)?
            .to_le_bytes(),
    );
    out.extend_from_slice(&prep);
    out.extend_from_slice(&entry.staged_units.to_le_bytes());
    out.push(u8::from(entry.complete));
    Ok(out)
}
pub(super) fn decode_source(bytes: &[u8]) -> Result<SourceEntry> {
    if bytes.len() > 64 * 1024 {
        return Err(E::Corrupt);
    }
    let mut cursor = Cursor::new(bytes);
    let source_size = usize::try_from(cursor.u32()?).map_err(|_| E::Corrupt)?;
    let source = crate::current_source::decode_record(cursor.take(source_size)?)
        .map_err(super::source_error)?;
    let prep_size = usize::try_from(cursor.u32()?).map_err(|_| E::Corrupt)?;
    let preparation =
        crate::current_preparation::decode(cursor.take(prep_size)?).map_err(|_| E::Corrupt)?;
    let staged_units = cursor.u64()?;
    let complete = cursor.boolean()?;
    cursor.done()?;
    if source.source_id != preparation.source_id
        || source.source_namespace_id != preparation.source_namespace_id
        || !preparation
            .outcome
            .as_ref()
            .is_some_and(|o| o.matches_input(&source, preparation.settings))
    {
        return Err(E::Corrupt);
    }
    Ok(SourceEntry {
        captured: CapturedSource {
            source,
            preparation,
        },
        staged_units,
        complete,
    })
}
