use std::fmt::Write as _;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use qdrant_client::Qdrant;
use sha2::{Digest, Sha256};

use crate::qualified::{QUALIFIED_EXE_BYTES, QUALIFIED_EXE_SHA256_HEX};

use super::LiveError;

/// Pinned native server under qualification.
pub const NATIVE_EXE_PATH: &str = r"C:\Tools\Qdrant\1.19.0\qdrant.exe";

/// Loopback-only endpoint. Construction rejects anything that is not an
/// explicit loopback host before any socket is opened.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveEndpoint {
    host: String,
    http_port: u16,
    grpc_port: u16,
}

impl LiveEndpoint {
    /// Builds an endpoint pair on one host port, rejecting non-loopback hosts.
    pub fn grpc(host: &str, grpc_port: u16) -> Result<Self, LiveError> {
        Self::loopback(host, grpc_port, grpc_port)
    }

    /// Builds an endpoint pair with distinct HTTP/gRPC ports.
    pub fn loopback(
        host: &str,
        http_port: u16,
        grpc_port: u16,
    ) -> Result<Self, LiveError> {
        let normalized = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();
        let is_loopback = normalized == "127.0.0.1"
            || normalized == "::1"
            || normalized == "localhost";
        if !is_loopback {
            return Err(LiveError::EndpointNotLoopback);
        }
        Ok(Self {
            host: normalized,
            http_port,
            grpc_port,
        })
    }

    /// gRPC URL of the disposable server.
    #[must_use]
    pub fn grpc_url(&self) -> String {
        format!("http://{}:{}", self.host, self.grpc_port)
    }

    /// Reserved HTTP port (server config evidence).
    #[must_use]
    pub const fn http_port(&self) -> u16 {
        self.http_port
    }

    /// Reserved gRPC port.
    #[must_use]
    pub const fn grpc_port(&self) -> u16 {
        self.grpc_port
    }
}

/// Reserves two distinct free loopback ports from the OS.
pub fn free_loopback_ports() -> Result<(u16, u16), LiveError> {
    let http = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .local_addr()
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .port();
    let grpc = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .local_addr()
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .port();
    if http == 0 || grpc == 0 || http == grpc {
        return Err(LiveError::EndpointNotLoopback);
    }
    Ok((http, grpc))
}

/// Measures the executable and verifies the exact qualified identity before
/// any process starts. A wrong hash or size fails here; the server is never
/// spawned from unqualified bytes.
pub fn verify_executable(path: &str) -> Result<(), LiveError> {
    let metadata =
        std::fs::metadata(path).map_err(|_| LiveError::ExecutableUnreadable)?;
    if metadata.len() != QUALIFIED_EXE_BYTES {
        return Err(LiveError::ArtifactSizeMismatch);
    }
    let mut file =
        std::fs::File::open(path).map_err(|_| LiveError::ExecutableUnreadable)?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0_u8; 65_536];
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|_| LiveError::ExecutableUnreadable)?;
        if read == 0 {
            break;
        }
        hasher.update(&chunk[..read]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        for nibble in [byte >> 4, byte & 0x0F] {
            hex.push(char::from_digit(u32::from(nibble), 16).unwrap_or('?'));
        }
    }
    if hex.to_ascii_uppercase() != QUALIFIED_EXE_SHA256_HEX {
        return Err(LiveError::ArtifactDigestMismatch);
    }
    Ok(())
}

/// A spawned disposable server. [`Drop`] kills the child and removes the
/// storage directory (best effort); qualification storage never escapes the
/// temp directory.
pub struct DisposableServer {
    child: Child,
    dir: PathBuf,
    endpoint: LiveEndpoint,
}

impl DisposableServer {
    /// Reserved loopback endpoint of the child.
    #[must_use]
    pub const fn endpoint(&self) -> &LiveEndpoint {
        &self.endpoint
    }

    /// Disposable storage directory (removed on drop).
    #[must_use]
    pub fn storage_dir(&self) -> &std::path::Path {
        &self.dir
    }

    /// Child process ID for evidence lines.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.child.id()
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
    let dir = std::env::temp_dir().join(format!(
        "eliot-t22-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("storage"))
        .map_err(|_| LiveError::StorageSetupFailed)?;
    let config = format!(
        "storage:\n  storage_path: ./storage\nservice:\n  host: 127.0.0.1\n  http_port: \
         {http_port}\n  grpc_port: {grpc_port}\n"
    );
    std::fs::write(dir.join("config.yaml"), config)
        .map_err(|_| LiveError::StorageSetupFailed)?;
    let stdout = std::fs::File::create(dir.join("qdrant-out.log"))
        .map_err(|_| LiveError::StorageSetupFailed)?;
    let stderr = std::fs::File::create(dir.join("qdrant-err.log"))
        .map_err(|_| LiveError::StorageSetupFailed)?;
    let child = Command::new(exe_path)
        .arg("--config-path")
        .arg(dir.join("config.yaml"))
        .current_dir(&dir)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|_| LiveError::SpawnFailed)?;
    let mut server = DisposableServer {
        child,
        dir,
        endpoint,
    };
    let client = connect(server.endpoint())?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        if client.health_check().await.is_ok() {
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

fn read_log_tail(dir: &std::path::Path) -> String {
    let mut combined = String::new();
    for name in ["qdrant-out.log", "qdrant-err.log"] {
        if let Ok(content) = std::fs::read_to_string(dir.join(name)) {
            let start = content.len().saturating_sub(1024);
            let _ = write!(
                combined,
                "--- {name} (tail) ---\n{}\n",
                &content[start..]
            );
        }
    }
    combined
}

pub(super) fn connect(endpoint: &LiveEndpoint) -> Result<Qdrant, LiveError> {
    // The client's own compatibility check is warn-only and tolerates ±1
    // minor, so it is skipped: the bridge enforces the exact qualified pair in
    // `probe_server_identity` and `QualifiedGate::admit` instead.
    Qdrant::from_url(&endpoint.grpc_url())
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .skip_compatibility_check()
        .build()
        .map_err(|_| LiveError::TransportFailed)
}
