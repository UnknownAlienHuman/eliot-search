//! Protocol-only command-line client for the local ELIOT Search daemon.

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

const DEFAULT_ADDRESS: &str = "127.0.0.1:39171";
const MAX_RESPONSE_BYTES: usize = 16 * 1_024;
const IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Health,
    Status,
    Version,
    Shutdown,
}

impl Command {
    const fn wire(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Status => "status",
            Self::Version => "version",
            Self::Shutdown => "shutdown",
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

    let mut stream = TcpStream::connect_timeout(&address, IO_TIMEOUT)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let request = format!("ELIOT_SEARCH/1 {token} {}\n", options.command.wire());
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
            "shutdown" => set_command(&mut command, Command::Shutdown)?,
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
    let body = fs::read_to_string(path)?;
    let mut lines = body.lines();
    if lines.next() != Some("ELIOT_SEARCH_ENDPOINT_V1") {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "unsupported endpoint descriptor",
        ));
    }
    for line in lines {
        if let Some(value) = line.strip_prefix("address=") {
            let address = value.parse::<SocketAddr>().map_err(|_| {
                io::Error::new(ErrorKind::InvalidData, "invalid endpoint descriptor address")
            })?;
            ensure_loopback(address.ip())?;
            return Ok(Some(address));
        }
    }
    Err(io::Error::new(
        ErrorKind::InvalidData,
        "endpoint descriptor has no address",
    ))
}

fn read_token(path: &Path) -> io::Result<String> {
    let token = fs::read_to_string(path)?;
    let token = token.trim().to_owned();
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
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.contains(&b'\n') {
            break;
        }
    }
    let newline = bytes.iter().position(|byte| *byte == b'\n');
    let end = newline.unwrap_or(bytes.len());
    if bytes[end.saturating_add(1)..]
        .iter()
        .any(|byte| !byte.is_ascii_whitespace())
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "daemon returned multiple protocol frames",
        ));
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

fn print_help() {
    println!(
        "eliot-search {}\n\nUSAGE:\n    eliot-search [health|status|version|shutdown] [OPTIONS]\n\nOPTIONS:\n    --address <IP:PORT>   Override the loopback endpoint\n    --data-root <PATH>    Read endpoint and token from this local state root\n    --token-file <PATH>   Override the local authentication token file\n    -V, --version         Print client version\n    -h, --help            Print help",
        env!("CARGO_PKG_VERSION")
    );
}
