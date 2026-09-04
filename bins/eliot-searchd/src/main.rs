//! Bootable local ELIOT Search daemon with retained-revision DIRECT and lexical search.

#![forbid(unsafe_code)]

mod control_store;
mod lexical;
mod snapshot;

use std::env;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use control_store::{DevelopmentControlStore, SnapshotControl};
use lexical::{LexicalIndex, LexicalIndexLimits, LexicalSearchResult};
use snapshot::{SnapshotIndex, SnapshotLimits, SnapshotSearchResult, hex32};

const DEFAULT_ADDRESS: &str = "127.0.0.1:39171";
const MAX_REQUEST_BYTES: usize = 4_096;
const MAX_RESPONSE_BYTES: usize = 64 * 1_024;
const MAX_QUERY_BYTES: usize = 1_024;
const DEFAULT_MAX_FILES: usize = 10_000;
const DEFAULT_MAX_FILE_BYTES: u64 = 2 * 1_024 * 1_024;
const DEFAULT_MAX_TOTAL_BYTES: u64 = 512 * 1_024 * 1_024;
const DEFAULT_MAX_RESULTS: usize = 8;
const MAX_CONFIGURED_RESULTS: usize = 8;
const DEFAULT_MAX_EXCERPT_CHARS: usize = 240;
const MAX_CONFIGURED_EXCERPT_CHARS: usize = 512;
const IO_TIMEOUT: Duration = Duration::from_secs(30);
const PROTOCOL_PREFIX: &str = "ELIOT_SEARCH/1";

#[derive(Debug)]
struct Options {
    address: SocketAddr,
    data_root: PathBuf,
    token_file: PathBuf,
    source_roots: Vec<PathBuf>,
    limits: SnapshotLimits,
    self_test: bool,
}

struct OwnerFile {
    file: File,
}

impl OwnerFile {
    fn acquire(runtime_dir: &Path) -> io::Result<Self> {
        let path = runtime_dir.join("owner.lock");
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    ErrorKind::AlreadyExists,
                    "data root is already owned by another live process",
                ));
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
        write_owner_state(&mut file, "ACTIVE")?;
        Ok(Self { file })
    }
}

impl Drop for OwnerFile {
    fn drop(&mut self) {
        let _ = write_owner_state(&mut self.file, "RELEASED");
        let _ = self.file.unlock();
    }
}

fn write_owner_state(file: &mut File, state: &str) -> io::Result<()> {
    let body = format!(
        concat!(
            "ELIOT_SEARCH_OWNER_V1\n",
            "pid={}\n",
            "state={}\n",
            "observed_unix_ms={}\n"
        ),
        process::id(),
        state,
        unix_millis()?,
    );
    if body.len() > 4 * 1_024 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "owner record exceeds its finite ceiling",
        ));
    }
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(body.as_bytes())?;
    file.sync_all()
}

struct EndpointFile {
    path: PathBuf,
}

impl EndpointFile {
    fn publish(
        runtime_dir: &Path,
        address: SocketAddr,
        source_root_count: usize,
        snapshot: &SnapshotIndex,
        lexical: &LexicalIndex,
    ) -> io::Result<Self> {
        let path = runtime_dir.join("endpoint.v1");
        let temporary = runtime_dir.join(format!(
            "endpoint.v1.{}.{}.tmp",
            process::id(),
            unix_millis()?
        ));
        let body = format!(
            concat!(
                "ELIOT_SEARCH_ENDPOINT_V1\n",
                "address={}\n",
                "protocol=1\n",
                "source_backed_search=true\n",
                "lexical_search=true\n",
                "production_ready=false\n",
                "encrypted_revisions=false\n",
                "source_roots={}\n",
                "snapshot_id={}\n",
                "manifest_fingerprint={}\n",
                "lexical_index_fingerprint={}\n"
            ),
            address,
            source_root_count,
            snapshot.snapshot_id(),
            hex32(snapshot.manifest_fingerprint()),
            hex32(lexical.index_fingerprint()),
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(body.as_bytes())?;
        file.sync_all()?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        fs::rename(&temporary, &path)?;
        Ok(Self { path })
    }
}

impl Drop for EndpointFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn main() {
    match parse_options().and_then(run) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("eliot-searchd: {error}");
            process::exit(1);
        }
    }
}

fn run(options: Options) -> io::Result<()> {
    if options.self_test {
        return self_test();
    }
    serve(options)
}

fn serve(mut options: Options) -> io::Result<()> {
    ensure_loopback(options.address.ip())?;
    options.data_root = canonical_local_directory(&options.data_root, true)?;
    options.source_roots = canonical_source_roots(options.source_roots)?;
    if options.source_roots.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "at least one readable source root is required",
        ));
    }

    let runtime_dir = options.data_root.join("runtime");
    fs::create_dir_all(&runtime_dir)?;
    let _owner = OwnerFile::acquire(&runtime_dir)?;
    let mut control = DevelopmentControlStore::open(&options.data_root)
        .map_err(io::Error::other)?;
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
            limits.max_results,
            limits.max_file_bytes,
            limits.max_excerpt_chars,
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
    if !constant_time_eq(token.unwrap_or_default().as_bytes(), expected_token.as_bytes()) {
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
            match SnapshotIndex::capture(data_root, source_roots, limits)
                .and_then(|new_snapshot| {
                    build_lexical_index(data_root, &new_snapshot, limits)
                        .map(|new_lexical| (new_snapshot, new_lexical))
                }) {
                Ok((new_snapshot, new_lexical)) => {
                    if let Err(error) = publish_snapshot_control(control, &new_snapshot) {
                        write_error(stream, "SNAPSHOT_CONTROL_COMMIT_FAILED", &error.to_string())?;
                        return Ok(false);
                    }
                    *snapshot = new_snapshot;
                    *lexical = new_lexical;
                    write_response(
                        stream,
                        &render_health(snapshot, lexical, control, true),
                    )?;
                }
                Err(error) => {
                    write_error(stream, "SNAPSHOT_REFRESH_FAILED", &error.to_string())?;
                }
            }
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

fn decode_query(stream: &mut TcpStream, encoded: &str) -> io::Result<Option<String>> {
    let query_bytes = match decode_hex(encoded) {
        Ok(bytes) => bytes,
        Err(error) => {
            write_error(stream, "INVALID_QUERY", &error.to_string())?;
            return Ok(None);
        }
    };
    if query_bytes.is_empty() || query_bytes.len() > MAX_QUERY_BYTES {
        write_error(stream, "INVALID_QUERY", "query exceeds its finite bounds")?;
        return Ok(None);
    }
    match String::from_utf8(query_bytes) {
        Ok(query) => Ok(Some(query)),
        Err(_) => {
            write_error(stream, "INVALID_QUERY", "query is not UTF-8")?;
            Ok(None)
        }
    }
}

fn capture_complete(snapshot: &SnapshotIndex) -> bool {
    let stats = snapshot.stats();
    !stats.truncated && stats.unreadable_files == 0 && stats.unstable_files == 0
}

fn publish_snapshot_control(
    control: &mut DevelopmentControlStore,
    snapshot: &SnapshotIndex,
) -> io::Result<()> {
    control
        .publish_ready(SnapshotControl {
            snapshot_id: snapshot.snapshot_id().to_owned(),
            manifest_fingerprint: hex32(snapshot.manifest_fingerprint()),
            fingerprint_algorithm: snapshot.fingerprint_algorithm().to_owned(),
            indexed_files: snapshot.stats().indexed_files,
            total_bytes: snapshot.stats().total_bytes,
            capture_complete: capture_complete(snapshot),
        })
        .map_err(io::Error::other)
}

fn render_health(
    snapshot: &SnapshotIndex,
    lexical: &LexicalIndex,
    control: &DevelopmentControlStore,
    refreshed: bool,
) -> String {
    let stats = snapshot.stats();
    format!(
        concat!(
            "{{\"ok\":true,\"service\":\"eliot-searchd\",",
            "\"state\":\"READY\",\"stage\":\"W3_LOCAL_LEXICAL\",",
            "\"snapshot_id\":\"{}\",\"manifest_fingerprint\":\"{}\",",
            "\"fingerprint_algorithm\":\"{}\",\"indexed_files\":{},",
            "\"snapshot_bytes\":{},\"capture_complete\":{},",
            "\"lexical_documents\":{},\"lexical_terms\":{},",
            "\"lexical_postings\":{},\"lexical_index_fingerprint\":\"{}\",",
            "\"source_backed_search\":true,\"retained_revision_readback\":true,",
            "\"lexical_search\":true,\"encrypted_revisions\":false,",
            "\"production_ready\":false,\"control_generation\":{},",
            "\"recovered_previous_active\":{},\"refreshed\":{}}}"
        ),
        snapshot.snapshot_id(),
        hex32(snapshot.manifest_fingerprint()),
        snapshot.fingerprint_algorithm(),
        stats.indexed_files,
        stats.total_bytes,
        capture_complete(snapshot),
        lexical.document_count(),
        lexical.term_count(),
        lexical.posting_count(),
        hex32(lexical.index_fingerprint()),
        control.generation(),
        control.recovered_previous_active(),
        refreshed,
    )
}

fn render_status(
    address: SocketAddr,
    source_root_count: usize,
    snapshot: &SnapshotIndex,
    lexical: &LexicalIndex,
    control: &DevelopmentControlStore,
) -> String {
    let stats = snapshot.stats();
    format!(
        concat!(
            "{{\"ok\":true,\"service\":\"eliot-searchd\",",
            "\"state\":\"READY\",\"stage\":\"W3_LOCAL_LEXICAL\",",
            "\"pid\":{},\"address\":\"{}\",\"source_roots\":{},",
            "\"snapshot_id\":\"{}\",\"manifest_path\":\"{}\",",
            "\"indexed_files\":{},\"snapshot_bytes\":{},",
            "\"written_revisions\":{},\"reused_revisions\":{},",
            "\"skipped_links\":{},\"skipped_policy\":{},",
            "\"skipped_binary\":{},\"unreadable_files\":{},",
            "\"unstable_files\":{},\"capture_truncated\":{},",
            "\"capture_complete\":{},\"lexical_analyzer\":\"{}\",",
            "\"lexical_documents\":{},\"lexical_terms\":{},",
            "\"lexical_postings\":{},\"lexical_index_fingerprint\":\"{}\",",
            "\"control_generation\":{},\"control_directory\":\"{}\",",
            "\"source_backed_search\":true,\"lexical_search\":true,",
            "\"encrypted_revisions\":false,\"production_ready\":false}}"
        ),
        process::id(),
        address,
        source_root_count,
        snapshot.snapshot_id(),
        escape_json(&snapshot.manifest_path().display().to_string()),
        stats.indexed_files,
        stats.total_bytes,
        stats.written_revisions,
        stats.reused_revisions,
        stats.skipped_links,
        stats.skipped_policy,
        stats.skipped_binary,
        stats.unreadable_files,
        stats.unstable_files,
        stats.truncated,
        capture_complete(snapshot),
        escape_json(lexical.analyzer_id()),
        lexical.document_count(),
        lexical.term_count(),
        lexical.posting_count(),
        hex32(lexical.index_fingerprint()),
        control.generation(),
        escape_json(&control.directory().display().to_string()),
    )
}

fn render_search_response(query: &str, result: &SnapshotSearchResult) -> String {
    let mut output = format!(
        concat!(
            "{{\"ok\":true,\"mode\":\"DIRECT_RETAINED_REVISION\",",
            "\"query\":\"{}\",\"query_case_policy\":\"exact_plus_ascii_insensitive\",",
            "\"snapshot_id\":\"{}\",\"manifest_fingerprint\":\"{}\",",
            "\"fingerprint_algorithm\":\"{}\",\"denominator_files\":{},",
            "\"scanned_revisions\":{},\"unavailable_revisions\":{},",
            "\"complete\":{},\"truncated\":{},\"source_backed\":true,",
            "\"retained_revision_readback\":true,\"encrypted_at_rest\":false,",
            "\"production_ready\":false,\"results\":["
        ),
        escape_json(query),
        result.snapshot_id,
        hex32(result.manifest_fingerprint),
        result.fingerprint_algorithm,
        result.denominator_files,
        result.scanned_revisions,
        result.unavailable_revisions,
        result.complete,
        result.truncated,
    );
    for (index, item) in result.matches.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(&format!(
            concat!(
                "{{\"root\":{},\"path\":\"{}\",",
                "\"revision_fingerprint\":\"{}\",\"line\":{},",
                "\"column_bytes\":{},\"byte_start\":{},\"byte_end\":{},",
                "\"excerpt\":\"{}\"}}"
            ),
            item.root_index,
            escape_json(&item.relative_path),
            hex32(item.revision_fingerprint),
            item.line,
            item.column_bytes,
            item.byte_start,
            item.byte_end,
            escape_json(&item.excerpt),
        ));
    }
    output.push_str("]}");
    output
}

fn render_lexical_response(query: &str, result: &LexicalSearchResult) -> String {
    let mut output = format!(
        concat!(
            "{{\"ok\":true,\"mode\":\"LEXICAL_BM25_RETAINED_REVISION\",",
            "\"query\":\"{}\",\"snapshot_id\":\"{}\",",
            "\"manifest_fingerprint\":\"{}\",\"lexical_index_fingerprint\":\"{}\",",
            "\"analyzer\":\"{}\",\"denominator_documents\":{},",
            "\"indexed_terms\":{},\"indexed_postings\":{},",
            "\"query_term_count\":{},\"candidate_documents\":{},",
            "\"unavailable_revisions\":{},\"complete\":{},\"truncated\":{},",
            "\"source_backed\":true,\"retained_revision_readback\":true,",
            "\"encrypted_at_rest\":false,\"production_ready\":false,",
            "\"results\":["
        ),
        escape_json(query),
        result.snapshot_id,
        hex32(result.snapshot_manifest_fingerprint),
        hex32(result.index_fingerprint),
        escape_json(&result.analyzer_id),
        result.denominator_documents,
        result.indexed_terms,
        result.indexed_postings,
        result.query_term_count,
        result.candidate_documents,
        result.unavailable_revisions,
        result.complete,
        result.truncated,
    );
    for (index, item) in result.matches.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(&format!(
            concat!(
                "{{\"root\":{},\"path\":\"{}\",",
                "\"revision_fingerprint\":\"{}\",\"score\":{:.8},",
                "\"matched_terms\":{},\"line\":{},\"column_bytes\":{},",
                "\"byte_start\":{},\"excerpt\":\"{}\"}}"
            ),
            item.root_index,
            escape_json(&item.relative_path),
            hex32(item.revision_fingerprint),
            item.score,
            item.matched_terms,
            item.line,
            item.column_bytes,
            item.byte_start,
            escape_json(&item.excerpt),
        ));
    }
    output.push_str("]}");
    output
}

fn canonical_source_roots(roots: Vec<PathBuf>) -> io::Result<Vec<PathBuf>> {
    let roots = if roots.is_empty() {
        vec![env::current_dir()?]
    } else {
        roots
    };
    let mut canonical = Vec::new();
    for root in roots {
        let root = canonical_local_directory(&root, false)?;
        if !canonical.contains(&root) {
            canonical.push(root);
        }
    }
    Ok(canonical)
}

fn canonical_local_directory(path: &Path, create: bool) -> io::Result<PathBuf> {
    if create {
        fs::create_dir_all(path)?;
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "directory must be a real local directory, not a symbolic link",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "directory reparse points are not accepted",
            ));
        }
    }
    let canonical = fs::canonicalize(path)?;
    let metadata = fs::symlink_metadata(&canonical)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "canonical directory identity is invalid",
        ));
    }
    Ok(canonical)
}

fn load_or_create_token(path: &Path) -> io::Result<String> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 128 {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "authentication token must be a small regular file",
            ));
        }
        restrict_token_permissions(path)?;
        let token = fs::read_to_string(path)?.trim().to_owned();
        validate_token(&token)?;
        return Ok(token);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let token = generate_local_token()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(token.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    restrict_token_permissions(path)?;
    Ok(token)
}

fn generate_local_token() -> io::Result<String> {
    let mut bytes = [0_u8; 32];
    fill_random_bytes(&mut bytes)?;
    let token = hex_bytes(&bytes);
    validate_token(&token)?;
    Ok(token)
}

#[cfg(unix)]
fn fill_random_bytes(bytes: &mut [u8]) -> io::Result<()> {
    File::open("/dev/urandom")?.read_exact(bytes)
}

#[cfg(not(unix))]
fn fill_random_bytes(bytes: &mut [u8]) -> io::Result<()> {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hash, Hasher};

    let now = unix_millis()?;
    for (index, chunk) in bytes.chunks_mut(8).enumerate() {
        let state = RandomState::new();
        let mut hasher = state.build_hasher();
        process::id().hash(&mut hasher);
        now.hash(&mut hasher);
        index.hash(&mut hasher);
        let value = hasher.finish().to_be_bytes();
        chunk.copy_from_slice(&value[..chunk.len()]);
    }
    Ok(())
}

fn validate_token(token: &str) -> io::Result<()> {
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "authentication token must be exactly 64 hexadecimal characters",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn restrict_token_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_token_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn ensure_loopback(ip: IpAddr) -> io::Result<()> {
    if ip.is_loopback() {
        Ok(())
    } else {
        Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "eliot-searchd may bind only to a loopback address",
        ))
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let width = left.len().max(right.len());
    for index in 0..width {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

fn write_error(stream: &mut TcpStream, code: &str, detail: &str) -> io::Result<()> {
    write_response(
        stream,
        &format!(
            "{{\"ok\":false,\"error\":\"{}\",\"detail\":\"{}\"}}",
            escape_json(code),
            escape_json(detail),
        ),
    )
}

fn write_response(stream: &mut TcpStream, response: &str) -> io::Result<()> {
    if response.len() > MAX_RESPONSE_BYTES {
        return write_error_fallback(stream, "RESPONSE_TOO_LARGE");
    }
    stream.write_all(response.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn write_error_fallback(stream: &mut TcpStream, code: &str) -> io::Result<()> {
    let response = format!("{{\"ok\":false,\"error\":\"{code}\"}}\n");
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn read_bounded_line(stream: &mut TcpStream, limit: usize) -> io::Result<String> {
    let mut bytes = Vec::with_capacity(256);
    let mut byte = [0_u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) => {
                if bytes.len() >= limit {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        "request exceeds the bounded protocol frame",
                    ));
                }
                bytes.push(byte[0]);
            }
            Err(error) => return Err(error),
        }
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "request is not UTF-8"))
}

fn decode_hex(value: &str) -> io::Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "invalid hexadecimal query",
        ));
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        output.push((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?);
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> io::Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(io::Error::new(
            ErrorKind::InvalidData,
            "invalid hexadecimal query",
        )),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn escape_json(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", u32::from(character)));
            }
            character => output.push(character),
        }
    }
    output
}

fn parse_options() -> io::Result<Options> {
    let mut address = DEFAULT_ADDRESS
        .parse::<SocketAddr>()
        .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "invalid default address"))?;
    let mut data_root = default_data_root()?;
    let mut token_file: Option<PathBuf> = None;
    let mut source_roots = Vec::new();
    let mut limits = SnapshotLimits {
        max_files: DEFAULT_MAX_FILES,
        max_file_bytes: DEFAULT_MAX_FILE_BYTES,
        max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
        max_results: DEFAULT_MAX_RESULTS,
        max_excerpt_chars: DEFAULT_MAX_EXCERPT_CHARS,
    };
    let mut self_test = false;

    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "serve" => {}
            "--address" => {
                address = next_value(&mut arguments, "--address")?
                    .parse()
                    .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "invalid address"))?;
            }
            "--data-root" => {
                data_root = PathBuf::from(next_value(&mut arguments, "--data-root")?);
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(next_value(&mut arguments, "--token-file")?));
            }
            "--source-root" => {
                source_roots.push(PathBuf::from(next_value(&mut arguments, "--source-root")?));
            }
            "--max-files" => {
                limits.max_files = parse_positive_usize(
                    &next_value(&mut arguments, "--max-files")?,
                    "--max-files",
                )?;
            }
            "--max-file-bytes" => {
                limits.max_file_bytes = parse_positive_u64(
                    &next_value(&mut arguments, "--max-file-bytes")?,
                    "--max-file-bytes",
                )?;
            }
            "--max-total-bytes" => {
                limits.max_total_bytes = parse_positive_u64(
                    &next_value(&mut arguments, "--max-total-bytes")?,
                    "--max-total-bytes",
                )?;
            }
            "--max-results" => {
                limits.max_results = parse_positive_usize(
                    &next_value(&mut arguments, "--max-results")?,
                    "--max-results",
                )?;
            }
            "--max-excerpt-chars" => {
                limits.max_excerpt_chars = parse_positive_usize(
                    &next_value(&mut arguments, "--max-excerpt-chars")?,
                    "--max-excerpt-chars",
                )?;
            }
            "--self-test" => self_test = true,
            "--help" | "-h" => {
                print_help();
                process::exit(0);
            }
            "--version" | "-V" => {
                println!("eliot-searchd {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            _ => {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("unknown argument: {argument}"),
                ));
            }
        }
    }

    ensure_loopback(address.ip())?;
    limits.validate()?;
    if limits.max_results > MAX_CONFIGURED_RESULTS
        || limits.max_excerpt_chars > MAX_CONFIGURED_EXCERPT_CHARS
    {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "response-shaping limits exceed the protocol ceiling",
        ));
    }
    let token_file = token_file.unwrap_or_else(|| data_root.join("runtime").join("auth.token"));
    Ok(Options {
        address,
        data_root,
        token_file,
        source_roots,
        limits,
        self_test,
    })
}

fn next_value(arguments: &mut impl Iterator<Item = String>, name: &str) -> io::Result<String> {
    arguments.next().ok_or_else(|| {
        io::Error::new(ErrorKind::InvalidInput, format!("{name} requires a value"))
    })
}

fn parse_positive_usize(value: &str, name: &str) -> io::Result<usize> {
    let value = value
        .parse::<usize>()
        .map_err(|_| io::Error::new(ErrorKind::InvalidInput, format!("invalid {name}")))?;
    if value == 0 {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("{name} must be positive"),
        ));
    }
    Ok(value)
}

fn parse_positive_u64(value: &str, name: &str) -> io::Result<u64> {
    let value = value
        .parse::<u64>()
        .map_err(|_| io::Error::new(ErrorKind::InvalidInput, format!("invalid {name}")))?;
    if value == 0 {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("{name} must be positive"),
        ));
    }
    Ok(value)
}

fn default_data_root() -> io::Result<PathBuf> {
    if let Some(value) = env::var_os("ELIOT_SEARCH_DATA_ROOT") {
        return Ok(PathBuf::from(value));
    }
    #[cfg(windows)]
    if let Some(value) = env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(value).join("Eliot").join("Search"));
    }
    #[cfg(not(windows))]
    if let Some(value) = env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(value).join("eliot-search"));
    }
    env::current_dir().map(|directory| directory.join(".eliot-search"))
}

fn unix_millis() -> io::Result<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|_| io::Error::other("system clock precedes the Unix epoch"))
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

fn print_help() {
    println!(
        concat!(
            "eliot-searchd {}\n\n",
            "USAGE:\n",
            "    eliot-searchd [serve] [OPTIONS]\n\n",
            "OPTIONS:\n",
            "    --address <IP:PORT>       Loopback endpoint (default {})\n",
            "    --data-root <PATH>        Owned local state root\n",
            "    --token-file <PATH>       Local authentication token file\n",
            "    --source-root <PATH>      Search root; repeatable (default cwd)\n",
            "    --max-files <N>           Snapshot file ceiling (default {})\n",
            "    --max-file-bytes <N>      Per-file byte ceiling (default {})\n",
            "    --max-total-bytes <N>     Snapshot byte ceiling (default {})\n",
            "    --max-results <N>         Result ceiling, at most {} (default {})\n",
            "    --max-excerpt-chars <N>   Excerpt ceiling, at most {} (default {})\n",
            "    --self-test               Run bounded startup self-test and exit\n",
            "    -V, --version             Print version\n",
            "    -h, --help                Print help\n\n",
            "The current local snapshot retains plaintext UTF-8 revisions. It is\n",
            "source-backed but not yet the encrypted production storage profile."
        ),
        env!("CARGO_PKG_VERSION"),
        DEFAULT_ADDRESS,
        DEFAULT_MAX_FILES,
        DEFAULT_MAX_FILE_BYTES,
        DEFAULT_MAX_TOTAL_BYTES,
        MAX_CONFIGURED_RESULTS,
        DEFAULT_MAX_RESULTS,
        MAX_CONFIGURED_EXCERPT_CHARS,
        DEFAULT_MAX_EXCERPT_CHARS,
    );
}
