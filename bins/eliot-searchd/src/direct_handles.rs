//! Process-scoped bounded locators for captured DIRECT result excerpts.
//!
//! A direct handle is not an authorization credential. Every operation still
//! requires the daemon bearer token. The handle is an opaque, finite-lifetime
//! locator bound to a configured source root; removing or losing authority for
//! that root makes the captured excerpt unreadable.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::direct_search::SearchHit;
use crate::source_roots::{SourceRootCatalog, SourceRootState};

pub(crate) const DIRECT_HANDLE_HEX_BYTES: usize = 32;
const DEFAULT_MAX_HANDLES: usize = 4_096;
const DEFAULT_MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_MAX_EXCERPT_BYTES: usize = 16 * 1024;
const DEFAULT_TTL: Duration = Duration::from_secs(10 * 60);
const ACCOUNTING_OVERHEAD: usize = 128;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DirectHandleId([u8; 16]);

impl DirectHandleId {
    pub(crate) fn parse_hex(value: &str) -> Result<Self, DirectHandleError> {
        if value.len() != DIRECT_HANDLE_HEX_BYTES {
            return Err(DirectHandleError::InvalidId);
        }
        let mut bytes = [0_u8; 16];
        for (target, pair) in bytes.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
            let high = from_hex(pair[0]).ok_or(DirectHandleError::InvalidId)?;
            let low = from_hex(pair[1]).ok_or(DirectHandleError::InvalidId)?;
            *target = (high << 4) | low;
        }
        Ok(Self(bytes))
    }

    pub(crate) fn to_hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(DIRECT_HANDLE_HEX_BYTES);
        for byte in self.0 {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectHandleView {
    pub(crate) handle_id: DirectHandleId,
    pub(crate) root_index: usize,
    pub(crate) relative_path: String,
    pub(crate) line_number: u64,
    pub(crate) excerpt: String,
    pub(crate) excerpt_truncated: bool,
}

#[derive(Clone, Debug)]
struct DirectHandleRecord {
    view: DirectHandleView,
    root_path: PathBuf,
    expires_at: Instant,
    accounted_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct DirectHandleRegistry {
    instance_nonce: u64,
    next_counter: u64,
    max_handles: usize,
    max_total_bytes: usize,
    max_excerpt_bytes: usize,
    ttl: Duration,
    records: BTreeMap<DirectHandleId, DirectHandleRecord>,
    issue_order: VecDeque<DirectHandleId>,
    accounted_bytes: usize,
}

impl DirectHandleRegistry {
    pub(crate) fn new(process_secret: &[u8]) -> Self {
        Self::with_limits(
            process_secret,
            DEFAULT_MAX_HANDLES,
            DEFAULT_MAX_TOTAL_BYTES,
            DEFAULT_MAX_EXCERPT_BYTES,
            DEFAULT_TTL,
        )
        .expect("baseline direct-handle limits are valid")
    }

    fn with_limits(
        process_secret: &[u8],
        max_handles: usize,
        max_total_bytes: usize,
        max_excerpt_bytes: usize,
        ttl: Duration,
    ) -> Result<Self, DirectHandleError> {
        if process_secret.is_empty()
            || max_handles == 0
            || max_total_bytes == 0
            || max_excerpt_bytes == 0
            || max_excerpt_bytes > max_total_bytes
            || ttl.is_zero()
        {
            return Err(DirectHandleError::InvalidLimits);
        }
        Ok(Self {
            instance_nonce: derive_instance_nonce(process_secret),
            next_counter: 0,
            max_handles,
            max_total_bytes,
            max_excerpt_bytes,
            ttl,
            records: BTreeMap::new(),
            issue_order: VecDeque::new(),
            accounted_bytes: 0,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn issue(
        &mut self,
        hits: &[SearchHit],
        now: Instant,
    ) -> Result<Vec<DirectHandleId>, DirectHandleError> {
        self.prune_expired(now);
        if hits.is_empty() {
            return Ok(Vec::new());
        }
        let required_handles = self
            .records
            .len()
            .checked_add(hits.len())
            .ok_or(DirectHandleError::HandleCapacityExceeded)?;
        if required_handles > self.max_handles {
            return Err(DirectHandleError::HandleCapacityExceeded);
        }
        let expires_at = now
            .checked_add(self.ttl)
            .ok_or(DirectHandleError::LeaseOverflow)?;

        let mut staged = Vec::with_capacity(hits.len());
        let mut staged_bytes = 0_usize;
        let mut next_counter = self.next_counter;
        for hit in hits {
            if hit.excerpt.as_bytes().len() > self.max_excerpt_bytes {
                return Err(DirectHandleError::ExcerptTooLarge);
            }
            let accounted_bytes = account_hit(hit)?;
            staged_bytes = staged_bytes
                .checked_add(accounted_bytes)
                .ok_or(DirectHandleError::MemoryCapacityExceeded)?;
            next_counter = next_counter
                .checked_add(1)
                .ok_or(DirectHandleError::CounterExhausted)?;
            let handle_id = make_handle_id(self.instance_nonce, next_counter);
            if self.records.contains_key(&handle_id)
                || staged
                    .iter()
                    .any(|(existing, _, _): &(DirectHandleId, DirectHandleRecord, usize)| {
                        *existing == handle_id
                    })
            {
                return Err(DirectHandleError::IdCollision);
            }
            let record = DirectHandleRecord {
                view: DirectHandleView {
                    handle_id,
                    root_index: hit.root_index,
                    relative_path: hit.relative_path.clone(),
                    line_number: hit.line_number,
                    excerpt: hit.excerpt.clone(),
                    excerpt_truncated: hit.excerpt_truncated,
                },
                root_path: hit.root_path.clone(),
                expires_at,
                accounted_bytes,
            };
            staged.push((handle_id, record, accounted_bytes));
        }
        let total_bytes = self
            .accounted_bytes
            .checked_add(staged_bytes)
            .ok_or(DirectHandleError::MemoryCapacityExceeded)?;
        if total_bytes > self.max_total_bytes {
            return Err(DirectHandleError::MemoryCapacityExceeded);
        }

        let mut issued = Vec::with_capacity(staged.len());
        for (handle_id, record, _) in staged {
            self.issue_order.push_back(handle_id);
            self.records.insert(handle_id, record);
            issued.push(handle_id);
        }
        self.next_counter = next_counter;
        self.accounted_bytes = total_bytes;
        Ok(issued)
    }

    pub(crate) fn read(
        &mut self,
        handle_id: DirectHandleId,
        roots: &SourceRootCatalog,
        now: Instant,
    ) -> Result<DirectHandleView, DirectHandleError> {
        if self
            .records
            .get(&handle_id)
            .is_some_and(|record| record.expires_at <= now)
        {
            self.remove_record(handle_id);
            return Err(DirectHandleError::Expired);
        }
        self.prune_expired(now);
        let record = self
            .records
            .get(&handle_id)
            .ok_or(DirectHandleError::NotFound)?;
        if !root_is_available(roots, &record.root_path) {
            return Err(DirectHandleError::RootUnavailable);
        }
        Ok(record.view.clone())
    }

    pub(crate) fn close(
        &mut self,
        handle_id: DirectHandleId,
    ) -> Result<(), DirectHandleError> {
        if self.remove_record(handle_id) {
            Ok(())
        } else {
            Err(DirectHandleError::NotFound)
        }
    }

    pub(crate) fn invalidate_root(&mut self, root_path: &Path) -> usize {
        let ids = self
            .records
            .iter()
            .filter(|(_, record)| record.root_path == root_path)
            .map(|(handle_id, _)| *handle_id)
            .collect::<Vec<_>>();
        let removed = ids.len();
        for handle_id in ids {
            self.remove_record(handle_id);
        }
        removed
    }

    fn prune_expired(&mut self, now: Instant) {
        loop {
            let Some(handle_id) = self.issue_order.front().copied() else {
                break;
            };
            match self.records.get(&handle_id) {
                None => {
                    self.issue_order.pop_front();
                }
                Some(record) if record.expires_at <= now => {
                    self.issue_order.pop_front();
                    self.remove_record(handle_id);
                }
                Some(_) => break,
            }
        }
    }

    fn remove_record(&mut self, handle_id: DirectHandleId) -> bool {
        let Some(record) = self.records.remove(&handle_id) else {
            return false;
        };
        self.accounted_bytes = self
            .accounted_bytes
            .saturating_sub(record.accounted_bytes);
        true
    }
}

fn account_hit(hit: &SearchHit) -> Result<usize, DirectHandleError> {
    let root = hit
        .root_path
        .to_str()
        .ok_or(DirectHandleError::RootPathNotUtf8)?;
    ACCOUNTING_OVERHEAD
        .checked_add(root.as_bytes().len())
        .and_then(|value| value.checked_add(hit.relative_path.as_bytes().len()))
        .and_then(|value| value.checked_add(hit.excerpt.as_bytes().len()))
        .ok_or(DirectHandleError::MemoryCapacityExceeded)
}

fn root_is_available(roots: &SourceRootCatalog, root_path: &Path) -> bool {
    let Some(root_path) = root_path.to_str() else {
        return false;
    };
    roots.views().is_ok_and(|views| {
        views.into_iter().any(|view| {
            view.path == root_path && view.state == SourceRootState::Available
        })
    })
}

fn derive_instance_nonce(process_secret: &[u8]) -> u64 {
    let mut state = 0xcbf2_9ce4_8422_2325_u64;
    for byte in process_secret {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for byte in std::process::id().to_be_bytes() {
        state ^= u64::from(byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    for byte in nanos.to_be_bytes() {
        state ^= u64::from(byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }
    state
}

fn make_handle_id(instance_nonce: u64, counter: u64) -> DirectHandleId {
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&instance_nonce.to_be_bytes());
    bytes[8..].copy_from_slice(&counter.to_be_bytes());
    DirectHandleId(bytes)
}

const fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum DirectHandleError {
    InvalidLimits,
    InvalidId,
    NotFound,
    Expired,
    RootUnavailable,
    RootPathNotUtf8,
    ExcerptTooLarge,
    HandleCapacityExceeded,
    MemoryCapacityExceeded,
    CounterExhausted,
    LeaseOverflow,
    IdCollision,
}

impl DirectHandleError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "DIRECT_HANDLE_INVALID_LIMITS",
            Self::InvalidId => "DIRECT_HANDLE_ID_INVALID",
            Self::NotFound => "DIRECT_HANDLE_NOT_FOUND",
            Self::Expired => "DIRECT_HANDLE_EXPIRED",
            Self::RootUnavailable => "DIRECT_HANDLE_ROOT_UNAVAILABLE",
            Self::RootPathNotUtf8 => "DIRECT_HANDLE_ROOT_PATH_NOT_UTF8",
            Self::ExcerptTooLarge => "DIRECT_HANDLE_EXCERPT_TOO_LARGE",
            Self::HandleCapacityExceeded => "DIRECT_HANDLE_CAPACITY",
            Self::MemoryCapacityExceeded => "DIRECT_HANDLE_MEMORY_CAPACITY",
            Self::CounterExhausted => "DIRECT_HANDLE_COUNTER_EXHAUSTED",
            Self::LeaseOverflow => "DIRECT_HANDLE_LEASE_OVERFLOW",
            Self::IdCollision => "DIRECT_HANDLE_ID_COLLISION",
        }
    }
}

impl std::fmt::Display for DirectHandleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for DirectHandleError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "eliot-search-direct-handles-{name}-{}-{unique}",
            std::process::id()
        ))
    }

    fn fixture() -> (PathBuf, SourceRootCatalog, SearchHit) {
        let root = temporary_directory("fixture");
        let source = root.join("source");
        fs::create_dir_all(&source).expect("create source");
        let canonical = fs::canonicalize(&source).expect("canonical source");
        let catalog = SourceRootCatalog::load(
            root.join("state").join("source-roots.txt"),
            std::slice::from_ref(&source),
        )
        .expect("load catalog");
        let hit = SearchHit {
            root_index: 0,
            root_path: canonical,
            relative_path: "example.txt".to_owned(),
            line_number: 4,
            preview: "needle".to_owned(),
            excerpt: "the needle line".to_owned(),
            excerpt_truncated: false,
        };
        (root, catalog, hit)
    }

    #[test]
    fn issue_read_and_close() {
        let (root, catalog, hit) = fixture();
        let now = Instant::now();
        let mut registry = DirectHandleRegistry::new(b"test-process-secret");
        let ids = registry.issue(&[hit], now).expect("issue handle");
        assert_eq!(ids.len(), 1);
        let view = registry.read(ids[0], &catalog, now).expect("read handle");
        assert_eq!(view.excerpt, "the needle line");
        assert!(registry.close(ids[0]).is_ok());
        assert!(matches!(
            registry.read(ids[0], &catalog, now),
            Err(DirectHandleError::NotFound)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unavailable_root_blocks_read() {
        let (root, mut catalog, hit) = fixture();
        let source = hit.root_path.clone();
        let now = Instant::now();
        let mut registry = DirectHandleRegistry::new(b"test-process-secret");
        let id = registry.issue(&[hit], now).expect("issue handle")[0];
        fs::remove_dir_all(&source).expect("remove source root");
        catalog.refresh();
        assert!(matches!(
            registry.read(id, &catalog, now),
            Err(DirectHandleError::RootUnavailable)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expired_handle_is_removed() {
        let (root, catalog, hit) = fixture();
        let now = Instant::now();
        let mut registry = DirectHandleRegistry::with_limits(
            b"test-process-secret",
            4,
            4096,
            1024,
            Duration::from_secs(1),
        )
        .expect("registry");
        let id = registry.issue(&[hit], now).expect("issue handle")[0];
        assert!(matches!(
            registry.read(id, &catalog, now + Duration::from_secs(2)),
            Err(DirectHandleError::Expired)
        ));
        assert_eq!(registry.len(), 0);
        let _ = fs::remove_dir_all(root);
    }
}
