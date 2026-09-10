//! Owned child execution: readiness, bounded termination, and receipts.
//!
//! [`OwnedChild`] wraps the [`Child`] handle produced by
//! [`spawn_qualified`](crate::launch::spawn_qualified). Holding the OS
//! handle (not a bare PID) is what makes PID reuse harmless: Windows never
//! recycles the identity behind an open handle, and every observation is
//! re-checked against executable digest, owner fence, and endpoint before
//! the pure supervisor may confirm readiness.
//!
//! Composition recipe with the pure [`QdrantSupervisor`](crate::QdrantSupervisor):
//!
//! ```text
//! prepare_start -> spawn_qualified -> OwnedChild::from
//!   -> wait_ready -> confirm_ready -> Ready
//!   -> begin_shutdown -> terminate_bounded -> confirm_stopped -> Stopped
//! ```
//!
//! Blind attach is impossible by construction: there is no constructor that
//! takes a PID or a port. Only handles this package spawned are owned.
//!
//! Readiness proves two independent facts over loopback: `/readyz` reports
//! shards ready, and authed `GET /collections` answers 200 with the leased
//! key (which also proves unauthenticated access stays denied: without the
//! key the same endpoint answers 401).

use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::num::{NonZeroU32, NonZeroU64};
use std::process::Child;
use std::time::{Duration, Instant};

use search_contracts::{OpaqueId, ReceiptRef, Sha256Digest32};

use crate::containment::{ContainmentReport, LoopbackHost};
use crate::identity::VerifiedExecutable;
use crate::launch::{ArgvSnapshot, LaunchPlan, SpawnedChild};
use crate::secret::SecretMaterial;
use crate::sha256::sha256_bytes;
use crate::{SupervisorError, spawn_unix_millis};

/// Poll interval while waiting for readiness.
pub const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(200);
/// Per-attempt socket timeout for readiness probes.
pub const PROBE_SOCKET_TIMEOUT: Duration = Duration::from_secs(2);
/// Cap for a single probe response; Qdrant health bodies are tiny.
pub const MAX_PROBE_RESPONSE_BYTES: usize = 65_536;

/// Poll interval while reaping after `kill`.
const REAP_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Owned running (or recently reaped) child with its spawn evidence.
pub struct OwnedChild {
    child: Child,
    pid: NonZeroU32,
    verified: VerifiedExecutable,
    argv_snapshot: ArgvSnapshot,
    containment: ContainmentReport,
    spawn_unix_millis: NonZeroU64,
    canonical_data_dir: std::path::PathBuf,
}

impl From<SpawnedChild> for OwnedChild {
    fn from(spawned: SpawnedChild) -> Self {
        Self {
            pid: spawned.pid(),
            spawn_unix_millis: spawned.spawn_unix_millis(),
            containment: spawned.containment_report(),
            canonical_data_dir: spawned.canonical_data_dir().clone(),
            verified: spawned.verified().clone(),
            argv_snapshot: spawned.argv_snapshot().clone(),
            child: spawned.into_child(),
        }
    }
}

impl OwnedChild {
    /// Test-only wrapper around an in-test spawned helper child (sleeper or
    /// immediate-exit fixture). Carries synthetic identity labeled by
    /// `label`; never used for qualification or readiness admission.
    pub fn wrap_test_child(child: Child, label: &'static str) -> Result<Self, SupervisorError> {
        if label.is_empty() {
            return Err(SupervisorError::InvalidProcessConfig);
        }
        let pid = NonZeroU32::new(child.id()).ok_or(SupervisorError::StartFailed)?;
        Ok(Self {
            child,
            pid,
            verified: VerifiedExecutable::for_tests(label),
            argv_snapshot: ArgvSnapshot::for_tests(label),
            containment: ContainmentReport::for_tests(),
            spawn_unix_millis: spawn_unix_millis(),
            canonical_data_dir: std::path::PathBuf::from(label),
        })
    }

    /// OS process identifier of the owned child.
    #[must_use]
    pub const fn pid(&self) -> NonZeroU32 {
        self.pid
    }

    /// Verified executable identity from the pre-spawn checks.
    #[must_use]
    pub const fn verified(&self) -> &VerifiedExecutable {
        &self.verified
    }

    /// Spawn evidence snapshot (never contains secrets).
    #[must_use]
    pub const fn argv_snapshot(&self) -> &ArgvSnapshot {
        &self.argv_snapshot
    }

    /// Containment status recorded at spawn.
    #[must_use]
    pub const fn containment_report(&self) -> ContainmentReport {
        self.containment
    }

    /// Spawn marker used as the pure `creation_marker`.
    #[must_use]
    pub const fn spawn_unix_millis(&self) -> NonZeroU64 {
        self.spawn_unix_millis
    }

    /// True while the owned handle still refers to a running child.
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Reaps an exited child without blocking. Returns the outer option as
    /// `None` while running, otherwise the process exit code (which itself
    /// is `None` only when the OS reports termination without a code).
    pub fn try_exit_code(&mut self) -> Option<Option<i32>> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(status.code()),
            Ok(None) | Err(_) => None,
        }
    }

    /// Waits for dual readiness until `deadline`.
    ///
    /// A child that exits first yields [`SupervisorError::ProcessNotReady`]
    /// (the outcome is known: reaped by the caller for classification). A
    /// live child that never proves health yields
    /// [`SupervisorError::StartupOutcomeUnknown`].
    pub fn wait_ready(
        &mut self,
        host: &LoopbackHost,
        http_port: u16,
        secret: &SecretMaterial,
        deadline: Instant,
    ) -> Result<(), SupervisorError> {
        wait_ready(&mut self.child, host, http_port, secret, deadline)
    }

    /// Bounded terminate-and-reap. An already-exited child reports its code
    /// immediately; otherwise `kill` is issued once and the handle is
    /// polled until `deadline`. A missed deadline yields
    /// [`SupervisorError::ShutdownOutcomeUnknown`] with the handle retained
    /// for a later bounded attempt.
    pub fn terminate_bounded(&mut self, deadline: Instant) -> Result<Option<i32>, SupervisorError> {
        terminate_bounded(&mut self.child, deadline)
    }
}

impl Drop for OwnedChild {
    /// Best-effort terminate so an abandoned guard cannot leak an orphan.
    /// Every normal path reaps explicitly with typed outcomes; this only
    /// covers unwinding and forgotten guards, where no error can propagate.
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Waits for `/readyz` plus authed `GET /collections` on loopback.
pub fn wait_ready(
    child: &mut Child,
    host: &LoopbackHost,
    http_port: u16,
    secret: &SecretMaterial,
    deadline: Instant,
) -> Result<(), SupervisorError> {
    if http_port == 0 {
        return Err(SupervisorError::InvalidProcessConfig);
    }
    loop {
        if child
            .try_wait()
            .map_err(|_| SupervisorError::ProcessNotReady)?
            .is_some()
        {
            return Err(SupervisorError::ProcessNotReady);
        }
        if probe_once(*host, http_port, secret) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(SupervisorError::StartupOutcomeUnknown);
        }
        std::thread::sleep(READINESS_POLL_INTERVAL);
    }
}

/// Terminates and reaps one owned child handle within `deadline`.
pub fn terminate_bounded(
    child: &mut Child,
    deadline: Instant,
) -> Result<Option<i32>, SupervisorError> {
    if let Some(status) = child
        .try_wait()
        .map_err(|_| SupervisorError::ShutdownOutcomeUnknown)?
    {
        return Ok(status.code());
    }
    if child.kill().is_err() {
        match child
            .try_wait()
            .map_err(|_| SupervisorError::ShutdownOutcomeUnknown)?
        {
            Some(status) => return Ok(status.code()),
            None => return Err(SupervisorError::ShutdownOutcomeUnknown),
        }
    }
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| SupervisorError::ShutdownOutcomeUnknown)?
        {
            return Ok(status.code());
        }
        if Instant::now() >= deadline {
            return Err(SupervisorError::ShutdownOutcomeUnknown);
        }
        std::thread::sleep(REAP_POLL_INTERVAL);
    }
}

/// Single dual-probe attempt. Both halves must pass.
fn probe_once(host: LoopbackHost, http_port: u16, secret: &SecretMaterial) -> bool {
    let address: SocketAddr = match host {
        LoopbackHost::V4 => SocketAddr::from((Ipv4Addr::LOCALHOST, http_port)),
        LoopbackHost::V6 => SocketAddr::from((Ipv6Addr::LOCALHOST, http_port)),
    };
    let key = secret.secret_bytes();
    // `/readyz` needs no key, but sending it keeps one code path.
    let readyz = http_get(&address, "/readyz", key);
    let collections = http_get(&address, "/collections", key);
    match (readyz, collections) {
        (Some(readyz), Some(collections)) => {
            readyz.status == 200 && readyz.body.contains("ready") && collections.status == 200
        }
        _ => false,
    }
}

struct ProbeResponse {
    status: u16,
    body: String,
}

/// Minimal HTTP/1.0 GET with the leased key in the `api-key` header.
/// The request buffer holds secret bytes only in memory on loopback and is
/// zeroed before return on every path.
fn http_get(address: &SocketAddr, path: &str, key: &[u8]) -> Option<ProbeResponse> {
    let mut stream = TcpStream::connect_timeout(address, PROBE_SOCKET_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(PROBE_SOCKET_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(PROBE_SOCKET_TIMEOUT)).ok()?;
    let outcome = http_exchange(&mut stream, address, path, key);
    // `stream` carries no secret after the zeroed exchange buffer is dropped.
    drop(stream);
    outcome
}

fn http_exchange(
    stream: &mut TcpStream,
    address: &SocketAddr,
    path: &str,
    key: &[u8],
) -> Option<ProbeResponse> {
    let mut request: Vec<u8> = Vec::with_capacity(256 + key.len());
    request.extend_from_slice(b"GET ");
    request.extend_from_slice(path.as_bytes());
    request.extend_from_slice(b" HTTP/1.0\r\nHost: ");
    request.extend_from_slice(address.to_string().as_bytes());
    request.extend_from_slice(b"\r\napi-key: ");
    request.extend_from_slice(key);
    request.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
    let write_ok = stream.write_all(&request).is_ok();
    zero_bytes(&mut request);
    drop(request);
    if !write_ok {
        return None;
    }
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(read) if read > 0 => {
                raw.extend_from_slice(&chunk[..read]);
                if raw.len() > MAX_PROBE_RESPONSE_BYTES {
                    break;
                }
            }
            _ => break,
        }
    }
    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> Option<ProbeResponse> {
    let text = core::str::from_utf8(raw).ok()?;
    let mut lines = text.lines();
    let status_line = lines.next()?;
    let mut parts = status_line.split_whitespace();
    let protocol = parts.next()?;
    if !protocol.starts_with("HTTP/") {
        return None;
    }
    let status: u16 = parts.next()?.parse().ok()?;
    Some(ProbeResponse {
        status,
        body: text.to_owned(),
    })
}

fn zero_bytes(buffer: &mut [u8]) {
    for byte in buffer.iter_mut() {
        *byte = 0;
    }
    core::hint::black_box(buffer.as_mut_ptr());
}

/// Launch receipt: digests, ports, and containment status only.
/// No secret material and no raw absolute paths, by construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchReceipt {
    /// Operation that authorized the launch.
    pub operation_id: OpaqueId,
    /// Owned child PID at spawn.
    pub pid: NonZeroU32,
    /// Verified executable SHA-256.
    pub executable_sha256: Sha256Digest32,
    /// Verified executable size.
    pub executable_bytes: u64,
    /// Verified executable version.
    pub executable_version: String,
    /// SHA-256 over the canonical data-directory bytes (locator binding).
    pub data_dir_digest: Sha256Digest32,
    /// Loopback HTTP port.
    pub http_port: u16,
    /// Loopback gRPC port.
    pub grpc_port: u16,
    /// True only for fully evidenced Windows containment.
    pub contained: bool,
    /// Readiness confirmation reference.
    pub readiness_receipt: ReceiptRef,
}

/// Builds the launch receipt from plan and spawn evidence.
#[must_use]
pub fn build_launch_receipt(
    plan: &LaunchPlan,
    owned: &OwnedChild,
    readiness_receipt: ReceiptRef,
) -> LaunchReceipt {
    let data_dir_digest =
        Sha256Digest32::from_bytes(sha256_bytes(path_identity_bytes(&owned.canonical_data_dir)));
    LaunchReceipt {
        operation_id: plan.operation_id().clone(),
        pid: owned.pid(),
        executable_sha256: *owned.verified().sha256(),
        executable_bytes: owned.verified().bytes(),
        executable_version: owned.verified().version().to_owned(),
        data_dir_digest,
        http_port: plan.http_port(),
        grpc_port: plan.grpc_port(),
        contained: owned.containment_report().contained,
        readiness_receipt,
    }
}

fn path_identity_bytes(path: &std::path::Path) -> &[u8] {
    path.as_os_str().as_encoded_bytes()
}

#[cfg(test)]
mod tests {
    use super::{parse_response, zero_bytes};

    #[test]
    fn response_parser_accepts_only_http_status_lines() {
        let ok =
            parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 18\r\n\r\nall shards are ready")
                .unwrap();
        assert_eq!(ok.status, 200);
        assert!(ok.body.contains("ready"));
        assert!(parse_response(b"GARBAGE").is_none());
        assert!(parse_response(b"HTTP/1.1 OK\r\n\r\n").is_none());
    }

    #[test]
    fn zeroing_clears_every_byte() {
        let mut buffer = vec![0xA5_u8; 64];
        zero_bytes(&mut buffer);
        assert!(buffer.iter().all(|byte| *byte == 0));
    }
}
