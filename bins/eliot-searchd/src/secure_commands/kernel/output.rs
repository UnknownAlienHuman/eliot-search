use std::io::{self, Write};
use std::process::ExitCode;

use crate::direct_store::{ReadOnlyStore, SourceSummary};
use crate::owner_composition::DataRootRequest;
use crate::service_output::{json_string, write_line};
use crate::storage_security::StorageSecurityStatus;

pub(super) fn emit_verification(
    store: &ReadOnlyStore<'_>,
    storage: &StorageSecurityStatus,
    request: &DataRootRequest,
) -> Result<(), String> {
    let verification = store.verify()?;
    write_stdout(
        request,
        &format!(
            concat!(
                "{{\"event\":\"direct_store_verified\",",
                "\"namespace_id\":{},\"source_events\":{},",
                "\"registered_sources\":{},\"active_sources\":{},",
                "\"referenced_revisions\":{},\"verified_revisions\":{},",
                "\"total_revision_bytes\":{},\"source_backed\":true,",
                "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
            ),
            json_string(&store.namespace_id()),
            verification.source_events,
            verification.registered_sources,
            verification.active_sources,
            verification.referenced_revisions,
            verification.verified_revisions,
            verification.total_revision_bytes,
            storage.json(),
            storage.encrypted_at_rest,
        ),
    )
}

pub(super) fn emit_sources(
    store: &ReadOnlyStore<'_>,
    storage: &StorageSecurityStatus,
    request: &DataRootRequest,
) -> Result<(), String> {
    let sources = store.list_sources();
    let mut stdout = io::stdout().lock();
    let mut output = request.output(&mut stdout);
    for source in &sources {
        emit_source(&mut output, source)?;
    }
    write_line(
        &mut output,
        &format!(
            concat!(
                "{{\"event\":\"source_list_complete\",",
                "\"namespace_id\":{},\"sources\":{},",
                "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
            ),
            json_string(&store.namespace_id()),
            sources.len(),
            storage.json(),
            storage.encrypted_at_rest,
        ),
    )
}

fn emit_source(writer: &mut impl Write, source: &SourceSummary) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"source\",\"source_id\":{},",
                "\"revision_id\":{},\"content_digest\":{},",
                "\"path_digest\":{},\"byte_length\":{},",
                "\"identity_strength\":{},\"active\":{},",
                "\"sequence\":{},\"diagnostic_internal_identifiers\":true}}"
            ),
            json_string(&source.source_id),
            json_string(&source.revision_id),
            json_string(&source.content_digest),
            json_string(&source.path_digest),
            source.byte_length,
            json_string(source.identity_strength),
            source.active,
            source.sequence,
        ),
    )
}

pub(super) fn write_stdout(request: &DataRootRequest, value: &str) -> Result<(), String> {
    let mut output = io::stdout().lock();
    write_line(&mut request.output(&mut output), value)
}

pub(super) fn emit_process_error(error: &str) -> ExitCode {
    // Legacy storage errors may append unrestricted OS text after a colon.
    // Public command failures expose only a finite closed reason token.
    let candidate = error.split(':').next().unwrap_or_default();
    let code = if !candidate.is_empty()
        && candidate.len() <= 96
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        candidate
    } else {
        "DATA_ROOT_OPERATION_FAILED"
    };
    eprintln!("{{\"error\":{}}}", json_string(code));
    ExitCode::from(2)
}
