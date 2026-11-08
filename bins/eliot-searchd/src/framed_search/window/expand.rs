//! Bounded context from an emitted item's original pinned representation.

use std::ops::Range;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_contracts::SourceId;
use search_control_redb::publication::PublishedView;
use search_provider_protocol::{
    DecodedUtf8Item, ExpansionContent, MAX_EXPANSION_BYTES, ObjectRefDto, RawExactItem,
    RawSourceRef, SESSION_WINDOW_ID, SearchItem, WindowExpansion, WindowItem,
};
use search_revision_store::{ObjectStoreError, read_object};
use search_runtime_owner::PlainRootGuard;
use search_source_registry::MAX_SOURCE_OBJECT_BYTES;

use crate::decoded_exact::visit_representation;
use crate::exact_literal::{check, hex, object_ref, source_ref};
use crate::verified_query::QueryFailure;

type Result<T> = std::result::Result<T, QueryFailure>;

struct Anchor<'a> {
    id: &'a str,
    source_namespace_id: &'a str,
    input: &'a ObjectRefDto,
    source: Option<&'a RawSourceRef>,
    prepared: Option<&'a ObjectRefDto>,
    range: Range<u64>,
    original: &'a str,
}

impl<'a> Anchor<'a> {
    fn from_item(item: &'a WindowItem) -> Result<Self> {
        match item {
            WindowItem::Search(SearchItem::Hit(hit)) => Ok(Self {
                id: &hit.evidence.source_id,
                source_namespace_id: &hit.evidence.source_namespace_id,
                input: &hit.evidence.input,
                source: None,
                prepared: Some(&hit.evidence.prepared),
                range: hit.evidence.start..hit.evidence.end,
                original: &hit.unit_text,
            }),
            WindowItem::Raw(RawExactItem::Match(hit)) => Ok(Self {
                id: &hit.source.source_id,
                source_namespace_id: &hit.source.source_namespace_id,
                input: &hit.source.input,
                source: Some(&hit.source),
                prepared: None,
                range: hit.start..hit.end,
                original: &hit.bytes_hex,
            }),
            WindowItem::DecodedUtf8(DecodedUtf8Item::Match(hit)) => Ok(Self {
                id: &hit.source.source_id,
                source_namespace_id: &hit.source.source_namespace_id,
                input: &hit.source.input,
                source: Some(&hit.source),
                prepared: Some(&hit.prepared),
                range: hit.representation_start..hit.representation_end,
                original: &hit.text,
            }),
            _ => Err(QueryFailure::Code("WINDOW_ITEM_UNAVAILABLE")),
        }
    }

    fn context(&self, before: u64, after: u64) -> Result<Range<usize>> {
        let start = self
            .range
            .start
            .checked_sub(before)
            .ok_or(QueryFailure::Code("WINDOW_RANGE_INVALID"))?;
        let end = self
            .range
            .end
            .checked_add(after)
            .filter(|end| *end <= self.input.bytes)
            .ok_or(QueryFailure::Code("WINDOW_RANGE_INVALID"))?;
        if self.range.start >= self.range.end || end - start > MAX_EXPANSION_BYTES {
            return Err(QueryFailure::Code("WINDOW_EXPANSION_LIMIT"));
        }
        Ok(
            usize::try_from(start).map_err(|_| QueryFailure::Code("WINDOW_RANGE_INVALID"))?
                ..usize::try_from(end).map_err(|_| QueryFailure::Code("WINDOW_RANGE_INVALID"))?,
        )
    }
}

const fn object_failure(error: ObjectStoreError) -> QueryFailure {
    match error {
        ObjectStoreError::Cancelled => QueryFailure::Cancelled,
        ObjectStoreError::Deadline => QueryFailure::Deadline,
        other => QueryFailure::Code(other.code()),
    }
}

pub fn execute(
    root: &PlainRootGuard,
    view: &PublishedView,
    item_index: u8,
    item: &WindowItem,
    context: (u64, u64),
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<WindowExpansion> {
    check(deadline, cancel)?;
    let anchor = Anchor::from_item(item)?;
    let range = anchor.context(context.0, context.1)?;
    let original = usize::try_from(anchor.range.start)
        .map_err(|_| QueryFailure::Code("WINDOW_RANGE_INVALID"))?
        ..usize::try_from(anchor.range.end)
            .map_err(|_| QueryFailure::Code("WINDOW_RANGE_INVALID"))?;
    let id =
        SourceId::parse(anchor.id).map_err(|_| QueryFailure::Code("EVIDENCE_BINDING_INVALID"))?;
    let captured = view
        .source(id, deadline, cancel)
        .map_err(QueryFailure::publication)?;
    let source = source_ref(&captured.source)?;
    if source.input != *anchor.input
        || source.source_namespace_id != anchor.source_namespace_id
        || anchor.source.is_some_and(|expected| expected != &source)
    {
        return Err(QueryFailure::Code("EVIDENCE_BINDING_INVALID"));
    }
    let mut expansion = WindowExpansion {
        window_id: SESSION_WINDOW_ID,
        item_index,
        source,
        prepared: None,
        representation_start: u64::try_from(range.start)
            .map_err(|_| QueryFailure::Code("WINDOW_RANGE_INVALID"))?,
        representation_end: u64::try_from(range.end)
            .map_err(|_| QueryFailure::Code("WINDOW_RANGE_INVALID"))?,
        source_start: 0,
        source_end: 0,
        content: ExpansionContent::Raw {
            bytes_hex: String::new(),
        },
        source_info: item.source_info().cloned(),
    };
    expansion.source_start = expansion.representation_start;
    expansion.source_end = expansion.representation_end;
    if let Some(expected) = anchor.prepared {
        let gap = visit_representation(root, &captured, deadline, cancel, |artifact, utf8| {
            let prepared = object_ref(artifact)?;
            if prepared != *expected || utf8.text().get(original.clone()) != Some(anchor.original) {
                return Err(QueryFailure::Code("EVIDENCE_TEXT_MISMATCH"));
            }
            // Exact byte counts are honored: a scalar-splitting request is refused.
            let text = utf8
                .text()
                .get(range.clone())
                .ok_or(QueryFailure::Code("WINDOW_UTF8_BOUNDARY"))?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(text.len())
                .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
            owned.push_str(text);
            expansion.prepared = Some(prepared);
            expansion.content = ExpansionContent::Utf8 { text: owned };
            Ok(())
        })?;
        if let Some(gap) = gap {
            return Err(QueryFailure::Code(gap.code));
        }
    } else {
        let bytes = read_object(
            root,
            captured.source.object_ref,
            MAX_SOURCE_OBJECT_BYTES,
            deadline,
            cancel,
        )
        .map_err(object_failure)?;
        let original = bytes
            .get(original)
            .ok_or(QueryFailure::Code("WINDOW_RANGE_INVALID"))?;
        if hex(original)? != anchor.original {
            return Err(QueryFailure::Code("EVIDENCE_TEXT_MISMATCH"));
        }
        let bytes = bytes
            .get(range)
            .ok_or(QueryFailure::Code("WINDOW_RANGE_INVALID"))?;
        expansion.content = ExpansionContent::Raw {
            bytes_hex: hex(bytes)?,
        };
    }
    check(deadline, cancel)?;
    expansion
        .validate()
        .map_err(|error| QueryFailure::Code(error.code()))?;
    Ok(expansion)
}
