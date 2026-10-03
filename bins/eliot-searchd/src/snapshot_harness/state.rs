//! Owner, endpoint and local-token filesystem state for the legacy harness.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, ErrorKind, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::lexical::LexicalIndex;
use crate::snapshot::{SnapshotIndex, hex32};

use super::protocol::hex_bytes;

pub(super) struct OwnerFile {
    file: File,
}

impl OwnerFile {
    pub(super) fn acquire(runtime_dir: &Path) -> io::Result<Self> {
        let path = runtime_dir.join("owner.lock");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    ErrorKind::AlreadyExists,
                    "data root is already owned by another live process",
                ));
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
        write_owner_state(&mut file, "ACTIVE")?;
        Ok(Self { file })
    }
}

impl Drop for OwnerFile {
    fn drop(&mut self) {
        let _ = write_owner_state(&mut self.file, "RELEASED");
        let _ = self.file.unlock();
    }
}

pub(super) struct EndpointFile {
    path: PathBuf,
}

impl EndpointFile {
    pub(super) fn publish(
        runtime_dir: &Path,
        address: std::net::SocketAddr,
        source_root_count: usize,
        snapshot: &SnapshotIndex,
        lexical: &LexicalIndex,
    ) -> io::Result<Self> {
        let path = runtime_dir.join("endpoint.v1");
        let temporary = runtime_dir.join(format!(
            "endpoint.v1.{}.{}.tmp",
            process::id(),
            unix_millis()?
        ));
        let body = format!(
            concat!(
                "ELIOT_SEARCH_ENDPOINT_V1\n",
                "address={}\n",
                "protocol=1\n",
                "source_backed_search=true\n",
                "lexical_search=true\n",
                "production_ready=false\n",
                "encrypted_revisions=false\n",
                "source_roots={}\n",
                "snapshot_id={}\n",
                "manifest_fingerprint={}\n",
                "lexical_index_fingerprint={}\n"
            ),
            address,
            source_root_count,
            snapshot.snapshot_id(),
            hex32(snapshot.manifest_fingerprint()),
            hex32(lexical.index_fingerprint()),
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(body.as_bytes())?;
        file.sync_all()?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        fs::rename(&temporary, &path)?;
        Ok(Self { path })
    }
}

impl Drop for EndpointFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(super) fn load_or_create_token(path: &Path) -> io::Result<String> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 128 {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "authentication token must be a small regular file",
            ));
        }
        #[cfg(unix)]
        restrict_token_permissions(path)?;
        #[cfg(not(unix))]
        restrict_token_permissions(path);
        let token = fs::read_to_string(path)?.trim().to_owned();
        validate_token(&token)?;
        return Ok(token);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let token = generate_local_token()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(token.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    #[cfg(unix)]
    restrict_token_permissions(path)?;
    #[cfg(not(unix))]
    restrict_token_permissions(path);
    Ok(token)
}

pub(super) fn generate_local_token() -> io::Result<String> {
    let mut bytes = [0_u8; 32];
    fill_random_bytes(&mut bytes)?;
    let token = hex_bytes(&bytes);
    validate_token(&token)?;
    Ok(token)
}

pub(super) fn validate_token(token: &str) -> io::Result<()> {
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "authentication token must be exactly 64 hexadecimal characters",
        ));
    }
    Ok(())
}

fn write_owner_state(file: &mut File, state: &str) -> io::Result<()> {
    let body = format!(
        concat!(
            "ELIOT_SEARCH_OWNER_V1\n",
            "pid={}\n",
            "state={}\n",
            "observed_unix_ms={}\n"
        ),
        process::id(),
        state,
        unix_millis()?,
    );
    if body.len() > 4 * 1_024 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "owner record exceeds its finite ceiling",
        ));
    }
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(body.as_bytes())?;
    file.sync_all()
}

#[cfg(unix)]
fn fill_random_bytes(bytes: &mut [u8]) -> io::Result<()> {
    File::open("/dev/urandom")?.read_exact(bytes)
}

#[cfg(not(unix))]
fn fill_random_bytes(bytes: &mut [u8]) -> io::Result<()> {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hash, Hasher};

    let now = unix_millis()?;
    for (index, chunk) in bytes.chunks_mut(8).enumerate() {
        let state = RandomState::new();
        let mut hasher = state.build_hasher();
        process::id().hash(&mut hasher);
        now.hash(&mut hasher);
        index.hash(&mut hasher);
        let value = hasher.finish().to_be_bytes();
        chunk.copy_from_slice(&value[..chunk.len()]);
    }
    Ok(())
}

#[cfg(unix)]
fn restrict_token_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
const fn restrict_token_permissions(_path: &Path) {}

fn unix_millis() -> io::Result<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|_| io::Error::other("system clock precedes the Unix epoch"))
}
