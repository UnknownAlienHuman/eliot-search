use std::ffi::OsString;
use std::io;
use std::path::Path;

use crate::development::DataRootGuard;
use crate::direct_store::DirectStore;
use crate::maintenance::repair_control_log;
use crate::maintenance_guard::guarded_collect_orphan_revisions;
use crate::service_output::{emit_indexed_source, json_string, write_line};
use crate::sha256;
use crate::storage_security::StorageSecurityStatus;

use super::output::write_stdout;
use super::store::{with_store, with_store_mut};
use super::support::{MAX_DIAGNOSTIC_REVISION_SLICE_BYTES, parse_u64};

pub(super) fn cmd_index_directory(arguments: &[OsString]) -> Result<(), String> {
    with_store_mut(Path::new(&arguments[1]), |root, store| {
        let indexed = store.index_directory(Path::new(&arguments[2]))?;
        let changed = indexed.iter().filter(|source| source.changed).count();
        store.verify()?;
        let storage = StorageSecurityStatus::inspect(root)?;
        let mut output = io::stdout().lock();
        for source in &indexed {
            emit_indexed_source(&mut output, source, 0, 0, &storage)?;
        }
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

pub(super) fn cmd_retire_source(arguments: &[OsString]) -> Result<(), String> {
    let source_id = arguments[2]
        .to_str()
        .ok_or_else(|| "DIRECT_SOURCE_ID_INVALID".to_owned())?;
    with_store_mut(Path::new(&arguments[1]), |root, store| {
        let source = store.retire_source(source_id)?;
        store.verify()?;
        let storage = StorageSecurityStatus::inspect(root)?;
        write_stdout(&format!(
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
        ))
    })
}

pub(super) fn cmd_read_revision(arguments: &[OsString]) -> Result<(), String> {
    let revision_id = arguments[2]
        .to_str()
        .ok_or_else(|| "DIRECT_REVISION_ID_INVALID".to_owned())?;
    let start = parse_u64(&arguments[3], "START_OFFSET")?;
    let end = parse_u64(&arguments[4], "END_OFFSET")?;
    if end.saturating_sub(start) > MAX_DIAGNOSTIC_REVISION_SLICE_BYTES {
        return Err("DIRECT_REVISION_SLICE_TOO_LARGE".to_owned());
    }
    with_store(Path::new(&arguments[1]), |_root, store, storage| {
        let slice = store.read_revision_range(revision_id, start, end)?;
        write_stdout(&format!(
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
        ))
    })
}

pub(super) fn cmd_repair_root(arguments: &[OsString]) -> Result<(), String> {
    let guard = DataRootGuard::acquire(Path::new(&arguments[1]))?;
    let repair = repair_control_log(guard.canonical_root())?;
    let store = DirectStore::open(guard.canonical_root())?;
    store.verify()?;
    let storage = StorageSecurityStatus::inspect(guard.canonical_root())?;
    write_stdout(&format!(
        concat!(
            "{{\"event\":\"direct_store_repair_complete\",",
            "\"namespace_id\":{},\"repaired\":{},",
            "\"removed_bytes\":{},\"retained_events\":{},",
            "\"last_sequence\":{},\"last_digest\":{},",
            "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
        ),
        json_string(&store.namespace_id()),
        repair.repaired,
        repair.removed_bytes,
        repair.retained_events,
        repair.last_sequence,
        json_string(&repair.last_digest),
        storage.json(),
        storage.encrypted_at_rest,
    ))
}

pub(super) fn cmd_gc_root(arguments: &[OsString]) -> Result<(), String> {
    let mode = arguments[2]
        .to_str()
        .ok_or_else(|| "DIRECT_GC_MODE_INVALID".to_owned())?;
    let apply = match mode {
        "--dry-run" => false,
        "--apply" => true,
        _ => return Err("DIRECT_GC_MODE_INVALID".to_owned()),
    };
    let guard = DataRootGuard::acquire(Path::new(&arguments[1]))?;
    let store = DirectStore::open(guard.canonical_root())?;
    store.verify()?;
    let result = guarded_collect_orphan_revisions(guard.canonical_root(), apply)?;
    store.verify()?;
    let storage = StorageSecurityStatus::inspect(guard.canonical_root())?;
    write_stdout(&format!(
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
        json_string(&store.namespace_id()),
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
    ))
}
