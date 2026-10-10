use std::ffi::OsString;
use std::io;
use std::path::Path;

use crate::maintenance_guard::guarded_collect_orphan_revisions;
use crate::owner_composition::DataRootRequest;
use crate::service_output::{emit_indexed_source, json_string, write_line};
use crate::sha256;
use crate::storage_security::StorageSecurityStatus;

use super::output::write_stdout;
use super::store::{with_store_mut_request, with_store_request};
use super::support::{MAX_DIAGNOSTIC_REVISION_SLICE_BYTES, parse_u64};

pub(super) fn cmd_index_directory(
    arguments: &[OsString],
    request: &DataRootRequest,
) -> Result<(), String> {
    with_store_mut_request(Path::new(&arguments[1]), request, |root, store| {
        let indexed = store.index_directory(Path::new(&arguments[2]))?;
        let changed = indexed.iter().filter(|source| source.changed).count();
        store.verify()?;
        let storage = StorageSecurityStatus::inspect(root)?;
        request.preflight()?;
        let mut stdout = io::stdout().lock();
        let mut output = request.output(&mut stdout);
        for source in &indexed {
            request.preflight()?;
            emit_indexed_source(&mut output, source, 0, 0, &storage)?;
        }
        request.preflight()?;
        write_line(
            &mut output,
            &format!(
                concat!(
                    "{{\"event\":\"directory_index_complete\",",
                    "\"namespace_id\":{},\"sources\":{},",
                    "\"changed\":{},\"storage_security\":{},",
                    "\"encrypted_at_rest\":{}}}"
                ),
                json_string(&store.namespace_id()),
                indexed.len(),
                changed,
                storage.json(),
                storage.encrypted_at_rest,
            ),
        )
    })
}

pub(super) fn cmd_retire_source(
    arguments: &[OsString],
    request: &DataRootRequest,
) -> Result<(), String> {
    let source_id = arguments[2]
        .to_str()
        .ok_or_else(|| "DIRECT_SOURCE_ID_INVALID".to_owned())?;
    with_store_mut_request(Path::new(&arguments[1]), request, |root, store| {
        let source = store.retire_source(source_id)?;
        store.verify()?;
        let storage = StorageSecurityStatus::inspect(root)?;
        request.preflight()?;
        write_stdout(
            request,
            &format!(
                concat!(
                    "{{\"event\":\"source_retired\",",
                    "\"source_id\":{},\"revision_id\":{},",
                    "\"sequence\":{},\"active\":false,",
                    "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
                ),
                json_string(&source.source_id),
                json_string(&source.revision_id),
                source.sequence,
                storage.json(),
                storage.encrypted_at_rest,
            ),
        )
    })
}

pub(super) fn cmd_read_revision(
    arguments: &[OsString],
    request: &DataRootRequest,
) -> Result<(), String> {
    let revision_id = arguments[2]
        .to_str()
        .ok_or_else(|| "DIRECT_REVISION_ID_INVALID".to_owned())?;
    let start = parse_u64(&arguments[3], "START_OFFSET")?;
    let end = parse_u64(&arguments[4], "END_OFFSET")?;
    if end.saturating_sub(start) > MAX_DIAGNOSTIC_REVISION_SLICE_BYTES {
        return Err("DIRECT_REVISION_SLICE_TOO_LARGE".to_owned());
    }
    with_store_request(
        Path::new(&arguments[1]),
        request,
        |_root, store, storage| {
            let slice = store.read_revision_range(revision_id, start, end)?;
            request.preflight()?;
            write_stdout(
                request,
                &format!(
                    concat!(
                        "{{\"event\":\"revision_slice\",",
                        "\"revision_id\":{},\"content_digest\":{},",
                        "\"byte_start\":{},\"byte_end\":{},",
                        "\"encoding\":\"hex\",\"bytes\":{},",
                        "\"source_backed\":true,\"storage_backend\":{},",
                        "\"encrypted_at_rest\":{}}}"
                    ),
                    json_string(&slice.revision_id),
                    json_string(&slice.content_digest),
                    slice.byte_start,
                    slice.byte_end,
                    json_string(&sha256::hex(&slice.bytes)),
                    json_string(storage.backend),
                    storage.encrypted_at_rest,
                ),
            )
        },
    )
}

pub(super) fn cmd_repair_root(_arguments: &[OsString]) -> Result<(), String> {
    // A root locator cannot name or authorize the uncertain durable operation.
    Err("DATA_ROOT_NAMED_RECOVERY_REQUIRED".to_owned())
}
pub(super) fn cmd_gc_root(arguments: &[OsString], request: &DataRootRequest) -> Result<(), String> {
    let mode = arguments[2]
        .to_str()
        .ok_or_else(|| "DIRECT_GC_MODE_INVALID".to_owned())?;
    let apply = match mode {
        "--dry-run" => false,
        "--apply" => true,
        _ => return Err("DIRECT_GC_MODE_INVALID".to_owned()),
    };
    if apply {
        with_store_mut_request(Path::new(&arguments[1]), request, |root, store| {
            store.verify()?;
            let result = guarded_collect_orphan_revisions(root, true)?;
            store.verify()?;
            let storage = StorageSecurityStatus::inspect(root)?;
            emit_gc(request, &store.namespace_id(), &result, &storage)
        })
    } else {
        with_store_request(Path::new(&arguments[1]), request, |root, store, storage| {
            store.verify_catalog()?;
            let result = guarded_collect_orphan_revisions(root, false)?;
            emit_gc(request, &store.namespace_id(), &result, storage)
        })
    }
}

fn emit_gc(
    request: &DataRootRequest,
    namespace: &str,
    result: &crate::maintenance::GarbageCollectionResult,
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    request.preflight()?;
    write_stdout(
        request,
        &format!(
            concat!(
                "{{\"event\":\"direct_store_gc_complete\",",
                "\"namespace_id\":{},\"applied\":{},",
                "\"referenced_revisions\":{},\"scanned_objects\":{},",
                "\"plaintext_objects\":{},\"protected_objects\":{},",
                "\"temporary_objects\":{},",
                "\"referenced_plaintext_objects\":{},",
                "\"referenced_protected_objects\":{},",
                "\"orphan_objects\":{},\"orphan_bytes\":{},",
                "\"deleted_objects\":{},\"deleted_bytes\":{},",
                "\"unexpected_objects\":{},\"storage_security\":{},",
                "\"encrypted_at_rest\":{}}}"
            ),
            json_string(namespace),
            result.applied,
            result.referenced_revisions,
            result.scanned_objects,
            result.plaintext_objects,
            result.protected_objects,
            result.temporary_objects,
            result.referenced_plaintext_objects,
            result.referenced_protected_objects,
            result.orphan_objects,
            result.orphan_bytes,
            result.deleted_objects,
            result.deleted_bytes,
            result.unexpected_objects,
            storage.json(),
            storage.encrypted_at_rest,
        ),
    )
}
