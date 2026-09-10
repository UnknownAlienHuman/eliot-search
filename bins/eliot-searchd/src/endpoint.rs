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
// Silent-client read bound and slow-reader write bound. They share a value
// but never a meaning: the read timeout is not the socket configuration, and
// neither is the proxy child request (120s) / startup (30s) / cleanup (5s)
// deadline owned by `proxy_child::ChildLimits`.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointAction {
    Continue,
    Shutdown,
    /// Unusable handler/output channel; close listener without any further reply.
    Abort,
}

pub fn serve_loopback<F>(
    port: u16,
    token_file: &Path,
    handler: F,
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
        "{{\"event\":\"loopback_ready\",\"address\":\"{local}\",\"protocol_version\":1,\"authentication\":\"sha256_challenge_v1\"}}",
    );

    serve_listener(&listener, token_verifier, handler)
}

fn serve_listener<F>(listener: &TcpListener, token_verifier: Sha256Digest, mut handler: F)
    -> Result<(), String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
{
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
            .set_read_timeout(Some(READ_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(WRITE_TIMEOUT)))
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
            // Not a per-client validation failure: the sole child is no longer
            // reusable. Dropping the listener also refuses queued/new clients.
            Ok(EndpointAction::Abort) => return Err("ENDPOINT_HANDLER_ABORTED".to_owned()),
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
        match complete_request(&mut stream, outcome, request_sequence) {
            EndpointAction::Continue => {
                request_sequence = request_sequence.checked_add(1)
                    .ok_or_else(|| "ENDPOINT_REQUEST_SEQUENCE_EXHAUSTED".to_owned())?;
            }
            action => return Ok(action),
        }
    }
}

/// No suffix may follow a failed handler exchange: a partial JSON frame may
/// already be on the socket. Failure of the outer acknowledgement is also a
/// post-dispatch failure, even if the child's own terminal frame was complete.
fn complete_request(
    writer: &mut impl Write,
    outcome: Result<EndpointAction, String>,
    sequence: u64,
) -> EndpointAction {
    let (action, status) = match outcome {
        Ok(EndpointAction::Abort) => return EndpointAction::Abort,
        Ok(action) => (action, "\"ok\":true".to_owned()),
        Err(error) => (EndpointAction::Continue,
            format!("\"ok\":false,\"error\":\"{}\"", sanitize_code(&error))),
    };
    let frame = format!("{{\"event\":\"request_complete\",\"sequence\":{sequence},{status}}}");
    if write_line(writer, &frame).is_err() { EndpointAction::Abort } else { action }
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
        .map_err(|error| match error.kind() {
            // A silent client holds the connection open without a frame. This
            // read timeout is distinct from a socket configuration failure and
            // from the proxy child request/cleanup deadlines.
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
                "ENDPOINT_READ_TIMEOUT".to_owned()
            }
            _ => format!("ENDPOINT_READ_ERROR:{error}"),
        })?;
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

fn write_line(stream: &mut impl Write, value: &str) -> io::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abort_does_not_append_a_terminal_to_partial_output() {
        let mut output = b"{\"partial\":".to_vec();
        let prior = output.clone();
        assert_eq!(complete_request(&mut output, Ok(EndpointAction::Abort), 3), EndpointAction::Abort);
        assert_eq!(output, prior);
    }

    #[test]
    fn complete_rejection_and_shutdown_keep_the_existing_wire_shape() {
        for (outcome, action, fields) in [
            (Ok(EndpointAction::Continue), EndpointAction::Continue, r#""ok":true"#),
            (Ok(EndpointAction::Shutdown), EndpointAction::Shutdown, r#""ok":true"#),
            (Err("SERVICE_HEX_INVALID:private detail".to_owned()), EndpointAction::Continue,
                r#""ok":false,"error":"SERVICE_HEX_INVALID""#),
        ] {
            let mut output = Vec::new();
            assert_eq!(complete_request(&mut output, outcome, 7), action);
            assert_eq!(output, format!("{{\"event\":\"request_complete\",\"sequence\":7,{fields}}}\n").as_bytes());
        }
    }

    struct FailingOutput { bytes: Vec<u8>, calls: usize, fail_flush: bool }
    impl Write for FailingOutput {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            if !self.fail_flush && self.calls == 2 { return Err(io::ErrorKind::BrokenPipe.into()); }
            let count = if self.fail_flush { bytes.len() } else { bytes.len().min(5) };
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }

    #[test]
    fn outer_ack_write_or_flush_failure_aborts_even_after_a_complete_child_reply() {
        for fail_flush in [false, true] {
            for outcome in [Ok(EndpointAction::Continue), Ok(EndpointAction::Shutdown), Err("REJECTED".to_owned())] {
                let mut writer = FailingOutput { bytes: Vec::new(), calls: 0, fail_flush };
                assert_eq!(complete_request(&mut writer, outcome, 0), EndpointAction::Abort);
                assert_eq!(writer.calls, 2); // One prefix + failure, or a frame + LF then failed flush.
                if !fail_flush { assert_eq!(writer.bytes, b"{\"eve"); }
            }
        }
    }

    #[test]
    fn fatal_handler_drops_listener_and_never_dispatches_the_next_queued_command() {
        use std::net::Shutdown;
        use std::sync::mpsc;
        use std::thread;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let verifier = digest_bytes(b"disposable non-secret endpoint fixture token");
        let (done, result) = mpsc::channel();
        let server = thread::spawn(move || {
            let mut calls = 0;
            let status = serve_listener(&listener, verifier, |_, stream| {
                calls += 1;
                stream.write_all(b"{\"partial\":").map_err(|_| "FIXTURE_WRITE_FAILED".to_owned())?;
                Ok(EndpointAction::Abort)
            });
            let _ = done.send((status, calls));
        });
        let timeout = Duration::from_secs(5);
        let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        client.set_write_timeout(Some(timeout)).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        let challenge = read_bounded_line(&mut reader, MAX_AUTH_LINE_BYTES).unwrap().unwrap();
        let challenge = Sha256Digest::from_hex(challenge.strip_prefix("CHALLENGE\t").unwrap()).unwrap();
        write_line(&mut client, &format!("AUTH\t{}", derive_response(verifier, challenge).hex())).unwrap();
        let ready = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
        assert!(ready.contains("\"event\":\"authenticated\""));
        client.write_all(b"first\nsecond\nshutdown\n").unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let mut output = String::new();
        let read = Read::take(&mut reader, 4096).read_to_string(&mut output);
        assert!(read.is_ok() || read.is_err_and(|error| error.kind() == io::ErrorKind::ConnectionReset));
        let (status, calls) = result.recv_timeout(timeout).expect("bounded listener exit");
        server.join().unwrap();
        assert_eq!(status, Err("ENDPOINT_HANDLER_ABORTED".to_owned()));
        assert_eq!(calls, 1);
        assert_eq!(output, "{\"event\":\"request_started\",\"sequence\":0}\n{\"partial\":");
        assert!(TcpStream::connect_timeout(&address, timeout).is_err());
    }

    #[test]
    fn read_and_write_timeouts_are_finite_declared_bounds() {
        assert_eq!(READ_TIMEOUT, Duration::from_secs(30));
        assert_eq!(WRITE_TIMEOUT, Duration::from_secs(30));
        assert!(!READ_TIMEOUT.is_zero());
        assert!(!WRITE_TIMEOUT.is_zero());
    }

    #[test]
    fn silent_client_read_timeout_is_typed_and_bounded() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Instant;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let (ready, accepted) = mpsc::channel();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let _ = ready.send(());
            stream
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            let mut reader = BufReader::new(stream);
            let start = Instant::now();
            let result = read_bounded_line(&mut reader, 1024);
            (result, start.elapsed())
        });
        let client = TcpStream::connect_timeout(&address, Duration::from_secs(5)).unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        accepted.recv_timeout(Duration::from_secs(5)).unwrap();
        // Silent client: hold the connection open without any frame.
        thread::sleep(Duration::from_millis(600));
        let (result, elapsed) = server.join().unwrap();
        assert_eq!(result, Err("ENDPOINT_READ_TIMEOUT".to_owned()));
        assert!(elapsed >= Duration::from_millis(150), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
        drop(client);
    }

    #[test]
    fn real_socket_disconnect_mid_large_response_then_clean_health_has_no_contamination() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Instant;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let large = vec![b'm'; 512 * 1024];
        let (done, finished) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let start = Instant::now();
            let mut outcome: io::Result<()> = Ok(());
            // Stream a large response in bounded chunks; the client drops
            // mid-response, so a later write must fail instead of hanging.
            for chunk in large.chunks(16 * 1024) {
                if let Err(error) = stream.write_all(chunk) {
                    outcome = Err(error);
                    break;
                }
            }
            if outcome.is_ok() {
                outcome = stream.write_all(b"\n").and_then(|()| stream.flush());
            }
            let elapsed = start.elapsed();
            let _ = done.send(());
            (outcome, elapsed)
        });
        let timeout = Duration::from_secs(5);
        let client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        let mut prefix = vec![0_u8; 4096];
        Read::take(&mut reader, 4096)
            .read_exact(&mut prefix)
            .unwrap();
        assert!(prefix.iter().all(|byte| *byte == b'm'));
        // Disconnect mid-large-response: no further read, drop the socket.
        drop(reader);
        drop(client);
        finished.recv_timeout(timeout).expect("bounded server exit");
        let (outcome, elapsed) = server.join().unwrap();
        // The server must observe the disconnect instead of hanging; the exact
        // kind (reset/broken-pipe/timeout) is platform-specific and not part
        // of the contract.
        assert!(
            outcome.is_err(),
            "large response to a dropped client must fail"
        );
        assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
        // A fresh connection serves clean health with none of the old bytes.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let (ready, health_ready) = mpsc::channel();
        let health = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_write_timeout(Some(timeout)).unwrap();
            let _ = ready.send(());
            write_line(&mut stream, "{\"event\":\"health\",\"ok\":true}")
        });
        let client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        health_ready.recv_timeout(timeout).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        let line = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
        assert_eq!(line, "{\"event\":\"health\",\"ok\":true}");
        assert!(!line.contains('m'.to_string().repeat(16).as_str()));
        health.join().unwrap().unwrap();
    }

    #[test]
    fn slow_reader_write_is_bounded_by_write_timeout() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Instant;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let (done, finished) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_write_timeout(Some(Duration::from_millis(300)))
                .unwrap();
            let payload = vec![b's'; 2 * 1024 * 1024];
            let start = Instant::now();
            let mut outcome: io::Result<()> = Ok(());
            for chunk in payload.chunks(64 * 1024) {
                if let Err(error) = stream.write_all(chunk) {
                    outcome = Err(error);
                    break;
                }
            }
            if outcome.is_ok() {
                outcome = stream.flush();
            }
            let elapsed = start.elapsed();
            let _ = done.send(());
            (outcome.is_err(), elapsed)
        });
        let timeout = Duration::from_secs(5);
        // Slow reader: connect but never read, so the sender must time out
        // instead of blocking indefinitely on a full socket buffer.
        let client = TcpStream::connect_timeout(&address, timeout).unwrap();
        finished.recv_timeout(timeout).expect("bounded server exit");
        let (failed, elapsed) = server.join().unwrap();
        assert!(failed, "a never-reading client must bound the write");
        assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
        drop(client);
    }
}
