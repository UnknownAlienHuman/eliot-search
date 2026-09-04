//! Authenticated bounded loopback transport for the development daemon.
//!
//! The endpoint binds only an explicit loopback socket. A token is read from one
//! non-symlink regular file, reduced to SHA-256, and zeroed from the temporary
//! byte buffer. Each connection receives a unique challenge and proves knowledge
//! of the token-derived verifier; plaintext token bytes never cross the socket.

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::sha256::{Sha256Digest, digest_bytes};

const MAX_TOKEN_FILE_BYTES: usize = 4096;
const MIN_TOKEN_BYTES: usize = 32;
const MAX_AUTH_LINE_BYTES: usize = 256;
const MAX_COMMAND_LINE_BYTES: usize = 128 * 1024;
const MAX_COMMANDS_PER_CONNECTION: usize = 4096;
const IO_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EndpointAction {
    Continue,
    Shutdown,
}

pub(crate) fn serve_loopback<F>(
    port: u16,
    token_file: &Path,
    mut handler: F,
) -> Result<(), String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
{
    let token_verifier = read_token_verifier(token_file)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .map_err(|error| format!("ENDPOINT_BIND_ERROR:{error}"))?;
    let local = listener
        .local_addr()
        .map_err(|error| format!("ENDPOINT_LOCAL_ADDRESS_ERROR:{error}"))?;
    if !local.ip().is_loopback() {
        return Err("ENDPOINT_NON_LOOPBACK_BIND_DENIED".to_owned());
    }
    println!(
        "{{\"event\":\"loopback_ready\",\"address\":\"{}\",\"protocol_version\":1,\"authentication\":\"sha256_challenge_v1\"}}",
        local,
    );

    let mut connection_sequence = 0_u64;
    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("{{\"error\":\"ENDPOINT_ACCEPT_ERROR\",\"detail_class\":\"{}\"}}", error.kind());
                continue;
            }
        };
        connection_sequence = connection_sequence
            .checked_add(1)
            .ok_or_else(|| "ENDPOINT_CONNECTION_SEQUENCE_EXHAUSTED".to_owned())?;
        let peer = stream
            .peer_addr()
            .map_err(|error| format!("ENDPOINT_PEER_ADDRESS_ERROR:{error}"))?;
        if !peer.ip().is_loopback() {
            let _ = write_line(&mut stream, "{\"error\":\"ENDPOINT_LOOPBACK_REQUIRED\"}");
            continue;
        }
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
            .map_err(|error| format!("ENDPOINT_TIMEOUT_CONFIGURATION_ERROR:{error}"))?;

        match serve_connection(
            stream,
            peer,
            connection_sequence,
            token_verifier,
            &mut handler,
        ) {
            Ok(EndpointAction::Continue) => {}
            Ok(EndpointAction::Shutdown) => return Ok(()),
            Err(error) => {
                eprintln!(
                    "{{\"error\":\"{}\",\"connection_sequence\":{}}}",
                    sanitize_code(&error),
                    connection_sequence,
                );
            }
        }
    }
    Ok(())
}

fn serve_connection<F>(
    mut stream: TcpStream,
    peer: SocketAddr,
    connection_sequence: u64,
    token_verifier: Sha256Digest,
    handler: &mut F,
) -> Result<EndpointAction, String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
{
    let challenge = derive_challenge(token_verifier, peer, connection_sequence)?;
    write_line(&mut stream, &format!("CHALLENGE\t{}", challenge.hex()))
        .map_err(|error| format!("ENDPOINT_CHALLENGE_WRITE_ERROR:{error}"))?;
    let read_stream = stream
        .try_clone()
        .map_err(|error| format!("ENDPOINT_STREAM_CLONE_ERROR:{error}"))?;
    let mut reader = BufReader::new(read_stream);
    let authentication = read_bounded_line(&mut reader, MAX_AUTH_LINE_BYTES)?
        .ok_or_else(|| "ENDPOINT_AUTHENTICATION_MISSING".to_owned())?;
    let presented = authentication
        .strip_prefix("AUTH\t")
        .ok_or_else(|| "ENDPOINT_AUTHENTICATION_INVALID".to_owned())?;
    let presented = Sha256Digest::from_hex(presented)
        .map_err(|_| "ENDPOINT_AUTHENTICATION_INVALID".to_owned())?;
    let expected = derive_response(token_verifier, challenge);
    if !constant_time_equal(expected, presented) {
        let _ = write_line(&mut stream, "{\"error\":\"AUTHENTICATION_FAILED\"}");
        return Err("ENDPOINT_AUTHENTICATION_FAILED".to_owned());
    }
    write_line(
        &mut stream,
        concat!(
            "{\"event\":\"authenticated\",\"protocol_version\":1,",
            "\"transport\":\"loopback_tcp\",",
            "\"authentication\":\"sha256_challenge_v1\"}"
        ),
    )
    .map_err(|error| format!("ENDPOINT_READY_WRITE_ERROR:{error}"))?;

    let mut request_sequence = 0_u64;
    loop {
        let Some(command) = read_bounded_line(&mut reader, MAX_COMMAND_LINE_BYTES)? else {
            return Ok(EndpointAction::Continue);
        };
        if command.is_empty() {
            return Err("ENDPOINT_EMPTY_COMMAND".to_owned());
        }
        if request_sequence
            >= u64::try_from(MAX_COMMANDS_PER_CONNECTION).unwrap_or(u64::MAX)
        {
            write_line(
                &mut stream,
                "{\"event\":\"request_complete\",\"ok\":false,\"error\":\"ENDPOINT_REQUEST_LIMIT_EXCEEDED\"}",
            )
            .map_err(|error| format!("ENDPOINT_WRITE_ERROR:{error}"))?;
            return Ok(EndpointAction::Continue);
        }
        write_line(
            &mut stream,
            &format!(
                "{{\"event\":\"request_started\",\"sequence\":{request_sequence}}}"
            ),
        )
        .map_err(|error| format!("ENDPOINT_WRITE_ERROR:{error}"))?;
        let outcome = handler(&command, &mut stream);
        match outcome {
            Ok(action) => {
                write_line(
                    &mut stream,
                    &format!(
                        "{{\"event\":\"request_complete\",\"sequence\":{request_sequence},\"ok\":true}}"
                    ),
                )
                .map_err(|error| format!("ENDPOINT_WRITE_ERROR:{error}"))?;
                request_sequence = request_sequence
                    .checked_add(1)
                    .ok_or_else(|| "ENDPOINT_REQUEST_SEQUENCE_EXHAUSTED".to_owned())?;
                if action == EndpointAction::Shutdown {
                    return Ok(EndpointAction::Shutdown);
                }
            }
            Err(error) => {
                write_line(
                    &mut stream,
                    &format!(
                        "{{\"event\":\"request_complete\",\"sequence\":{request_sequence},\"ok\":false,\"error\":\"{}\"}}",
                        sanitize_code(&error),
                    ),
                )
                .map_err(|write_error| format!("ENDPOINT_WRITE_ERROR:{write_error}"))?;
                request_sequence = request_sequence
                    .checked_add(1)
                    .ok_or_else(|| "ENDPOINT_REQUEST_SEQUENCE_EXHAUSTED".to_owned())?;
            }
        }
    }
}

fn read_token_verifier(path: &Path) -> Result<Sha256Digest, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("ENDPOINT_TOKEN_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("ENDPOINT_TOKEN_FILE_INVALID".to_owned());
    }
    if metadata.len() > u64::try_from(MAX_TOKEN_FILE_BYTES).unwrap_or(u64::MAX) {
        return Err("ENDPOINT_TOKEN_FILE_TOO_LARGE".to_owned());
    }
    let mut file = File::open(path)
        .map_err(|error| format!("ENDPOINT_TOKEN_OPEN_ERROR:{error}"))?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .map_err(|_| "ENDPOINT_TOKEN_FILE_TOO_LARGE".to_owned())?,
    );
    (&mut file)
        .take(u64::try_from(MAX_TOKEN_FILE_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("ENDPOINT_TOKEN_READ_ERROR:{error}"))?;
    if bytes.len() > MAX_TOKEN_FILE_BYTES {
        bytes.fill(0);
        return Err("ENDPOINT_TOKEN_FILE_TOO_LARGE".to_owned());
    }
    let (start, end) = trim_ascii_bounds(&bytes);
    if end.saturating_sub(start) < MIN_TOKEN_BYTES {
        bytes.fill(0);
        return Err("ENDPOINT_TOKEN_TOO_SHORT".to_owned());
    }
    let verifier = digest_bytes(&bytes[start..end]);
    bytes.fill(0);
    Ok(verifier)
}

fn trim_ascii_bounds(bytes: &[u8]) -> (usize, usize) {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |index| index + 1);
    (start, end)
}

fn derive_challenge(
    token_verifier: Sha256Digest,
    peer: SocketAddr,
    connection_sequence: u64,
) -> Result<Sha256Digest, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "ENDPOINT_CLOCK_INVALID".to_owned())?
        .as_nanos();
    let mut input = Vec::new();
    input.extend_from_slice(b"eliot-search/loopback-challenge/v1\0");
    input.extend_from_slice(&token_verifier.as_bytes());
    input.extend_from_slice(&std::process::id().to_be_bytes());
    input.extend_from_slice(&connection_sequence.to_be_bytes());
    input.extend_from_slice(&now.to_be_bytes());
    match peer {
        SocketAddr::V4(address) => {
            input.extend_from_slice(&address.ip().octets());
            input.extend_from_slice(&address.port().to_be_bytes());
        }
        SocketAddr::V6(address) => {
            input.extend_from_slice(&address.ip().octets());
            input.extend_from_slice(&address.port().to_be_bytes());
        }
    }
    Ok(digest_bytes(&input))
}

fn derive_response(
    token_verifier: Sha256Digest,
    challenge: Sha256Digest,
) -> Sha256Digest {
    let mut input = Vec::with_capacity(96);
    input.extend_from_slice(b"eliot-search/loopback-response/v1\0");
    input.extend_from_slice(&token_verifier.as_bytes());
    input.extend_from_slice(&challenge.as_bytes());
    digest_bytes(&input)
}

fn constant_time_equal(left: Sha256Digest, right: Sha256Digest) -> bool {
    let mut difference = 0_u8;
    for (left, right) in left.as_bytes().iter().zip(right.as_bytes().iter()) {
        difference |= left ^ right;
    }
    difference == 0
}

fn read_bounded_line(
    reader: &mut BufReader<TcpStream>,
    maximum_bytes: usize,
) -> Result<Option<String>, String> {
    let mut bytes = Vec::new();
    let mut limited = reader.take(
        u64::try_from(maximum_bytes.saturating_add(1)).unwrap_or(u64::MAX),
    );
    let read = limited
        .read_until(b'\n', &mut bytes)
        .map_err(|error| format!("ENDPOINT_READ_ERROR:{error}"))?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > maximum_bytes || !bytes.ends_with(b"\n") {
        return Err("ENDPOINT_FRAME_TOO_LARGE".to_owned());
    }
    bytes.pop();
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "ENDPOINT_FRAME_INVALID_UTF8".to_owned())
}

fn write_line(stream: &mut TcpStream, value: &str) -> io::Result<()> {
    stream.write_all(value.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn sanitize_code(error: &str) -> String {
    let code = error.split(':').next().unwrap_or("ENDPOINT_ERROR");
    let mut output = String::with_capacity(code.len().min(128));
    for character in code.chars().take(128) {
        if character.is_ascii_uppercase()
            || character.is_ascii_digit()
            || matches!(character, '_' | '-' | '.')
        {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "ENDPOINT_ERROR".to_owned()
    } else {
        output
    }
}
