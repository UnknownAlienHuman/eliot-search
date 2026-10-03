//! Listener lifetime and command orchestration for the legacy snapshot harness.

use std::fs;
use std::io::{self, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::control_store::{DevelopmentControlStore, SnapshotControl};
use crate::lexical::{LexicalIndex, LexicalIndexLimits};
use crate::snapshot::{SnapshotIndex, SnapshotLimits};

use super::config::{self, Options};
use super::protocol::{
    MAX_REQUEST_BYTES, PROTOCOL_PREFIX, constant_time_eq, decode_hex, decode_query,
    read_bounded_line, write_error, write_response,
};
use super::response::{
    capture_complete, render_health, render_lexical_response, render_search_response,
    render_status,
};
use super::state::{
    EndpointFile, OwnerFile, generate_local_token, load_or_create_token, validate_token,
};

const IO_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn run(options: Options) -> io::Result<()> {
    if options.self_test {
        return self_test();
    }
    serve(options)
}

fn serve(mut options: Options) -> io::Result<()> {
    config::ensure_loopback(options.address.ip())?;
    options.data_root = config::canonical_local_directory(&options.data_root, true)?;
    options.source_roots = config::canonical_source_roots(options.source_roots)?;
    if options.source_roots.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "at least one readable source root is required",
        ));
    }

    let runtime_dir = options.data_root.join("runtime");
    fs::create_dir_all(&runtime_dir)?;
    let _owner = OwnerFile::acquire(&runtime_dir)?;
    let mut control = DevelopmentControlStore::open(&options.data_root).map_err(io::Error::other)?;
    let token = load_or_create_token(&options.token_file)?;
    let mut snapshot = SnapshotIndex::capture(
        &options.data_root,
        &options.source_roots,
        options.limits,
    )?;
    let mut lexical = build_lexical_index(&options.data_root, &snapshot, options.limits)?;
    publish_snapshot_control(&mut control, &snapshot)?;

    let listener = TcpListener::bind(options.address)?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let _endpoint = EndpointFile::publish(
        &runtime_dir,
        address,
        options.source_roots.len(),
        &snapshot,
        &lexical,
    )?;

    println!(
        concat!(
            "{{\"service\":\"eliot-searchd\",\"state\":\"READY\",",
            "\"stage\":\"W3_LOCAL_LEXICAL\",\"address\":\"{}\",",
            "\"source_roots\":{},\"snapshot_id\":\"{}\",",
            "\"indexed_files\":{},\"lexical_terms\":{},",
            "\"source_backed_search\":true,\"lexical_search\":true,",
            "\"encrypted_revisions\":false,\"production_ready\":false}}"
        ),
        address,
        options.source_roots.len(),
        snapshot.snapshot_id(),
        snapshot.stats().indexed_files,
        lexical.term_count(),
    );

    loop {
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                stream.set_read_timeout(Some(IO_TIMEOUT))?;
                stream.set_write_timeout(Some(IO_TIMEOUT))?;
                if handle_connection(
                    &mut stream,
                    &token,
                    address,
                    &options.data_root,
                    &options.source_roots,
                    options.limits,
                    &mut snapshot,
                    &mut lexical,
                    &mut control,
                )? {
                    break;
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(error),
        }
    }

    control.mark_stopped().map_err(io::Error::other)?;
    Ok(())
}

fn build_lexical_index(
    data_root: &Path,
    snapshot: &SnapshotIndex,
    limits: SnapshotLimits,
) -> io::Result<LexicalIndex> {
    LexicalIndex::build(
        data_root,
        snapshot.snapshot_id(),
        snapshot.manifest_path(),
        snapshot.manifest_fingerprint(),
        LexicalIndexLimits::baseline(
            limits.results,
            limits.file_bytes,
            limits.excerpt_chars,
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn handle_connection(
    stream: &mut TcpStream,
    expected_token: &str,
    address: SocketAddr,
    data_root: &Path,
    source_roots: &[PathBuf],
    limits: SnapshotLimits,
    snapshot: &mut SnapshotIndex,
    lexical: &mut LexicalIndex,
    control: &mut DevelopmentControlStore,
) -> io::Result<bool> {
    let request = match read_bounded_line(stream, MAX_REQUEST_BYTES) {
        Ok(request) => request,
        Err(error) => {
            let _ = write_error(stream, "MALFORMED_REQUEST", &error.to_string());
            return Ok(false);
        }
    };
    let mut fields = request.split_whitespace();
    let protocol = fields.next();
    let token = fields.next();
    let command = fields.next();
    let trailing = fields.next();

    if protocol != Some(PROTOCOL_PREFIX) || command.is_none() || trailing.is_some() {
        write_error(stream, "MALFORMED_REQUEST", "invalid protocol frame")?;
        return Ok(false);
    }
    if !constant_time_eq(
        token.unwrap_or_default().as_bytes(),
        expected_token.as_bytes(),
    ) {
        write_error(stream, "AUTHENTICATION_FAILED", "invalid local token")?;
        return Ok(false);
    }

    match command.unwrap_or_default() {
        "health" => {
            write_response(stream, &render_health(snapshot, lexical, control, false))?;
            Ok(false)
        }
        "status" => {
            write_response(
                stream,
                &render_status(
                    address,
                    source_roots.len(),
                    snapshot,
                    lexical,
                    control,
                ),
            )?;
            Ok(false)
        }
        "version" => {
            let response = format!(
                concat!(
                    "{{\"ok\":true,\"service\":\"eliot-searchd\",",
                    "\"version\":\"{}\",\"protocol\":1}}"
                ),
                env!("CARGO_PKG_VERSION")
            );
            write_response(stream, &response)?;
            Ok(false)
        }
        "refresh" => {
            handle_refresh(
                stream,
                data_root,
                source_roots,
                limits,
                snapshot,
                lexical,
                control,
            )?;
            Ok(false)
        }
        "shutdown" => {
            write_response(
                stream,
                "{\"ok\":true,\"service\":\"eliot-searchd\",\"state\":\"DRAINING\"}",
            )?;
            Ok(true)
        }
        value if value.starts_with("search:") => {
            let Some(query) = decode_query(stream, &value[7..])? else {
                return Ok(false);
            };
            match snapshot.search(&query) {
                Ok(mut result) => {
                    result.complete &= capture_complete(snapshot);
                    write_response(stream, &render_search_response(&query, &result))?;
                }
                Err(error) => write_error(stream, "SEARCH_FAILED", &error.to_string())?,
            }
            Ok(false)
        }
        value if value.starts_with("lexical:") => {
            let Some(query) = decode_query(stream, &value[8..])? else {
                return Ok(false);
            };
            match lexical.search(&query) {
                Ok(mut result) => {
                    result.complete &= capture_complete(snapshot);
                    write_response(stream, &render_lexical_response(&query, &result))?;
                }
                Err(error) => write_error(stream, "LEXICAL_SEARCH_FAILED", &error.to_string())?,
            }
            Ok(false)
        }
        _ => {
            write_error(stream, "UNKNOWN_COMMAND", "unsupported command")?;
            Ok(false)
        }
    }
}

fn handle_refresh(
    stream: &mut TcpStream,
    data_root: &Path,
    source_roots: &[PathBuf],
    limits: SnapshotLimits,
    snapshot: &mut SnapshotIndex,
    lexical: &mut LexicalIndex,
    control: &mut DevelopmentControlStore,
) -> io::Result<()> {
    match SnapshotIndex::capture(data_root, source_roots, limits).and_then(|new_snapshot| {
        build_lexical_index(data_root, &new_snapshot, limits)
            .map(|new_lexical| (new_snapshot, new_lexical))
    }) {
        Ok((new_snapshot, new_lexical)) => {
            if let Err(error) = publish_snapshot_control(control, &new_snapshot) {
                write_error(stream, "SNAPSHOT_CONTROL_COMMIT_FAILED", &error.to_string())?;
                return Ok(());
            }
            *snapshot = new_snapshot;
            *lexical = new_lexical;
            write_response(stream, &render_health(snapshot, lexical, control, true))?;
        }
        Err(error) => {
            write_error(stream, "SNAPSHOT_REFRESH_FAILED", &error.to_string())?;
        }
    }
    Ok(())
}

fn publish_snapshot_control(
    control: &mut DevelopmentControlStore,
    snapshot: &SnapshotIndex,
) -> io::Result<()> {
    control
        .publish_ready(SnapshotControl {
            snapshot_id: snapshot.snapshot_id().to_owned(),
            manifest_fingerprint: crate::snapshot::hex32(snapshot.manifest_fingerprint()),
            fingerprint_algorithm: SnapshotIndex::fingerprint_algorithm().to_owned(),
            indexed_files: snapshot.stats().indexed_files,
            total_bytes: snapshot.stats().total_bytes,
            capture_complete: capture_complete(snapshot),
        })
        .map_err(io::Error::other)
}

fn self_test() -> io::Result<()> {
    let token = generate_local_token()?;
    validate_token(&token)?;
    let round_trip = decode_hex("656c696f74")?;
    if round_trip != b"eliot"
        || !constant_time_eq(token.as_bytes(), token.as_bytes())
        || constant_time_eq(token.as_bytes(), b"wrong")
    {
        return Err(io::Error::other("runtime self-test failed"));
    }
    println!(
        concat!(
            "{{\"ok\":true,\"service\":\"eliot-searchd\",",
            "\"self_test\":\"PASS\",\"mode\":\"W3_LOCAL_LEXICAL\"}}"
        )
    );
    Ok(())
}
