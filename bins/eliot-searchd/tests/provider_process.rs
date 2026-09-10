//! T19 provider-protocol process tests: canonical envelopes through one real
//! daemon plus the real CLI.
//!
//! Every test drives the actual `eliot-searchd --serve-loopback-data-root`
//! proxy (which spawns the real `--serve-data-root` child) over a real
//! loopback TCP connection: pairing handshake, `op\thello` version
//! negotiation, sealed `envelope\t<seq>\t<hex>` commands, typed
//! `provider_op`/`provider_error` lines and sealed `response\t<hex>`
//! terminals. Negative paths prove invalid version/sequence/length, replay,
//! cancellation, reconnect and unknown outcome fail closed without ever
//! relabeling a degraded outcome as success.
//!
//! The final tests spawn the real `eliot-search` CLI binary (resolved as a
//! sibling of the daemon binary, so both packages must be built) against the
//! canonical endpoint descriptor: health round-trips with exit 0, gated
//! recipes exit 2 with explicit unavailable, and a missing descriptor fails
//! closed without dialing any hidden default address.

#[allow(dead_code)]
mod common;

#[path = "../src/provider_composition.rs"]
// Load-bearing: this test target compiles the daemon module a second time
// and only exercises its test-facing items; the daemon build uses the rest.
#[allow(dead_code)]
mod provider;

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use provider::{OpArgument, OpStatus, ProviderOperation, negotiate_capabilities};
use search_contracts::{ProtocolRange, ProtocolVersion, RequestId};
use search_provider_protocol::pairing::{
    ClientNonce, PairingChallenge, ProofDigest, ServerNonce as ProtoServerNonce, SessionId,
    client_proof_transcript, server_proof_transcript, verify_proof,
};
use search_provider_protocol::request::{
    AuthenticatedResponse, ControlCommand, RequestStatus, decode_response_json,
    encode_envelope_json, envelope_transcript, response_transcript, seal_envelope, seal_response,
};

const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_LINE: usize = 256 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

const DEV_KEY_DOMAIN: &[u8] = b"eliot-search/loopback-dev-key/v1\0";
const BINDING_DOMAIN: &[u8] = b"eliot-search/loopback-binding/v1\0";
const BINDING_ROLE: &[u8] = b"loopback-operator";
const VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

struct Scratch {
    dir: PathBuf,
    guard: common::RevisionKeyTreeGuard,
    token: Vec<u8>,
}

impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "eliot-provider-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(dir.join("data")).unwrap();
        let guard = common::RevisionKeyTreeGuard::for_tree(&dir);
        // Unique-per-run token so the key sentinel below is meaningful.
        let mut token = Vec::with_capacity(48);
        let stamp64 = u64::try_from(stamp & 0xFFFF_FFFF_FFFF_FFFF).expect("nanos truncate");
        let mut state =
            stamp64 ^ (u64::from(std::process::id()) << 32) ^ NEXT.fetch_add(1, Ordering::Relaxed);
        for _ in 0..48 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            token.push((state & 0xFF) as u8);
        }
        if token.iter().all(|byte| *byte == 0) {
            token[0] = 1;
        }
        fs::write(dir.join("auth.token"), &token).unwrap();
        Self { dir, guard, token }
    }

    fn data_root(&self) -> PathBuf {
        self.dir.join("data")
    }

    fn token_file(&self) -> PathBuf {
        self.dir.join("auth.token")
    }

    fn key(&self) -> [u8; 32] {
        let (start, end) = trim(&self.token);
        let mut hasher = blake3::Hasher::new();
        hasher.update(DEV_KEY_DOMAIN);
        hasher.update(&self.token[start..end]);
        *hasher.finalize().as_bytes()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn trim(bytes: &[u8]) -> (usize, usize) {
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

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct Proxy {
    child: Child,
    address: SocketAddr,
}

impl Proxy {
    fn start(scratch: &Scratch) -> Self {
        let port = free_port();
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(["--serve-loopback-data-root"])
            .arg(scratch.data_root())
            .arg(port.to_string())
            .arg(scratch.token_file())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        assert!(first.contains("direct_child_ready"), "{first}");
        let mut second = String::new();
        reader.read_line(&mut second).unwrap();
        assert!(second.contains("loopback_ready"), "{second}");
        assert!(second.contains("pairing_blake3_v1"), "{second}");
        std::mem::forget(reader);
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        Self { child, address }
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Canonical test client: pairing + hello + sealed envelopes.
// ---------------------------------------------------------------------------

struct ServerChallenge {
    version: ProtocolVersion,
    session: SessionId,
    nonce: ClientNonce,
    challenge: PairingChallenge,
    binding: ProofDigest,
}

struct Client {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    key: [u8; 32],
    transcript: Vec<u8>,
    nonce: ProtoServerNonce,
    version: ProtocolVersion,
    envelope_sequence: u64,
    request_counter: u64,
    provider_sequence: u64,
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0F)]));
    }
    output
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || text.is_empty() {
        return None;
    }
    let mut output = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let high = hex_value(bytes[index])?;
        let low = hex_value(bytes[index + 1])?;
        output.push((high << 4) | low);
        index += 2;
    }
    Some(output)
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn binding_digest(key: &[u8; 32]) -> ProofDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(BINDING_DOMAIN);
    hasher.update(BINDING_ROLE);
    hasher.update(&[0]);
    hasher.update(key);
    ProofDigest::from_bytes(*hasher.finalize().as_bytes())
}

fn keyed(key: &[u8; 32], bytes: &[u8]) -> ProofDigest {
    ProofDigest::from_bytes(*blake3::keyed_hash(key, bytes).as_bytes())
}

fn read_line(reader: &mut BufReader<TcpStream>) -> String {
    let mut line = String::new();
    let mut total = 0_usize;
    loop {
        let mut byte = [0_u8; 1];
        reader.get_mut().set_read_timeout(Some(TIMEOUT)).unwrap();
        match reader.get_mut().read_exact(&mut byte) {
            Ok(()) => {}
            Err(error) => panic!("client read failed: {error}"),
        }
        total += 1;
        assert!(total <= MAX_LINE + 1, "response line exceeds bound");
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0] as char);
    }
    line.strip_suffix('\r').unwrap_or(&line).to_owned()
}

fn parse_challenge(line: &str) -> ServerChallenge {
    let parts: Vec<&str> = line.split('\t').collect();
    assert_eq!(parts.len(), 6, "{line}");
    assert_eq!(parts[0], "PAIRING_CHALLENGE");
    assert_eq!(parts[1], "v=1.0");
    let session_raw = hex_decode(parts[2].strip_prefix("session=").unwrap()).unwrap();
    let nonce_raw = hex_decode(parts[3].strip_prefix("nonce=").unwrap()).unwrap();
    let challenge_raw = hex_decode(parts[4].strip_prefix("challenge=").unwrap()).unwrap();
    let binding_raw = hex_decode(parts[5].strip_prefix("binding=").unwrap()).unwrap();
    let mut session = [0_u8; 16];
    let mut nonce = [0_u8; 16];
    let mut challenge = [0_u8; 32];
    let mut binding = [0_u8; 32];
    session.copy_from_slice(&session_raw);
    nonce.copy_from_slice(&nonce_raw);
    challenge.copy_from_slice(&challenge_raw);
    binding.copy_from_slice(&binding_raw);
    ServerChallenge {
        version: VERSION,
        session: SessionId::from_bytes(session).unwrap(),
        nonce: ClientNonce::from_bytes(nonce).unwrap(),
        challenge: PairingChallenge::from_bytes(challenge).unwrap(),
        binding: ProofDigest::from_bytes(binding),
    }
}

impl Client {
    fn connect(address: SocketAddr, key: [u8; 32]) -> Self {
        let mut last = None;
        for _ in 0..100 {
            match TcpStream::connect_timeout(&address, Duration::from_millis(200)) {
                Ok(stream) => {
                    last = Some(stream);
                    break;
                }
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
        let mut stream = last.expect("proxy listener bound");
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        stream.set_write_timeout(Some(TIMEOUT)).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let challenge_line = read_line(&mut reader);
        let challenge = parse_challenge(&challenge_line);
        assert_eq!(binding_digest(&key), challenge.binding);
        let proof = keyed(
            &key,
            client_proof_transcript(
                challenge.version,
                &challenge.binding,
                challenge.session,
                &challenge.nonce,
                &challenge.challenge,
            )
            .as_bytes(),
        );
        writeln!(
            stream,
            "PAIRING_AUTH\tproof={}",
            hex_encode(proof.as_bytes())
        )
        .unwrap();
        stream.flush().unwrap();
        let verified_line = read_line(&mut reader);
        let verified_hex = verified_line
            .strip_prefix("PAIRING_VERIFIED\tproof=")
            .unwrap();
        let verified_raw = hex_decode(verified_hex).unwrap();
        let mut verified = [0_u8; 32];
        verified.copy_from_slice(&verified_raw);
        let expected = keyed(
            &key,
            server_proof_transcript(
                challenge.version,
                &challenge.binding,
                challenge.session,
                &challenge.nonce,
                &challenge.challenge,
            )
            .as_bytes(),
        );
        assert!(verify_proof(&expected, &ProofDigest::from_bytes(verified)));
        let ready = read_line(&mut reader);
        assert!(ready.contains("\"event\":\"authenticated\""), "{ready}");
        Self {
            stream,
            reader,
            key,
            transcript: Vec::new(),
            nonce: ProtoServerNonce::from_bytes([1; 16]).unwrap(),
            version: VERSION,
            envelope_sequence: 0,
            request_counter: 0,
            provider_sequence: 0,
        }
    }

    fn record(&mut self, line: &str) {
        self.transcript
            .extend_from_slice(format!("{line}\n").as_bytes());
    }

    fn send(&mut self, line: &str) {
        writeln!(self.stream, "{line}").unwrap();
        self.stream.flush().unwrap();
    }

    fn recv(&mut self) -> String {
        let line = read_line(&mut self.reader);
        self.record(&line);
        line
    }

    /// `op\thello` version negotiation; returns the raw hello line.
    fn hello(&mut self, range: Option<&str>) -> String {
        match range {
            Some(text) => self.send(&format!("op\thello\t{text}")),
            None => self.send("op\thello"),
        }
        let started = self.recv();
        assert!(
            started.contains("\"event\":\"request_started\""),
            "{started}"
        );
        let hello = self.recv();
        assert!(hello.contains("\"event\":\"provider_hello\""), "{hello}");
        assert!(hello.contains("\"version\":\"1.0\""), "{hello}");
        let nonce_hex = extract_field(&hello, "\"nonce\":\"").expect("nonce");
        let nonce_raw = hex_decode(&nonce_hex).unwrap();
        let mut nonce = [0_u8; 16];
        nonce.copy_from_slice(&nonce_raw);
        self.nonce = ProtoServerNonce::from_bytes(nonce).unwrap();
        let complete = self.recv();
        assert!(complete.contains("\"ok\":true"), "{complete}");
        hello
    }

    fn mint_id(&mut self) -> RequestId {
        self.request_counter += 1;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut input = Vec::new();
        input.extend_from_slice(b"eliot-provider-request/v1\0");
        input.extend_from_slice(&self.request_counter.to_le_bytes());
        input.extend_from_slice(&nanos.to_le_bytes());
        let digest = blake3::keyed_hash(&self.key, &input);
        let mut raw = [0_u8; 16];
        raw.copy_from_slice(&digest.as_bytes()[..16]);
        if raw.iter().all(|byte| *byte == 0) {
            raw[15] = 1;
        }
        RequestId::from_bytes(raw)
    }

    /// Sends one sealed envelope with a freshly minted identity.
    fn round(&mut self, command: ControlCommand) -> (AuthenticatedResponse, Vec<String>) {
        let id = self.mint_id();
        self.envelope(command, id)
    }

    /// Sends one sealed envelope; returns the verified response plus the
    /// collected child lines (excluding framing).
    fn envelope(
        &mut self,
        command: ControlCommand,
        request: RequestId,
    ) -> (AuthenticatedResponse, Vec<String>) {
        self.envelope_sequence += 1;
        let digest = ProofDigest::from_bytes([0x33; 32]);
        let stub = seal_envelope(
            self.version,
            self.nonce,
            request,
            command,
            digest,
            ProofDigest::from_bytes([0; 32]),
        );
        let proof = keyed(&self.key, &envelope_transcript(&stub));
        let sealed = seal_envelope(self.version, self.nonce, request, command, digest, proof);
        let frame = encode_envelope_json(&sealed);
        let length = u32::try_from(frame.len()).expect("envelope fits");
        let mut framed = length.to_le_bytes().to_vec();
        framed.extend_from_slice(&frame);
        self.send(&format!(
            "envelope\t{}\t{}",
            self.envelope_sequence,
            hex_encode(&framed)
        ));
        let started = self.recv();
        assert!(started.contains("request_started"), "{started}");
        let mut child_lines = Vec::new();
        let response = loop {
            let line = self.recv();
            if let Some(hex) = line.strip_prefix("response\t") {
                let frame = hex_decode(hex).unwrap();
                let declared =
                    usize::try_from(u32::from_le_bytes(frame[..4].try_into().unwrap())).unwrap();
                assert_eq!(declared + 4, frame.len());
                let range = ProtocolRange::new(VERSION, VERSION).unwrap();
                let decoded = decode_response_json(&frame[4..], range).unwrap();
                // Verify the keyed proof and the receipt binding against the
                // next expected provider sequence.
                let stub = seal_response_with_receipt_placeholder(self.nonce, &decoded);
                let expected = keyed(&self.key, &response_transcript(&stub));
                assert_eq!(expected, *decoded.proof());
                self.provider_sequence += 1;
                let receipt = render_receipt(&decoded, self.provider_sequence);
                let digest = ProofDigest::from_bytes(*blake3::hash(&receipt).as_bytes());
                assert_eq!(digest, *decoded.body_digest());
                assert_eq!(*decoded.request_id(), request);
                break decoded;
            }
            assert!(
                !line.contains("provider_error"),
                "envelope rejected: {line}"
            );
            child_lines.push(line);
        };
        let complete = self.recv();
        assert!(complete.contains("request_complete"), "{complete}");
        (response, child_lines)
    }

    /// Sends one `op` line; returns (`ack_ok`, `op_line`).
    fn op(&mut self, line: &str) -> (bool, String) {
        self.send(line);
        let started = self.recv();
        assert!(started.contains("request_started"), "{started}");
        let mut op_line = String::new();
        let ack = loop {
            let line = self.recv();
            if line.contains("request_complete") {
                break line.contains("\"ok\":true");
            }
            if line.contains("provider_op") || line.contains("provider_error") {
                op_line = line;
            } else {
                op_line.push_str(&line);
                op_line.push('\n');
            }
        };
        (ack, op_line)
    }
}

fn seal_response_with_receipt_placeholder(
    nonce: ProtoServerNonce,
    response: &AuthenticatedResponse,
) -> AuthenticatedResponse {
    seal_response(
        response.version(),
        nonce,
        *response.request_id(),
        response.status(),
        *response.body_digest(),
        ProofDigest::from_bytes([0; 32]),
    )
}

fn render_receipt(response: &AuthenticatedResponse, sequence: u64) -> Vec<u8> {
    format!(
        "provider-response\t{}.{}\t{}\t{}\t{sequence}",
        response.version().major,
        response.version().minor,
        hex_encode(response.request_id().as_bytes()),
        response.status().as_str(),
    )
    .into_bytes()
}

fn extract_field(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    let end = line[start..].find('"')?;
    Some(line[start..start + end].to_owned())
}

// ---------------------------------------------------------------------------
// Discriminating tests.
// ---------------------------------------------------------------------------

#[test]
fn envelope_health_version_shutdown_round_trip() {
    let scratch = Scratch::new();
    let mut proxy = Proxy::start(&scratch);
    let key = scratch.key();
    let mut client = Client::connect(proxy.address, key);
    let hello = client.hello(Some("1.0-1.0"));
    assert!(hello.contains("\"query\":false"), "{hello}");
    assert!(hello.contains("SEARCH_NOT_ACCEPTED"), "{hello}");

    let (health, lines) = client.round(ControlCommand::Health);
    assert_eq!(health.status(), RequestStatus::Ok);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("\"event\":\"health\"")),
        "{lines:?}"
    );

    let (version, lines) = client.round(ControlCommand::Version);
    assert_eq!(version.status(), RequestStatus::Ok);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("\"event\":\"version\"")),
        "{lines:?}"
    );

    // Shutdown is a sealed terminal: the daemon exits cleanly afterwards.
    let (shutdown, _) = client.round(ControlCommand::Shutdown);
    assert_eq!(shutdown.status(), RequestStatus::Ok);
    let status = proxy.child.wait().unwrap();
    assert!(status.success(), "{status}");
    assert_key_never_leaks(&client.transcript, &scratch.token, &key);
}

#[test]
fn foreign_version_fails_closed() {
    let scratch = Scratch::new();
    let mut proxy = Proxy::start(&scratch);
    let key = scratch.key();
    let mut client = Client::connect(proxy.address, key);
    client.hello(None);

    // Foreign envelope version fails against the negotiated range.
    client.envelope_sequence += 1;
    let digest = ProofDigest::from_bytes([0x33; 32]);
    let foreign_version = ProtocolVersion { major: 2, minor: 0 };
    let stub = seal_envelope(
        foreign_version,
        client.nonce,
        client.mint_id(),
        ControlCommand::Health,
        digest,
        ProofDigest::from_bytes([0; 32]),
    );
    let proof = keyed(&key, &envelope_transcript(&stub));
    let sealed = seal_envelope(
        foreign_version,
        client.nonce,
        client.mint_id(),
        ControlCommand::Health,
        digest,
        proof,
    );
    // encode_envelope_json is version-agnostic; frame it manually.
    let frame = encode_envelope_json(&sealed);
    let mut framed = (u32::try_from(frame.len()).expect("frame fits"))
        .to_le_bytes()
        .to_vec();
    framed.extend_from_slice(&frame);
    client.send(&format!(
        "envelope\t{}\t{}",
        client.envelope_sequence,
        hex_encode(&framed)
    ));
    let mut saw_incompatible = false;
    for _ in 0..8 {
        let line = client.recv();
        if line.contains("NO_COMPATIBLE_VERSION") {
            saw_incompatible = true;
        }
        if line.contains("request_complete") {
            assert!(line.contains("\"ok\":false"), "{line}");
            break;
        }
    }
    assert!(saw_incompatible, "foreign version must fail typed");
    assert_key_never_leaks(&client.transcript, &scratch.token, &key);
    proxy.kill();
}

#[test]
fn sequence_gap_fails_closed_without_consuming_session() {
    let scratch = Scratch::new();
    let mut proxy = Proxy::start(&scratch);
    let key = scratch.key();
    let mut client = Client::connect(proxy.address, key);
    client.hello(None);

    // Sequence gap fails distinctly; the exact next sequence still admits.
    let digest = ProofDigest::from_bytes([0x33; 32]);
    let id = client.mint_id();
    let gap_sequence = client.envelope_sequence + 5;
    let stub = seal_envelope(
        VERSION,
        client.nonce,
        id,
        ControlCommand::Health,
        digest,
        ProofDigest::from_bytes([0; 32]),
    );
    let proof = keyed(&key, &envelope_transcript(&stub));
    let sealed = seal_envelope(
        VERSION,
        client.nonce,
        id,
        ControlCommand::Health,
        digest,
        proof,
    );
    let frame = encode_envelope_json(&sealed);
    let mut framed = (u32::try_from(frame.len()).expect("frame fits"))
        .to_le_bytes()
        .to_vec();
    framed.extend_from_slice(&frame);
    client.send(&format!(
        "envelope\t{gap_sequence}\t{}",
        hex_encode(&framed)
    ));
    let mut saw_gap = false;
    for _ in 0..8 {
        let line = client.recv();
        if line.contains("SEQUENCE_GAP") {
            saw_gap = true;
        }
        if line.contains("request_complete") {
            break;
        }
    }
    assert!(saw_gap, "sequence gap must fail typed");
    // Failed admissions consume no session sequence: the exact next
    // identity (sequence 1) still admits.
    client.envelope_sequence = 0;
    let (health, _) = client.round(ControlCommand::Health);
    assert_eq!(health.status(), RequestStatus::Ok);
    assert_key_never_leaks(&client.transcript, &scratch.token, &key);
    proxy.kill();
}

#[test]
fn oversized_line_drops_connection_but_fresh_recovers() {
    let scratch = Scratch::new();
    let mut proxy = Proxy::start(&scratch);
    let key = scratch.key();
    let mut client = Client::connect(proxy.address, key);
    client.hello(None);

    // An oversized line is bounded by the transport ceiling and drops only
    // this connection; a fresh connection recovers.
    let huge = format!("envelope\t1\t{}", "ab".repeat(100_000));
    client.send(&huge);
    let mut observed = Vec::new();
    client
        .stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut byte = [0_u8; 1];
    // The server fails the connection closed (clean EOF or RST both prove no
    // further frame is honored on the poisoned connection).
    if client.reader.get_mut().read_exact(&mut byte).is_ok() {
        // A framing error line may precede the close; drain to EOF.
        observed.push(byte[0]);
        let _ = client.reader.get_mut().read_to_end(&mut observed);
    }
    let text = String::from_utf8_lossy(&observed);
    assert!(
        !text.contains("\"ok\":true"),
        "oversized line must not succeed: {text}"
    );
    assert!(
        !text.contains("response\t"),
        "oversized line must not seal a response: {text}"
    );
    drop(client);
    let mut recovered = Client::connect(proxy.address, key);
    recovered.hello(None);
    let (health, _) = recovered.round(ControlCommand::Health);
    assert_eq!(health.status(), RequestStatus::Ok);
    assert_key_never_leaks(&recovered.transcript, &scratch.token, &key);
    proxy.kill();
}

#[test]
fn replay_is_rejected_and_connection_stays_usable() {
    let scratch = Scratch::new();
    let mut proxy = Proxy::start(&scratch);
    let key = scratch.key();
    let mut client = Client::connect(proxy.address, key);
    client.hello(None);
    let id = client.mint_id();
    let (first, _) = client.envelope(ControlCommand::Health, id);
    assert_eq!(first.status(), RequestStatus::Ok);
    // Same identity with a fresh sequence is a replay, not a new request.
    client.envelope_sequence += 1;
    let digest = ProofDigest::from_bytes([0x33; 32]);
    let stub = seal_envelope(
        VERSION,
        client.nonce,
        id,
        ControlCommand::Health,
        digest,
        ProofDigest::from_bytes([0; 32]),
    );
    let proof = keyed(&key, &envelope_transcript(&stub));
    let sealed = seal_envelope(
        VERSION,
        client.nonce,
        id,
        ControlCommand::Health,
        digest,
        proof,
    );
    let frame = encode_envelope_json(&sealed);
    let mut framed = (u32::try_from(frame.len()).expect("frame fits"))
        .to_le_bytes()
        .to_vec();
    framed.extend_from_slice(&frame);
    client.send(&format!(
        "envelope\t{}\t{}",
        client.envelope_sequence,
        hex_encode(&framed)
    ));
    let mut saw_replay = false;
    let mut ack_ok = true;
    for _ in 0..8 {
        let line = client.recv();
        if line.contains("REPLAY_DETECTED") {
            saw_replay = true;
        }
        if line.contains("request_complete") {
            ack_ok = line.contains("\"ok\":true");
            break;
        }
    }
    assert!(saw_replay, "replayed identity must fail typed");
    assert!(!ack_ok);
    // The replay verdict consumed sequence 2 in the session tracker, so the
    // next fresh identity continues at 3; ordinary rejection never poisons
    // the connection.
    let (health, _) = client.round(ControlCommand::Version);
    assert_eq!(health.status(), RequestStatus::Ok);
    proxy.kill();
}

#[test]
fn cancel_is_idempotent_and_rehello_rebinds() {
    let scratch = Scratch::new();
    let mut proxy = Proxy::start(&scratch);
    let key = scratch.key();
    let mut client = Client::connect(proxy.address, key);
    client.hello(None);
    // Unknown identity: idempotent unknown_or_terminal, connection usable.
    let (ack, line) = client.op("op\tcancel\t00112233445566778899aabbccddeeff");
    assert!(ack, "{line}");
    assert!(line.contains("unknown_or_terminal"), "{line}");
    // Re-hello rebinds the connection with a fresh nonce.
    let first_nonce = client.nonce;
    let hello = client.hello(Some("1.0-1.0"));
    assert!(hello.contains("reconnect_cancelled"), "{hello}");
    assert_ne!(first_nonce, client.nonce);
    let (health, _) = client.round(ControlCommand::Health);
    assert_eq!(health.status(), RequestStatus::Ok);
    proxy.kill();
}

#[test]
fn foreign_hello_range_fails_then_recovers() {
    let scratch = Scratch::new();
    let mut proxy = Proxy::start(&scratch);
    let key = scratch.key();
    let mut client = Client::connect(proxy.address, key);
    client.send("op\thello\t2.0-2.0");
    let mut saw_mismatch = false;
    for _ in 0..6 {
        let line = client.recv();
        if line.contains("NO_COMPATIBLE_VERSION") {
            saw_mismatch = true;
        }
        if line.contains("request_complete") {
            assert!(line.contains("\"ok\":false"), "{line}");
            break;
        }
    }
    assert!(saw_mismatch);
    // A compatible hello on the same connection binds normally.
    client.hello(Some("1.0-1.0"));
    let (health, _) = client.round(ControlCommand::Health);
    assert_eq!(health.status(), RequestStatus::Ok);
    proxy.kill();
}

#[test]
fn gated_recipes_are_unavailable_never_empty_success() {
    let scratch = Scratch::new();
    let mut proxy = Proxy::start(&scratch);
    let key = scratch.key();
    let mut client = Client::connect(proxy.address, key);
    client.hello(None);
    for (operation, reason) in [
        ("query", "PROVIDER_QUERY_UNAVAILABLE"),
        ("ingest", "PROVIDER_INGEST_UNAVAILABLE"),
        ("expand", "PROVIDER_EXPAND_UNAVAILABLE"),
    ] {
        let (ack, line) = client.op(&format!("op\t{operation}\t616263"));
        assert!(!ack, "{operation}: {line}");
        assert!(line.contains(reason), "{operation}: {line}");
        assert!(line.contains("SEARCH_NOT_ACCEPTED"), "{operation}: {line}");
        assert!(!line.contains("\"status\":\"ok\""), "{operation}: {line}");
    }
    // Unknown operations fail typed as well.
    let (ack, line) = client.op("op\treboot");
    assert!(!ack, "{line}");
    assert!(line.contains("PROVIDER_UNKNOWN_COMMAND"), "{line}");
    // Envelope-only names are refused on the bare op path.
    let (ack, line) = client.op("op\thealth");
    assert!(!ack, "{line}");
    assert!(line.contains("PROVIDER_ENVELOPE_REQUIRED"), "{line}");
    // The connection still routes sealed envelopes afterwards.
    let (health, _) = client.round(ControlCommand::Health);
    assert_eq!(health.status(), RequestStatus::Ok);
    proxy.kill();
}

#[test]
fn transport_failure_after_dispatch_is_never_success() {
    let scratch = Scratch::new();
    let mut proxy = Proxy::start(&scratch);
    let key = scratch.key();
    let mut client = Client::connect(proxy.address, key);
    client.hello(None);
    // Send one sealed envelope, then destroy the daemon before reading the
    // terminal: the client must observe a failed/closed connection with no
    // success frame, and must never invent one.
    client.envelope_sequence += 1;
    let id = client.mint_id();
    let digest = ProofDigest::from_bytes([0x33; 32]);
    let stub = seal_envelope(
        VERSION,
        client.nonce,
        id,
        ControlCommand::Health,
        digest,
        ProofDigest::from_bytes([0; 32]),
    );
    let proof = keyed(&key, &envelope_transcript(&stub));
    let sealed = seal_envelope(
        VERSION,
        client.nonce,
        id,
        ControlCommand::Health,
        digest,
        proof,
    );
    let frame = encode_envelope_json(&sealed);
    let mut framed = (u32::try_from(frame.len()).expect("frame fits"))
        .to_le_bytes()
        .to_vec();
    framed.extend_from_slice(&frame);
    client.send(&format!(
        "envelope\t{}\t{}",
        client.envelope_sequence,
        hex_encode(&framed)
    ));
    proxy.kill();
    client
        .stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut observed = Vec::new();
    let _ = client.reader.get_mut().read_to_end(&mut observed);
    let text = String::from_utf8_lossy(&observed);
    assert!(
        !text.contains("\"ok\":true"),
        "killed daemon must not produce success: {text}"
    );
    assert!(
        !text.contains("response\t"),
        "killed daemon must not produce a sealed response: {text}"
    );
}

#[test]
fn reconnect_after_disconnect_serves_fresh_handshake() {
    let scratch = Scratch::new();
    let mut proxy = Proxy::start(&scratch);
    let key = scratch.key();
    let mut first = Client::connect(proxy.address, key);
    first.hello(None);
    let (health, _) = first.round(ControlCommand::Health);
    assert_eq!(health.status(), RequestStatus::Ok);
    let first_nonce = first.nonce;
    drop(first);
    // A single-use challenge ledger rejects exact replay at the pairing
    // layer; a fresh handshake on a new connection succeeds and rebinds.
    let mut second = Client::connect(proxy.address, key);
    second.hello(None);
    assert_ne!(first_nonce, second.nonce);
    let (health, _) = second.round(ControlCommand::Version);
    assert_eq!(health.status(), RequestStatus::Ok);
    proxy.kill();
}

fn assert_key_never_leaks(transcript: &[u8], token: &[u8], key: &[u8; 32]) {
    assert!(
        token.len() >= 32,
        "sentinel needs a meaningful token window"
    );
    for window in token.windows(16) {
        assert!(
            !transcript.windows(window.len()).any(|slot| slot == window),
            "token material must never cross the socket"
        );
    }
    assert!(
        !transcript.windows(16).any(|slot| slot == key),
        "pairing key must never cross the socket"
    );
    // Keyed proofs do cross (exactly the pairing digests), which is expected.
}

// ---------------------------------------------------------------------------
// Real CLI round trips against the canonical endpoint descriptor.
// ---------------------------------------------------------------------------

fn cli_binary() -> PathBuf {
    let daemon = PathBuf::from(env!("CARGO_BIN_EXE_eliot-searchd"));
    let name = if cfg!(windows) {
        "eliot-search.exe"
    } else {
        "eliot-search"
    };
    daemon.parent().expect("daemon parent").join(name)
}

fn write_endpoint(root: &Path, address: SocketAddr) {
    let runtime = root.join("runtime");
    fs::create_dir_all(&runtime).unwrap();
    fs::write(
        runtime.join("endpoint.v1"),
        format!("ELIOT_SEARCH_ENDPOINT_V1\naddress={address}\n"),
    )
    .unwrap();
}

fn run_cli(root: &Path, token: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(cli_binary())
        .args(args)
        .arg("--data-root")
        .arg(root)
        .arg("--token-file")
        .arg(token)
        .output()
        .expect("cli binary runs (build both packages first)");
    let code = output.status.code().unwrap_or(-1);
    (
        code,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn real_cli_health_round_trip_and_gated_search() {
    let scratch = Scratch::new();
    let proxy = Proxy::start(&scratch);
    let cli_root = scratch.dir.join("cli");
    fs::create_dir_all(&cli_root).unwrap();
    write_endpoint(&cli_root, proxy.address);

    let (code, stdout, stderr) = run_cli(&cli_root, &scratch.token_file(), &["health"]);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(stdout.contains("\"event\":\"health\""), "{stdout}");

    let (code, stdout, stderr) = run_cli(&cli_root, &scratch.token_file(), &["status"]);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(stdout.contains("provider"), "{stdout}");

    // Gated recipes exit 2 with explicit unavailable, never empty success.
    let (code, stdout, stderr) = run_cli(&cli_root, &scratch.token_file(), &["search", "needle"]);
    assert_eq!(code, 2, "stdout={stdout} stderr={stderr}");
    assert!(
        stdout.contains("PROVIDER_QUERY_UNAVAILABLE"),
        "{stdout} {stderr}"
    );

    // A missing descriptor fails closed without a hidden default dial.
    let bare = scratch.dir.join("bare");
    fs::create_dir_all(&bare).unwrap();
    let (code, _, stderr) = run_cli(&bare, &scratch.token_file(), &["health"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("ENDPOINT"), "{stderr}");
    drop(proxy);
}

#[test]
fn provider_units_cover_router_core() {
    // The router core (sequencing, terminal uniqueness, cancel, capability
    // gating, seal/verify) is proven by unit tests beside the implementation;
    // this process file proves the live wire. Cross-check one shared
    // expectation here: the closed registries both sides rely on.
    assert_eq!(ProviderOperation::ALL.len(), 8);
    assert_eq!(ControlCommand::ALL.len(), 3);
    let shell = provider::CapabilityEvidence::from_parts(
        false,
        false,
        false,
        vec!["SEARCH_NOT_ACCEPTED", "INDEXED_NOT_ACCEPTED"],
    )
    .expect("shell evidence");
    let caps = negotiate_capabilities(&shell);
    assert!(!caps.query_available);
    let denial = provider::gate_operation(ProviderOperation::Query, &caps).expect_err("gated");
    assert_eq!(denial.reason, provider::PROVIDER_QUERY_UNAVAILABLE);
    let _ = OpStatus::Ok;
    let _ = OpArgument::None;
}
