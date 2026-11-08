//! One concrete original result vector, pinned view and flush-committed cursor.

mod expand;

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_control_redb::publication::{PublishedView, pin_published};
use search_provider_protocol::{
    ProtocolError, SESSION_WINDOW_ID, SessionOutcome, SessionReply, StopReason, WindowEntry,
    WindowItem, WindowPage, WindowQuery, WindowSummary, encode_session_reply,
};
use search_runtime_owner::{PlainRootGuard, open_plain};

use crate::exact_literal::check;
use crate::verified_query::{self, QueryFailure};
use crate::{decoded_exact, raw_exact};

type Result<T> = std::result::Result<T, QueryFailure>;

pub struct PreparedPage {
    pub bytes: Vec<u8>,
    pub expected_cursor: u8,
    pub next_cursor: u8,
}

pub struct Window {
    // Declaration order releases the snapshot before exclusive root ownership.
    view: PublishedView,
    root: PlainRootGuard,
    pub expires: Instant,
    items: Vec<WindowItem>,
    summary: WindowSummary,
    cursor: u8,
}

impl Window {
    pub fn open(
        path: &Path,
        endpoint: Option<&str>,
        query: &WindowQuery,
        include_summary: bool,
        expires: Instant,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<Self> {
        let deadline = deadline.min(expires);
        check(deadline, cancel)?;
        query
            .validate()
            .map_err(|error| QueryFailure::Code(error.code()))?;
        // Reserve the finite wrapper vector before retrieval. Existing query
        // owners bound source/artifact, candidate and complete-item allocations.
        let mut items = Vec::new();
        items
            .try_reserve_exact(usize::from(query.limit()))
            .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
        let root = open_plain(path, deadline, cancel).map_err(QueryFailure::root)?;
        let view = pin_published(&root.control_path(), deadline, cancel)
            .map_err(QueryFailure::publication)?;
        let mut summary = WindowSummary {
            generation: view.generation.sequence,
            retained_items: 0,
            found_matches: None,
            scan_complete: None,
            complete_negative: None,
            result_stop: StopReason::Complete,
            corpus: None,
        };
        let (selected_sources, selected_preparation_gaps) = match query {
            WindowQuery::Search(request) => {
                let endpoint = endpoint.ok_or(QueryFailure::Code("QUERY_ENDPOINT_REQUIRED"))?;
                items.extend(
                    verified_query::execute(&root, &view, endpoint, request, deadline, cancel)?
                        .into_iter()
                        .map(WindowItem::Search),
                );
                (view.generation.source_count, view.generation.gap_count)
            }
            WindowQuery::Raw(request) => {
                let (result, gaps) = raw_exact::execute(&root, &view, request, deadline, cancel)?;
                summary.found_matches = Some(result.matches);
                summary.scan_complete = Some(result.scan_complete);
                summary.complete_negative = Some(result.complete_negative);
                summary.result_stop = result.stop;
                items.extend(result.items.into_iter().map(WindowItem::Raw));
                (result.selected_sources, gaps)
            }
            WindowQuery::DecodedUtf8(request) => {
                let (result, gaps) =
                    decoded_exact::execute(&root, &view, request, deadline, cancel)?;
                summary.found_matches = Some(result.matches);
                summary.scan_complete = Some(result.scan_complete);
                summary.complete_negative = Some(result.complete_negative);
                summary.result_stop = result.stop;
                items.extend(result.items.into_iter().map(WindowItem::DecodedUtf8));
                (result.selected_sources, gaps)
            }
        };
        summary.corpus = super::query_summary(
            &view,
            selected_sources,
            selected_preparation_gaps,
            include_summary,
            deadline,
            cancel,
        )?;
        summary.retained_items =
            u8::try_from(items.len()).map_err(|_| QueryFailure::Code("WINDOW_ITEM_LIMIT"))?;
        check(deadline, cancel)?;
        Ok(Self {
            view,
            root,
            expires,
            items,
            summary,
            cursor: 0,
        })
    }

    pub fn page(
        &self,
        request_id: u64,
        page_size: u8,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<PreparedPage> {
        let deadline = deadline.min(self.expires);
        check(deadline, cancel)?;
        let end = self
            .items
            .len()
            .min(usize::from(self.cursor) + usize::from(page_size));
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(end - usize::from(self.cursor))
            .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
        for index in usize::from(self.cursor)..end {
            entries.push(WindowEntry {
                item_index: u8::try_from(index)
                    .map_err(|_| QueryFailure::Code("WINDOW_ITEM_LIMIT"))?,
                item: self.items[index].clone(),
            });
        }
        let mut reply = SessionReply {
            request_id,
            outcome: SessionOutcome::Page {
                page: WindowPage {
                    window_id: SESSION_WINDOW_ID,
                    summary: self.summary.clone(),
                    start_index: self.cursor,
                    next_index: u8::try_from(end)
                        .map_err(|_| QueryFailure::Code("WINDOW_ITEM_LIMIT"))?,
                    items: entries,
                    has_more: end < self.items.len(),
                    page_stop: StopReason::Complete,
                },
            },
        };
        loop {
            check(deadline, cancel)?;
            let encoded = encode_session_reply(&reply);
            check(deadline, cancel)?;
            match encoded {
                Ok(bytes) => {
                    let SessionOutcome::Page { page } = reply.outcome else {
                        return Err(QueryFailure::Code("QUERY_INTERNAL_ERROR"));
                    };
                    return Ok(PreparedPage {
                        bytes,
                        expected_cursor: self.cursor,
                        next_cursor: page.next_index,
                    });
                }
                Err(ProtocolError::FrameTooLarge) => {
                    let SessionOutcome::Page { page } = &mut reply.outcome else {
                        return Err(QueryFailure::Code("QUERY_INTERNAL_ERROR"));
                    };
                    if page.items.len() <= 1 {
                        return Err(QueryFailure::Code("WINDOW_ITEM_TOO_LARGE"));
                    }
                    page.items.pop();
                    page.next_index -= 1;
                    page.has_more = true;
                    page.page_stop = StopReason::OutputLimit;
                }
                Err(error) => return Err(QueryFailure::Code(error.code())),
            }
        }
    }

    // Only the I/O owner's full-frame flush acknowledgement calls this method.
    pub fn commit_page(&mut self, expected: u8, next: u8) -> Result<()> {
        if self.cursor != expected || next < expected || usize::from(next) > self.items.len() {
            return Err(QueryFailure::Code("WINDOW_CURSOR_STALE"));
        }
        self.cursor = next;
        Ok(())
    }

    pub fn expand(
        &self,
        item_index: u8,
        before: u64,
        after: u64,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<search_provider_protocol::WindowExpansion> {
        if item_index >= self.cursor {
            return Err(QueryFailure::Code("WINDOW_ITEM_NOT_EMITTED"));
        }
        let item = self
            .items
            .get(usize::from(item_index))
            .ok_or(QueryFailure::Code("WINDOW_ITEM_NOT_EMITTED"))?;
        expand::execute(
            &self.root,
            &self.view,
            item_index,
            item,
            (before, after),
            deadline.min(self.expires),
            cancel,
        )
    }
}
