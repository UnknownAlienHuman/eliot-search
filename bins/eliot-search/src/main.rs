//! Bounded protocol-only CLI for the local ELIOT Search daemon.

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
enum ClientCommand {
    Plain(&'static str),
    Query { wire_name: &'static str, value: String },
}

impl ClientCommand {
    fn wire(&self) -> io::Result<String> {
        match self {
            Self::Plain(value) => Ok((*value).to_owned()),
            Self::Query { wire_name, value } => {
                if value.is_empty() || value.len() > MAX_QUERY_BYTES {
                    return Err(io::Error::new(
                        ErrorKind::InvalidInput,
                        "query must be non-empty and at most 1024 UTF-8 bytes",
                    ));
                }
                Ok(format!("{wire_name}:{}", hex_encode(value.as_bytes())))
            }
        }
    }
}

#[derive(Debug)]
struct Options {
    command: ClientCommand,
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
    let address = options
        .address
        .or(read_endpoint(&options.data_root)?)
        .unwrap_or(DEFAULT_ADDRESS.parse().map_err(|_| {
            io::Error::new(ErrorKind::InvalidData, "invalid default endpoint")
        })?);
    ensure_loopback(address.ip())?;
    let token_path = options
        .token_file
        .unwrap_or_else(|| options.data_root.join("runtime").join("auth.token"));
    let token = read_small_regular_utf8(&token_path, 128)?.trim().to_owned();
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "authentication token must be exactly 64 hexadecimal characters",
        ));
    }
    let command = options.command.wire()?;
    let mut stream = TcpStream::connect_timeout(&address, IO_TIMEOUT)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    write!(stream, "ELIOT_SEARCH/1 {token} {command}\n")?;
    stream.flush()?;
    read_response(&mut stream)
}

fn parse_options() -> io::Result<Options> {
    let mut arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.first().is_some_and(|value| matches!(value.as_str(), "-h" | "--help")) {
        print_help();
        process::exit(0);
    }
    if arguments.first().is_some_and(|value| matches!(value.as_str(), "-V" | "--version")) {
        println!("eliot-search {}", env!("CARGO_PKG_VERSION"));
        process::exit(0);
    }

    let command_name = if arguments.is_empty() {
        "health".to_owned()
    } else {
        arguments.remove(0)
    };
    let command = match command_name.as_str() {
        "health" | "status" | "version" | "refresh" | "shutdown" => {
            ClientCommand::Plain(match command_name.as_str() {
                "health" => "health",
                "status" => "status",
                "version" => "version",
                "refresh" => "refresh",
                "shutdown" => "shutdown",
                _ => unreachable!(),
            })
        }
        "search" | "lexical" => {
            let value = take_value(&mut arguments, &command_name)?;
            ClientCommand::Query {
                wire_name: if command_name == "search" { "search" } else { "lexical" },
                value,
            }
        }
        _ => {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!("unknown command: {command_name}"),
            ));
        }
    };

    let mut address = None;
    let mut data_root = default_data_root()?;
    let mut token_file = None;
    while !arguments.is_empty() {
        let flag = arguments.remove(0);
        match flag.as_str() {
            "--address" => {
                let value = take_value(&mut arguments, "--address")?;
                let parsed = value.parse::<SocketAddr>().map_err(|_| {
                    io::Error::new(ErrorKind::InvalidInput, "invalid socket address")
                })?;
                ensure_loopback(parsed.ip())?;
                address = Some(parsed);
            }
            "--data-root" => {
                data_root = PathBuf::from(take_value(&mut arguments, "--data-root")?);
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(take_value(&mut arguments, "--token-file")?));
            }
            _ => {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("unknown option: {flag}"),
                ));
            }
        }
    }
    Ok(Options {
        command,
        address,
        data_root,
        token_file,
    })
}

fn take_value(arguments: &mut Vec<String>, name: &str) -> io::Result<String> {
    if arguments.is_empty() {
        Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("{name} requires a value"),
        ))
    } else {
        Ok(arguments.remove(0))
    }
}

fn read_endpoint(data_root: &Path) -> io::Result<Option<SocketAddr>> {
    let path = data_root.join("runtime").join("endpoint.v1");
    if !path.exists() {
        return Ok(None);
    }
    let body = read_small_regular_utf8(&path, 16 * 1_024)?;
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
                io::Error::new(ErrorKind::InvalidData, "invalid endpoint address")
            })?;
            ensure_loopback(parsed.ip())?;
            address = Some(parsed);
        }
    }
    address.map(Some).ok_or_else(|| {
        io::Error::new(ErrorKind::InvalidData, "endpoint descriptor has no address")
    })
}

fn read_small_regular_utf8(path: &Path, maximum: u64) -> io::Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "expected a bounded regular file",
        ));
    }
    fs::read_to_string(path)
}

fn read_response(stream: &mut TcpStream) -> io::Result<String> {
    let mut bytes = Vec::new();
    (&mut *stream)
        .take(u64::try_from(MAX_RESPONSE_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "response exceeds the bounded protocol frame",
        ));
    }
    while bytes.last().is_some_and(u8::is_ascii_whitespace) {
        bytes.pop();
    }
    if bytes.contains(&b'\n') {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "daemon returned multiple protocol frames",
        ));
    }
    String::from_utf8(bytes)
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
            "  eliot-search [health|status|version|refresh|shutdown] [OPTIONS]\n",
            "  eliot-search search \"LITERAL\" [OPTIONS]\n",
            "  eliot-search lexical \"TERMS\" [OPTIONS]\n\n",
            "OPTIONS:\n",
            "  --address <IP:PORT>   Override the loopback endpoint\n",
            "  --data-root <PATH>    Read endpoint and token from this state root\n",
            "  --token-file <PATH>   Override the local token file\n",
            "  -V, --version         Print client version\n",
            "  -h, --help            Print help\n\n",
            "search: retained-revision literal matching.\n",
            "lexical: deterministic BM25 ranking over the active snapshot.\n",
            "refresh: atomically rebuild snapshot plus lexical index."
        ),
        env!("CARGO_PKG_VERSION")
    );
}
