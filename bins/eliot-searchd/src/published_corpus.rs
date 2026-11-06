//! One owned-child corpus publication and actual backend nomination readback.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_control_redb::current_preparation::PreparationOutcome;
use search_control_redb::publication::{
    self as control, CapturedSource, Generation, PendingGeneration, PendingOperation,
    PointEvidence, PublicationError, PublishedView, UploadPage, UploadPoint,
};
use search_qdrant_bridge::{
    Backend, BackendError, BackendLimits, Hit, MAX_HITS, MAX_PAGE_BYTES, MAX_PAGE_POINTS,
    MAX_QUERY_BYTES, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, PointInput,
};
use search_revision_store::{ObjectStoreError, read_object};
use search_runtime_owner::{PlainRootError, PlainRootGuard, open_plain};
use search_source_registry::{MAX_PUBLICATION_RESPONSE_BYTES, ObjectRef};
use search_unitizer::{PreparationError, PreparationLimits, decode_prepared_utf8};
use serde_json::{Value, json};

use crate::app::PublicationRequest;

fn check(deadline: Instant, cancel: &AtomicBool) -> Result<(), &'static str> {
    if cancel.load(Ordering::Relaxed) {
        Err("PUBLICATION_CANCELLED")
    } else if Instant::now() >= deadline {
        Err("PUBLICATION_DEADLINE")
    } else {
        Ok(())
    }
}
const fn limits() -> PreparationLimits {
    PreparationLimits {
        max_input_bytes: 8 * 1024 * 1024,
        max_output_bytes: 8 * 1024 * 1024,
        max_lines: 1_000_000,
        max_units: 1_000_000,
        max_encoded_bytes: 64 * 1024 * 1024,
    }
}
fn inputs(page: &UploadPage) -> Vec<PointInput<'_>> {
    page.points
        .iter()
        .map(|point| PointInput {
            id: point.id,
            source_id: point.evidence.source_id,
            retrieval_text: &point.retrieval_text,
        })
        .collect()
}
fn stage_complete_page(
    path: &Path,
    pending: &PendingGeneration,
    page: UploadPage,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    let collection = pending.generation.collection();
    let mut stack = vec![page];
    while let Some(mut page) = stack.pop() {
        check(deadline, cancel)?;
        let oversized = match control::upload_page_bytes(&page) {
            Err(PublicationError::Limit) => true,
            Err(error) => return Err(error.code()),
            Ok(size) if size > MAX_PAGE_BYTES => true,
            Ok(_) => {
                match Backend::batch_request_bytes(&collection, &inputs(&page), deadline, cancel) {
                    Ok(wire) => wire > MAX_REQUEST_BYTES,
                    Err(BackendError::RequestTooLarge) => true,
                    Err(error) => return Err(error.code()),
                }
            }
        };
        if oversized {
            if page.points.len() <= 1 {
                return Err("PUBLICATION_UNIT_TOO_LARGE");
            }
            let right = page.points.split_off(page.points.len() / 2);
            stack.push(UploadPage { points: right });
            stack.push(page);
        } else {
            control::stage_page(path, pending, &page, deadline, cancel)
                .map_err(PublicationError::code)?;
        }
    }
    Ok(())
}
fn stage_source(
    root: &PlainRootGuard,
    pending: &PendingGeneration,
    captured: &CapturedSource,
    next_point: &mut u64,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    let path = root.control_path();
    let outcome = captured
        .preparation
        .outcome
        .as_ref()
        .ok_or("PUBLICATION_SOURCE_NOT_PREPARED")?;
    match outcome.result {
        PreparationOutcome::Gap { .. } => control::finish_source(
            &path,
            pending,
            captured.source.source_id,
            0,
            deadline,
            cancel,
        )
        .map_err(PublicationError::code),
        PreparationOutcome::Ready { artifact } => {
            let stored = read_object(root, artifact, 64 * 1024 * 1024, deadline, cancel)
                .map_err(ObjectStoreError::code)?;
            let view = decode_prepared_utf8(
                &stored,
                &captured.source,
                &captured.preparation.settings,
                limits(),
                deadline,
                cancel,
            )
            .map_err(PreparationError::code)?;
            let mut page = UploadPage { points: Vec::new() };
            for (ordinal, unit) in view.units().iter().enumerate() {
                check(deadline, cancel)?;
                let text = view
                    .text()
                    .get(unit.source_start..unit.source_end)
                    .ok_or("PUBLICATION_ARTIFACT_RANGE_INVALID")?;
                let mut retrieval_text = String::new();
                retrieval_text
                    .try_reserve_exact(text.len())
                    .map_err(|_| "PUBLICATION_ALLOCATION_FAILED")?;
                retrieval_text.push_str(text);
                let mut id = [0; 16];
                id[..8].copy_from_slice(&pending.generation.sequence.to_be_bytes());
                id[8..].copy_from_slice(&next_point.to_be_bytes());
                *next_point = next_point.checked_add(1).ok_or("PUBLICATION_LIMIT")?;
                page.points
                    .try_reserve(1)
                    .map_err(|_| "PUBLICATION_ALLOCATION_FAILED")?;
                page.points.push(UploadPoint {
                    id,
                    retrieval_text,
                    evidence: PointEvidence {
                        source_namespace_id: captured.source.source_namespace_id,
                        source_id: captured.source.source_id,
                        input: captured.source.object_ref,
                        prepared: artifact,
                        unit: u64::try_from(ordinal).map_err(|_| "PUBLICATION_LIMIT")?,
                        start: u64::try_from(unit.source_start).map_err(|_| "PUBLICATION_LIMIT")?,
                        end: u64::try_from(unit.source_end).map_err(|_| "PUBLICATION_LIMIT")?,
                        line_start: unit.logical_line_start,
                        line_end: unit.logical_line_end,
                        starts_at_line_boundary: unit.starts_at_line_boundary,
                        ends_at_line_boundary: unit.ends_at_line_boundary,
                    },
                });
                if page.points.len() == MAX_PAGE_POINTS
                    || matches!(
                        control::upload_page_bytes(&page),
                        Err(PublicationError::Limit)
                    )
                {
                    let full = std::mem::replace(&mut page, UploadPage { points: Vec::new() });
                    stage_complete_page(&path, pending, full, deadline, cancel)?;
                }
            }
            if !page.points.is_empty() {
                stage_complete_page(&path, pending, page, deadline, cancel)?;
            }
            control::finish_source(
                &path,
                pending,
                captured.source.source_id,
                u64::try_from(view.units().len()).map_err(|_| "PUBLICATION_LIMIT")?,
                deadline,
                cancel,
            )
            .map_err(PublicationError::code)
        }
    }
}
fn settle(
    path: &Path,
    pending: &PendingGeneration,
    operation: PendingOperation,
    operation_revision: u64,
    result: Result<(), BackendError>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    if matches!(result, Err(BackendError::UnknownDispatch)) {
        return Err(BackendError::UnknownDispatch.code());
    }
    control::finish_dispatch(
        path,
        pending,
        operation,
        operation_revision,
        result.is_ok(),
        deadline,
        cancel,
    )
    .map_err(PublicationError::code)?;
    result.map_err(BackendError::code)
}
fn upload(
    backend: &Backend,
    path: &Path,
    mut pending: PendingGeneration,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PendingGeneration, &'static str> {
    let collection = pending.generation.collection();
    if !pending.created {
        if backend
            .exists(&collection, deadline, cancel)
            .map_err(BackendError::code)?
        {
            return Err("PUBLICATION_COLLECTION_CONFLICT");
        }
        let revision =
            control::record_dispatch(path, &pending, PendingOperation::Create, deadline, cancel)
                .map_err(PublicationError::code)?;
        settle(
            path,
            &pending,
            PendingOperation::Create,
            revision,
            backend.create_generation(&collection, deadline, cancel),
            deadline,
            cancel,
        )?;
        pending = control::load_pending(path, deadline, cancel)
            .map_err(PublicationError::code)?
            .ok_or("PUBLICATION_STALE")?;
    }
    backend
        .verify_schema(&collection, deadline, cancel)
        .map_err(BackendError::code)?;
    for number in pending.uploaded_pages..pending.generation.page_count {
        let page = control::read_upload_page(path, &pending.generation, number, deadline, cancel)
            .map_err(PublicationError::code)?;
        let revision = control::record_dispatch(
            path,
            &pending,
            PendingOperation::Insert(number),
            deadline,
            cancel,
        )
        .map_err(PublicationError::code)?;
        settle(
            path,
            &pending,
            PendingOperation::Insert(number),
            revision,
            backend.insert_only(&collection, &inputs(&page), deadline, cancel),
            deadline,
            cancel,
        )?;
        pending = control::load_pending(path, deadline, cancel)
            .map_err(PublicationError::code)?
            .ok_or("PUBLICATION_STALE")?;
    }
    control::load_pending(path, deadline, cancel)
        .map_err(PublicationError::code)?
        .ok_or("PUBLICATION_STALE")
}
fn verify_and_query(
    backend: &Backend,
    path: &Path,
    generation: &Generation,
    request: &PublicationRequest,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Vec<Hit>, &'static str> {
    control::validate_generation(path, generation, deadline, cancel)
        .map_err(PublicationError::code)?;
    let collection = generation.collection();
    backend
        .verify_schema(&collection, deadline, cancel)
        .map_err(BackendError::code)?;
    let mut canary = None;
    let mut point_count = 0_u64;
    for number in 0..generation.page_count {
        check(deadline, cancel)?;
        let page = control::read_upload_page(path, generation, number, deadline, cancel)
            .map_err(PublicationError::code)?;
        backend
            .verify_points(&collection, &inputs(&page), deadline, cancel)
            .map_err(BackendError::code)?;
        if canary.is_none() {
            canary = page.points.iter().find_map(|point| {
                canary_query(&point.retrieval_text).map(|query| (point.clone(), query.to_owned()))
            });
        }
        point_count = point_count
            .checked_add(u64::try_from(page.points.len()).map_err(|_| "PUBLICATION_LIMIT")?)
            .ok_or("PUBLICATION_LIMIT")?;
    }
    let remote_count = backend
        .count_exact(&collection, deadline, cancel)
        .map_err(BackendError::code)?;
    if remote_count != generation.point_count || point_count != generation.point_count {
        return Err("PUBLICATION_POINT_COUNT_MISMATCH");
    }
    if point_count == 0 {
        return Ok(Vec::new());
    }
    let (point, query) = canary.ok_or("PUBLICATION_READINESS_NOT_PROVEN")?;
    backend
        .verify_canary(
            &collection,
            PointInput {
                id: point.id,
                source_id: point.evidence.source_id,
                retrieval_text: &point.retrieval_text,
            },
            &query,
            deadline,
            cancel,
        )
        .map_err(BackendError::code)?;
    request.query.as_deref().map_or_else(
        || Ok(Vec::new()),
        |text| {
            backend
                .query_text(&collection, text, request.limit, deadline, cancel)
                .map_err(BackendError::code)
        },
    )
}

fn canary_query(text: &str) -> Option<&str> {
    // Select an existing complete ASCII word under the fixed Code/BM25 profile.
    // This does not reconstruct tokenization, aliases or scores, and does not establish
    // that all other text is tokenless. No candidate means readiness remains unproven.
    text.split_ascii_whitespace().find(|word| {
        word.len() <= MAX_QUERY_BYTES && word.bytes().all(|byte| byte.is_ascii_alphabetic())
    })
}
fn cleanup(
    backend: &Backend,
    path: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    for retired in
        control::retired_generations(path, deadline, cancel).map_err(PublicationError::code)?
    {
        check(deadline, cancel)?;
        if retired.remote_owned {
            let collection = retired.generation.collection();
            backend
                .delete_generation(&collection, deadline, cancel)
                .map_err(BackendError::code)?;
            if backend
                .exists(&collection, deadline, cancel)
                .map_err(BackendError::code)?
            {
                return Err("PUBLICATION_CLEANUP_UNRESOLVED");
            }
        }
        control::finish_retirement(path, &retired, deadline, cancel)
            .map_err(PublicationError::code)?;
    }
    Ok(())
}
fn prepare_view(
    root: &PlainRootGuard,
    backend: &Backend,
    request: &PublicationRequest,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(PublishedView, Vec<Hit>), &'static str> {
    let path = root.control_path();
    let capture =
        control::capture_corpus(&path, deadline, cancel).map_err(PublicationError::code)?;
    if let Some(pending) = &capture.pending {
        if pending.in_flight.is_some() {
            return Err("PUBLICATION_EFFECT_UNRESOLVED");
        }
        if !pending.capture_complete || !capture.pending_matches() {
            control::abandon_build(&path, pending, deadline, cancel)
                .map_err(PublicationError::code)?;
            return Err("PUBLICATION_STALE");
        }
    }
    cleanup(backend, &path, deadline, cancel)?;
    let generation;
    let hits;
    if capture.pending.is_none() && capture.current_matches() {
        generation = capture.current.ok_or("PUBLICATION_NOT_PUBLISHED")?;
        hits = verify_and_query(backend, &path, &generation, request, deadline, cancel)?;
    } else {
        let pending = if let Some(pending) = capture.pending.clone() {
            pending
        } else {
            let pending = control::start_build(&path, &capture, deadline, cancel)
                .map_err(PublicationError::code)?;
            let mut next_point = 0;
            for source in &capture.sources {
                stage_source(root, &pending, source, &mut next_point, deadline, cancel)?;
            }
            control::complete_capture(&path, &pending, deadline, cancel)
                .map_err(PublicationError::code)?
        };
        let pending = upload(backend, &path, pending, deadline, cancel)?;
        hits = verify_and_query(
            backend,
            &path,
            &pending.generation,
            request,
            deadline,
            cancel,
        )?;
        generation = control::commit_generation(&path, &pending, deadline, cancel)
            .map_err(PublicationError::code)?;
    }
    let view = control::open_published(&path, deadline, cancel).map_err(PublicationError::code)?;
    if view.generation != generation {
        return Err("PUBLICATION_STALE");
    }
    Ok((view, hits))
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    output
}
fn object(reference: ObjectRef) -> Value {
    json!({"version":reference.format_version,"kind":reference.kind.name(),
        "digest":hex(reference.digest.as_bytes()),"bytes":reference.byte_length})
}
fn response(
    view: &PublishedView,
    hits: Vec<Hit>,
    request: &PublicationRequest,
) -> Result<Value, &'static str> {
    if hits.len() > request.limit || hits.len() > MAX_HITS {
        return Err("BACKEND_RESPONSE_MALFORMED");
    }
    let mut resolved = Vec::new();
    resolved
        .try_reserve_exact(hits.len())
        .map_err(|_| "PUBLICATION_ALLOCATION_FAILED")?;
    for hit in hits {
        let e = view.evidence(hit.id).map_err(PublicationError::code)?;
        if e.source_id != hit.source_id || !hit.score.is_finite() {
            return Err("PUBLICATION_HIT_MISMATCH");
        }
        resolved.push(json!({"point_id":hex(&hit.id),"source_namespace_id":e.source_namespace_id.to_string(),
            "source_id":e.source_id.to_string(),"score":hit.score,"input":object(e.input),"prepared":object(e.prepared),
            "unit":e.unit,"start":e.start,"end":e.end,"line_start":e.line_start,"line_end":e.line_end,
            "starts_at_line_boundary":e.starts_at_line_boundary,"ends_at_line_boundary":e.ends_at_line_boundary}));
    }
    let g = &view.generation;
    Ok(
        json!({"operation":"publish-prepared","state":"published","collection":g.collection(),
        "generation":g.sequence,"source_count":g.source_count,"gap_count":g.gap_count,
        "point_count":g.point_count,"page_count":g.page_count,
        "query_performed":request.query.is_some() && g.point_count != 0,"hits":resolved}),
    )
}
fn api_key() -> Result<Option<String>, &'static str> {
    let Some(value) = std::env::var_os("ELIOT_QDRANT_API_KEY") else {
        return Ok(None);
    };
    let value = value
        .into_string()
        .map_err(|_| "BACKEND_AUTHENTICATION_FAILED")?;
    if value.is_empty() || value.len() > 4096 || value.as_bytes().contains(&0) {
        return Err("BACKEND_AUTHENTICATION_FAILED");
    }
    Ok(Some(value))
}
pub fn run(
    path: &Path,
    request: &PublicationRequest,
    deadline: Instant,
    cancel: &AtomicBool,
) -> ExitCode {
    let result = (|| {
        let root = open_plain(path, deadline, cancel).map_err(PlainRootError::code)?;
        let key = api_key()?;
        let backend = Backend::connect(
            &request.endpoint,
            key.as_deref(),
            BackendLimits {
                max_batch_points: MAX_PAGE_POINTS,
                max_request_bytes: MAX_REQUEST_BYTES,
                max_response_bytes: MAX_RESPONSE_BYTES,
                max_query_bytes: MAX_QUERY_BYTES,
                max_hits: MAX_HITS,
            },
            deadline,
            cancel,
        )
        .map_err(BackendError::code)?;
        let (view, hits) = prepare_view(&root, &backend, request, deadline, cancel)?;
        let result = response(&view, hits, request)?;
        let bytes = serde_json::to_vec(&result).map_err(|_| "PUBLICATION_RESPONSE_INVALID")?;
        if bytes.len() >= MAX_PUBLICATION_RESPONSE_BYTES {
            return Err("PUBLICATION_RESPONSE_TOO_LARGE");
        }
        check(deadline, cancel)?;
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(&bytes)
            .and_then(|()| stdout.write_all(b"\n"))
            .and_then(|()| stdout.flush())
            .map_err(|_| "OUTPUT_WRITE_ERROR")?;
        drop(stdout);
        view.close().map_err(PublicationError::code)?;
        drop(root);
        Ok::<(), &'static str>(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            eprintln!("{{\"error\":\"{code}\"}}");
            ExitCode::from(2)
        }
    }
}
