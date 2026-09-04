//! Bootable local ELIOT Search daemon with bounded DIRECT source scanning.

#![forbid(unsafe_code)]

use std::collections::hash_map::RandomState;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hash, Hasher};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_ADDRESS: &str = "127.0.0.1:39171";
const MAX_REQUEST_BYTES: usize = 4_096;
const MAX_RESPONSE_BYTES: usize = 64 * 1_024;
const MAX_QUERY_BYTES: usize = 1_024;
const DEFAULT_MAX_FILES: usize = 10_000;
const DEFAULT_MAX_FILE_BYTES: u64 = 2 * 1_024 * 1_024;
const DEFAULT_MAX_RESULTS: usize = 32;
const MAX_EXCERPT_CHARS: usize = 240;
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const PROTOCOL_PREFIX: &str = "ELIOT_SEARCH/1";

#[derive(Clone, Copy, Debug)]
struct SearchLimits {
    max_files: usize,
    max_file_bytes: u64,
    max_results: usize,
}

#[derive(Debug)]
struct Options {
    address: SocketAddr,
    data_root: PathBuf,
    token_file: PathBuf,
    source_roots: Vec<PathBuf>,
    limits: SearchLimits,
    self_test: bool,
}

struct OwnerFile {
    path: PathBuf,
    _file: File,
}

impl OwnerFile {
    fn acquire(runtime_dir: &Path) -> io::Result<Self> {
        let path = runtime_dir.join("owner.lock");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                if error.kind() == ErrorKind::AlreadyExists {
                    io::Error::new(
                        ErrorKind::AlreadyExists,
                        "data root is already owned or requires explicit stale-lock cleanup",
                    )
                } else {
                    error
                }
            })?;
        writeln!(file, "pid={}", process::id())?;
        writeln!(file, "started_unix_ms={}", unix_millis()?)?;
        file.sync_all()?;
        Ok(Self { path, _file: file })
    }
}

impl Drop for OwnerFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct EndpointFile {
    path: PathBuf,
}

impl EndpointFile {
    fn publish(runtime_dir: &Path, address: SocketAddr, root_count: usize) -> io::Result<Self> {
        let path = runtime_dir.join("endpoint.v1");
        let temporary = runtime_dir.join(format!("endpoint.v1.{}.tmp", process::id()));
        let body = format!(
            "ELIOT_SEARCH_ENDPOINT_V1\naddress={address}\nprotocol=1\nsearch_available=true\nsource_roots={root_count}\n"
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

#[derive(Debug)]
struct DirectMatch {
    root_index: usize,
    relative_path: String,
    line: usize,
    excerpt: String,
}

#[derive(Debug)]
struct DirectSearchResult {
    matches: Vec<DirectMatch>,
    scanned_files: usize,
    unreadable_files: usize,
    truncated: bool,
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
    let token = load_or_create_token(&options.token_file)?;

    let listener = TcpListener::bind(options.address)?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let _endpoint = EndpointFile::publish(&runtime_dir, address, options.source_roots.len())?;

    println!(
        "{{\"service\":\"eliot-searchd\",\"state\":\"READY\",\"stage\":\"W2_DIRECT\",\"address\":\"{address}\",\"source_roots\":{},\"search_available\":true}}",
        options.source_roots.len()
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
                    &options.source_roots,
                    options.limits,
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

    Ok(())
}

fn handle_connection(
    stream: &mut TcpStream,
    expected_token: &str,
    address: SocketAddr,
    source_roots: &[PathBuf],
    limits: SearchLimits,
) -> io::Result<bool> {
    let request = read_bounded_line(stream, MAX_REQUEST_BYTES)?;
    let mut fields = request.split_whitespace();
    let protocol = fields.next();
    let token = fields.next();
    let command = fields.next();
    let trailing = fields.next();

    if protocol != Some(PROTOCOL_PREFIX) || command.is_none() || trailing.is_some() {
        write_response(stream, "{\"ok\":false,\"error\":\"MALFORMED_REQUEST\"}")?;
        return Ok(false);
    }
    if !constant_time_eq(token.unwrap_or_default().as_bytes(), expected_token.as_bytes()) {
        write_response(stream, "{\"ok\":false,\"error\":\"AUTHENTICATION_FAILED\"}")?;
        return Ok(false);
    }

    let command = command.unwrap_or_default();
    match command {
        "health" => {
            let response = format!(
                "{{\"ok\":true,\"service\":\"eliot-searchd\",\"state\":\"READY\",\"stage\":\"W2_DIRECT\",\"source_roots\":{},\"search_available\":true}}",
                source_roots.len()
            );
            write_response(stream, &response)?;
            Ok(false)
        }
        "status" => {
            let response = format!(
                "{{\"ok\":true,\"service\":\"eliot-searchd\",\"state\":\"READY\",\"stage\":\"W2_DIRECT\",\"pid\":{},\"address\":\"{}\",\"source_roots\":{},\"search_available\":true}}",
                process::id(),
                address,
                source_roots.len()
            );
            write_response(stream, &response)?;
            Ok(false)
        }
        "version" => {
            let response = format!(
                "{{\"ok\":true,\"service\":\"eliot-searchd\",\"version\":\"{}\",\"protocol\":1}}",
                env!("CARGO_PKG_VERSION")
            );
            write_response(stream, &response)?;
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
            let query_bytes = decode_hex(&value[7..])?;
            if query_bytes.is_empty() || query_bytes.len() > MAX_QUERY_BYTES {
                write_response(stream, "{\"ok\":false,\"error\":\"INVALID_QUERY\"}")?;
                return Ok(false);
            }
            let query = String::from_utf8(query_bytes)
                .map_err(|_| io::Error::new(ErrorKind::InvalidData, "query is not UTF-8"))?;
            let result = direct_search(source_roots, &query, limits)?;
            write_response(stream, &render_search_response(&query, &result))?;
            Ok(false)
        }
        _ => {
            write_response(stream, "{\"ok\":false,\"error\":\"UNKNOWN_COMMAND\"}")?;
            Ok(false)
        }
    }
}

fn direct_search(
    source_roots: &[PathBuf],
    query: &str,
    limits: SearchLimits,
) -> io::Result<DirectSearchResult> {
    let normalized_query = query.to_lowercase();
    if normalized_query.is_empty() {
        return Err(io::Error::new(ErrorKind::InvalidInput, "query is empty"));
    }

    let mut stack = source_roots
        .iter()
        .enumerate()
        .rev()
        .map(|(index, root)| (index, root.clone()))
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    let mut scanned_files = 0_usize;
    let mut unreadable_files = 0_usize;
    let mut truncated = false;

    while let Some((root_index, path)) = stack.pop() {
        if scanned_files >= limits.max_files || matches.len() >= limits.max_results {
            truncated = true;
            break;
        }

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                unreadable_files = unreadable_files.saturating_add(1);
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if should_skip_directory(&path, &source_roots[root_index]) {
                continue;
            }
            let mut children = match fs::read_dir(&path) {
                Ok(entries) => entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .collect::<Vec<_>>(),
                Err(_) => {
                    unreadable_files = unreadable_files.saturating_add(1);
                    continue;
                }
            };
            children.sort();
            for child in children.into_iter().rev() {
                stack.push((root_index, child));
            }
            continue;
        }
        if !metadata.is_file() || metadata.len() > limits.max_file_bytes {
            continue;
        }

        scanned_files = scanned_files.saturating_add(1);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                unreadable_files = unreadable_files.saturating_add(1);
                continue;
            }
        };
        if bytes.iter().take(8_192).any(|byte| *byte == 0) {
            continue;
        }
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => continue,
        };

        for (line_index, line) in text.lines().enumerate() {
            if line.to_lowercase().contains(&normalized_query) {
                let relative = path
                    .strip_prefix(&source_roots[root_index])
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                matches.push(DirectMatch {
                    root_index,
                    relative_path: relative,
                    line: line_index.saturating_add(1),
                    excerpt: truncate_chars(line.trim(), MAX_EXCERPT_CHARS),
                });
                if matches.len() >= limits.max_results {
                    truncated = true;
                    break;
                }
            }
        }
    }

    Ok(DirectSearchResult {
        matches,
        scanned_files,
        unreadable_files,
        truncated,
    })
}

fn render_search_response(query: &str, result: &DirectSearchResult) -> String {
    let mut output = format!(
        "{{\"ok\":true,\"mode\":\"DIRECT\",\"query\":\"{}\",\"scanned_files\":{},\"unreadable_files\":{},\"truncated\":{},\"results\":[",
        escape_json(query),
        result.scanned_files,
        result.unreadable_files,
        result.truncated
    );
    for (index, item) in result.matches.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"root\":{},\"path\":\"{}\",\"line\":{},\"excerpt\":\"{}\"}}",
            item.root_index,
            escape_json(&item.relative_path),
            item.line,
            escape_json(&item.excerpt)
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
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "source root is not a directory",
            ));
        }
        if !canonical.contains(&root) {
            canonical.push(root);
        }
    }
    Ok(canonical)
}

fn should_skip_directory(path: &Path, root: &Path) -> bool {
    if path == root {
        return false;
    }
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | "target" | ".eliot-search")
    )
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut output = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        output.push('…');
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

fn write_response(stream: &mut TcpStream, response: &str) -> io::Result<()> {
    if response.len() > MAX_RESPONSE_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "response exceeds the bounded protocol frame",
        ));
    }
    stream.write_all(response.as_bytes())?;
    stream.write_all(b"\n")?;
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

fn parse_options() -> io::Result<Options> {
    let mut address = DEFAULT_ADDRESS
        .parse::<SocketAddr>()
        .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "invalid default address"))?;
    let mut data_root = default_data_root()?;
    let mut token_file: Option<PathBuf> = None;
    let mut source_roots = Vec::new();
    let mut limits = SearchLimits {
        max_files: DEFAULT_MAX_FILES,
        max_file_bytes: DEFAULT_MAX_FILE_BYTES,
        max_results: DEFAULT_MAX_RESULTS,
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
            "--max-results" => {
                limits.max_results = parse_positive_usize(
                    &next_value(&mut arguments, "--max-results")?,
                    "--max-results",
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

fn load_or_create_token(path: &Path) -> io::Result<String> {
    if path.exists() {
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
    restrict_token_permissions(&file)?;
    file.write_all(token.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(token)
}

fn generate_local_token() -> io::Result<String> {
    let state = RandomState::new();
    let now = unix_millis()?;
    let mut output = String::with_capacity(64);
    for counter in 0_u64..4 {
        let mut hasher = state.build_hasher();
        process::id().hash(&mut hasher);
        now.hash(&mut hasher);
        counter.hash(&mut hasher);
        output.push_str(&format!("{:016x}", hasher.finish()));
    }
    validate_token(&output)?;
    Ok(output)
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
fn restrict_token_permissions(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_token_permissions(_file: &File) -> io::Result<()> {
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

fn decode_hex(value: &str) -> io::Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return Err(io::Error::new(ErrorKind::InvalidData, "invalid hexadecimal query"));
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> io::Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(io::Error::new(ErrorKind::InvalidData, "invalid hexadecimal query")),
    }
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
        "{{\"ok\":true,\"service\":\"eliot-searchd\",\"self_test\":\"PASS\",\"mode\":\"DIRECT\"}}"
    );
    Ok(())
}

fn print_help() {
    println!(
        "eliot-searchd {}\n\nUSAGE:\n    eliot-searchd [serve] [OPTIONS]\n\nOPTIONS:\n    --address <IP:PORT>      Loopback endpoint (default {DEFAULT_ADDRESS})\n    --data-root <PATH>       Owned local state root\n    --token-file <PATH>      Local authentication token file\n    --source-root <PATH>     Search root; may be repeated (default current directory)\n    --max-files <N>          Per-request file ceiling (default {DEFAULT_MAX_FILES})\n    --max-file-bytes <N>     Per-file byte ceiling (default {DEFAULT_MAX_FILE_BYTES})\n    --max-results <N>        Result ceiling (default {DEFAULT_MAX_RESULTS})\n    --self-test              Run bounded startup self-test and exit\n    -V, --version            Print version\n    -h, --help               Print help",
        env!("CARGO_PKG_VERSION")
    );
}
