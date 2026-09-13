//! Qualification-only child lifecycle and disposable storage orchestration.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::artifact::verify_executable;
use super::diagnostics::read_log_tail;
use super::endpoint::{LiveEndpoint, connect};
use super::super::LiveError;

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
    pub fn storage_dir(&self) -> &Path {
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
