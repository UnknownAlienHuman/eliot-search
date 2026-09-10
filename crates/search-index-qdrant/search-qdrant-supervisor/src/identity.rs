//! Exact executable identity: path, size, SHA-256, and version.
//!
//! The qualified artifact is pinned by `qualification/qdrant/artifact.toml`
//! and `docs/runtime/QDRANT_NATIVE_WINDOWS.md`: Qdrant 1.19.0
//! (`x86_64-pc-windows-msvc`), `84_184_576` bytes, SHA-256
//! `369C562E…EA9D4`. Every spawn verifies size, streams the SHA-256, and
//! runs a bounded `--version` probe before the server may start. Any
//! mismatch, substitution, or inconclusive probe fails closed; the server
//! is never started on suspicion.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use search_contracts::Sha256Digest32;

use crate::SupervisorError;
use crate::sha256::sha256_file;

/// Pinned qualified Qdrant version.
pub const QUALIFIED_QDRANT_VERSION: &str = "1.19.0";
/// Pinned qualified executable size in bytes.
pub const QUALIFIED_EXE_BYTES: u64 = 84_184_576;
/// Pinned qualified executable SHA-256 (lowercase hex).
pub const QUALIFIED_EXE_SHA256_HEX: &str =
    "369c562eae3d89333a13abfdb522fa209e3f587c1217a1059d817e80814ea9d4";
/// Probe argument answered by the qualified executable without side effects.
pub const VERSION_PROBE_ARG: &str = "--version";
/// Lower bound for the version-probe deadline.
pub const MIN_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
/// Upper bound for the version-probe deadline.
pub const MAX_PROBE_TIMEOUT: Duration = Duration::from_secs(120);
/// Poll interval while waiting for the version probe to exit.
pub const PROBE_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Expected identity of one executable file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableExpectation {
    path: PathBuf,
    sha256: Sha256Digest32,
    bytes: u64,
    version: String,
}

impl ExecutableExpectation {
    /// Expectation for the pinned qualified artifact at `path`.
    pub fn qualified_default(path: PathBuf) -> Result<Self, SupervisorError> {
        let sha256 = Sha256Digest32::parse_hex(QUALIFIED_EXE_SHA256_HEX)
            .map_err(|_| SupervisorError::InvalidArtifact)?;
        Self::custom(
            path,
            sha256,
            QUALIFIED_EXE_BYTES,
            QUALIFIED_QDRANT_VERSION.to_owned(),
        )
    }

    /// Explicit expectation, used by tests and future qualifications.
    pub fn custom(
        path: PathBuf,
        sha256: Sha256Digest32,
        bytes: u64,
        version: String,
    ) -> Result<Self, SupervisorError> {
        if path.as_os_str().is_empty() || bytes == 0 || !is_plausible_version(&version) {
            return Err(SupervisorError::InvalidArtifact);
        }
        Ok(Self {
            path,
            sha256,
            bytes,
            version,
        })
    }

    /// Test-only version override to prove version mismatches fail closed
    /// against the real executable.
    pub fn set_expected_version_for_tests(&mut self, version: String) {
        self.version = version;
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Accepts non-empty versions up to 128 chars from a closed alphabet.
fn is_plausible_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 128
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// Verified identity returned only after all three checks pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedExecutable {
    canonical_path: PathBuf,
    sha256: Sha256Digest32,
    bytes: u64,
    version: String,
}

impl VerifiedExecutable {
    /// Test-only synthetic identity for lifecycle fixtures (sleepers and
    /// immediate-exit helpers). Never produced by verification and never
    /// accepted for qualification or readiness admission.
    #[must_use]
    pub fn for_tests(label: &'static str) -> Self {
        Self {
            canonical_path: PathBuf::from(label),
            sha256: Sha256Digest32::from_bytes([0xF0; 32]),
            bytes: 1,
            version: "test-only".to_owned(),
        }
    }

    #[must_use]
    pub const fn canonical_path(&self) -> &PathBuf {
        &self.canonical_path
    }

    #[must_use]
    pub const fn sha256(&self) -> &Sha256Digest32 {
        &self.sha256
    }

    /// Raw digest bytes for artifact bindings.
    #[must_use]
    pub const fn sha256_bytes(&self) -> [u8; 32] {
        *self.sha256.as_bytes()
    }

    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Parses `qdrant --version` output of the exact form `qdrant <version>`.
pub fn parse_qdrant_version_output(output: &str) -> Result<String, SupervisorError> {
    let trimmed = output.trim();
    let Some((name, version)) = trimmed.split_once(' ') else {
        return Err(SupervisorError::InvalidArtifact);
    };
    if name != "qdrant" {
        return Err(SupervisorError::InvalidArtifact);
    }
    if version.chars().any(char::is_whitespace) || !is_plausible_version(version) {
        return Err(SupervisorError::InvalidArtifact);
    }
    Ok(version.to_owned())
}

/// Verifies size, SHA-256, and reported version, in that order.
///
/// Timeouts, spawn failures, unreadable files, and non-zero probe exits
/// yield [`SupervisorError::ExecutableProbeFailed`]; size/hash mismatches
/// yield [`SupervisorError::ArtifactDigestMismatch`]; a parseable but wrong
/// version yields [`SupervisorError::ArtifactVersionMismatch`].
pub fn verify_executable_identity(
    expectation: &ExecutableExpectation,
    timeout: Duration,
) -> Result<VerifiedExecutable, SupervisorError> {
    if timeout < MIN_PROBE_TIMEOUT || timeout > MAX_PROBE_TIMEOUT {
        return Err(SupervisorError::InvalidProcessConfig);
    }
    let metadata =
        std::fs::metadata(&expectation.path).map_err(|_| SupervisorError::InvalidArtifact)?;
    if !metadata.is_file() || metadata.len() != expectation.bytes {
        return Err(SupervisorError::ArtifactDigestMismatch);
    }
    let observed =
        sha256_file(&expectation.path).map_err(|_| SupervisorError::ExecutableProbeFailed)?;
    if observed != *expectation.sha256.as_bytes() {
        return Err(SupervisorError::ArtifactDigestMismatch);
    }
    let version = probe_version(&expectation.path, timeout)?;
    if version != expectation.version {
        return Err(SupervisorError::ArtifactVersionMismatch);
    }
    let canonical_path = std::fs::canonicalize(&expectation.path)
        .map_err(|_| SupervisorError::ExecutableProbeFailed)?;
    Ok(VerifiedExecutable {
        canonical_path,
        sha256: expectation.sha256,
        bytes: expectation.bytes,
        version,
    })
}

/// Runs `<exe> --version` with a finite deadline; kills and reaps on timeout.
fn probe_version(exe: &Path, timeout: Duration) -> Result<String, SupervisorError> {
    let mut child = Command::new(exe)
        .arg(VERSION_PROBE_ARG)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| SupervisorError::ExecutableProbeFailed)?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(SupervisorError::ExecutableProbeFailed)?;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| SupervisorError::ExecutableProbeFailed)?
        {
            if !status.success() {
                return Err(SupervisorError::ExecutableProbeFailed);
            }
            let output = child
                .wait_with_output()
                .map_err(|_| SupervisorError::ExecutableProbeFailed)?;
            let text = String::from_utf8_lossy(&output.stdout);
            return parse_qdrant_version_output(&text);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SupervisorError::ExecutableProbeFailed);
        }
        std::thread::sleep(PROBE_POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use search_contracts::Sha256Digest32;

    use super::{
        ExecutableExpectation, QUALIFIED_EXE_BYTES, QUALIFIED_QDRANT_VERSION, is_plausible_version,
    };
    use crate::SupervisorError;

    #[test]
    fn qualified_default_pins_exact_artifact() {
        let expectation =
            ExecutableExpectation::qualified_default(PathBuf::from("qdrant.exe")).unwrap();
        assert_eq!(expectation.version, QUALIFIED_QDRANT_VERSION);
        assert_eq!(expectation.bytes, QUALIFIED_EXE_BYTES);
        assert_eq!(
            expectation.sha256,
            Sha256Digest32::parse_hex(super::QUALIFIED_EXE_SHA256_HEX).unwrap()
        );
    }

    #[test]
    fn degenerate_expectations_are_rejected() {
        let digest = Sha256Digest32::from_bytes([7; 32]);
        assert_eq!(
            ExecutableExpectation::custom(PathBuf::new(), digest, 10, "1.19.0".to_owned())
                .unwrap_err(),
            SupervisorError::InvalidArtifact
        );
        assert_eq!(
            ExecutableExpectation::custom(
                PathBuf::from("qdrant.exe"),
                digest,
                0,
                "1.19.0".to_owned()
            )
            .unwrap_err(),
            SupervisorError::InvalidArtifact
        );
        assert_eq!(
            ExecutableExpectation::custom(PathBuf::from("qdrant.exe"), digest, 10, String::new())
                .unwrap_err(),
            SupervisorError::InvalidArtifact
        );
        assert!(!is_plausible_version("1.19.0; rm -rf /"));
    }
}
