//! Qualification-only child lifecycle and disposable storage orchestration.

use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use qdrant_client::QdrantError;
use search_contracts::{
    ArtifactDigest, Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch,
};

use super::super::LiveError;
use super::artifact::verify_executable;
use super::diagnostics::read_log_tail;
use super::endpoint::{LiveEndpoint, connect, connect_with_api_key};
use crate::{
    BridgeError, QdrantApiKeyLease, QdrantApiKeyLeaseProvider, QdrantConnectionBinding,
    QdrantEndpointIdentity, QdrantLoopbackHost,
};

/// A spawned disposable server. [`Drop`] kills the child and removes the
/// storage directory (best effort); qualification storage never escapes the
/// temp directory.
pub struct DisposableServer {
    child: Child,
    dir: PathBuf,
    endpoint: LiveEndpoint,
    binding: QdrantConnectionBinding,
}

impl DisposableServer {
    /// Reserved loopback endpoint of the child.
    #[must_use]
    pub const fn endpoint(&self) -> &LiveEndpoint {
        &self.endpoint
    }

    /// Disposable storage directory (removed on drop).
    #[must_use]
    pub fn storage_dir(&self) -> &Path {
        &self.dir
    }

    /// Child process ID for evidence lines.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Content-free fixture identity for authenticated client tests.
    ///
    /// This synthetic fixture binding is useful only with this disposable
    /// server. It is not an OS-verification receipt for a production process.
    #[must_use]
    pub const fn fixture_connection_binding(&self) -> QdrantConnectionBinding {
        self.binding
    }

    /// Creates the private fixture's callback-only API-key capability.
    #[must_use]
    pub fn fixture_api_key_lease(&self) -> QdrantApiKeyLease {
        QdrantApiKeyLease::new(
            self.binding,
            FixtureApiKeyLeaseProvider {
                binding: self.binding,
                successful_checks: None,
                checks: AtomicUsize::new(0),
            },
        )
    }

    /// Creates a test-only lease that invalidates after the requested number
    /// of successful callback checks. This permits a live mutation/readback
    /// boundary fault without changing production providers.
    #[must_use]
    pub fn fixture_api_key_lease_expiring_after(
        &self,
        successful_checks: usize,
    ) -> QdrantApiKeyLease {
        QdrantApiKeyLease::new(
            self.binding,
            FixtureApiKeyLeaseProvider {
                binding: self.binding,
                successful_checks: Some(successful_checks),
                checks: AtomicUsize::new(0),
            },
        )
    }

    /// Proves the server rejects both an incorrect key and no key.
    pub async fn verify_authentication_required(&self) -> Result<(), LiveError> {
        for key in [Some(WRONG_FIXTURE_API_KEY), None] {
            let client = connect_with_api_key(&self.endpoint, key)?;
            let result = tokio::time::timeout(Duration::from_secs(5), client.list_collections())
                .await
                .map_err(|_| LiveError::TransportFailed)?;
            match result {
                Err(QdrantError::ResponseError { status })
                    if status.code() as i32 == GRPC_UNAUTHENTICATED_CODE => {}
                Err(_) => return Err(LiveError::TransportFailed),
                Ok(_) => return Err(LiveError::AuthenticationNotEnforced),
            }
        }
        Ok(())
    }
}

impl Drop for DisposableServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        for _ in 0..20 {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Spawns the qualified server on disposable storage with the given loopback
/// ports and waits for gRPC readiness. Only loopback endpoints are accepted.
///
/// This is a qualification fixture, not product process ownership. The
/// production daemon consumes a supervisor-qualified endpoint instead.
pub async fn spawn_disposable_server(
    exe_path: &str,
    http_port: u16,
    grpc_port: u16,
) -> Result<DisposableServer, LiveError> {
    verify_executable(exe_path)?;
    let endpoint = LiveEndpoint::loopback("127.0.0.1", http_port, grpc_port)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let dir = std::env::temp_dir().join(format!("eliot-t22-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(dir.join("storage")).map_err(|_| LiveError::StorageSetupFailed)?;
    let config = format!(
        "storage:\n  storage_path: ./storage\nservice:\n  host: 127.0.0.1\n  http_port: \
         {http_port}\n  grpc_port: {grpc_port}\n"
    );
    std::fs::write(dir.join("config.yaml"), config).map_err(|_| LiveError::StorageSetupFailed)?;
    let stdout = std::fs::File::create(dir.join("qdrant-out.log"))
        .map_err(|_| LiveError::StorageSetupFailed)?;
    let stderr = std::fs::File::create(dir.join("qdrant-err.log"))
        .map_err(|_| LiveError::StorageSetupFailed)?;
    let child = Command::new(exe_path)
        .arg("--config-path")
        .arg(dir.join("config.yaml"))
        .current_dir(&dir)
        .env("QDRANT__SERVICE__API_KEY", crate::live::FIXTURE_API_KEY)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|_| LiveError::SpawnFailed)?;
    let binding = match fixture_connection_binding(endpoint.clone(), child.id(), nanos) {
        Ok(binding) => binding,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&dir);
            return Err(error);
        }
    };
    let mut server = DisposableServer {
        child,
        dir,
        endpoint,
        binding,
    };
    let client = connect(server.endpoint())?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        if client.health_check().await.is_ok() && client.list_collections().await.is_ok() {
            return Ok(server);
        }
        match server.child.try_wait() {
            Ok(Some(status)) => {
                eprintln!("qdrant exited during startup: {status}");
                eprintln!("{}", read_log_tail(&server.dir));
                return Err(LiveError::ServerNotReady);
            }
            Ok(None) => {}
            Err(_) => return Err(LiveError::ServerNotReady),
        }
        if tokio::time::Instant::now() >= deadline {
            eprintln!("qdrant not ready in 60s");
            eprintln!("{}", read_log_tail(&server.dir));
            return Err(LiveError::ServerNotReady);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

const WRONG_FIXTURE_API_KEY: &str = "eliot-qdrant-wrong-fixture-key";
const GRPC_UNAUTHENTICATED_CODE: i32 = 16;

struct FixtureApiKeyLeaseProvider {
    binding: QdrantConnectionBinding,
    successful_checks: Option<usize>,
    checks: AtomicUsize,
}

impl QdrantApiKeyLeaseProvider for FixtureApiKeyLeaseProvider {
    fn monotonic_now_ticks(&self) -> Result<u64, BridgeError> {
        Ok(1)
    }

    fn with_secret(
        &self,
        binding: &QdrantConnectionBinding,
        now_tick: u64,
        callback: &mut dyn FnMut(&[u8]) -> Result<(), BridgeError>,
    ) -> Result<(), BridgeError> {
        if binding != &self.binding {
            return Err(BridgeError::SupervisorReceiptMismatch);
        }
        if now_tick >= binding.secret_expires_at_tick().get() {
            return Err(BridgeError::AuthenticationLeaseExpired);
        }
        if self
            .successful_checks
            .is_some_and(|allowed| self.checks.fetch_add(1, Ordering::SeqCst) >= allowed)
        {
            return Err(BridgeError::AuthenticationLeaseExpired);
        }
        callback(crate::live::FIXTURE_API_KEY.as_bytes())
    }
}

fn fixture_connection_binding(
    endpoint: LiveEndpoint,
    pid: u32,
    creation_tick: u128,
) -> Result<QdrantConnectionBinding, LiveError> {
    let process_id = NonZeroU32::new(pid).ok_or(LiveError::FixtureNotRepresentable)?;
    let marker =
        u64::try_from(creation_tick.max(1)).map_err(|_| LiveError::FixtureNotRepresentable)?;
    let creation_marker = NonZeroU64::new(marker).ok_or(LiveError::FixtureNotRepresentable)?;
    let id_bytes = |tag: u8| {
        let mut bytes = [tag; 16];
        bytes[..4].copy_from_slice(&pid.to_le_bytes());
        bytes[4..12].copy_from_slice(&marker.to_le_bytes());
        bytes[12..16].copy_from_slice(&(!pid).to_le_bytes());
        bytes
    };
    let digest_bytes = |tag: u8| {
        let mut bytes = [tag; 32];
        bytes[..16].copy_from_slice(&id_bytes(tag));
        bytes
    };
    let installation_incarnation_id = InstallationIncarnationId::from_bytes(id_bytes(0x49));
    let data_root_id = DataRootId::from_bytes(id_bytes(0x52));
    let owner_epoch = OwnerEpoch::new(1).map_err(|_| LiveError::FixtureNotRepresentable)?;
    let host = match endpoint.host() {
        "127.0.0.1" => QdrantLoopbackHost::Ipv4,
        "::1" => QdrantLoopbackHost::Ipv6,
        _ => return Err(LiveError::EndpointNotLoopback),
    };
    let endpoint_identity = QdrantEndpointIdentity::new(
        host,
        NonZeroU16::new(endpoint.http_port()).ok_or(LiveError::EndpointNotLoopback)?,
        NonZeroU16::new(endpoint.grpc_port()).ok_or(LiveError::EndpointNotLoopback)?,
        Blake3Digest32::from_bytes(digest_bytes(0x45)),
    );
    QdrantConnectionBinding::new(
        process_id,
        creation_marker,
        data_root_id,
        installation_incarnation_id,
        owner_epoch,
        ArtifactDigest::from_bytes(digest_bytes(0x41)),
        endpoint_identity,
        Blake3Digest32::from_bytes(digest_bytes(0x53)),
        installation_incarnation_id,
        Blake3Digest32::from_bytes(digest_bytes(0x50)),
        NonZeroU64::new(100).ok_or(LiveError::FixtureNotRepresentable)?,
    )
    .map_err(|_| LiveError::FixtureNotRepresentable)
}
