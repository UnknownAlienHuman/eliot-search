//! Render only one fully validated terminal after owned-child exit/reap.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_provider_client::{ClientError, QueryClient};
use search_provider_protocol::{QueryOutcome, SearchRequest};

fn failure(error: ClientError) -> i32 {
    eprintln!("{{\"error\":\"{}\"}}", error.code());
    match error {
        ClientError::Deadline => 124,
        ClientError::Cancelled => 130,
        _ => 2,
    }
}
pub fn run(
    daemon: &Path,
    arguments: &[OsString],
    request: SearchRequest,
    deadline: Instant,
) -> i32 {
    let mut client = match QueryClient::new() {
        Ok(client) => client,
        Err(error) => return failure(error),
    };
    client.enable_ctrl_c();
    let cancel = AtomicBool::new(false);
    let completion = match client.query_owned_child(daemon, arguments, request, deadline, &cancel) {
        Ok(completion) => completion,
        Err(error) => return failure(error),
    };
    let exit = match &completion.terminal.outcome {
        QueryOutcome::Complete { .. } => 0,
        QueryOutcome::Cancelled => 130,
        QueryOutcome::Failed { .. } => 2,
    };
    // Serde escapes untrusted controls while preserving the exact decoded UTF-8 unit.
    // No child bytes or diagnostics are streamed before complete terminal validation.
    let Ok(bytes) = serde_json::to_vec(&completion.terminal) else {
        eprintln!("{{\"error\":\"QUERY_SERIALIZATION_FAILED\"}}");
        return 2;
    };
    let mut stdout = std::io::stdout().lock();
    if stdout
        .write_all(&bytes)
        .and_then(|()| stdout.write_all(b"\n"))
        .and_then(|()| stdout.flush())
        .is_err()
    {
        eprintln!("{{\"error\":\"OUTPUT_WRITE_ERROR\"}}");
        return 2;
    }
    exit
}
