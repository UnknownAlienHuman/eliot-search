//! Real file-capture regression for the readiness path shared by status/health.

use super::current_readiness;
use crate::config_composition::{CliConfigArgs, effective_from_process};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
const VALID: &[u8] = b"schema_version=2\nprofile=\"direct\"\n[secrets]\nlease_ttl_ms=30000\nrotation_grace_ms=60000\n";

struct ConfigFile(PathBuf, bool);
impl ConfigFile {
    fn create(bytes: &[u8]) -> Self {
        let mut guard = Self(
            std::env::temp_dir().join(format!(
                "eliot-search-588-{}-{}.toml",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed),
            )),
            false,
        );
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&guard.0)
            .expect("create isolated config fixture");
        guard.1 = true;
        file.write_all(bytes).expect("write config fixture");
        guard
    }

    fn arguments(&self) -> CliConfigArgs {
        CliConfigArgs {
            config_file: Some(self.0.clone()),
            overrides: Vec::new(),
        }
    }

    fn remove(&self) {
        std::fs::remove_file(&self.0).expect("remove exact config fixture");
        assert!(!self.0.try_exists().expect("verify fixture absence"));
    }
}
impl Drop for ConfigFile {
    fn drop(&mut self) {
        if !self.1 {
            return;
        }
        if let Err(error) = std::fs::remove_file(&self.0) {
            if error.kind() != std::io::ErrorKind::NotFound && !std::thread::panicking() {
                panic!("exact config fixture cleanup failed");
            }
        }
    }
}

#[test]
fn status_health_readiness_retains_capture_after_file_corruption_and_removal() {
    let file = ConfigFile::create(VALID);
    let arguments = file.arguments();
    let retained = effective_from_process(&arguments).expect("capture valid startup config");
    let fingerprint = retained.fingerprint();
    let original = current_readiness(&retained);

    std::fs::write(&file.0, b"[secrets\ninvalid-588-canary").unwrap();
    assert!(
        effective_from_process(&arguments).is_err(),
        "fresh capture sees corruption"
    );
    assert_eq!(current_readiness(&retained), original);
    assert_eq!(retained.fingerprint(), fingerprint);

    file.remove();
    assert!(
        effective_from_process(&arguments).is_err(),
        "fresh capture sees missing file"
    );
    assert_eq!(current_readiness(&retained), original);
    assert_eq!(retained.fingerprint(), fingerprint);
    assert!(!format!("{original:?}").contains("invalid-588-canary"));
}

#[test]
fn invalid_startup_capture_returns_no_retained_configuration() {
    let file =
        ConfigFile::create(b"schema_version=2\nprofile=\"direct\"\n[secrets]\nlease_ttl_ms=999\n");
    let error = effective_from_process(&file.arguments()).unwrap_err();
    assert!(!error.contains("eliot-search-588-"));
    file.remove();
}
