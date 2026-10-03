//! Handle-backed Qdrant readiness, exit observation, recovery, and shutdown.
//!
//! Every post-create operation retains the native Job/process guard. Readiness
//! is tied to its saved loopback endpoint and the same opaque API-key lease
//! used for launch. Shutdown receipts can only be made by the private adapter
//! after process-tree and endpoint readback.

use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::launch::{ArgvSnapshot, SpawnOutcome, SpawnedChild};
use crate::secret::{QdrantSecretLease, SecretLeaseBinding};
use crate::win32::NativeProcess;
use crate::{
    ContainmentReport, ExitObservation, ProcessIdentity, ProcessReadiness, QdrantEndpointIdentity,
    ShutdownReceipt, StartRecoveryReceipt, SupervisorError,
};

/// Poll interval while waiting for readiness.
pub const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(200);
/// Per-attempt socket timeout for readiness probes.
pub const PROBE_SOCKET_TIMEOUT: Duration = Duration::from_secs(2);
/// Cap for a single probe response.
pub const MAX_PROBE_RESPONSE_BYTES: usize = 65_536;

/// Owned Qdrant process tree and its launch evidence.
///
/// No constructor accepts a PID, endpoint, or caller-made containment proof.
/// The only production constructor consumes the guard returned by
/// spawn_qualified.
pub struct OwnedChild {
    native: NativeProcess,
    argv_snapshot: ArgvSnapshot,
    containment: ContainmentReport,
    identity: Option<ProcessIdentity>,
}

impl From<SpawnedChild> for OwnedChild {
    fn from(spawned: SpawnedChild) -> Self {
        let containment = spawned.containment_report();
        let argv_snapshot = spawned.argv_snapshot().clone();
        Self {
            native: spawned.into_native(),
            argv_snapshot,
            containment,
            identity: None,
        }
    }
}

impl OwnedChild {
    /// Converts a spawn result while retaining its ambiguous-outcome reason.
    #[must_use]
    pub fn from_spawn_outcome(outcome: SpawnOutcome) -> (Self, Option<SupervisorError>) {
        let (guard, reason) = outcome.into_parts();
        (Self::from(guard), reason)
    }

    /// Content-free argument snapshot; never contains the API key.
    #[must_use]
    pub const fn argv_snapshot(&self) -> &ArgvSnapshot {
        &self.argv_snapshot
    }

    /// Windows containment was established internally before process resume.
    #[must_use]
    pub const fn containment_report(&self) -> ContainmentReport {
        self.containment
    }

    /// Re-reads process, executable, Job membership, owner, endpoint, and
    /// lease binding from the handles owned by this guard.
    pub fn verify_identity(&mut self) -> Result<ProcessIdentity, SupervisorError> {
        let observed = self.native.identity()?;
        if self.identity.is_some_and(|prior| prior != observed) {
            return Err(SupervisorError::ProcessIdentityMismatch);
        }
        self.identity = Some(observed);
        Ok(observed)
    }

    /// Waits for authenticated Qdrant readiness at the endpoint sealed into
    /// this guard. Cancellation and timeout preserve this guard and report an
    /// unknown startup outcome for caller-directed recovery.
    pub fn wait_ready(
        &mut self,
        lease: &impl QdrantSecretLease,
        now_tick: NonZeroU64,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<ProcessReadiness, SupervisorError> {
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err(SupervisorError::StartupOutcomeUnknown);
            }
            validate_live_lease(&self.native, lease, now_tick)?;
            if !self.native.is_running()? {
                return Err(SupervisorError::ProcessNotReady);
            }
            let before_probe = self.verify_identity()?;
            let endpoint = before_probe.endpoint();
            if probe_once(endpoint, self.native.secret(), deadline) {
                if cancel.load(Ordering::Acquire) {
                    return Err(SupervisorError::StartupOutcomeUnknown);
                }
                validate_live_lease(&self.native, lease, now_tick)?;
                let after_probe = self.verify_identity()?;
                if before_probe != after_probe {
                    return Err(SupervisorError::ProcessIdentityMismatch);
                }
                return Ok(ProcessReadiness::from_owned_process(
                    after_probe,
                    self.native.config_digest(),
                ));
            }
            if Instant::now() >= deadline {
                return Err(SupervisorError::StartupOutcomeUnknown);
            }
            std::thread::sleep(
                READINESS_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    }

    /// Resolves a CreateProcessW return whose outcome was ambiguous and had
    /// no process handle. The native guard remains owned by this value.
    pub fn recover_ambiguous_start(
        &mut self,
        deadline: Instant,
    ) -> Result<StartRecoveryReceipt, SupervisorError> {
        self.native.recover_ambiguous_start(deadline)
    }

    /// Captures an exit only after root-handle and Job accounting readback.
    /// A live root returns None. A nonempty Job is preserved as typed data so
    /// the lifecycle owner can quarantine instead of silently restarting.
    pub fn observe_exit(
        &mut self,
        expected_shutdown: bool,
        observed_tick: NonZeroU64,
    ) -> Result<Option<ExitObservation>, SupervisorError> {
        if !self.native.has_process_handle() {
            return Err(SupervisorError::StartupOutcomeUnknown);
        }
        if self.native.is_running()? {
            return Ok(None);
        }
        let identity = match self.identity {
            Some(identity) => {
                self.native.verify_root_handle(identity)?;
                identity
            }
            None => self.verify_identity()?,
        };
        let exit_code = self
            .native
            .try_exit_code()?
            .ok_or(SupervisorError::ProcessIdentityMismatch)?;
        let job_empty = self.native.job_empty()?;
        Ok(Some(ExitObservation::from_process_handle(
            identity,
            expected_shutdown,
            exit_code,
            observed_tick,
            job_empty,
        )))
    }

    /// Performs graceful signal, bounded Job termination, and verified
    /// absence checks using only the endpoint saved in the native guard.
    pub fn shutdown_bounded(
        &mut self,
        deadline: Instant,
    ) -> Result<ShutdownReceipt, SupervisorError> {
        if !self.native.has_process_handle() {
            return Err(SupervisorError::StartupOutcomeUnknown);
        }
        let identity = match self.identity {
            Some(identity) => identity,
            None => match self.native.identity() {
                Ok(identity) => {
                    self.identity = Some(identity);
                    identity
                }
                Err(identity_error) => {
                    self.native.terminate_unverified_tree(deadline)?;
                    return Err(identity_error);
                }
            },
        };
        self.native.shutdown_tree(identity, deadline)
    }
}

fn validate_live_lease(
    native: &NativeProcess,
    lease: &impl QdrantSecretLease,
    now_tick: NonZeroU64,
) -> Result<(), SupervisorError> {
    let binding = SecretLeaseBinding::from_lease(lease);
    if binding != native.secret().binding()
        || binding.purpose_digest() != native.expected_secret_purpose()
        || binding.expires_at_tick() <= now_tick
    {
        return Err(SupervisorError::SecretLeaseInvalid);
    }
    let exact_bytes =
        lease.with_secret_bytes(|bytes| constant_time_equal(bytes, native.secret().secret_bytes()));
    if exact_bytes {
        Ok(())
    } else {
        Err(SupervisorError::SecretLeaseInvalid)
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (*left ^ *right)
        })
        == 0
}

/// Authenticated readyz and collections checks, plus an unauthenticated
/// collections request that must be rejected.
fn probe_once(
    endpoint: QdrantEndpointIdentity,
    secret: &crate::SecretMaterial,
    deadline: Instant,
) -> bool {
    let http_port = endpoint.http_port().get();
    let address = match endpoint.host() {
        crate::LoopbackHost::V4 => SocketAddr::from((Ipv4Addr::LOCALHOST, http_port)),
        crate::LoopbackHost::V6 => SocketAddr::from((Ipv6Addr::LOCALHOST, http_port)),
    };
    let key = secret.secret_bytes();
    let readyz = http_get(&address, "/readyz", Some(key), deadline);
    let collections = http_get(&address, "/collections", Some(key), deadline);
    let unauthenticated = http_get(&address, "/collections", None, deadline);
    match (readyz, collections, unauthenticated) {
        (Some(readyz), Some(collections), Some(unauthenticated)) => {
            readyz.status == 200
                && readyz.body.to_ascii_lowercase().contains("ready")
                && collections.status == 200
                && matches!(unauthenticated.status, 401 | 403)
        }
        _ => false,
    }
}

fn probe_timeout(deadline: Instant) -> Duration {
    PROBE_SOCKET_TIMEOUT.min(deadline.saturating_duration_since(Instant::now()))
}

struct ProbeResponse {
    status: u16,
    body: String,
}

/// Minimal HTTP/1.0 GET. Key bytes stay in a bounded request buffer which is
/// cleared before return and are never formatted or logged.
fn http_get(
    address: &SocketAddr,
    path: &str,
    key: Option<&[u8]>,
    deadline: Instant,
) -> Option<ProbeResponse> {
    let timeout = probe_timeout(deadline);
    if timeout.is_zero() {
        return None;
    }
    let mut stream = TcpStream::connect_timeout(address, timeout).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let mut request = Vec::with_capacity(256 + key.map_or(0, |key| key.len()));
    request.extend_from_slice(b"GET ");
    request.extend_from_slice(path.as_bytes());
    request.extend_from_slice(b" HTTP/1.0\r\nHost: ");
    request.extend_from_slice(address.to_string().as_bytes());
    request.extend_from_slice(b"\r\n");
    if let Some(key) = key {
        request.extend_from_slice(b"api-key: ");
        request.extend_from_slice(key);
        request.extend_from_slice(b"\r\n");
    }
    request.extend_from_slice(b"Connection: close\r\n\r\n");
    if Instant::now() >= deadline {
        request.fill(0);
        return None;
    }
    let write_ok = stream.write_all(&request).is_ok();
    request.fill(0);
    core::hint::black_box(request.as_mut_ptr());
    if !write_ok {
        return None;
    }
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let remaining = probe_timeout(deadline);
        if remaining.is_zero() {
            raw.fill(0);
            return None;
        }
        stream.set_read_timeout(Some(remaining)).ok()?;
        match stream.read(&mut chunk) {
            Ok(read) if read > 0 => {
                if raw.len().saturating_add(read) > MAX_PROBE_RESPONSE_BYTES {
                    raw.fill(0);
                    return None;
                }
                raw.extend_from_slice(&chunk[..read]);
            }
            _ => break,
        }
    }
    let response = parse_response(&raw);
    raw.fill(0);
    response
}

fn parse_response(raw: &[u8]) -> Option<ProbeResponse> {
    let text = core::str::from_utf8(raw).ok()?;
    let (headers, body) = text.split_once("\r\n\r\n")?;
    let status_line = headers.lines().next()?;
    let mut parts = status_line.split_whitespace();
    if !parts.next()?.starts_with("HTTP/") {
        return None;
    }
    let status = parts.next()?.parse().ok()?;
    Some(ProbeResponse {
        status,
        body: body.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_response;

    #[test]
    fn parser_requires_a_complete_http_header_boundary() {
        assert_eq!(
            parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nready")
                .expect("response")
                .body,
            "ready"
        );
        assert!(parse_response(b"HTTP/1.1 200 OK\r\n").is_none());
        assert!(parse_response(b"garbage\r\n\r\nbody").is_none());
    }
}
