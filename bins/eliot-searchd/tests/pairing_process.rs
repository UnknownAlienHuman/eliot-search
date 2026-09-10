//! T18 pairing-composition process tests: secret leases plus replay-resistant
//! loopback pairing over real sockets.
//!
//! Black-box coverage through the new wire (`pairing_blake3_v1`): a
//! lease-bound handshake proves mutually with keyed BLAKE3 over the approved
//! pairing transcripts, then dispatches commands on the unchanged command
//! loop. Negative paths prove replay, wrong version, tampered fields, wrong
//! keys, rotation, revocation, absence readback and expiry fail closed while
//! the listener keeps serving honest retries.
//!
//! Native Credential Manager coverage uses a test-local OS vault with real
//! `CredWrite`/`CredRead`/`CredDelete` traffic plus Drop-based cleanup that
//! mirrors `tests/common` (cross-process vault mutex, verify-after-delete,
//! bounded retries, never panics so cleanup cannot mask a result). Target
//! names carry a unique per-run tag and a final `cmdkey /list` scan proves
//! zero leftovers. A `RevisionKeyTreeGuard` from `tests/common` is held by
//! the native fixture to prove the pairing vault creates no revision-key
//! side effects.
//!
//! A sentinel test scans the full session transcript: key bytes never appear
//! while the expected (keyed, one-way) proofs do.

#![allow(unsafe_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

use search_contracts::{Blake3Digest32, InstallationId, InstallationIncarnationId};
use search_ports::MonotonicInstant;

#[allow(dead_code)]
mod common;
#[path = "../src/endpoint.rs"]
mod endpoint;
#[path = "../src/secret_composition.rs"]
mod secret_composition;

use endpoint::{EndpointAction, EndpointKeySource, ParsedChallenge, serve_loopback_with_source};
use secret_composition::{
    MemoryPairingVault, MutationOutcome, PairingSecretComposer, PairingVault,
    SecretCompositionError, fresh_operation, pairing_binding, test_nonce,
};

const TIMEOUT: Duration = Duration::from_secs(10);
const NOW_TICKS: u64 = 1_000_000;

fn test_binding() -> search_os_secrets::SecretBinding {
    pairing_binding(
        InstallationId::from_bytes([0x11; 16]),
        InstallationIncarnationId::from_bytes([0x22; 16]),
        Blake3Digest32::from_bytes([0x33; 32]),
    )
    .expect("binding")
}

fn provisioned_memory() -> (PairingSecretComposer, MemoryPairingVault, [u8; 32]) {
    let mut composer = PairingSecretComposer::new(test_binding()).expect("composer");
    let mut vault = MemoryPairingVault::new();
    let outcome = composer
        .provision(
            &mut vault,
            fresh_operation("provision", &test_nonce(0xA1)).expect("operation"),
        )
        .expect("provision");
    let _ = outcome.receipt;
    let now = MonotonicInstant::from_ticks(NOW_TICKS);
    let key = composer
        .with_pairing_key(&mut vault, now, |key| *key)
        .expect("key");
    (composer, vault, key)
}

/// Lease-bound key source: the only key path the server uses.
struct ServedSecrets<V> {
    composer: PairingSecretComposer,
    vault: V,
    now: MonotonicInstant,
}

impl<V: PairingVault> EndpointKeySource for ServedSecrets<V> {
    fn with_endpoint_key<T>(&mut self, use_key: impl FnOnce(&[u8; 32]) -> T) -> Result<T, String> {
        self.composer
            .with_pairing_key(&mut self.vault, self.now, use_key)
            .map_err(|error| error.code().to_owned())
    }
}

struct LiveServer {
    address: SocketAddr,
    handle: Option<JoinHandle<()>>,
    done: mpsc::Receiver<()>,
    dispatches: Arc<AtomicUsize>,
    shutdown_key: Option<[u8; 32]>,
}

impl LiveServer {
    fn start<V>(mut secrets: ServedSecrets<V>) -> Self
    where
        V: PairingVault + Send + 'static,
    {
        let shutdown_key = secrets
            .composer
            .with_pairing_key(&mut secrets.vault, secrets.now, |key| *key)
            .ok();
        // Probe one free loopback port, then serve exactly it. The probe race
        // is closed by a bounded client connect retry below.
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .expect("probe")
            .local_addr()
            .expect("probe address")
            .port();
        let dispatches = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&dispatches);
        let (done_tx, done_rx) = mpsc::channel::<()>();
        let handle = std::thread::spawn(move || {
            let mut secrets = secrets;
            let _ = serve_loopback_with_source(port, &mut secrets, move |command, stream| {
                counter.fetch_add(1, Ordering::Relaxed);
                if command == "shutdown" {
                    return Ok(EndpointAction::Shutdown);
                }
                stream
                    .write_all(b"{\"echo\":true}\n")
                    .map_err(|_| "FIXTURE_WRITE_FAILED".to_owned())?;
                Ok(EndpointAction::Continue)
            });
            let _ = done_tx.send(());
        });
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        Self {
            address,
            handle: Some(handle),
            done: done_rx,
            dispatches,
            shutdown_key,
        }
    }

    fn connect(&self) -> (TcpStream, BufReader<TcpStream>) {
        let mut last = None;
        for _ in 0..100 {
            match TcpStream::connect_timeout(&self.address, Duration::from_millis(200)) {
                Ok(stream) => {
                    stream
                        .set_read_timeout(Some(TIMEOUT))
                        .expect("read timeout");
                    stream
                        .set_write_timeout(Some(TIMEOUT))
                        .expect("write timeout");
                    let reader = BufReader::new(stream.try_clone().expect("clone"));
                    return (stream, reader);
                }
                Err(error) => {
                    last = Some(error);
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        panic!("loopback connect failed: {last:?}");
    }

    fn dispatches(&self) -> usize {
        self.dispatches.load(Ordering::Relaxed)
    }

    /// Clean shutdown with a full handshake: proves the server exits
    /// boundedly instead of leaking the listener thread.
    fn shutdown(mut self, key: &[u8; 32]) {
        let (mut stream, mut reader) = self.connect();
        try_handshake(&mut stream, &mut reader, key).expect("shutdown handshake");
        write_line(&mut stream, "shutdown");
        let started = read_line(&mut reader, 1024).expect("started");
        assert!(started.contains("\"event\":\"request_started\""));
        let complete = read_line(&mut reader, 1024).expect("complete");
        assert!(complete.contains("\"ok\":true"));
        self.done.recv_timeout(TIMEOUT).expect("clean server exit");
        if let Some(handle) = self.handle.take() {
            handle.join().expect("join");
        }
    }
}

impl Drop for LiveServer {
    fn drop(&mut self) {
        if self.handle.is_none() {
            return;
        }
        // Best effort only, never panics: a test that failed early leaves
        // the listener behind; the detached thread dies with the process.
        let Some(key) = self.shutdown_key else {
            return;
        };
        let Ok(stream) = TcpStream::connect_timeout(&self.address, Duration::from_secs(2)) else {
            return;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
        let Ok(clone) = stream.try_clone() else {
            return;
        };
        let mut stream = stream;
        let mut reader = BufReader::new(clone);
        if try_handshake_best_effort(&mut stream, &mut reader, &key) {
            let _ = stream.write_all(b"shutdown\n");
            let _ = stream.flush();
        }
        if self.done.recv_timeout(Duration::from_secs(2)).is_ok()
            && let Some(handle) = self.handle.take()
        {
            let _ = handle.join();
        }
    }
}

fn read_line(reader: &mut BufReader<TcpStream>, maximum: usize) -> Option<String> {
    let mut bytes = Vec::new();
    let mut limited = reader.take(u64::try_from(maximum + 1).unwrap_or(u64::MAX));
    let read = match limited.read_until(b'\n', &mut bytes) {
        Ok(read) => read,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) =>
        {
            return None;
        }
        Err(error) => panic!("socket read failed: {error}"),
    };
    if read == 0 {
        return None;
    }
    assert!(
        bytes.len() <= maximum && bytes.ends_with(b"\n"),
        "frame bound"
    );
    bytes.pop();
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    Some(String::from_utf8(bytes).expect("utf8"))
}

fn write_line(stream: &mut TcpStream, value: &str) {
    stream.write_all(value.as_bytes()).expect("write");
    stream.write_all(b"\n").expect("write");
    stream.flush().expect("flush");
}

fn hex_of(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0F)]));
    }
    out
}

/// Full honest client handshake; returns the parsed challenge, the verified
/// provider proof and every server line for sentinel scanning.
fn try_handshake(
    stream: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    key: &[u8; 32],
) -> Result<(ParsedChallenge, String, Vec<String>), String> {
    let mut transcript = Vec::new();
    let challenge_line = read_line(reader, 512).ok_or_else(|| "CHALLENGE_MISSING".to_owned())?;
    transcript.push(challenge_line.clone());
    let challenge = endpoint::parse_challenge_line(&challenge_line)?;
    if endpoint::pairing_binding_digest(key) != challenge.binding {
        return Err("CLIENT_BINDING_MISMATCH".to_owned());
    }
    let proof = endpoint::client_proof_for_challenge(key, &challenge);
    write_line(
        stream,
        &format!("PAIRING_AUTH\tproof={}", hex_of(proof.as_bytes())),
    );
    let verified_line = read_line(reader, 256).ok_or_else(|| "VERIFIED_MISSING".to_owned())?;
    transcript.push(verified_line.clone());
    if verified_line.contains("AUTHENTICATION_FAILED") {
        return Err("SERVER_REJECTED_PROOF".to_owned());
    }
    let provider_proof = endpoint::parse_verified_line(&verified_line)?;
    if !endpoint::verify_provider_proof(key, &challenge, &provider_proof) {
        return Err("PROVIDER_PROOF_INVALID".to_owned());
    }
    let ready = read_line(reader, 1024).ok_or_else(|| "READY_MISSING".to_owned())?;
    transcript.push(ready.clone());
    if !ready.contains("\"event\":\"authenticated\"") {
        return Err("READY_INVALID".to_owned());
    }
    Ok((challenge, hex_of(provider_proof.as_bytes()), transcript))
}

/// Panic-free variant for `Drop` cleanup paths.
fn try_handshake_best_effort(
    stream: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    key: &[u8; 32],
) -> bool {
    let Some(challenge_line) = read_line(reader, 512) else {
        return false;
    };
    let Ok(challenge) = endpoint::parse_challenge_line(&challenge_line) else {
        return false;
    };
    if endpoint::pairing_binding_digest(key) != challenge.binding {
        return false;
    }
    let proof = endpoint::client_proof_for_challenge(key, &challenge);
    if stream
        .write_all(format!("PAIRING_AUTH\tproof={}\n", hex_of(proof.as_bytes())).as_bytes())
        .is_err()
    {
        return false;
    }
    if stream.flush().is_err() {
        return false;
    }
    let Some(verified_line) = read_line(reader, 256) else {
        return false;
    };
    let Ok(provider_proof) = endpoint::parse_verified_line(&verified_line) else {
        return false;
    };
    if !endpoint::verify_provider_proof(key, &challenge, &provider_proof) {
        return false;
    }
    matches!(read_line(reader, 1024), Some(ready) if ready.contains("\"event\":\"authenticated\""))
}

fn provisioned_server() -> (LiveServer, [u8; 32]) {
    let (composer, vault, key) = provisioned_memory();
    let server = LiveServer::start(ServedSecrets {
        composer,
        vault,
        now: MonotonicInstant::from_ticks(NOW_TICKS),
    });
    (server, key)
}

#[test]
fn binding_digest_agrees_between_composition_and_endpoint_on_fixed_vectors() {
    for key in [[0x42; 32], [0x01; 32], [0xFF; 32], test_nonce(0x5A)] {
        assert_eq!(
            secret_composition::derive_binding_digest(&key),
            endpoint::pairing_binding_digest(&key),
            "composition/endpoint agreement"
        );
    }
}

#[test]
fn lease_bound_handshake_proves_mutually_and_dispatches() {
    let (server, key) = provisioned_server();
    let (mut stream, mut reader) = server.connect();
    let (_challenge, _proof, transcript) =
        try_handshake(&mut stream, &mut reader, &key).expect("handshake");
    assert!(transcript[0].starts_with("PAIRING_CHALLENGE\tv=1.0\t"));
    write_line(&mut stream, "health");
    let started = read_line(&mut reader, 1024).expect("started");
    assert!(started.contains("\"event\":\"request_started\""));
    let echo = read_line(&mut reader, 1024).expect("echo");
    assert_eq!(echo, "{\"echo\":true}");
    let complete = read_line(&mut reader, 1024).expect("complete");
    assert!(complete.contains("\"ok\":true"));
    assert_eq!(server.dispatches(), 1);
    // Sentinel: the session carried proofs, never the key.
    let key_hex = hex_of(&key);
    for line in &transcript {
        assert!(!line.contains(&key_hex), "key material on the wire");
    }
    assert!(
        transcript
            .iter()
            .any(|line| line.starts_with("PAIRING_VERIFIED\tproof="))
    );
    drop(stream);
    drop(reader);
    server.shutdown(&key);
}

#[test]
fn replay_captured_proof_on_a_new_challenge_fails_while_listener_survives() {
    let (server, key) = provisioned_server();
    // First connection captures a valid proof for its own challenge.
    let (mut first, mut first_reader) = server.connect();
    let (challenge_one, _, _) = try_handshake(&mut first, &mut first_reader, &key).expect("first");
    let captured = endpoint::client_proof_for_challenge(&key, &challenge_one);
    drop(first);
    drop(first_reader);
    // Second connection replays that proof against a fresh challenge.
    let (mut stream, mut reader) = server.connect();
    let fresh = read_line(&mut reader, 512).expect("challenge");
    let parsed = endpoint::parse_challenge_line(&fresh).expect("parse");
    assert_ne!(parsed.challenge, challenge_one.challenge);
    write_line(
        &mut stream,
        &format!("PAIRING_AUTH\tproof={}", hex_of(captured.as_bytes())),
    );
    let rejection = read_line(&mut reader, 1024).expect("rejection");
    assert!(rejection.contains("AUTHENTICATION_FAILED"));
    drop(stream);
    drop(reader);
    // The listener still serves an honest retry.
    let (mut stream, mut reader) = server.connect();
    try_handshake(&mut stream, &mut reader, &key).expect("retry");
    drop(stream);
    drop(reader);
    server.shutdown(&key);
}

#[test]
fn wrong_version_tampered_fields_and_wrong_key_all_fail() {
    let (server, key) = provisioned_server();
    // Wrong version: proof computed over a downgraded transcript.
    let (mut stream, mut reader) = server.connect();
    let line = read_line(&mut reader, 512).expect("challenge");
    let mut challenge = endpoint::parse_challenge_line(&line).expect("parse");
    challenge.version = search_contracts::ProtocolVersion { major: 1, minor: 1 };
    let proof = endpoint::client_proof_for_challenge(&key, &challenge);
    write_line(
        &mut stream,
        &format!("PAIRING_AUTH\tproof={}", hex_of(proof.as_bytes())),
    );
    assert!(
        read_line(&mut reader, 1024)
            .expect("rejection")
            .contains("AUTHENTICATION_FAILED")
    );
    drop(stream);
    drop(reader);
    // Tampered challenge bytes with a matching proof still fail: the server
    // binds the original challenge it issued.
    let (mut stream, mut reader) = server.connect();
    let line = read_line(&mut reader, 512).expect("challenge");
    let mut challenge = endpoint::parse_challenge_line(&line).expect("parse");
    let mut raw = *challenge.challenge.as_bytes();
    raw[0] ^= 1;
    challenge.challenge =
        search_provider_protocol::pairing::PairingChallenge::from_bytes(raw).expect("challenge");
    let proof = endpoint::client_proof_for_challenge(&key, &challenge);
    write_line(
        &mut stream,
        &format!("PAIRING_AUTH\tproof={}", hex_of(proof.as_bytes())),
    );
    assert!(
        read_line(&mut reader, 1024)
            .expect("rejection")
            .contains("AUTHENTICATION_FAILED")
    );
    drop(stream);
    drop(reader);
    // Wrong key: the honest client itself refuses on the binding mismatch,
    // and a forged proof would fail server-side all the same.
    let (mut stream, mut reader) = server.connect();
    assert!(matches!(
        try_handshake(&mut stream, &mut reader, &[0x77; 32]),
        Err(error) if error == "CLIENT_BINDING_MISMATCH"
    ));
    drop(stream);
    drop(reader);
    // Honest retry still works.
    let (mut stream, mut reader) = server.connect();
    try_handshake(&mut stream, &mut reader, &key).expect("retry");
    drop(stream);
    drop(reader);
    server.shutdown(&key);
}

#[test]
fn rotated_key_rejects_the_old_client_and_accepts_the_new() {
    let (mut composer, mut vault, old_key) = provisioned_memory();
    assert!(matches!(
        composer
            .rotate(
                &mut vault,
                &fresh_operation("rotate", &test_nonce(0xB2)).expect("operation")
            )
            .expect("rotate"),
        MutationOutcome::Committed(_)
    ));
    let now = MonotonicInstant::from_ticks(NOW_TICKS);
    let new_key = composer
        .with_pairing_key(&mut vault, now, |key| *key)
        .expect("key");
    assert_ne!(old_key, new_key);
    let server = LiveServer::start(ServedSecrets {
        composer,
        vault,
        now,
    });
    // Old client: binding mismatch before any proof crosses.
    let (mut stream, mut reader) = server.connect();
    assert!(matches!(
        try_handshake(&mut stream, &mut reader, &old_key),
        Err(error) if error == "CLIENT_BINDING_MISMATCH"
    ));
    drop(stream);
    drop(reader);
    // New client: full mutual handshake.
    let (mut stream, mut reader) = server.connect();
    try_handshake(&mut stream, &mut reader, &new_key).expect("rotated handshake");
    drop(stream);
    drop(reader);
    server.shutdown(&new_key);
}

#[test]
fn revoked_secret_denies_new_handshakes_and_proves_absence() {
    let (mut composer, mut vault, key) = provisioned_memory();
    assert!(matches!(
        composer
            .revoke(
                &mut vault,
                &fresh_operation("revoke", &test_nonce(0xC3)).expect("operation")
            )
            .expect("revoke"),
        MutationOutcome::Committed(_)
    ));
    assert!(composer.absence_verified(&mut vault).expect("absence"));
    let now = MonotonicInstant::from_ticks(NOW_TICKS);
    let server = LiveServer::start(ServedSecrets {
        composer,
        vault,
        now,
    });
    // No challenge is ever written once the lease source is gone.
    let (stream, mut reader) = server.connect();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    assert_eq!(read_line(&mut reader, 512), None);
    drop(stream);
    drop(reader);
    // No clean shutdown is possible without a key: detach best-effort.
    core::mem::forget(server);
    let _ = key;
}

#[test]
fn lease_expiry_is_enforced_at_use_not_at_issue() {
    let (composer, mut vault, _key) = provisioned_memory();
    let lease = composer
        .issue_lease(&mut vault, MonotonicInstant::from_ticks(2_000), 100)
        .expect("lease");
    assert!(
        lease
            .with_secret(MonotonicInstant::from_ticks(2_050), |_| ())
            .is_ok()
    );
    assert_eq!(
        lease
            .with_secret(MonotonicInstant::from_ticks(2_100), |_| ())
            .map_err(SecretCompositionError::from),
        Err(SecretCompositionError::LeaseExpired)
    );
}

#[test]
fn memory_vault_advertises_no_os_backing() {
    assert!(!MemoryPairingVault::new().is_os_backed());
}

#[test]
fn no_secret_material_in_session_transcript() {
    let (server, key) = provisioned_server();
    let (mut stream, mut reader) = server.connect();
    let (_challenge, _proof, transcript) =
        try_handshake(&mut stream, &mut reader, &key).expect("handshake");
    let key_hex = hex_of(&key);
    let joined = transcript.join("\n");
    assert!(!joined.contains(&key_hex), "raw key hex in transcript");
    // Proofs are one-way digests by design: present, but useless for
    // recovering the key, and the legacy scheme is gone from the wire.
    assert!(joined.contains("PAIRING_VERIFIED\tproof="));
    assert!(joined.contains("\"authentication\":\"pairing_blake3_v1\""));
    assert!(!joined.contains("sha256_challenge_v1"));
    // The legacy bare challenge line is gone; the pairing line always carries
    // its full prefix.
    assert!(
        !transcript
            .iter()
            .any(|line| line.starts_with("CHALLENGE\t"))
    );
    assert!(
        transcript
            .iter()
            .any(|line| line.starts_with("PAIRING_CHALLENGE\t"))
    );
    drop(stream);
    drop(reader);
    server.shutdown(&key);
}

// ---------------------------------------------------------------------------
// Native Credential Manager vault (Windows): real Cred* traffic with cleanup.
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod native {
    use super::secret_composition::VaultWriteEvidence;
    use super::secret_composition::derive_binding_digest;
    use super::{
        Blake3Digest32, InstallationId, InstallationIncarnationId, MonotonicInstant,
        MutationOutcome, PairingSecretComposer, PairingVault, SecretCompositionError, common,
        fresh_operation, pairing_binding,
    };
    use search_contracts::OpaqueId;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const CRED_TYPE_GENERIC: u32 = 1;
    const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
    const ERROR_NOT_FOUND: u32 = 1_168;
    const VAULT_MUTEX_NAME: &str = "ELIOT-Search-PairingTest-v1";
    const VAULT_LOCK_WAIT_MILLIS: u32 = 5_000;
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_ABANDONED: u32 = 0x80;
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 2;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn run_tag() -> String {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!(
            "t18-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    }

    fn test_binding() -> search_os_secrets::SecretBinding {
        pairing_binding(
            InstallationId::from_bytes([0x11; 16]),
            InstallationIncarnationId::from_bytes([0x22; 16]),
            Blake3Digest32::from_bytes([0x33; 32]),
        )
        .expect("binding")
    }

    #[link(name = "Advapi32")]
    // `cred_read_w` intentionally redeclares the same DLL symbol as
    // `tests/common`: both signatures are ABI-identical (u32/pointer/i32
    // params; only the nominal credential-struct type behind the pointer
    // differs, with a field-for-field identical layout verified above).
    #[allow(clashing_extern_declarations)]
    unsafe extern "system" {
        #[link_name = "CredReadW"]
        fn cred_read_w(
            target_name: *const u16,
            credential_type: u32,
            flags: u32,
            credential: *mut *mut CredentialW,
        ) -> i32;
        #[link_name = "CredWriteW"]
        fn cred_write_w(credential: *const CredentialW, flags: u32) -> i32;
        #[link_name = "CredDeleteW"]
        fn cred_delete_w(target_name: *const u16, credential_type: u32, flags: u32) -> i32;
        #[link_name = "CredFree"]
        fn cred_free(buffer: *mut core::ffi::c_void);
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        #[link_name = "GetLastError"]
        fn get_last_error() -> u32;
        #[link_name = "CreateMutexW"]
        fn create_mutex_w(
            security_attributes: *mut core::ffi::c_void,
            initial_owner: i32,
            name: *const u16,
        ) -> *mut core::ffi::c_void;
        #[link_name = "WaitForSingleObject"]
        fn wait_for_single_object(handle: *mut core::ffi::c_void, milliseconds: u32) -> u32;
        #[link_name = "ReleaseMutex"]
        fn release_mutex(handle: *mut core::ffi::c_void) -> i32;
        #[link_name = "CloseHandle"]
        fn close_handle(handle: *mut core::ffi::c_void) -> i32;
    }

    #[link(name = "Bcrypt")]
    unsafe extern "system" {
        #[link_name = "BCryptGenRandom"]
        fn bcrypt_gen_random(
            algorithm: *mut core::ffi::c_void,
            buffer: *mut u8,
            buffer_bytes: u32,
            flags: u32,
        ) -> i32;
    }

    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct CredentialW {
        flags: u32,
        credential_type: u32,
        target_name: *mut u16,
        comment: *mut u16,
        last_written: FileTime,
        credential_blob_size: u32,
        credential_blob: *mut u8,
        persist: u32,
        attribute_count: u32,
        attributes: *mut core::ffi::c_void,
        target_alias: *mut u16,
        user_name: *mut u16,
    }

    struct CredentialAllocation(*mut CredentialW);

    impl Drop for CredentialAllocation {
        fn drop(&mut self) {
            if self.0.is_null() {
                return;
            }
            unsafe {
                let credential = &mut *self.0;
                let size = usize::try_from(credential.credential_blob_size)
                    .unwrap_or(0)
                    .min(256);
                if !credential.credential_blob.is_null() && size > 0 {
                    core::ptr::write_bytes(credential.credential_blob, 0, size);
                }
                cred_free(self.0.cast());
            }
        }
    }

    struct VaultLock(*mut core::ffi::c_void);

    impl Drop for VaultLock {
        fn drop(&mut self) {
            if self.0.is_null() {
                return;
            }
            unsafe {
                release_mutex(self.0);
                close_handle(self.0);
            }
        }
    }

    fn acquire_vault_lock() -> Option<VaultLock> {
        let wide: Vec<u16> = VAULT_MUTEX_NAME
            .encode_utf16()
            .chain(core::iter::once(0))
            .collect();
        let handle = unsafe { create_mutex_w(core::ptr::null_mut(), 0, wide.as_ptr()) };
        if handle.is_null() {
            return None;
        }
        let status = unsafe { wait_for_single_object(handle, VAULT_LOCK_WAIT_MILLIS) };
        if status == WAIT_OBJECT_0 || status == WAIT_ABANDONED {
            Some(VaultLock(handle))
        } else {
            unsafe {
                close_handle(handle);
            }
            None
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(core::iter::once(0)).collect()
    }

    /// Test-local OS vault: real Credential Manager entries under a unique
    /// per-run tag. `Send` by construction (only owned strings and counters).
    pub struct NativePairingVault {
        tag: String,
        created: Vec<String>,
        draws: u64,
    }

    impl NativePairingVault {
        pub const fn new(tag: String) -> Self {
            Self {
                tag,
                created: Vec::new(),
                draws: 0,
            }
        }
        fn target_for(&self, id: &OpaqueId) -> String {
            format!(
                "ELIOT Search/loopback-pairing-test/{}/{}",
                self.tag,
                id.as_str()
            )
        }

        fn track(&mut self, target: &str) {
            if !self.created.iter().any(|entry| entry == target) {
                self.created.push(target.to_owned());
            }
        }

        fn write_entry(target: &str, key: &[u8; 32]) -> Result<(), String> {
            let _lock = acquire_vault_lock().ok_or_else(|| "VAULT_BUSY".to_owned())?;
            let mut target_wide = wide(target);
            let mut user_wide = wide("ELIOT Search pairing test");
            let mut blob = *key;
            let credential = CredentialW {
                flags: 0,
                credential_type: CRED_TYPE_GENERIC,
                target_name: target_wide.as_mut_ptr(),
                comment: core::ptr::null_mut(),
                last_written: FileTime { low: 0, high: 0 },
                credential_blob_size: 32,
                credential_blob: blob.as_mut_ptr(),
                persist: CRED_PERSIST_LOCAL_MACHINE,
                attribute_count: 0,
                attributes: core::ptr::null_mut(),
                target_alias: core::ptr::null_mut(),
                user_name: user_wide.as_mut_ptr(),
            };
            let success = unsafe { cred_write_w(&raw const credential, 0) };
            blob.fill(0);
            if success == 0 {
                let error = unsafe { get_last_error() };
                return Err(format!("CRED_WRITE_FAILED:{error}"));
            }
            Ok(())
        }

        fn read_entry(target: &str) -> Result<Option<[u8; 32]>, String> {
            let _lock = acquire_vault_lock().ok_or_else(|| "VAULT_BUSY".to_owned())?;
            let wide_target = wide(target);
            let mut pointer = core::ptr::null_mut::<CredentialW>();
            let success = unsafe {
                cred_read_w(wide_target.as_ptr(), CRED_TYPE_GENERIC, 0, &raw mut pointer)
            };
            if success == 0 {
                let error = unsafe { get_last_error() };
                return if error == ERROR_NOT_FOUND {
                    Ok(None)
                } else {
                    Err(format!("CRED_READ_FAILED:{error}"))
                };
            }
            if pointer.is_null() {
                return Err("CRED_READBACK_INVALID".to_owned());
            }
            let allocation = CredentialAllocation(pointer);
            let credential = unsafe { &*allocation.0 };
            let size = usize::try_from(credential.credential_blob_size)
                .map_err(|_| "CRED_READBACK_INVALID".to_owned())?;
            if credential.credential_type != CRED_TYPE_GENERIC
                || size != 32
                || credential.credential_blob.is_null()
            {
                return Err("CRED_READBACK_INVALID".to_owned());
            }
            let mut out = [0_u8; 32];
            unsafe {
                core::ptr::copy_nonoverlapping(credential.credential_blob, out.as_mut_ptr(), 32);
            }
            drop(allocation);
            Ok(Some(out))
        }

        fn delete_entry_best_effort(target: &str) -> bool {
            for attempt in 0..16_u32 {
                // Scope the lock to the delete only: verification re-acquires
                // the same non-reentrant mutex below.
                {
                    let _lock = acquire_vault_lock();
                    let wide_target = wide(target);
                    unsafe {
                        let _ = cred_delete_w(wide_target.as_ptr(), CRED_TYPE_GENERIC, 0);
                    }
                }
                if Self::read_entry(target).is_ok_and(|entry| entry.is_none()) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(10_u64 << attempt.min(7)));
            }
            false
        }

        /// Deletes every entry this vault created, verifying absence.
        /// Best-effort and never panics; reports unverified targets.
        pub fn cleanup(&mut self) {
            let targets = core::mem::take(&mut self.created);
            for target in &targets {
                if !Self::delete_entry_best_effort(target) {
                    eprintln!(
                        "ELIOT_TEST_CLEANUP: pairing credential still present: target={target}"
                    );
                }
            }
        }

        pub fn created_targets(&self) -> &[String] {
            &self.created
        }
    }

    impl Drop for NativePairingVault {
        fn drop(&mut self) {
            self.cleanup();
        }
    }

    impl PairingVault for NativePairingVault {
        fn generate_key(&mut self) -> Result<zeroize::Zeroizing<[u8; 32]>, SecretCompositionError> {
            self.draws = self
                .draws
                .checked_add(1)
                .ok_or(SecretCompositionError::ContractExhausted)?;
            let mut key = [0_u8; 32];
            let status = unsafe {
                bcrypt_gen_random(
                    core::ptr::null_mut(),
                    key.as_mut_ptr(),
                    32,
                    BCRYPT_USE_SYSTEM_PREFERRED_RNG,
                )
            };
            if status < 0 {
                key.fill(0);
                return Err(SecretCompositionError::VaultUnavailable);
            }
            // Mix the counter so two draws in the same test never collide
            // even if the RNG repeated (defense in depth, still OS-random).
            let mix = u8::try_from(self.draws & 0xFF).unwrap_or(0).max(1);
            key[0] ^= mix;
            if key.iter().all(|byte| *byte == 0) {
                key[0] = 1;
            }
            Ok(zeroize::Zeroizing::new(key))
        }

        fn store_blob(
            &mut self,
            id: &OpaqueId,
            key: &[u8; 32],
        ) -> Result<VaultWriteEvidence, SecretCompositionError> {
            let target = self.target_for(id);
            // Bounded write+verify: a content mismatch fails closed
            // immediately and is never papered over by another attempt.
            let mut last = SecretCompositionError::VaultWriteFailed;
            for _ in 0..8_u32 {
                match Self::write_entry(&target, key) {
                    Ok(()) => match Self::read_entry(&target) {
                        Ok(Some(observed)) => {
                            let mut mismatch = 0_u8;
                            for (left, right) in key.iter().zip(observed.iter()) {
                                mismatch |= left ^ right;
                            }
                            let mut observed = observed;
                            observed.fill(0);
                            if mismatch != 0 {
                                return Err(SecretCompositionError::ReadbackMismatch);
                            }
                            self.track(&target);
                            let digest = Blake3Digest32::from_bytes(*blake3::hash(key).as_bytes());
                            return Ok(VaultWriteEvidence {
                                blob_digest: digest,
                                receipt: VaultWriteEvidence::receipt_for(digest)?,
                            });
                        }
                        Ok(None) => {
                            last = SecretCompositionError::VaultWriteFailed;
                        }
                        Err(_) => {
                            last = SecretCompositionError::VaultReadFailed;
                        }
                    },
                    Err(_) => {
                        last = SecretCompositionError::VaultWriteFailed;
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(last)
        }

        fn load_blob(&mut self, id: &OpaqueId) -> Result<Option<Vec<u8>>, SecretCompositionError> {
            let target = self.target_for(id);
            Self::read_entry(&target)
                .map(|option| option.map(|key| key.to_vec()))
                .map_err(|_| SecretCompositionError::VaultReadFailed)
        }

        fn remove_blob(&mut self, id: &OpaqueId) -> Result<(), SecretCompositionError> {
            let target = self.target_for(id);
            let _lock = acquire_vault_lock().ok_or(SecretCompositionError::VaultWriteFailed)?;
            let wide_target = wide(&target);
            unsafe {
                let _ = cred_delete_w(wide_target.as_ptr(), CRED_TYPE_GENERIC, 0);
            }
            Ok(())
        }

        fn is_os_backed(&self) -> bool {
            true
        }
    }

    struct NativeScratch {
        base: PathBuf,
        tree: common::RevisionKeyTreeGuard,
        tag: String,
    }

    impl NativeScratch {
        fn new() -> Self {
            let tag = run_tag();
            let base = std::env::temp_dir().join(format!("eliot-pairing-{tag}"));
            fs::create_dir_all(&base).expect("scratch");
            let tree = common::RevisionKeyTreeGuard::for_tree(&base);
            Self { base, tree, tag }
        }

        fn cmdkey_hits_for_tag(&self) -> usize {
            let output = std::process::Command::new("cmdkey")
                .arg("/list")
                .output()
                .expect("cmdkey /list runs on Windows");
            assert!(output.status.success(), "cmdkey /list must succeed");
            let text = String::from_utf8_lossy(&output.stdout);
            text.lines()
                .filter(|line| line.contains(self.tag.as_str()))
                .count()
        }
    }

    impl Drop for NativeScratch {
        fn drop(&mut self) {
            self.tree.cleanup();
            let base = self.base.clone();
            let _ = fs::remove_dir_all(&base);
        }
    }

    const NOW_TICKS: u64 = super::NOW_TICKS;

    #[test]
    fn native_credential_lifecycle_with_cleanup_leaves_nothing_behind() {
        let scratch = NativeScratch::new();
        // Pre-run hygiene: no stale entry may carry our fresh unique tag.
        assert_eq!(scratch.cmdkey_hits_for_tag(), 0, "fresh tag must be absent");
        let mut vault = NativePairingVault::new(scratch.tag.clone());
        assert!(vault.is_os_backed());
        let mut composer = PairingSecretComposer::new(test_binding()).expect("composer");
        // Provision through the real vault.
        composer
            .provision(
                &mut vault,
                fresh_operation("provision", &super::test_nonce(0xD1)).expect("operation"),
            )
            .expect("native provision");
        let now = MonotonicInstant::from_ticks(NOW_TICKS);
        let first = composer
            .with_pairing_key(&mut vault, now, |key| *key)
            .expect("lease");
        assert!(!first.iter().all(|byte| *byte == 0));
        // Rotation replaces the durable key; the old one stops proving.
        assert!(matches!(
            composer
                .rotate(
                    &mut vault,
                    &fresh_operation("rotate", &super::test_nonce(0xD2)).expect("operation")
                )
                .expect("native rotate"),
            MutationOutcome::Committed(_)
        ));
        let second = composer
            .with_pairing_key(&mut vault, now, |key| *key)
            .expect("lease");
        assert_ne!(first, second);
        assert_ne!(
            derive_binding_digest(&first),
            derive_binding_digest(&second)
        );
        // Revocation proves absence by exact readback.
        assert!(matches!(
            composer
                .revoke(
                    &mut vault,
                    &fresh_operation("revoke", &super::test_nonce(0xD3)).expect("operation")
                )
                .expect("native revoke"),
            MutationOutcome::Committed(_)
        ));
        assert!(composer.absence_verified(&mut vault).expect("absence"));
        // Explicit cleanup, then the vault-level and cmdkey-level readbacks
        // must both report zero entries for our tag: cmdkey 0/0.
        let created = vault.created_targets().to_vec();
        assert!(!created.is_empty(), "native entries were created");
        vault.cleanup();
        for target in &created {
            let wide_target: Vec<u16> = target.encode_utf16().chain(core::iter::once(0)).collect();
            let mut pointer = core::ptr::null_mut::<CredentialW>();
            let found = unsafe {
                cred_read_w(wide_target.as_ptr(), CRED_TYPE_GENERIC, 0, &raw mut pointer)
            };
            // Zeroize-before-free even on the failure path: the allocation
            // wrapper owns the pointer and releases it on drop.
            let _allocation = (!pointer.is_null()).then(|| CredentialAllocation(pointer));
            assert_eq!(found, 0, "credential still present after cleanup: {target}");
            let error = unsafe { get_last_error() };
            assert_eq!(
                error, ERROR_NOT_FOUND,
                "unexpected vault state for {target}"
            );
        }
        assert_eq!(
            scratch.cmdkey_hits_for_tag(),
            0,
            "cmdkey must list zero entries for our tag"
        );
    }

    #[test]
    fn native_vault_loss_is_detected_not_relabelled() {
        let scratch = NativeScratch::new();
        let mut vault = NativePairingVault::new(scratch.tag.clone());
        let mut composer = PairingSecretComposer::new(test_binding()).expect("composer");
        composer
            .provision(
                &mut vault,
                fresh_operation("provision", &super::test_nonce(0xE4)).expect("operation"),
            )
            .expect("provision");
        // External loss: delete behind the composer's back, then prove the
        // composer detects it instead of issuing a lease.
        let id = composer.active_id().expect("active").clone();
        let target = vault.target_for(&id);
        assert!(NativePairingVault::delete_entry_best_effort(&target));
        assert!(matches!(
            composer.issue_lease(&mut vault, MonotonicInstant::from_ticks(NOW_TICKS), 60_000),
            Err(SecretCompositionError::EvidenceMissing)
        ));
        vault.cleanup();
        assert_eq!(scratch.cmdkey_hits_for_tag(), 0);
    }
}

#[cfg(not(windows))]
#[test]
fn non_windows_has_no_os_vault_and_says_so() {
    // Native Credential Manager coverage requires Windows (first qualified
    // runtime). Off Windows only the explicit memory seam exists, and it
    // advertises no OS backing.
    assert!(!MemoryPairingVault::new().is_os_backed());
}
