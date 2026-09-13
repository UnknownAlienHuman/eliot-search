//! Bounded, lossy diagnostic tails for failed disposable-server startup.

use std::fmt::Write as _;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const LOG_TAIL_BYTES: u64 = 1_024;

pub(super) fn read_log_tail(dir: &Path) -> String {
    let mut combined = String::new();
    for name in ["qdrant-out.log", "qdrant-err.log"] {
        let path = dir.join(name);
        let Ok(mut file) = std::fs::File::open(path) else {
            continue;
        };
        let Ok(metadata) = file.metadata() else {
            continue;
        };
        let start = metadata.len().saturating_sub(LOG_TAIL_BYTES);
        if file.seek(SeekFrom::Start(start)).is_err() {
            continue;
        }
        let mut bytes = Vec::with_capacity(
            usize::try_from(metadata.len().saturating_sub(start))
                .unwrap_or(LOG_TAIL_BYTES as usize),
        );
        if file
            .take(LOG_TAIL_BYTES)
            .read_to_end(&mut bytes)
            .is_err()
        {
            continue;
        }
        let tail = String::from_utf8_lossy(&bytes);
        let _ = write!(combined, "--- {name} (tail) ---\n{tail}\n");
    }
    combined
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn log_tail_is_bounded_and_handles_split_utf8() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "eliot-qdrant-log-tail-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create fixture dir");
        let mut bytes = vec![b'x'; 2_048];
        bytes.extend_from_slice("é-END".as_bytes());
        std::fs::write(dir.join("qdrant-out.log"), bytes).expect("write log");

        let tail = read_log_tail(&dir);
        let _ = std::fs::remove_dir_all(dir);
        assert!(tail.contains("-END"));
        assert!(tail.len() < 1_200, "bounded tail grew to {}", tail.len());
    }

    #[test]
    fn missing_logs_produce_empty_diagnostics() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "eliot-qdrant-log-empty-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create fixture dir");
        assert!(read_log_tail(&dir).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
