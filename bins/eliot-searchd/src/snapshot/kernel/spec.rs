use std::sync::atomic::AtomicU64;

pub(super) const FINGERPRINT_ALGORITHM: &str = "eliot-fnv4-v1";
pub(super) const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
pub(super) static SNAPSHOT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
pub(super) static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
