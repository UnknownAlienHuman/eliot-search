#![allow(unsafe_code)]

use core::ffi::c_void;
use core::ptr::{self, null, null_mut};
use std::fs;
use std::path::Path;
use std::slice;

use crate::sha256;

const CRED_TYPE_GENERIC: u32 = 1;
const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
const ERROR_NOT_FOUND: u32 = 1_168;
const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x0000_0001;
const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
const ROOT_SECRET_BYTES: usize = 32;
const MAX_CREDENTIAL_BLOB_BYTES: usize = 5 * 512;
const MAX_OBJECT_SCAN: usize = 2_000_000;
/// Cross-process serialization for Credential Manager traffic. Dozens of
/// parallel test fixtures (plus spawned daemon children) hammer `CredWrite` /
/// `CredDelete` concurrently, and the vault drops or stalls operations under
/// that storm. One named mutex makes every revision-key vault sequence
/// mutually exclusive across processes; waits are bounded and every outcome
/// stays fail-closed. The name is content-free (no secret, no path).
const VAULT_MUTEX_NAME: &str = "ELIOT-Search-RevisionVault-v1";
/// Per-acquisition wait ceiling. Holders only run single vault calls, so a
/// healthy wait is milliseconds; anything beyond this is fail-closed.
const VAULT_LOCK_WAIT_MILLIS: u32 = 5_000;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_ABANDONED: u32 = 0x0000_0080;

#[allow(dead_code)]
#[repr(C)]
struct FileTime {
    low_date_time: u32,
    high_date_time: u32,
}

#[allow(dead_code)]
#[repr(C)]
struct CredentialW {
    flags: u32,
    credential_type: u32,
    target_name: *mut u16,
    comment: *mut u16,
    last_written: FileTime,
    credential_blob_size: u32,
    credential_blob: *mut u8,
    persist: u32,
    attribute_count: u32,
    attributes: *mut c_void,
    target_alias: *mut u16,
    user_name: *mut u16,
}

#[repr(C)]
struct DataBlob {
    byte_length: u32,
    bytes: *mut u8,
}

#[link(name = "Advapi32")]
unsafe extern "system" {
    #[link_name = "CredReadW"]
    fn cred_read_w(
        target_name: *const u16,
        credential_type: u32,
        flags: u32,
        credential: *mut *mut CredentialW,
    ) -> i32;
    #[link_name = "CredWriteW"]
    fn cred_write_w(credential: *const CredentialW, flags: u32) -> i32;
    #[cfg(test)]
    #[link_name = "CredDeleteW"]
    fn cred_delete_w(target_name: *const u16, credential_type: u32, flags: u32) -> i32;
    #[link_name = "CredFree"]
    fn cred_free(buffer: *mut c_void);
}

#[link(name = "Crypt32")]
unsafe extern "system" {
    #[link_name = "CryptProtectData"]
    fn crypt_protect_data(
        data_in: *const DataBlob,
        description: *const u16,
        optional_entropy: *const DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
    #[link_name = "CryptUnprotectData"]
    fn crypt_unprotect_data(
        data_in: *const DataBlob,
        description: *mut *mut u16,
        optional_entropy: *const DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
}

#[link(name = "Kernel32")]
unsafe extern "system" {
    #[link_name = "GetLastError"]
    fn get_last_error() -> u32;
    #[link_name = "LocalFree"]
    fn local_free(memory: *mut c_void) -> *mut c_void;
    #[link_name = "CreateMutexW"]
    fn create_mutex_w(
        security_attributes: *mut c_void,
        initial_owner: i32,
        name: *const u16,
    ) -> *mut c_void;
    #[link_name = "WaitForSingleObject"]
    fn wait_for_single_object(handle: *mut c_void, milliseconds: u32) -> u32;
    #[link_name = "ReleaseMutex"]
    fn release_mutex(handle: *mut c_void) -> i32;
    #[link_name = "CloseHandle"]
    fn close_handle(handle: *mut c_void) -> i32;
}

#[link(name = "Bcrypt")]
unsafe extern "system" {
    #[link_name = "BCryptGenRandom"]
    fn bcrypt_gen_random(
        algorithm: *mut c_void,
        buffer: *mut u8,
        buffer_bytes: u32,
        flags: u32,
    ) -> i32;
}

struct CredentialAllocation(*mut CredentialW);

impl Drop for CredentialAllocation {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe {
            let credential = &mut *self.0;
            let size = usize::try_from(credential.credential_blob_size)
                .unwrap_or(0)
                .min(MAX_CREDENTIAL_BLOB_BYTES);
            if !credential.credential_blob.is_null() && size > 0 {
                ptr::write_bytes(credential.credential_blob, 0, size);
            }
            cred_free(self.0.cast());
        }
    }
}

struct LocalAllocation(DataBlob);

/// Held cross-process vault mutex. Released and closed on drop; never
/// panics. One acquisition covers one short vault sequence only — never
/// held across retry sleeps (callers acquire per attempt).
struct VaultLock(*mut c_void);

impl Drop for VaultLock {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe {
            release_mutex(self.0);
            close_handle(self.0);
        }
    }
}

fn vault_mutex_name_wide() -> Vec<u16> {
    VAULT_MUTEX_NAME
        .encode_utf16()
        .chain(core::iter::once(0))
        .collect()
}

/// Acquire the cross-process vault mutex with a bounded wait. `WAIT_ABANDONED`
/// (previous holder died mid-sequence) is safe to take: every critical
/// section is a single self-contained vault call with verify-after-write, so
/// no multi-step invariant can be left half-held.
fn acquire_vault_lock() -> Result<VaultLock, String> {
    let name = vault_mutex_name_wide();
    let handle = unsafe { create_mutex_w(null_mut(), 0, name.as_ptr()) };
    if handle.is_null() {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_REVISION_VAULT_LOCK_FAILED:{error}"));
    }
    let status = unsafe { wait_for_single_object(handle, VAULT_LOCK_WAIT_MILLIS) };
    if status == WAIT_OBJECT_0 || status == WAIT_ABANDONED {
        return Ok(VaultLock(handle));
    }
    unsafe {
        close_handle(handle);
    }
    Err(format!("DIRECT_REVISION_VAULT_BUSY:{status}"))
}
impl LocalAllocation {
    fn into_vec(mut self, max_bytes: usize) -> Result<Vec<u8>, String> {
        let length = usize::try_from(self.0.byte_length)
            .map_err(|_| "DIRECT_DPAPI_OUTPUT_TOO_LARGE".to_owned())?;
        if self.0.bytes.is_null() || length == 0 || length > max_bytes {
            return Err("DIRECT_DPAPI_OUTPUT_INVALID".to_owned());
        }
        let output = unsafe { slice::from_raw_parts(self.0.bytes, length) }.to_vec();
        unsafe {
            ptr::write_bytes(self.0.bytes, 0, length);
            let _ = local_free(self.0.bytes.cast());
        }
        self.0.bytes = null_mut();
        self.0.byte_length = 0;
        Ok(output)
    }
}

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        if self.0.bytes.is_null() {
            return;
        }
        let length = usize::try_from(self.0.byte_length).unwrap_or(0);
        unsafe {
            if length > 0 {
                ptr::write_bytes(self.0.bytes, 0, length);
            }
            let _ = local_free(self.0.bytes.cast());
        }
    }
}

pub(super) fn load_or_create_root_secret(
    namespace_id: [u8; 32],
    revision_root: &Path,
) -> Result<[u8; 32], String> {
    let target_text = format!(
        "ELIOT Search/revision-key/{}",
        sha256::hex(&namespace_id),
    );
    let target = wide(&target_text);
    if let Some(secret) = read_credential(&target)? {
        return Ok(secret);
    }
    if contains_protected_objects(revision_root)? {
        // The key was written by an earlier command but is not visible yet:
        // under parallel vault load the commit can lag the writer. Re-read
        // bounded before declaring the revision unrecoverable. Never creates.
        // Each attempt serializes on the cross-process vault mutex so the
        // read cannot interleave with a concurrent writer's commit.
        for attempt in 0..8_u32 {
            std::thread::sleep(core::time::Duration::from_millis(
                10_u64 << attempt.min(5),
            ));
            let Ok(_lock) = acquire_vault_lock() else {
                continue;
            };
            if let Some(secret) = read_credential(&target)? {
                return Ok(secret);
            }
        }
        return Err("DIRECT_REVISION_KEY_MISSING".to_owned());
    }
    // Create path: single RNG draw, then a bounded write+verify loop. Only
    // transient vault outcomes are retried (failed writes, lagging commit
    // visibility); a content mismatch fails closed immediately and is never
    // papered over by another attempt.
    let mut secret = [0_u8; ROOT_SECRET_BYTES];
    let status = unsafe {
        bcrypt_gen_random(
            null_mut(),
            secret.as_mut_ptr(),
            u32::try_from(secret.len())
                .map_err(|_| "DIRECT_REVISION_RNG_FAILED".to_owned())?,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status < 0 {
        super::zeroize(&mut secret);
        return Err(format!("DIRECT_REVISION_RNG_FAILED:{status}"));
    }
    for attempt in 0..8_u32 {
        if attempt > 0 {
            std::thread::sleep(core::time::Duration::from_millis(
                10_u64 << attempt.min(5),
            ));
        }
        // Serialize the write+verify pair on the cross-process mutex: the
        // vault drops or stalls concurrent writers, so an unverified write
        // is retried rather than trusted. A busy lock only skips the
        // attempt; the bounded loop still fails closed on exhaustion.
        let Ok(_lock) = acquire_vault_lock() else {
            continue;
        };
        if let Err(error) = write_credential(&target, &mut secret) {
            if !is_transient_vault_outcome(&error) {
                super::zeroize(&mut secret);
                return Err(error);
            }
            continue;
        }
        match read_credential(&target) {
            Ok(Some(observed)) => {
                if !constant_time_equal(&secret, &observed) {
                    super::zeroize(&mut secret);
                    let mut observed = observed;
                    super::zeroize(&mut observed);
                    return Err("DIRECT_REVISION_KEY_READBACK_MISMATCH".to_owned());
                }
                let mut observed = observed;
                super::zeroize(&mut observed);
                return Ok(secret);
            }
            Ok(None) => {}
            Err(error) => {
                if !is_transient_vault_outcome(&error) {
                    super::zeroize(&mut secret);
                    return Err(error);
                }
            }
        }
    }
    super::zeroize(&mut secret);
    Err("DIRECT_REVISION_KEY_WRITE_OUTCOME_UNKNOWN".to_owned())
}

/// True for vault outcomes that parallel load can cause transiently (busy
/// writes, lagging commit visibility). Content/shape verdicts are never
/// transient: retrying those would mask a real defect.
fn is_transient_vault_outcome(error: &str) -> bool {
    error.starts_with("DIRECT_REVISION_KEY_WRITE_FAILED")
        || error == "DIRECT_REVISION_KEY_WRITE_OUTCOME_UNKNOWN"
        || error.starts_with("DIRECT_REVISION_KEY_READ_FAILED")
}

fn read_credential(target: &[u16]) -> Result<Option<[u8; 32]>, String> {
    let mut pointer = null_mut::<CredentialW>();
    let success = unsafe {
        cred_read_w(
            target.as_ptr(),
            CRED_TYPE_GENERIC,
            0,
            &raw mut pointer,
        )
    };
    if success == 0 {
        let error = unsafe { get_last_error() };
        return if error == ERROR_NOT_FOUND {
            Ok(None)
        } else {
            Err(format!("DIRECT_REVISION_KEY_READ_FAILED:{error}"))
        };
    }
    if pointer.is_null() {
        return Err("DIRECT_REVISION_KEY_READBACK_INVALID".to_owned());
    }
    let allocation = CredentialAllocation(pointer);
    let credential = unsafe { &*allocation.0 };
    let size = usize::try_from(credential.credential_blob_size)
        .map_err(|_| "DIRECT_REVISION_KEY_READBACK_INVALID".to_owned())?;
    if credential.credential_type != CRED_TYPE_GENERIC
        || credential.persist != CRED_PERSIST_LOCAL_MACHINE
        || size != ROOT_SECRET_BYTES
        || credential.credential_blob.is_null()
    {
        return Err("DIRECT_REVISION_KEY_READBACK_INVALID".to_owned());
    }
    let mut secret = [0_u8; ROOT_SECRET_BYTES];
    unsafe {
        ptr::copy_nonoverlapping(
            credential.credential_blob,
            secret.as_mut_ptr(),
            ROOT_SECRET_BYTES,
        );
    }
    drop(allocation);
    Ok(Some(secret))
}

fn write_credential(
    target: &[u16],
    secret: &mut [u8; 32],
) -> Result<(), String> {
    let mut target = target.to_vec();
    let mut user_name = wide("ELIOT Search");
    let credential = CredentialW {
        flags: 0,
        credential_type: CRED_TYPE_GENERIC,
        target_name: target.as_mut_ptr(),
        comment: null_mut(),
        last_written: FileTime {
            low_date_time: 0,
            high_date_time: 0,
        },
        credential_blob_size: u32::try_from(secret.len())
            .map_err(|_| "DIRECT_REVISION_KEY_TOO_LARGE".to_owned())?,
        credential_blob: secret.as_mut_ptr(),
        persist: CRED_PERSIST_LOCAL_MACHINE,
        attribute_count: 0,
        attributes: null_mut(),
        target_alias: null_mut(),
        user_name: user_name.as_mut_ptr(),
    };
    let success = unsafe { cred_write_w(&raw const credential, 0) };
    if success == 0 {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_REVISION_KEY_WRITE_FAILED:{error}"));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn delete_test_credential_for_data_root(data_root: &Path) {
    let derived = read_test_namespace_hex(data_root);
    if let Some(namespace_hex) = derived
        && !delete_test_credential_verified(&namespace_hex)
    {
        // Best-effort cleanup exhausted its bounded retries without
        // verifying the entry gone. Report, never panic: cleanup must
        // not mask the test result.
        eprintln!(
            "ELIOT_TEST_CLEANUP: revision-key credential still present after bounded retries: root={} namespace={namespace_hex}",
            data_root.display(),
        );
    }
}

/// Delete one namespace credential and verify it is really gone, retrying
/// across drop-storm vault contention. A full parallel run drops dozens of
/// fixtures within the same moment, and a lone `CredDelete` can lose to that
/// contention while reporting success or transient failure either way; the
/// read-back below is the only signal trusted here. A missing entry is
/// already gone. Returns true when absence was verified. Every outcome stays
/// best-effort so cleanup in `Drop` can never mask a test result.
#[cfg(test)]
fn delete_test_credential_verified(namespace_hex: &str) -> bool {
    debug_assert!(is_test_namespace_hex(namespace_hex));
    let wide = wide(&format!("ELIOT Search/revision-key/{namespace_hex}"));
    for attempt in 0..16_u32 {
        // Same cross-process serialization as the daemon path: concurrent
        // drops race the same way concurrent writes do.
        let Ok(_lock) = acquire_vault_lock() else {
            std::thread::sleep(core::time::Duration::from_millis(
                10_u64 << attempt.min(7),
            ));
            continue;
        };
        unsafe {
            let _ = cred_delete_w(wide.as_ptr(), CRED_TYPE_GENERIC, 0);
        }
        if credential_absent(&wide) {
            return true;
        }
        std::thread::sleep(core::time::Duration::from_millis(
            10_u64 << attempt.min(7),
        ));
    }
    false
}

/// True when no credential exists under `target`. Present-but-unreadable
/// counts as present so the caller retries instead of declaring victory.
/// Never panics; returned secret bytes are zeroized before release.
#[cfg(test)]
fn credential_absent(target: &[u16]) -> bool {
    match read_credential(target) {
        Ok(None) => true,
        Ok(Some(mut secret)) => {
            super::zeroize(&mut secret);
            false
        }
        Err(_) => false,
    }
}

/// Read this data root's namespace hex without creating anything. This reads
/// the same `control/namespace.id` file that
/// `plaintext_direct_store::load_or_create_namespace` owns; a missing or
/// malformed file means there is no credential to delete.
#[cfg(test)]
pub(super) fn read_test_namespace_hex(data_root: &Path) -> Option<String> {
    let bytes = fs::read(data_root.join("control").join("namespace.id")).ok()?;
    let text = core::str::from_utf8(&bytes).ok()?.trim().to_owned();
    is_test_namespace_hex(&text).then_some(text)
}

#[cfg(test)]
fn is_test_namespace_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn protect_data(
    input: &mut [u8],
    entropy: &[u8; 32],
) -> Result<Vec<u8>, String> {
    let input_len = u32::try_from(input.len())
        .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?;
    let mut entropy_copy = *entropy;
    let input_blob = DataBlob {
        byte_length: input_len,
        bytes: input.as_mut_ptr(),
    };
    let entropy_blob = DataBlob {
        byte_length: u32::try_from(entropy_copy.len())
            .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?,
        bytes: entropy_copy.as_mut_ptr(),
    };
    let mut output = DataBlob {
        byte_length: 0,
        bytes: null_mut(),
    };
    let success = unsafe {
        crypt_protect_data(
            &raw const input_blob,
            null(),
            &raw const entropy_blob,
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    };
    super::zeroize(&mut entropy_copy);
    if success == 0 {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_DPAPI_PROTECT_FAILED:{error}"));
    }
    LocalAllocation(output).into_vec(super::MAX_PROTECTED_OBJECT_BYTES)
}

pub(super) fn unprotect_data(
    input: &mut [u8],
    entropy: &[u8; 32],
) -> Result<Vec<u8>, String> {
    let input_len = u32::try_from(input.len())
        .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?;
    let mut entropy_copy = *entropy;
    let input_blob = DataBlob {
        byte_length: input_len,
        bytes: input.as_mut_ptr(),
    };
    let entropy_blob = DataBlob {
        byte_length: u32::try_from(entropy_copy.len())
            .map_err(|_| "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned())?,
        bytes: entropy_copy.as_mut_ptr(),
    };
    let mut output = DataBlob {
        byte_length: 0,
        bytes: null_mut(),
    };
    let success = unsafe {
        crypt_unprotect_data(
            &raw const input_blob,
            null_mut(),
            &raw const entropy_blob,
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    };
    super::zeroize(&mut entropy_copy);
    if success == 0 {
        let error = unsafe { get_last_error() };
        return Err(format!("DIRECT_DPAPI_UNPROTECT_FAILED:{error}"));
    }
    LocalAllocation(output).into_vec(super::MAX_PROTECTED_OBJECT_BYTES)
}

fn contains_protected_objects(root: &Path) -> Result<bool, String> {
    if !root.exists() {
        return Ok(false);
    }
    let mut observed = 0_usize;
    for shard in fs::read_dir(root)
        .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?
    {
        let shard = shard
            .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
        let metadata = fs::symlink_metadata(shard.path())
            .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err("DIRECT_REVISION_DIRECTORY_LINK_DENIED".to_owned());
        }
        if !metadata.is_dir() {
            continue;
        }
        for entry in fs::read_dir(shard.path())
            .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?
        {
            let entry = entry
                .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
            observed = observed.saturating_add(1);
            if observed > MAX_OBJECT_SCAN {
                return Err("DIRECT_REVISION_OBJECT_LIMIT_EXCEEDED".to_owned());
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("DIRECT_REVISION_DIRECTORY_READ_FAILED:{error}"))?;
            if metadata.file_type().is_symlink() || is_reparse(&metadata) {
                return Err("DIRECT_REVISION_OBJECT_LINK_DENIED".to_owned());
            }
            if path
                .extension()
                .is_some_and(|value| value == std::ffi::OsStr::new(super::PROTECTED_OBJECT_EXTENSION))
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right.iter()) {
        difference |= left ^ right;
    }
    difference == 0
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(core::iter::once(0)).collect()
}

impl super::RevisionProtector {
    /// Resolve only the existing namespace credential for explicit migration.
    /// Missing keys stay missing: no RNG, `CredWrite`, revision scan or conversion.
    /// The reader still refuses any protected object without its original key.
    pub(crate) fn open_existing(namespace_id: [u8; 32]) -> Result<Option<Self>, String> {
        let target = wide(&format!("ELIOT Search/revision-key/{}", sha256::hex(&namespace_id)));
        let Some(secret) = read_credential(&target)? else { return Ok(None); };
        let secret = zeroize::Zeroizing::new(secret);
        let key_binding_digest = sha256::digest_parts(
            b"eliot-search/revision-key-binding/v1", &[&namespace_id, &secret[..]],
        );
        let entropy = sha256::digest_parts(
            b"eliot-search/revision-dpapi-entropy/v1", &[&namespace_id, &secret[..]],
        );
        Ok(Some(Self { namespace_id, key_binding_digest, entropy }))
    }
}
