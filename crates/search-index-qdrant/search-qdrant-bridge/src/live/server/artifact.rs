//! Exact executable measurement for the disposable qualification harness.

use std::io::Read;

use sha2::{Digest, Sha256};

use crate::qualified::{QUALIFIED_EXE_BYTES, QUALIFIED_EXE_SHA256_HEX};

use super::super::LiveError;

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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn wrong_sized_artifact_is_rejected_before_digest_work() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "eliot-qdrant-artifact-{}-{stamp}.exe",
            std::process::id()
        ));
        std::fs::write(&path, b"not-qdrant").expect("write fixture");
        let result = verify_executable(path.to_string_lossy().as_ref());
        let _ = std::fs::remove_file(path);
        assert_eq!(result, Err(LiveError::ArtifactSizeMismatch));
    }
}
