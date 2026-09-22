//! Streaming search, paging, continuation and opaque-handle expansion.

use std::io::Write;

use crate::continuation::{ContinuationError, LiveExpansionBarrier};
use crate::direct_preparation::verify_spine_gate;
use crate::result_handles::ResultHandleError;
use crate::service_output::{
    emit_handle_expansion, emit_search_page, emit_streaming_search,
};

use super::codec::{decode_query, parse_page_size, parse_search_mode, parse_u64};
use super::diagnostics::refresh_storage;
use super::output_deadline::{page_deadline, with_deadline};
use super::state::CommandState;

pub(super) fn cmd_streaming_search<W: Write>(
    state: &mut CommandState<'_, W>,
    mode: &str,
    query_hex: &str,
) -> Result<(), String> {
    let query = decode_query(query_hex)?;
    let result = state.store.search(&query, parse_search_mode(mode)?)?;
    enforce_spine_gate(&result)?;
    refresh_storage(state.storage, state.canonical_root)?;
    emit_streaming_search(
        state.writer,
        &state.store.namespace_id(),
        &result,
        state.storage,
    )
}

pub(super) fn cmd_search_page<W: Write>(
    state: &mut CommandState<'_, W>,
    mode: &str,
    page_size: &str,
    query_hex: &str,
) -> Result<(), String> {
    let CommandState {
        writer,
        store,
        continuations,
        handles,
        canonical_root,
        storage,
        ..
    } = &mut *state;
    let query = decode_query(query_hex)?;
    let page_size = parse_page_size(page_size)?;
    let result = store.search(&query, parse_search_mode(mode)?)?;
    enforce_spine_gate(&result)?;
    let page = continuations
        .prepare_page(store, result, page_size)
        .map_err(continuation_error)?;
    let mut public = handles
        .prepare_mint_page(store, &page.page().matches)
        .map_err(handle_error)?;
    refresh_storage(storage, canonical_root)?;
    let deadline = page_deadline(page.expires_at(), public.expires_at());
    page.deliver(|page| {
        public.revalidate().map_err(handle_error)?;
        with_deadline(writer, deadline, |output| {
            emit_search_page(output, page, public.matches(), storage)
        })
    })?;
    // No recoverable work remains after complete output. Both catalogs were
    // exclusively borrowed throughout preparation and emission.
    let _ = public.commit();
    Ok(())
}

pub(super) fn cmd_continue<W: Write>(
    state: &mut CommandState<'_, W>,
    token: &str,
    page_size: &str,
) -> Result<(), String> {
    let CommandState {
        writer,
        store,
        continuations,
        handles,
        canonical_root,
        storage,
        ..
    } = &mut *state;
    let page_size = parse_page_size(page_size)?;
    let page = continuations
        .prepare_continue_page(
            store,
            token,
            page_size,
            LiveExpansionBarrier::clean(),
        )
        .map_err(continuation_error)?;
    let mut public = handles
        .prepare_mint_page(store, &page.page().matches)
        .map_err(handle_error)?;
    refresh_storage(storage, canonical_root)?;
    let deadline = page_deadline(page.expires_at(), public.expires_at());
    page.deliver(|page| {
        public.revalidate().map_err(handle_error)?;
        with_deadline(writer, deadline, |output| {
            emit_search_page(output, page, public.matches(), storage)
        })
    })?;
    // No recoverable work remains after complete output. Both catalogs were
    // exclusively borrowed throughout preparation and emission.
    let _ = public.commit();
    Ok(())
}

pub(super) fn cmd_expand_handle<W: Write>(
    state: &mut CommandState<'_, W>,
    token: &str,
    start: &str,
    end: &str,
) -> Result<(), String> {
    let CommandState {
        writer,
        store,
        canonical_root,
        storage,
        handles,
        ..
    } = &mut *state;
    let start = parse_u64(start, "SERVICE_START_OFFSET_INVALID")?;
    let end = parse_u64(end, "SERVICE_END_OFFSET_INVALID")?;
    let expansion = handles
        .prepare_expand(store, token, start, end)
        .map_err(handle_error)?;
    refresh_storage(storage, canonical_root)?;
    expansion.deliver(|expansion, expires_at| {
        with_deadline(
            writer,
            Some((expires_at, ResultHandleError::Expired.code())),
            |output| emit_handle_expansion(output, expansion, storage),
        )
    })
}

fn enforce_spine_gate(
    result: &crate::direct_store::StoreSearchResult,
) -> Result<(), String> {
    verify_spine_gate(
        result.active_sources,
        result.searched_sources,
        result.gaps.is_empty(),
        result.complete,
        result.match_limit_reached,
    )
    .map_err(str::to_owned)
}

fn continuation_error(error: ContinuationError) -> String {
    error.code().to_owned()
}

fn handle_error(error: ResultHandleError) -> String {
    error.code().to_owned()
}
