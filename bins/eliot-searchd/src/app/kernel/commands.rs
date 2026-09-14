//! DIRECT corpus and maintenance command handlers.

use std::path::Path;

use crate::development::{
    DataRootGuard, read_file_bounded, read_stdin_bounded,
};
use crate::direct_store::DirectStore;
use crate::maintenance::{collect_orphan_revisions, repair_control_log};
use crate::sha256;

use super::output::{
    emit_indexed_source, emit_one_shot_scan, emit_store_search,
};
use super::protocol::serve_stdio;
use super::status::direct_health_effective;

fn open_direct_store(root: &Path) -> Result<(DataRootGuard, DirectStore), String> {
    let guard = DataRootGuard::acquire(root)?;
    let store = DirectStore::open(guard.canonical_root())?;
    Ok((guard, store))
}

fn parse_u64(value: &str, name: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("INVALID_{name}"))
}

pub(super) fn cmd_serve_data_root(arguments: &[String]) -> Result<(), String> {
    let (mut guard, store) = open_direct_store(Path::new(&arguments[1]))?;
    store.verify()?;
    println!(
        "{{\"event\":\"data_root_ready\",\"namespace_id\":\"{}\",\"encrypted_at_rest\":false}}",
        store.namespace_id(),
    );
    serve_stdio(direct_health_effective()?)
        .map_err(|error| format!("STDIO_ERROR:{error}"))?;
    guard.begin_drain(search_runtime_owner::DrainReason::Shutdown)?;
    guard.release_cleanly().map(|_| ())?;
    drop(store);
    Ok(())
}

pub(super) fn cmd_scan_stdin(
    arguments: &[String],
    argument: &str,
) -> Result<(), String> {
    let text = read_stdin_bounded()?;
    emit_one_shot_scan(
        "stdin",
        &arguments[1],
        argument == "--scan-stdin-ascii-insensitive",
        &text,
        false,
        false,
    )
}

pub(super) fn cmd_scan_file(
    arguments: &[String],
    argument: &str,
) -> Result<(), String> {
    let text = read_file_bounded(Path::new(&arguments[2]))?;
    emit_one_shot_scan(
        "file",
        &arguments[1],
        argument == "--scan-file-ascii-insensitive",
        &text,
        true,
        true,
    )
}

pub(super) fn cmd_index_file(arguments: &[String]) -> Result<(), String> {
    let (_guard, mut store) = open_direct_store(Path::new(&arguments[1]))?;
    let indexed = store.index_file(Path::new(&arguments[2]))?;
    emit_indexed_source(&indexed);
    Ok(())
}

pub(super) fn cmd_retire_source(arguments: &[String]) -> Result<(), String> {
    let (_guard, mut store) = open_direct_store(Path::new(&arguments[1]))?;
    let source = store.retire_source(&arguments[2])?;
    println!(
        concat!(
            "{{\"event\":\"source_retired\",\"source_id\":\"{}\",",
            "\"revision_id\":\"{}\",\"sequence\":{},",
            "\"active\":false}}"
        ),
        source.source_id,
        source.revision_id,
        source.sequence,
    );
    Ok(())
}

pub(super) fn cmd_health_data_root(arguments: &[String]) -> Result<(), String> {
    let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
    let verification = store.verify()?;
    println!(
        concat!(
            "{{\"event\":\"health\",\"namespace_id\":\"{}\",",
            "\"registered_sources\":{},\"active_sources\":{},",
            "\"verified_revisions\":{},\"health\":{}}}"
        ),
        store.namespace_id(),
        verification.registered_sources,
        verification.active_sources,
        verification.verified_revisions,
        direct_health_effective()?.json(),
    );
    Ok(())
}

pub(super) fn cmd_index_directory(arguments: &[String]) -> Result<(), String> {
    let (_guard, mut store) = open_direct_store(Path::new(&arguments[1]))?;
    let indexed = store.index_directory(Path::new(&arguments[2]))?;
    let changed = indexed.iter().filter(|source| source.changed).count();
    for source in &indexed {
        emit_indexed_source(source);
    }
    println!(
        concat!(
            "{{\"event\":\"directory_index_complete\",",
            "\"namespace_id\":\"{}\",\"sources\":{},",
            "\"changed\":{},\"source_backed\":true,",
            "\"encrypted_at_rest\":false}}"
        ),
        store.namespace_id(),
        indexed.len(),
        changed,
    );
    Ok(())
}

pub(super) fn cmd_search_root(
    arguments: &[String],
    argument: &str,
) -> Result<(), String> {
    let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
    let result = store.search(
        &arguments[2],
        argument == "--search-root-ascii-insensitive",
    )?;
    emit_store_search(&store.namespace_id(), &result);
    Ok(())
}

pub(super) fn cmd_list_sources(arguments: &[String]) -> Result<(), String> {
    let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
    let sources = store.list_sources();
    for source in &sources {
        println!(
            concat!(
                "{{\"event\":\"source\",\"source_id\":\"{}\",",
                "\"revision_id\":\"{}\",\"content_digest\":\"{}\",",
                "\"path_digest\":\"{}\",\"byte_length\":{},",
                "\"identity_strength\":\"{}\",\"active\":{},",
                "\"sequence\":{}}}"
            ),
            source.source_id,
            source.revision_id,
            source.content_digest,
            source.path_digest,
            source.byte_length,
            source.identity_strength,
            source.active,
            source.sequence,
        );
    }
    println!(
        "{{\"event\":\"source_list_complete\",\"namespace_id\":\"{}\",\"sources\":{}}}",
        store.namespace_id(),
        sources.len(),
    );
    Ok(())
}

pub(super) fn cmd_verify_root(arguments: &[String]) -> Result<(), String> {
    let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
    let verification = store.verify()?;
    println!(
        concat!(
            "{{\"event\":\"direct_store_verified\",",
            "\"namespace_id\":\"{}\",\"source_events\":{},",
            "\"registered_sources\":{},\"active_sources\":{},",
            "\"referenced_revisions\":{},\"verified_revisions\":{},",
            "\"total_revision_bytes\":{},\"source_backed\":true,",
            "\"encrypted_at_rest\":false}}"
        ),
        store.namespace_id(),
        verification.source_events,
        verification.registered_sources,
        verification.active_sources,
        verification.referenced_revisions,
        verification.verified_revisions,
        verification.total_revision_bytes,
    );
    Ok(())
}

pub(super) fn cmd_read_revision(arguments: &[String]) -> Result<(), String> {
    let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
    let start = parse_u64(&arguments[3], "START_OFFSET")?;
    let end = parse_u64(&arguments[4], "END_OFFSET")?;
    let slice = store.read_revision_range(&arguments[2], start, end)?;
    println!(
        concat!(
            "{{\"event\":\"revision_slice\",",
            "\"revision_id\":\"{}\",\"content_digest\":\"{}\",",
            "\"byte_start\":{},\"byte_end\":{},",
            "\"encoding\":\"hex\",\"bytes\":\"{}\",",
            "\"source_backed\":true,\"encrypted_at_rest\":false}}"
        ),
        slice.revision_id,
        slice.content_digest,
        slice.byte_start,
        slice.byte_end,
        sha256::hex(&slice.bytes),
    );
    Ok(())
}

pub(super) fn cmd_repair_root(arguments: &[String]) -> Result<(), String> {
    let guard = DataRootGuard::acquire(Path::new(&arguments[1]))?;
    let repair = repair_control_log(guard.canonical_root())?;
    let store = DirectStore::open(guard.canonical_root())?;
    store.verify()?;
    println!(
        concat!(
            "{{\"event\":\"direct_store_repair_complete\",",
            "\"namespace_id\":\"{}\",\"repaired\":{},",
            "\"removed_bytes\":{},\"retained_events\":{},",
            "\"last_sequence\":{},\"last_digest\":\"{}\"}}"
        ),
        store.namespace_id(),
        repair.repaired,
        repair.removed_bytes,
        repair.retained_events,
        repair.last_sequence,
        repair.last_digest,
    );
    Ok(())
}

pub(super) fn cmd_gc_root(arguments: &[String]) -> Result<(), String> {
    let apply = match arguments[2].as_str() {
        "--dry-run" => false,
        "--apply" => true,
        _ => return Err("USAGE_ERROR".to_owned()),
    };
    let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
    store.verify()?;
    let result = collect_orphan_revisions(Path::new(&arguments[1]), apply)?;
    println!(
        concat!(
            "{{\"event\":\"direct_store_gc_complete\",",
            "\"namespace_id\":\"{}\",\"applied\":{},",
            "\"referenced_revisions\":{},\"scanned_objects\":{},",
            "\"orphan_objects\":{},\"orphan_bytes\":{},",
            "\"deleted_objects\":{},\"deleted_bytes\":{},",
            "\"unexpected_objects\":{}}}"
        ),
        store.namespace_id(),
        result.applied,
        result.referenced_revisions,
        result.scanned_objects,
        result.orphan_objects,
        result.orphan_bytes,
        result.deleted_objects,
        result.deleted_bytes,
        result.unexpected_objects,
    );
    Ok(())
}
