//! Bootable W1 local service shell for ELIOT Search.

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
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const PROTOCOL_PREFIX: &str = "ELIOT_SEARCH/1";

#[derive(Debug)]
struct Options {
    address: SocketAddr,
    data_root: PathBuf,
    token_file: PathBuf,
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
                        "data root is already owned or requires explicit stale-lock recovery",
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
    fn publish(runtime_dir: &Path, address: SocketAddr) -> io::Result<Self> {
        let path = runtime_dir.join("endpoint.v1");
        let temporary = runtime_dir.join(format!("endpoint.v1.{}.tmp", process::id()));
        let body = format!(
            "ELIOT_SEARCH_ENDPOINT_V1\naddress={address}\nprotocol=1\nsearch_available=false\n"
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

fn serve(options: Options) -> io::Result<()> {
    ensure_loopback(options.address.ip())?;
    let runtime_dir = options.data_root.join("runtime");
    fs::create_dir_all(&runtime_dir)?;
    let _owner = OwnerFile::acquire(&runtime_dir)?;
    let token = load_or_create_token(&options.token_file)?;

    let listener = TcpListener::bind(options.address)?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let _endpoint = EndpointFile::publish(&runtime_dir, address)?;

    println!(
        "{{\"service\":\"eliot-searchd\",\"state\":\"READY\",\"stage\":\"W1\",\"address\":\"{address}\",\"search_available\":false}}"
    );

    loop {
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                stream.set_read_timeout(Some(IO_TIMEOUT))?;
                stream.set_write_timeout(Some(IO_TIMEOUT))?;
                if handle_connection(&mut stream, &token, address)? {
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

    match command.unwrap_or_default() {
        "health" => {
            write_response(
                stream,
                "{\"ok\":true,\"service\":\"eliot-searchd\",\"state\":\"READY\",\"stage\":\"W1\",\"search_available\":false}",
            )?;
            Ok(false)
        }
        "status" => {
            let response = format!(
                "{{\"ok\":true,\"service\":\"eliot-searchd\",\"state\":\"READY\",\"stage\":\"W1\",\"pid\":{},\"address\":\"{}\",\"search_available\":false}}",
                process::id(),
                address
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
        _ => {
            write_response(stream, "{\"ok\":false,\"error\":\"UNKNOWN_COMMAND\"}")?;
            Ok(false)
        }
    }
}

fn write_response(stream: &mut TcpStream, response: &str) -> io::Result<()> {
    if response.len() > MAX_REQUEST_BYTES {
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
    let mut self_test = false;

    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "serve" => {}
            "--address" => {
                let value = arguments.next().ok_or_else(|| {
                    io::Error::new(ErrorKind::InvalidInput, "--address requires a value")
                })?;
                address = value.parse().map_err(|_| {
                    io::Error::new(ErrorKind::InvalidInput, "invalid socket address")
                })?;
            }
            "--data-root" => {
                data_root = PathBuf::from(arguments.next().ok_or_else(|| {
                    io::Error::new(ErrorKind::InvalidInput, "--data-root requires a value")
                })?);
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(arguments.next().ok_or_else(|| {
                    io::Error::new(ErrorKind::InvalidInput, "--token-file requires a value")
                })?));
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
        self_test,
    })
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
        let token = fs::read_to_string(path)?;
        let token = token.trim().to_owned();
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
        format!("{:016x}", hasher.finish()).hash(&mut hasher);
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

fn unix_millis() -> io::Result<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|_| io::Error::other("system clock precedes the Unix epoch"))
}

fn self_test() -> io::Result<()> {
    ensure_loopback("127.0.0.1".parse().map_err(|_| {
        io::Error::new(ErrorKind::InvalidData, "loopback parser failed")
    })?)?;
    let token = generate_local_token()?;
    validate_token(&token)?;
    if !constant_time_eq(token.as_bytes(), token.as_bytes())
        || constant_time_eq(token.as_bytes(), b"wrong")
    {
        return Err(io::Error::other("authentication comparison self-test failed"));
    }
    println!(
        "{{\"ok\":true,\"service\":\"eliot-searchd\",\"self_test\":\"PASS\",\"search_available\":false}}"
    );
    Ok(())
}

fn print_help() {
    println!(
        "eliot-searchd {}\n\nUSAGE:\n    eliot-searchd [serve] [OPTIONS]\n\nOPTIONS:\n    --address <IP:PORT>   Loopback endpoint (default {DEFAULT_ADDRESS})\n    --data-root <PATH>    Owned local state root\n    --token-file <PATH>   Local authentication token file\n    --self-test           Run bounded startup self-test and exit\n    -V, --version         Print version\n    -h, --help            Print help",
        env!("CARGO_PKG_VERSION")
    );
}
