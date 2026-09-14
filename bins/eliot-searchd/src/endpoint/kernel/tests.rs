use super::*;

use std::io::{self, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::Duration;

use search_provider_protocol::pairing::{PairingLedger, ProofDigest};

struct TestKeySource {
    key: [u8; 32],
    failures_remaining: usize,
}

impl TestKeySource {
    fn new(key: [u8; 32]) -> Self {
        Self {
            key,
            failures_remaining: 0,
        }
    }

    fn failing_once() -> Self {
        Self {
            key: [0xA5; 32],
            failures_remaining: 1,
        }
    }
}

impl Drop for TestKeySource {
    fn drop(&mut self) {
        self.key.fill(0);
    }
}

impl EndpointKeySource for TestKeySource {
    fn with_endpoint_key<T>(
        &mut self,
        use_key: impl FnOnce(&[u8; 32]) -> T,
    ) -> Result<T, String> {
        if self.failures_remaining > 0 {
            self.failures_remaining -= 1;
            return Err("TEST_KEY_UNAVAILABLE".to_owned());
        }
        Ok(use_key(&self.key))
    }
}

fn client_handshake(
    reader: &mut BufReader<TcpStream>,
    stream: &mut TcpStream,
    key: &[u8; 32],
) -> Result<ProofDigest, String> {
    let challenge_line = read_bounded_line(reader, MAX_CHALLENGE_LINE_BYTES)
        .map_err(|error| format!("TEST_CHALLENGE_READ:{error}"))?
        .ok_or_else(|| "TEST_CHALLENGE_MISSING".to_owned())?;
    let challenge =
        parse_challenge_line(&challenge_line).map_err(|error| format!("TEST_PARSE:{error}"))?;
    if pairing_binding_digest(key) != challenge.binding {
        return Err("TEST_BINDING_MISMATCH".to_owned());
    }
    let proof = client_proof_for_challenge(key, &challenge);
    write_line(
        stream,
        &format!("PAIRING_AUTH\tproof={}", hex_encode(proof.as_bytes())),
    )
    .map_err(|error| format!("TEST_AUTH_WRITE:{error}"))?;
    let verified_line = read_bounded_line(reader, MAX_VERIFIED_LINE_BYTES)
        .map_err(|error| format!("TEST_VERIFIED_READ:{error}"))?
        .ok_or_else(|| "TEST_VERIFIED_MISSING".to_owned())?;
    let provider_proof =
        parse_verified_line(&verified_line).map_err(|error| format!("TEST_PARSE:{error}"))?;
    if !verify_provider_proof(key, &challenge, &provider_proof) {
        return Err("TEST_PROVIDER_PROOF_INVALID".to_owned());
    }
    let ready = read_bounded_line(reader, 1024)
        .map_err(|error| format!("TEST_READY_READ:{error}"))?
        .ok_or_else(|| "TEST_READY_MISSING".to_owned())?;
    if !ready.contains("\"event\":\"authenticated\"")
        || !ready.contains("\"authentication\":\"pairing_blake3_v1\"")
    {
        return Err("TEST_READY_INVALID".to_owned());
    }
    Ok(provider_proof)
}

type SpawnedListener = (
    std::net::SocketAddr,
    std::sync::mpsc::Receiver<(Result<(), String>, usize)>,
    std::thread::JoinHandle<()>,
);

fn spawn_listener(
    source: TestKeySource,
    handler: impl FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String> + Send + 'static,
) -> SpawnedListener {
    use std::sync::mpsc;
    let (done, result) = mpsc::channel();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut source = source;
        let mut calls = 0_usize;
        let mut handler = handler;
        let ledger = PairingLedger::new(MAX_PAIRING_CHALLENGES).unwrap();
        let status = serve_listener(&listener, &mut source, ledger, |command, stream| {
            calls += 1;
            handler(command, stream)
        });
        let _ = done.send((status, calls));
    });
    (address, result, server)
}

#[test]
fn binding_digest_is_deterministic_and_role_bound() {
    let first = pairing_binding_digest(&[0x42; 32]);
    let second = pairing_binding_digest(&[0x42; 32]);
    assert_eq!(first, second);
    assert_ne!(first, pairing_binding_digest(&[0x43; 32]));
    assert_eq!(format!("{first:?}"), "ProofDigest(<redacted>)");
}

#[test]
fn challenge_line_round_trips_with_strict_parse() {
    let key = [0x11; 32];
    let binding = pairing_binding_digest(&key);
    let (session, nonce, challenge) = derive_ceremony_material(&key, 7).unwrap();
    let line = encode_challenge(binding, session, &nonce, &challenge);
    assert!(line.len() < MAX_CHALLENGE_LINE_BYTES);
    let parsed = parse_challenge_line(&line).unwrap();
    assert_eq!(parsed.version, PAIRING_PROTOCOL_VERSION);
    assert_eq!(parsed.session, session);
    assert_eq!(parsed.nonce, nonce);
    assert_eq!(parsed.challenge, challenge);
    assert_eq!(parsed.binding, binding);
    let mut reordered = line.clone();
    reordered = reordered.replacen("session=", "nonce=", 1);
    assert!(parse_challenge_line(&reordered).is_err());
    assert!(parse_challenge_line(&line[..line.len() - 1]).is_err());
    assert!(parse_challenge_line(&line.to_uppercase()).is_err());
    let tampered = line.replacen("v=1.0", "v=1.1", 1);
    assert!(matches!(
        parse_challenge_line(&tampered),
        Err(error) if error == "ENDPOINT_PAIRING_VERSION_MISMATCH"
    ));
}

#[test]
fn full_handshake_proves_mutually_then_dispatches() {
    let key = [0x2A; 32];
    let (address, result, server) =
        spawn_listener(TestKeySource::new(key), |command, stream| {
            write_line(stream, &format!("{{\"echo\":\"{command}\"}}"))
                .map_err(|_| "FIXTURE_WRITE_FAILED".to_owned())?;
            if command == "shutdown" {
                Ok(EndpointAction::Shutdown)
            } else {
                Ok(EndpointAction::Continue)
            }
        });
    let timeout = Duration::from_secs(5);
    let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
    client.set_read_timeout(Some(timeout)).unwrap();
    client.set_write_timeout(Some(timeout)).unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    client_handshake(&mut reader, &mut client, &key).unwrap();
    write_line(&mut client, "hello").unwrap();
    let started = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
    assert!(started.contains("\"event\":\"request_started\""));
    let echo = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
    assert_eq!(echo, "{\"echo\":\"hello\"}");
    let complete = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
    assert!(complete.contains("\"ok\":true"));
    write_line(&mut client, "shutdown").unwrap();
    let _ = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
    let _ = read_bounded_line(&mut reader, 4096).unwrap();
    let _ = read_bounded_line(&mut reader, 4096).unwrap();
    let (status, calls) = result.recv_timeout(timeout).expect("bounded listener exit");
    server.join().unwrap();
    assert_eq!(status, Ok(()));
    assert_eq!(calls, 2);
}

#[test]
fn wrong_key_tampered_proof_and_reused_ledger_entry_fail() {
    let key = [0x3B; 32];
    let (address, result, server) =
        spawn_listener(TestKeySource::new(key), |_, _| Ok(EndpointAction::Continue));
    let timeout = Duration::from_secs(5);
    let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
    client.set_read_timeout(Some(timeout)).unwrap();
    client.set_write_timeout(Some(timeout)).unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    assert_eq!(
        client_handshake(&mut reader, &mut client, &[0x3C; 32]),
        Err("TEST_BINDING_MISMATCH".to_owned())
    );
    drop(client);
    drop(reader);

    let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
    client.set_read_timeout(Some(timeout)).unwrap();
    client.set_write_timeout(Some(timeout)).unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    let challenge_line = read_bounded_line(&mut reader, MAX_CHALLENGE_LINE_BYTES)
        .unwrap()
        .unwrap();
    let challenge = parse_challenge_line(&challenge_line).unwrap();
    let mut proof = *client_proof_for_challenge(&key, &challenge).as_bytes();
    proof[0] ^= 1;
    write_line(
        &mut client,
        &format!("PAIRING_AUTH\tproof={}", hex_encode(&proof)),
    )
    .unwrap();
    let rejection = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
    assert!(rejection.contains("AUTHENTICATION_FAILED"));
    drop(client);
    drop(reader);

    let mut ledger = PairingLedger::new(8).unwrap();
    let (session, _, challenge) = derive_ceremony_material(&key, 99).unwrap();
    ledger.consume(session, &challenge).unwrap();
    assert!(ledger.consume(session, &challenge).is_err());
    assert!(ledger.contains(session, &challenge));
    let _ = (result, server);
}

#[test]
fn ledger_capacity_fails_closed_without_eviction() {
    let mut ledger = PairingLedger::new(2).unwrap();
    let key = [0x55; 32];
    for sequence in [1_u64, 2] {
        let (session, _, challenge) = derive_ceremony_material(&key, sequence).unwrap();
        ledger.consume(session, &challenge).unwrap();
    }
    let (session, _, challenge) = derive_ceremony_material(&key, 3).unwrap();
    assert!(ledger.consume(session, &challenge).is_err());
    assert_eq!(ledger.len(), 2);
}

#[test]
fn key_source_failure_fails_the_connection_without_a_challenge() {
    let source_key = [0xA5; 32];
    let (address, result, server) =
        spawn_listener(TestKeySource::failing_once(), |command, _| {
            if command == "shutdown" {
                Ok(EndpointAction::Shutdown)
            } else {
                Ok(EndpointAction::Continue)
            }
        });
    let timeout = Duration::from_secs(5);
    let client = TcpStream::connect_timeout(&address, timeout).unwrap();
    client.set_read_timeout(Some(timeout)).unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    assert_eq!(
        read_bounded_line(&mut reader, MAX_CHALLENGE_LINE_BYTES).unwrap(),
        None
    );
    drop(client);
    drop(reader);

    let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
    client.set_read_timeout(Some(timeout)).unwrap();
    client.set_write_timeout(Some(timeout)).unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    client_handshake(&mut reader, &mut client, &source_key).unwrap();
    write_line(&mut client, "shutdown").unwrap();
    let (status, _) = result.recv_timeout(timeout).expect("bounded listener exit");
    server.join().unwrap();
    assert_eq!(status, Ok(()));
}

#[test]
fn abort_does_not_append_a_terminal_to_partial_output() {
    let mut output = b"{\"partial\":".to_vec();
    let prior = output.clone();
    assert_eq!(
        complete_request(&mut output, Ok(EndpointAction::Abort), 3),
        EndpointAction::Abort
    );
    assert_eq!(output, prior);
}

#[test]
fn complete_rejection_and_shutdown_keep_the_existing_wire_shape() {
    for (outcome, action, fields) in [
        (
            Ok(EndpointAction::Continue),
            EndpointAction::Continue,
            r#""ok":true"#,
        ),
        (
            Ok(EndpointAction::Shutdown),
            EndpointAction::Shutdown,
            r#""ok":true"#,
        ),
        (
            Err("SERVICE_HEX_INVALID:private detail".to_owned()),
            EndpointAction::Continue,
            r#""ok":false,"error":"SERVICE_HEX_INVALID""#,
        ),
    ] {
        let mut output = Vec::new();
        assert_eq!(complete_request(&mut output, outcome, 7), action);
        assert_eq!(
            output,
            format!("{{\"event\":\"request_complete\",\"sequence\":7,{fields}}}\n").as_bytes()
        );
    }
}

struct FailingOutput {
    bytes: Vec<u8>,
    calls: usize,
    fail_flush: bool,
}

impl Write for FailingOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        if !self.fail_flush && self.calls == 2 {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let count = if self.fail_flush {
            bytes.len()
        } else {
            bytes.len().min(5)
        };
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn outer_ack_write_or_flush_failure_aborts_even_after_a_complete_child_reply() {
    for fail_flush in [false, true] {
        for outcome in [
            Ok(EndpointAction::Continue),
            Ok(EndpointAction::Shutdown),
            Err("REJECTED".to_owned()),
        ] {
            let mut writer = FailingOutput {
                bytes: Vec::new(),
                calls: 0,
                fail_flush,
            };
            assert_eq!(
                complete_request(&mut writer, outcome, 0),
                EndpointAction::Abort
            );
            assert_eq!(writer.calls, 2);
            if !fail_flush {
                assert_eq!(writer.bytes, b"{\"eve");
            }
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
    let key = [0x6D; 32];
    let (done, result) = mpsc::channel();
    let server = thread::spawn(move || {
        let mut calls = 0;
        let mut source = TestKeySource::new(key);
        let ledger = PairingLedger::new(MAX_PAIRING_CHALLENGES).unwrap();
        let status = serve_listener(&listener, &mut source, ledger, |_, stream| {
            calls += 1;
            stream
                .write_all(b"{\"partial\":")
                .map_err(|_| "FIXTURE_WRITE_FAILED".to_owned())?;
            Ok(EndpointAction::Abort)
        });
        let _ = done.send((status, calls));
    });
    let timeout = Duration::from_secs(5);
    let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
    client.set_read_timeout(Some(timeout)).unwrap();
    client.set_write_timeout(Some(timeout)).unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    client_handshake(&mut reader, &mut client, &key).unwrap();
    client.write_all(b"first\nsecond\nshutdown\n").unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let mut output = String::new();
    let _ = Read::take(&mut reader, 4096).read_to_string(&mut output);
    assert_eq!(
        output,
        "{\"event\":\"request_started\",\"sequence\":0}\n{\"partial\":"
    );
    let (status, calls) = result.recv_timeout(timeout).expect("bounded listener exit");
    server.join().unwrap();
    assert_eq!(status, Err("ENDPOINT_HANDLER_ABORTED".to_owned()));
    assert_eq!(calls, 1);
    assert!(TcpStream::connect_timeout(&address, timeout).is_err());
}

#[test]
fn io_errors_carry_closed_kind_tokens_without_os_prose() {
    let refused = io::Error::new(io::ErrorKind::ConnectionRefused, "some 127.0.0.1 prose");
    let redacted = redacted_io_error("ENDPOINT_BIND_ERROR", &refused);
    assert_eq!(redacted, "ENDPOINT_BIND_ERROR:ConnectionRefused");
    assert!(!redacted.contains("127.0.0.1"));
    assert!(!redacted.contains(' '));
    let timeout = io::Error::from(io::ErrorKind::TimedOut);
    assert_eq!(
        redacted_io_error("ENDPOINT_READ_ERROR", &timeout),
        "ENDPOINT_READ_ERROR:TimedOut"
    );
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
    drop(reader);
    drop(client);
    finished.recv_timeout(timeout).expect("bounded server exit");
    let (outcome, elapsed) = server.join().unwrap();
    assert!(outcome.is_err(), "large response to a dropped client must fail");
    assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");

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
    let client = TcpStream::connect_timeout(&address, timeout).unwrap();
    finished.recv_timeout(timeout).expect("bounded server exit");
    let (failed, elapsed) = server.join().unwrap();
    assert!(failed, "a never-reading client must bound the write");
    assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
    drop(client);
}
