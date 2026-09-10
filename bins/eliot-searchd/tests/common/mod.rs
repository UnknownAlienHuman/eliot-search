//! Shared best-effort cleanup for Windows Credential Manager entries created
//! by `bins/eliot-searchd` process tests.
//!
//! Every primary-daemon command that opens the revision-protected store on a
//! fresh data root creates one `ELIOT Search/revision-key/<namespace-hex>`
//! generic credential via `CredWriteW`. Each fixture guard below deletes
//! exactly the entries its own temporary tree created when the test finishes —
//! on both the pass and the panic paths, because `Drop` runs on unwind — so
//! entries never accumulate across runs and `CredWrite` never starts failing
//! with `DIRECT_REVISION_KEY_WRITE_FAILED`.
//!
//! Cleanup is strictly scoped: only targets under the exact
//! `ELIOT Search/revision-key/` prefix with exactly 64 hexadecimal characters
//! are ever deleted, and only for data roots inside the fixture's own
//! temporary tree. Every failure — including a missing `namespace.id` file —
//! is ignored so cleanup in `Drop` can never mask a test result.
//!
//! The single `unsafe` block in this module is the `CredDeleteW` Win32 call
//! itself. `cmdkey /delete` cannot be used: it silently fails on target names
//! containing a space.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

const CRED_TYPE_GENERIC: u32 = 1;
const ERROR_NOT_FOUND: u32 = 1_168;
const CREDENTIAL_PREFIX: &str = "ELIOT Search/revision-key/";

#[cfg(windows)]
#[link(name = "Advapi32")]
unsafe extern "system" {
    #[link_name = "CredDeleteW"]
    fn cred_delete_w(target_name: *const u16, credential_type: u32, flags: u32) -> i32;
    #[link_name = "CredReadW"]
    fn cred_read_w(
        target_name: *const u16,
        credential_type: u32,
        flags: u32,
        credential: *mut *mut CredentialW,
    ) -> i32;
    #[link_name = "CredFree"]
    fn cred_free(buffer: *mut core::ffi::c_void);
    #[link_name = "GetLastError"]
    fn get_last_error() -> u32;
}

/// Layout mirror of Win32 `CREDENTIALW`, kept in sync with the same struct in
/// `src/revision_protection_windows.rs`. Only used here to locate and zeroize
/// the secret blob before release during existence checks.
#[cfg(windows)]
#[repr(C)]
struct CredentialW {
    flags: u32,
    credential_type: u32,
    target_name: *mut u16,
    comment: *mut u16,
    last_written_low: u32,
    last_written_high: u32,
    credential_blob_size: u32,
    credential_blob: *mut u8,
    persist: u32,
    attribute_count: u32,
    attributes: *mut core::ffi::c_void,
    target_alias: *mut u16,
    user_name: *mut u16,
}

/// Frees a `CredReadW` allocation, zeroizing secret bytes first.
/// Mirrors the guard in `src/revision_protection_windows.rs`.
#[cfg(windows)]
struct CredentialAllocation(*mut CredentialW);

#[cfg(windows)]
impl Drop for CredentialAllocation {
    fn drop(&mut self) {
        const MAX_BLOB_BYTES: usize = 5 * 512;
        if self.0.is_null() {
            return;
        }
        unsafe {
            let credential = &mut *self.0;
            let size =
                usize::try_from(credential.credential_blob_size).unwrap_or(0).min(MAX_BLOB_BYTES);
            if !credential.credential_blob.is_null() && size > 0 {
                core::ptr::write_bytes(credential.credential_blob, 0, size);
            }
            cred_free(self.0.cast());
        }
    }
}

fn is_namespace_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Read this data root's namespace hex without creating anything: the same
/// `control/namespace.id` file the daemon's namespace initializer owns.
/// A missing or malformed file means there is no credential to delete.
fn read_namespace_hex(data_root: &Path) -> Option<String> {
    let bytes = fs::read(data_root.join("control").join("namespace.id")).ok()?;
    let text = core::str::from_utf8(&bytes).ok()?.trim().to_owned();
    is_namespace_hex(&text).then_some(text)
}

fn delete_for_namespace_hex(namespace_hex: &str) -> bool {
    #[cfg(windows)]
    {
        if !is_namespace_hex(namespace_hex) {
            return true;
        }
        let target = format!("{CREDENTIAL_PREFIX}{namespace_hex}");
        let wide: Vec<u16> = target.encode_utf16().chain(core::iter::once(0)).collect();
        // A full parallel run drops dozens of fixtures within the same
        // moment, and a lone CredDelete can lose to that vault contention;
        // the read-back below is the only signal trusted here. A missing
        // entry is already gone. Every outcome stays best-effort so cleanup
        // in Drop can never mask a test result.
        for attempt in 0..8_u32 {
            unsafe {
                let _ = cred_delete_w(wide.as_ptr(), CRED_TYPE_GENERIC, 0);
            }
            if credential_absent(&wide) {
                return true;
            }
            std::thread::sleep(core::time::Duration::from_millis(
                10_u64 << attempt.min(5),
            ));
        }
        false
    }
    #[cfg(not(windows))]
    {
        let _ = namespace_hex;
        true
    }
}

/// True when no credential exists under `target`. Present-but-unreadable
/// counts as present so the caller retries instead of declaring victory.
/// Never panics.
#[cfg(windows)]
fn credential_absent(wide_target: &[u16]) -> bool {
    let mut pointer = core::ptr::null_mut::<CredentialW>();
    let success = unsafe {
        cred_read_w(wide_target.as_ptr(), CRED_TYPE_GENERIC, 0, &raw mut pointer)
    };
    if success == 0 {
        let error = unsafe { get_last_error() };
        return error == ERROR_NOT_FOUND;
    }
    if pointer.is_null() {
        return false;
    }
    let _allocation = CredentialAllocation(pointer);
    false
}

/// Best-effort deletion of one data root's revision-key credential.
/// Returns true when absence was verified. Never panics; safe to call at the
/// top of a fixture `Drop`.
pub fn delete_revision_key_for_data_root(data_root: &Path) -> bool {
    read_namespace_hex(data_root).is_none_or(|namespace_hex| delete_for_namespace_hex(&namespace_hex))
}

/// Best-effort deletion for every revision-key credential under `base`: the
/// base itself when it is a data root, plus each immediate child directory
/// that is one. Bounded to a single level; only `control/namespace.id`-backed
/// targets are ever deleted. Never panics; unverified deletions are reported
/// via stderr without failing the test.
pub fn delete_revision_keys_under_tree(base: &Path) {
    if !delete_revision_key_for_data_root(base) {
        eprintln!(
            "ELIOT_TEST_CLEANUP: revision-key credential still present after bounded retries: root={}",
            base.display(),
        );
    }
    let entries = match fs::read_dir(base) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(_) => return,
    };
    for entry in entries {
        let path = entry.path();
        let is_dir = fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_dir());
        if is_dir && !delete_revision_key_for_data_root(&path) {
            eprintln!(
                "ELIOT_TEST_CLEANUP: revision-key credential still present after bounded retries: root={}",
                path.display(),
            );
        }
    }
}

/// Drop-based guard for a fixture with one known data root.
///
/// Construct before the first daemon invocation; call [`Self::refresh`]
/// after each invocation so the original namespace stays captured even if a
/// later step deletes `control/namespace.id` (catalog-loss regressions do).
/// [`Self::cleanup`] deletes the captured credential, falling back to the
/// current `namespace.id` file when nothing was captured. Best-effort and
/// never panics.
pub struct RevisionKeyGuard {
    data_root: PathBuf,
    captured: RefCell<Option<String>>,
}

impl RevisionKeyGuard {
    /// Guard the credential belonging to `data_root`. Nothing is read or
    /// deleted until [`Self::refresh`]/[`Self::cleanup`] runs.
    pub fn for_data_root(data_root: &Path) -> Self {
        Self { data_root: data_root.to_owned(), captured: RefCell::new(None) }
    }

    /// Remember the current `control/namespace.id` value, if any. First value
    /// wins: the first namespace is the one whose credential exists; a
    /// replacement file written by a later failed open has no credential.
    pub fn refresh(&self) {
        if self.captured.try_borrow().is_ok_and(|captured| captured.is_some()) {
            return;
        }
        if let Some(namespace_hex) = read_namespace_hex(&self.data_root)
            && let Ok(mut captured) = self.captured.try_borrow_mut()
            && captured.is_none()
        {
            *captured = Some(namespace_hex);
        }
    }

    /// Delete the guarded credential. Best-effort, never panics. Call at the
    /// top of a fixture `Drop`, before the temporary directory (and
    /// `namespace.id` with it) is removed.
    pub fn cleanup(&self) {
        let captured =
            self.captured.try_borrow().ok().and_then(|captured| captured.clone());
        let verified = captured.map_or_else(
            || delete_revision_key_for_data_root(&self.data_root),
            |namespace_hex| delete_for_namespace_hex(&namespace_hex),
        );
        if !verified {
            // Best-effort cleanup exhausted its bounded retries without
            // verifying the entry gone. Report, never panic: cleanup must
            // not mask the test result.
            eprintln!(
                "ELIOT_TEST_CLEANUP: revision-key credential still present after bounded retries: root={}",
                self.data_root.display(),
            );
        }
    }
}

impl Drop for RevisionKeyGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Drop-based guard for a fixture whose data root varies per test (either the
/// base itself or one level below it). Cleanup scans the tree; see
/// [`delete_revision_keys_under_tree`]. Best-effort and never panics.
pub struct RevisionKeyTreeGuard {
    base: PathBuf,
}

impl RevisionKeyTreeGuard {
    /// Guard every revision-key credential under `base`.
    pub fn for_tree(base: &Path) -> Self {
        Self { base: base.to_owned() }
    }

    /// Delete the guarded credentials. Best-effort, never panics. Call at the
    /// top of a fixture `Drop`, before the temporary tree is removed.
    pub fn cleanup(&self) {
        delete_revision_keys_under_tree(&self.base);
    }
}

impl Drop for RevisionKeyTreeGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}
