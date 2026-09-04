//! Protocol-only CLI for the local ELIOT Search daemon.

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

const DEFAULT_ADDRESS: &str = "127.0.0.1:39171";
const MAX_QUERY_BYTES: usize = 1_024;
const MAX_RESPONSE_BYTES: usize = 64 * 1_024;
const IO_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
enum Command {
    Health,
    Status,
    Version,
    Refresh,
    Shutdown,
    Search(String),
}

impl Command {
    fn wire(&self) -> io::Result<String> {
        match self {
            Self::Health => Ok("health".to_owned()),
            Self::Status => Ok("status".to_owned()),
            Self::Version => Ok("version".to_owned()),
            Self::Refresh => Ok("refresh".to_owned()),
            Self::Shutdown => Ok("shutdown".to_owned()),
            Self::Search(query) => {
                if query.is_empty() || query.len() > MAX_QUERY_BYTES {
                    return Err(io::Error::new(
                        ErrorKind::InvalidInput,
                        "search query must be non-empty and at most 1024 UTF-8 bytes",
                    ));
                }
                Ok(format!("search:{}", hex_encode(query.as_bytes())))
            }
        }
    }
}

#[derive(Debug)]
struct Options {
    command: Command,
    address: Option<SocketAddr>,
    data_root: PathBuf,
    token_file: Option<PathBuf>,
}

fn main() {
    match parse_options().and_then(run) {
        Ok(response) => {
            println!("{response}");
            if response.contains("\"ok\":false") {
                process::exit(2);
            }
        }
        Err(error) => {
            eprintln!("eliot-search: {error}");
            process::exit(1);
        }
    }
}

fn run(options: Options) -> io::Result<String> {
    let address = match options.address {
        Some(address) => address,
        None => read_endpoint(&options.data_root)?.unwrap_or(
            DEFAULT_ADDRESS
                .parse()
                .map_err(|_| io::Error::new(ErrorKind::InvalidData, "invalid default endpoint"))?,
        ),
    };
    ensure_loopback(address.ip())?;

    let token_file = options
        .token_file
        .unwrap_or_else(|| options.data_root.join("runtime").join("auth.token"));
    let token = read_token(&token_file)?;
    let command = options.command.wire()?;

    let mut stream = TcpStream::connect_timeout(&address, IO_TIMEOUT)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let request = format!("ELIOT_SEARCH/1 {token} {command}\n");
    stream.write_all(request.as_bytes())?;
    stream.flush()?;
    read_bounded_response(&mut stream, MAX_RESPONSE_BYTES)
}

fn parse_options() -> io::Result<Options> {
    let mut command: Option<Command> = None;
    let mut address = None;
    let mut data_root = default_data_root()?;
    let mut token_file = None;

    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "health" => set_command(&mut command, Command::Health)?,
            "status" => set_command(&mut command, Command::Status)?,
            "version" => set_command(&mut command, Command::Version)?,
            "refresh" => set_command(&mut command, Command::Refresh)?,
            "shutdown" => set_command(&mut command, Command::Shutdown)?,
            "search" => {
                let query = arguments.next().ok_or_else(|| {
                    io::Error::new(ErrorKind::InvalidInput, "search requires a quoted query")
                })?;
                set_command(&mut command, Command::Search(query))?;
            }
            "--address" => {
                let value = arguments.next().ok_or_else(|| {
                    io::Error::new(ErrorKind::InvalidInput, "--address requires a value")
                })?;
                let parsed = value.parse::<SocketAddr>().map_err(|_| {
                    io::Error::new(ErrorKind::InvalidInput, "invalid socket address")
                })?;
                ensure_loopback(parsed.ip())?;
                address = Some(parsed);
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
            "--help" | "-h" => {
                print_help();
                process::exit(0);
            }
            "--version" | "-V" => {
                println!("eliot-search {}", env!("CARGO_PKG_VERSION"));
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

    Ok(Options {
        command: command.unwrap_or(Command::Health),
        address,
        data_root,
        token_file,
    })
}

fn set_command(slot: &mut Option<Command>, command: Command) -> io::Result<()> {
    if slot.replace(command).is_some() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "exactly one command may be supplied",
        ));
    }
    Ok(())
}

fn read_endpoint(data_root: &Path) -> io::Result<Option<SocketAddr>> {
    let path = data_root.join("runtime").join("endpoint.v1");
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 16 * 1_024 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "endpoint descriptor must be a bounded regular file",
        ));
    }
    let body = fs::read_to_string(path)?;
    let mut lines = body.lines();
    if lines.next() != Some("ELIOT_SEARCH_ENDPOINT_V1") {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "unsupported endpoint descriptor",
        ));
    }
    let mut address = None;
    for line in lines {
        let Some((key, value)) = line.split_once('=') else {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "malformed endpoint descriptor",
            ));
        };
        if key == "address" {
            if address.is_some() {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "endpoint descriptor repeats address",
                ));
            }
            let parsed = value.parse::<SocketAddr>().map_err(|_| {
                io::Error::new(ErrorKind::InvalidData, "invalid endpoint descriptor address")
            })?;
            ensure_loopback(parsed.ip())?;
            address = Some(parsed);
        }
    }
    address.map(Some).ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidData,
            "endpoint descriptor has no address",
        )
    })
}

fn read_token(path: &Path) -> io::Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 128 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "authentication token must be a small regular file",
        ));
    }
    let token = fs::read_to_string(path)?.trim().to_owned();
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "authentication token must be exactly 64 hexadecimal characters",
        ));
    }
    Ok(token)
}

fn read_bounded_response(stream: &mut TcpStream, limit: usize) -> io::Result<String> {
    let mut bytes = Vec::with_capacity(512);
    let mut buffer = [0_u8; 512];
    let mut newline = None;
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = limit
            .checked_sub(bytes.len())
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "response too large"))?;
        if read > remaining {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "response exceeds the bounded protocol frame",
            ));
        }
        let start = bytes.len();
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(relative) = buffer[..read].iter().position(|byte| *byte == b'\n') {
            newline = Some(start + relative);
            break;
        }
    }

    let end = newline.unwrap_or(bytes.len());
    if let Some(newline_index) = newline {
        if bytes[newline_index + 1..]
            .iter()
            .any(|byte| !byte.is_ascii_whitespace())
        {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "daemon returned multiple protocol frames",
            ));
        }
    }
    String::from_utf8(bytes[..end].to_vec())
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "response is not UTF-8"))
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

fn ensure_loopback(ip: IpAddr) -> io::Result<()> {
    if ip.is_loopback() {
        Ok(())
    } else {
        Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "eliot-search may connect only to a loopback address",
        ))
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn print_help() {
    println!(
        concat!(
            "eliot-search {}\n\n",
            "USAGE:\n",
            "    eliot-search [health|status|version|refresh|shutdown] [OPTIONS]\n",
            "    eliot-search search \"QUERY\" [OPTIONS]\n\n",
            "OPTIONS:\n",
            "    --address <IP:PORT>   Override the loopback endpoint\n",
            "    --data-root <PATH>    Read endpoint and token from this local state root\n",
            "    --token-file <PATH>   Override the local authentication token file\n",
            "    -V, --version         Print client version\n",
            "    -h, --help            Print help\n\n",
            "refresh captures and publishes a new immutable retained-revision snapshot."
        ),
        env!("CARGO_PKG_VERSION")
    );
}
