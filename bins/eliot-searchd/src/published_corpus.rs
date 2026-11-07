//! One owned-child corpus publication from immutable retained preparation.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_control_redb::current_preparation::PreparationOutcome;
use search_control_redb::plain_control::{ControlWriter, PlainControlError};
use search_control_redb::publication::{
    self as control, CapturedSource, Generation, PendingGeneration, PendingOperation,
    PointEvidence, PublicationError, PublishedView, UploadPage, UploadPoint,
};
use search_qdrant_bridge::{
    Backend, BackendError, BackendLimits, MAX_HITS, MAX_PAGE_BYTES, MAX_PAGE_POINTS,
    MAX_QUERY_BYTES, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, PointInput,
};
use search_revision_store::{ObjectStoreError, read_object};
use search_runtime_owner::{PlainRootError, PlainRootGuard, open_plain};
use search_source_registry::{CurrentSourceRecord, MAX_PUBLICATION_RESPONSE_BYTES, ObjectRef};
use search_unitizer::{PreparationError, PreparationLimits, PreparedUtf8, decode_prepared_utf8};
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
struct HydratedPoint {
    point: UploadPoint,
    text: String,
}

fn inputs(page: &[HydratedPoint]) -> Vec<PointInput<'_>> {
    page.iter()
        .map(|point| PointInput {
            id: point.point.id,
            source_id: point.point.evidence.source_id,
            retrieval_text: &point.text,
        })
        .collect()
}

fn unit_text<'a>(view: &'a PreparedUtf8<'_>, e: &PointEvidence) -> Result<&'a str, &'static str> {
    let unit = view
        .units()
        .get(usize::try_from(e.unit).map_err(|_| "PUBLICATION_CORRUPT")?)
        .ok_or("PUBLICATION_CORRUPT")?;
    if u64::try_from(unit.source_start).ok() != Some(e.start)
        || u64::try_from(unit.source_end).ok() != Some(e.end)
        || unit.logical_line_start != e.line_start
        || unit.logical_line_end != e.line_end
        || unit.starts_at_line_boundary != e.starts_at_line_boundary
        || unit.ends_at_line_boundary != e.ends_at_line_boundary
    {
        return Err("PUBLICATION_CORRUPT");
    }
    view.text()
        .get(unit.source_start..unit.source_end)
        .ok_or("PUBLICATION_CORRUPT")
}

fn hydrate_point(
    view: &PreparedUtf8<'_>,
    source: &CurrentSourceRecord,
    artifact: ObjectRef,
    point: UploadPoint,
) -> Result<HydratedPoint, &'static str> {
    if point.evidence.source_namespace_id != source.source_namespace_id
        || point.evidence.input != source.object_ref
        || point.evidence.prepared != artifact
    {
        return Err("PUBLICATION_CORRUPT");
    }
    let text = unit_text(view, &point.evidence)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| "PUBLICATION_ALLOCATION_FAILED")?;
    owned.push_str(text);
    Ok(HydratedPoint { point, text: owned })
}

// One pass retains one verified object and its borrowing decoder view until the
// source changes. Only one page of transient upload text is accumulated.
fn visit_pages(
    root: &PlainRootGuard,
    writer: Option<&ControlWriter>,
    generation: &Generation,
    pages: std::ops::Range<u64>,
    deadline: Instant,
    cancel: &AtomicBool,
    mut visit: impl FnMut(u64, &[HydratedPoint]) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    let mut number = pages.start;
    if number >= pages.end {
        return Ok(());
    }
    let path = root.control_path();
    let read_page = |number| {
        writer.map_or_else(
            || control::read_upload_page(&path, generation, number, deadline, cancel),
            |writer| writer.read_upload_page(generation, number, deadline, cancel),
        )
    };
    let page = read_page(number).map_err(PublicationError::code)?;
    let mut request_bytes = page.request_bytes;
    let mut points = page.points.into_iter().peekable();
    let mut hydrated = Vec::new();
    hydrated
        .try_reserve_exact(points.len())
        .map_err(|_| "PUBLICATION_ALLOCATION_FAILED")?;
    while let Some(first) = points.peek() {
        check(deadline, cancel)?;
        let captured = writer
            .map_or_else(
                || {
                    control::read_generation_source(
                        &path,
                        generation,
                        first.evidence.source_id,
                        deadline,
                        cancel,
                    )
                },
                |writer| {
                    writer.read_generation_source(
                        generation,
                        first.evidence.source_id,
                        deadline,
                        cancel,
                    )
                },
            )
            .map_err(PublicationError::code)?;
        let Some(PreparationOutcome::Ready { artifact }) =
            captured.preparation.outcome.as_ref().map(|o| o.result)
        else {
            return Err("PUBLICATION_CORRUPT");
        };
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
        loop {
            while points
                .peek()
                .is_some_and(|p| p.evidence.source_id == captured.source.source_id)
            {
                check(deadline, cancel)?;
                let point = points.next().ok_or("PUBLICATION_CORRUPT")?;
                hydrated.push(hydrate_point(&view, &captured.source, artifact, point)?);
            }
            if points.peek().is_some() {
                break;
            }
            let wire = Backend::batch_request_bytes(
                &generation.collection(),
                &inputs(&hydrated),
                deadline,
                cancel,
            )
            .map_err(BackendError::code)?;
            if request_bytes.is_some_and(|saved| u64::try_from(wire).ok() != Some(saved)) {
                return Err("PUBLICATION_CORRUPT");
            }
            visit(number, &hydrated)?;
            hydrated.clear();
            number = number.checked_add(1).ok_or("PUBLICATION_LIMIT")?;
            if number == pages.end {
                return Ok(());
            }
            check(deadline, cancel)?;
            let page = read_page(number).map_err(PublicationError::code)?;
            request_bytes = page.request_bytes;
            points = page.points.into_iter().peekable();
            hydrated
                .try_reserve_exact(points.len())
                .map_err(|_| "PUBLICATION_ALLOCATION_FAILED")?;
        }
    }
    Err("PUBLICATION_CORRUPT")
}

fn stage_complete_page(
    staging: &ControlWriter,
    pending: &PendingGeneration,
    page: UploadPage,
    prepared: &PreparedUtf8<'_>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    let collection = pending.generation.collection();
    let mut stack = vec![page];
    while let Some(mut page) = stack.pop() {
        check(deadline, cancel)?;
        let page_inputs = page
            .points
            .iter()
            .map(|point| {
                Ok(PointInput {
                    id: point.id,
                    source_id: point.evidence.source_id,
                    retrieval_text: unit_text(prepared, &point.evidence)?,
                })
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        let wire = Backend::batch_request_bytes(&collection, &page_inputs, deadline, cancel);
        let oversized = match control::upload_page_bytes(&page) {
            Err(PublicationError::Limit) => true,
            Err(error) => return Err(error.code()),
            Ok(size) if size > MAX_PAGE_BYTES => true,
            Ok(_) => match wire {
                Ok(size) => size > MAX_REQUEST_BYTES,
                Err(BackendError::RequestTooLarge) => true,
                Err(error) => return Err(error.code()),
            },
        };
        if oversized {
            if page.points.len() <= 1 {
                return Err("PUBLICATION_UNIT_TOO_LARGE");
            }
            let right = page.points.split_off(page.points.len() / 2);
            stack.push(UploadPage {
                points: right,
                request_bytes: None,
            });
            stack.push(page);
        } else {
            page.request_bytes = Some(
                u64::try_from(wire.map_err(BackendError::code)?)
                    .map_err(|_| "PUBLICATION_LIMIT")?,
            );
            staging
                .stage_page(pending, &page, deadline, cancel)
                .map_err(PublicationError::code)?;
        }
    }
    Ok(())
}
fn stage_source(
    root: &PlainRootGuard,
    staging: &ControlWriter,
    pending: &PendingGeneration,
    captured: &CapturedSource,
    next_point: &mut u64,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    let outcome = captured
        .preparation
        .outcome
        .as_ref()
        .ok_or("PUBLICATION_SOURCE_NOT_PREPARED")?;
    match outcome.result {
        PreparationOutcome::Gap { .. } => staging
            .finish_source(pending, captured.source.source_id, 0, deadline, cancel)
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
            let mut page = UploadPage {
                points: Vec::new(),
                request_bytes: None,
            };
            for (ordinal, unit) in view.units().iter().enumerate() {
                check(deadline, cancel)?;
                let mut id = [0; 16];
                id[..8].copy_from_slice(&pending.generation.sequence.to_be_bytes());
                id[8..].copy_from_slice(&next_point.to_be_bytes());
                *next_point = next_point.checked_add(1).ok_or("PUBLICATION_LIMIT")?;
                page.points
                    .try_reserve(1)
                    .map_err(|_| "PUBLICATION_ALLOCATION_FAILED")?;
                page.points.push(UploadPoint {
                    id,
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
                    let full = std::mem::replace(
                        &mut page,
                        UploadPage {
                            points: Vec::new(),
                            request_bytes: None,
                        },
                    );
                    stage_complete_page(staging, pending, full, &view, deadline, cancel)?;
                }
            }
            if !page.points.is_empty() {
                stage_complete_page(staging, pending, page, &view, deadline, cancel)?;
            }
            staging
                .finish_source(
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
    writer: &ControlWriter,
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
    writer
        .finish_dispatch(
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
    root: &PlainRootGuard,
    writer: &ControlWriter,
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
        let revision = writer
            .record_dispatch(&pending, PendingOperation::Create, deadline, cancel)
            .map_err(PublicationError::code)?;
        settle(
            writer,
            &pending,
            PendingOperation::Create,
            revision,
            backend.create_generation(&collection, deadline, cancel),
            deadline,
            cancel,
        )?;
        pending = writer
            .load_pending(deadline, cancel)
            .map_err(PublicationError::code)?
            .ok_or("PUBLICATION_STALE")?;
    }
    backend
        .verify_schema(&collection, deadline, cancel)
        .map_err(BackendError::code)?;
    let generation = pending.generation.clone();
    visit_pages(
        root,
        Some(writer),
        &generation,
        pending.uploaded_pages..generation.page_count,
        deadline,
        cancel,
        |number, page| {
            let revision = writer
                .record_dispatch(&pending, PendingOperation::Insert(number), deadline, cancel)
                .map_err(PublicationError::code)?;
            settle(
                writer,
                &pending,
                PendingOperation::Insert(number),
                revision,
                backend.insert_only(&collection, &inputs(page), deadline, cancel),
                deadline,
                cancel,
            )?;
            pending = writer
                .load_pending(deadline, cancel)
                .map_err(PublicationError::code)?
                .ok_or("PUBLICATION_STALE")?;
            Ok(())
        },
    )?;
    writer
        .load_pending(deadline, cancel)
        .map_err(PublicationError::code)?
        .ok_or("PUBLICATION_STALE")
}
fn verify_generation(
    backend: &Backend,
    root: &PlainRootGuard,
    generation: &Generation,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    let path = &root.control_path();
    control::validate_generation(path, generation, deadline, cancel)
        .map_err(PublicationError::code)?;
    let collection = generation.collection();
    backend
        .verify_schema(&collection, deadline, cancel)
        .map_err(BackendError::code)?;
    let mut canary = None;
    let mut tokenless = true;
    let mut point_count = 0_u64;
    visit_pages(
        root,
        None,
        generation,
        0..generation.page_count,
        deadline,
        cancel,
        |_, page| {
            check(deadline, cancel)?;
            backend
                .verify_points(&collection, &inputs(page), deadline, cancel)
                .map_err(BackendError::code)?;
            for point in page {
                tokenless &= !point.text.chars().any(char::is_alphanumeric);
                if canary.is_none()
                    && let Some(query) = canary_query(&point.text)
                {
                    backend
                        .verify_canary(
                            &collection,
                            PointInput {
                                id: point.point.id,
                                source_id: point.point.evidence.source_id,
                                retrieval_text: &point.text,
                            },
                            query,
                            deadline,
                            cancel,
                        )
                        .map_err(BackendError::code)?;
                    canary = Some(());
                }
            }
            point_count = point_count
                .checked_add(u64::try_from(page.len()).map_err(|_| "PUBLICATION_LIMIT")?)
                .ok_or("PUBLICATION_LIMIT")?;
            Ok(())
        },
    )?;
    let remote_count = backend
        .count_exact(&collection, deadline, cancel)
        .map_err(BackendError::code)?;
    if remote_count != generation.point_count || point_count != generation.point_count {
        return Err("PUBLICATION_POINT_COUNT_MISMATCH");
    }
    if point_count != 0 && canary.is_none() && !tokenless {
        return Err("PUBLICATION_READINESS_NOT_PROVEN");
    }
    Ok(())
}

fn canary_query(text: &str) -> Option<&str> {
    // Official CodeTokenizer (222b2e44, code.rs:87-116) separates maximal
    // Unicode alphanumeric/_ runs. Documents and ranking queries share their
    // subword/joined path. Underscore-only input emits no tokens (split_classes).
    // Select a real complete identifier, without reproducing its token stream or score.
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .find(|word| word.len() <= MAX_QUERY_BYTES && word.chars().any(char::is_alphanumeric))
}
fn cleanup(
    backend: &Backend,
    writer: &ControlWriter,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), &'static str> {
    for retired in writer
        .retired_generations(deadline, cancel)
        .map_err(PublicationError::code)?
    {
        check(deadline, cancel)?;
        if retired.unresolved_build.is_some() {
            continue;
        }
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
        writer
            .finish_retirement(&retired, deadline, cancel)
            .map_err(PublicationError::code)?;
    }
    Ok(())
}
fn stage_capture(
    root: &PlainRootGuard,
    writer: &ControlWriter,
    pending: &PendingGeneration,
    sources: &[CapturedSource],
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PendingGeneration, &'static str> {
    let mut next_point = 0;
    for source in sources {
        stage_source(
            root,
            writer,
            pending,
            source,
            &mut next_point,
            deadline,
            cancel,
        )?;
    }
    writer
        .complete_capture(pending, deadline, cancel)
        .map_err(PublicationError::code)
}

fn prepare_view(
    root: &PlainRootGuard,
    writer: ControlWriter,
    backend: &Backend,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PublishedView, &'static str> {
    let path = root.control_path();
    let prepared = (|| {
        let mut capture = writer
            .capture_corpus(deadline, cancel)
            .map_err(PublicationError::code)?;
        let mut superseded_unknown_build = false;
        if let Some(pending) = capture.pending.clone() {
            if matches!(
                pending.in_flight.as_ref(),
                Some(PendingOperation::Create | PendingOperation::Insert(_))
            ) {
                writer
                    .supersede_unknown_build(&pending, deadline, cancel)
                    .map_err(PublicationError::code)?;
                superseded_unknown_build = true;
                capture = writer
                    .capture_corpus(deadline, cancel)
                    .map_err(PublicationError::code)?;
            } else if pending.in_flight.is_some() {
                return Err("PUBLICATION_EFFECT_UNRESOLVED");
            }
        }
        if let Some(pending) = &capture.pending
            && (!pending.capture_complete || !capture.pending_matches())
        {
            writer
                .abandon_build(pending, deadline, cancel)
                .map_err(PublicationError::code)?;
            return Err("PUBLICATION_STALE");
        }
        cleanup(backend, &writer, deadline, cancel)?;
        let reuse_current = if capture.pending.is_none()
            && capture.current_matches()
            && !superseded_unknown_build
        {
            let generation = capture
                .current
                .as_ref()
                .ok_or("PUBLICATION_NOT_PUBLISHED")?;
            backend
                .exists(&generation.collection(), deadline, cancel)
                .map_err(BackendError::code)?
        } else {
            false
        };
        if reuse_current {
            let generation = capture.current.ok_or("PUBLICATION_NOT_PUBLISHED")?;
            Ok((generation, None))
        } else {
            let pending = if let Some(pending) = capture.pending.clone() {
                pending
            } else {
                let pending = writer
                    .start_build(&capture, deadline, cancel)
                    .map_err(PublicationError::code)?;
                stage_capture(root, &writer, &pending, &capture.sources, deadline, cancel)?
            };
            let pending = upload(backend, root, &writer, pending, deadline, cancel)?;
            Ok((pending.generation.clone(), Some(pending)))
        }
    })();
    // Close even after cancellation or a failed local commit. Native read-only
    // verification and final publication never observe an open writer.
    writer
        .close(deadline, cancel)
        .map_err(PlainControlError::code)?;
    let (mut generation, pending) = prepared?;
    verify_generation(backend, root, &generation, deadline, cancel)?;
    if let Some(pending) = pending {
        generation = control::commit_generation(&path, &pending, deadline, cancel)
            .map_err(PublicationError::code)?;
    }
    let view = control::open_published(&path, deadline, cancel).map_err(PublicationError::code)?;
    if view.generation != generation {
        return Err("PUBLICATION_STALE");
    }
    Ok(view)
}
fn response(view: &PublishedView) -> Value {
    let g = &view.generation;
    json!({"operation":"publish-prepared","state":"published","collection":g.collection(),
        "generation":g.sequence,"source_count":g.source_count,"gap_count":g.gap_count,
        "point_count":g.point_count,"page_count":g.page_count})
}
pub fn api_key() -> Result<Option<String>, &'static str> {
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

/// Publishes and opens the validated view under an already-held root guard.
pub fn publish(
    root: &PlainRootGuard,
    writer: ControlWriter,
    request: &PublicationRequest,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PublishedView, &'static str> {
    let connected = (|| {
        let key = api_key()?;
        Backend::connect(
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
        .map_err(BackendError::code)
    })();
    match connected {
        Ok(backend) => prepare_view(root, writer, &backend, deadline, cancel),
        Err(error) => {
            writer
                .close(deadline, cancel)
                .map_err(PlainControlError::code)?;
            Err(error)
        }
    }
}

pub fn run(
    path: &Path,
    request: &PublicationRequest,
    deadline: Instant,
    cancel: &AtomicBool,
) -> ExitCode {
    let result = (|| {
        let root = open_plain(path, deadline, cancel).map_err(PlainRootError::code)?;
        let writer = ControlWriter::open(&root.control_path(), deadline, cancel)
            .map_err(PlainControlError::code)?;
        let view = publish(&root, writer, request, deadline, cancel)?;
        let result = response(&view);
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
