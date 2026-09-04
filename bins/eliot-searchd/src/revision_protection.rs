//! Platform revision-object protection for the daemon composition root.
//!
//! Windows uses one Credential Manager secret per data-root namespace as DPAPI
//! optional entropy. The protected object contains a versioned outer binding and
//! a second copy of every load-bearing field inside the authenticated DPAPI
//! payload. Other platforms retain the explicit plaintext development profile.

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unused_self
)]

use core::fmt;
use std::path::Path;

use crate::sha256;

pub(crate) const PROTECTED_OBJECT_EXTENSION: &str = "dpapi";
const OUTER_MAGIC: [u8; 8] = *b"ELSRV2\0\0";
#[cfg(windows)]
const INNER_MAGIC: [u8; 8] = *b"ELSIN2\0\0";
#[cfg(windows)]
const OBJECT_VERSION: u32 = 1;
const MAX_PROTECTED_OBJECT_BYTES: usize = 65 * 1024 * 1024;
#[cfg(windows)]
const OUTER_HEADER_BYTES: usize = 8 + 4 + 32 + 32 + 32 + 32 + 8 + 8;
#[cfg(windows)]
const INNER_HEADER_BYTES: usize = 8 + 4 + 32 + 32 + 32 + 32 + 8;

/// Per-namespace revision protection capability.
pub(crate) struct RevisionProtector {
    namespace_id: [u8; 32],
    #[cfg(windows)]
    key_binding_digest: [u8; 32],
    #[cfg(windows)]
    entropy: [u8; 32],
}

impl fmt::Debug for RevisionProtector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RevisionProtector")
            .field("namespace_id", &sha256::hex(&self.namespace_id))
            .field("backend", &self.backend_name())
            .field("secret", &"<redacted>")
            .finish()
    }
}

#[cfg(windows)]
impl Drop for RevisionProtector {
    fn drop(&mut self) {
        zeroize(&mut self.entropy);
    }
}

impl RevisionProtector {
    /// Opens the platform protector and creates a new credential only when no
    /// existing protected object proves that a prior secret must be recovered.
    pub(crate) fn open(
        namespace_id: [u8; 32],
        revision_root: &Path,
    ) -> Result<Self, String> {
        #[cfg(windows)]
        {
            let mut root_secret = windows::load_or_create_root_secret(
                namespace_id,
                revision_root,
            )?;
            let key_binding_digest = sha256::digest_parts(
                b"eliot-search/revision-key-binding/v1",
                &[&namespace_id, &root_secret],
            );
            let entropy = sha256::digest_parts(
                b"eliot-search/revision-dpapi-entropy/v1",
                &[&namespace_id, &root_secret],
            );
            zeroize(&mut root_secret);
            return Ok(Self {
                namespace_id,
                key_binding_digest,
                entropy,
            });
        }
        #[cfg(not(windows))]
        {
            let _ = revision_root;
            Ok(Self { namespace_id })
        }
    }

    /// Stable backend name for status and receipts.
    #[must_use]
    pub(crate) const fn backend_name(&self) -> &'static str {
        #[cfg(windows)]
        {
            "windows-dpapi-credential-manager-v1"
        }
        #[cfg(not(windows))]
        {
            "plaintext-development-v1"
        }
    }

    /// Whether newly persisted revision objects are protected at rest.
    #[must_use]
    pub(crate) const fn encrypts_new_objects(&self) -> bool {
        cfg!(windows)
    }

    /// Returns whether bytes carry the versioned protected-object marker.
    #[must_use]
    pub(crate) fn is_protected_object(bytes: &[u8]) -> bool {
        bytes.starts_with(&OUTER_MAGIC)
    }

    /// Protects one exact immutable revision object.
    pub(crate) fn protect(
        &self,
        revision_id: &str,
        content_digest: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, String> {
        let revision_id = decode_digest(revision_id, "DIRECT_REVISION_ID_INVALID")?;
        let content_digest = decode_digest(
            content_digest,
            "DIRECT_CONTENT_DIGEST_INVALID",
        )?;
        if plaintext.len() > 64 * 1024 * 1024 {
            return Err("DIRECT_REVISION_PLAINTEXT_TOO_LARGE".to_owned());
        }
        #[cfg(windows)]
        {
            self.protect_windows(revision_id, content_digest, plaintext)
        }
        #[cfg(not(windows))]
        {
            let _ = (revision_id, content_digest);
            Ok(plaintext.to_vec())
        }
    }

    /// Opens one protected or explicitly legacy plaintext object.
    pub(crate) fn unprotect(
        &self,
        object: &[u8],
        revision_id: &str,
        content_digest: &str,
        expected_plaintext_len: u64,
    ) -> Result<Vec<u8>, String> {
        if object.len() > MAX_PROTECTED_OBJECT_BYTES {
            return Err("DIRECT_REVISION_OBJECT_TOO_LARGE".to_owned());
        }
        let revision_id = decode_digest(revision_id, "DIRECT_REVISION_ID_INVALID")?;
        let content_digest = decode_digest(
            content_digest,
            "DIRECT_CONTENT_DIGEST_INVALID",
        )?;
        if !Self::is_protected_object(object) {
            let observed = u64::try_from(object.len())
                .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH".to_owned())?;
            if observed != expected_plaintext_len {
                return Err("DIRECT_REVISION_LENGTH_MISMATCH".to_owned());
            }
            return Ok(object.to_vec());
        }
        #[cfg(windows)]
        {
            self.unprotect_windows(
                object,
                revision_id,
                content_digest,
                expected_plaintext_len,
            )
        }
        #[cfg(not(windows))]
        {
            let _ = (revision_id, content_digest, expected_plaintext_len);
            Err("DIRECT_REVISION_ENCRYPTION_UNAVAILABLE".to_owned())
        }
    }
}

#[cfg(windows)]
impl RevisionProtector {
    fn protect_windows(
        &self,
        revision_id: [u8; 32],
        content_digest: [u8; 32],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, String> {
        let plaintext_len = u64::try_from(plaintext.len())
            .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH".to_owned())?;
        let mut inner = Vec::with_capacity(INNER_HEADER_BYTES + plaintext.len());
        inner.extend_from_slice(&INNER_MAGIC);
        inner.extend_from_slice(&OBJECT_VERSION.to_be_bytes());
        inner.extend_from_slice(&self.namespace_id);
        inner.extend_from_slice(&self.key_binding_digest);
        inner.extend_from_slice(&revision_id);
        inner.extend_from_slice(&content_digest);
        inner.extend_from_slice(&plaintext_len.to_be_bytes());
        inner.extend_from_slice(plaintext);

        let protected = windows::protect_data(&mut inner, &self.entropy);
        zeroize(&mut inner);
        let protected = protected?;
        let protected_len = u64::try_from(protected.len())
            .map_err(|_| "DIRECT_REVISION_OBJECT_TOO_LARGE".to_owned())?;
        let total = OUTER_HEADER_BYTES
            .checked_add(protected.len())
            .ok_or_else(|| "DIRECT_REVISION_OBJECT_TOO_LARGE".to_owned())?;
        if total > MAX_PROTECTED_OBJECT_BYTES {
            return Err("DIRECT_REVISION_OBJECT_TOO_LARGE".to_owned());
        }

        let mut object = Vec::with_capacity(total);
        object.extend_from_slice(&OUTER_MAGIC);
        object.extend_from_slice(&OBJECT_VERSION.to_be_bytes());
        object.extend_from_slice(&self.namespace_id);
        object.extend_from_slice(&self.key_binding_digest);
        object.extend_from_slice(&revision_id);
        object.extend_from_slice(&content_digest);
        object.extend_from_slice(&plaintext_len.to_be_bytes());
        object.extend_from_slice(&protected_len.to_be_bytes());
        object.extend_from_slice(&protected);
        Ok(object)
    }

    fn unprotect_windows(
        &self,
        object: &[u8],
        expected_revision_id: [u8; 32],
        expected_content_digest: [u8; 32],
        expected_plaintext_len: u64,
    ) -> Result<Vec<u8>, String> {
        let mut cursor = 0_usize;
        let magic = take::<8>(object, &mut cursor)?;
        let version = read_u32(object, &mut cursor)?;
        let namespace_id = take::<32>(object, &mut cursor)?;
        let key_binding_digest = take::<32>(object, &mut cursor)?;
        let revision_id = take::<32>(object, &mut cursor)?;
        let content_digest = take::<32>(object, &mut cursor)?;
        let plaintext_len = read_u64(object, &mut cursor)?;
        let protected_len = read_u64(object, &mut cursor)?;
        if magic != OUTER_MAGIC || version != OBJECT_VERSION {
            return Err("DIRECT_REVISION_ENVELOPE_INVALID".to_owned());
        }
        if namespace_id != self.namespace_id {
            return Err("DIRECT_REVISION_NAMESPACE_MISMATCH".to_owned());
        }
        if key_binding_digest != self.key_binding_digest {
            return Err("DIRECT_REVISION_KEY_BINDING_MISMATCH".to_owned());
        }
        if revision_id != expected_revision_id
            || content_digest != expected_content_digest
            || plaintext_len != expected_plaintext_len
        {
            return Err("DIRECT_REVISION_ENVELOPE_BINDING_MISMATCH".to_owned());
        }
        let protected_len = usize::try_from(protected_len)
            .map_err(|_| "DIRECT_REVISION_ENVELOPE_INVALID".to_owned())?;
        if object.len().saturating_sub(cursor) != protected_len {
            return Err("DIRECT_REVISION_ENVELOPE_INVALID".to_owned());
        }
        let mut protected = object[cursor..].to_vec();
        let inner_result = windows::unprotect_data(&mut protected, &self.entropy);
        zeroize(&mut protected);
        let mut inner = inner_result?;
        let result = self.parse_inner(
            &inner,
            expected_revision_id,
            expected_content_digest,
            expected_plaintext_len,
        );
        zeroize(&mut inner);
        result
    }

    fn parse_inner(
        &self,
        inner: &[u8],
        expected_revision_id: [u8; 32],
        expected_content_digest: [u8; 32],
        expected_plaintext_len: u64,
    ) -> Result<Vec<u8>, String> {
        let mut cursor = 0_usize;
        let magic = take::<8>(inner, &mut cursor)?;
        let version = read_u32(inner, &mut cursor)?;
        let namespace_id = take::<32>(inner, &mut cursor)?;
        let key_binding_digest = take::<32>(inner, &mut cursor)?;
        let revision_id = take::<32>(inner, &mut cursor)?;
        let content_digest = take::<32>(inner, &mut cursor)?;
        let plaintext_len = read_u64(inner, &mut cursor)?;
        if magic != INNER_MAGIC || version != OBJECT_VERSION {
            return Err("DIRECT_REVISION_INNER_ENVELOPE_INVALID".to_owned());
        }
        if namespace_id != self.namespace_id
            || key_binding_digest != self.key_binding_digest
            || revision_id != expected_revision_id
            || content_digest != expected_content_digest
            || plaintext_len != expected_plaintext_len
        {
            return Err("DIRECT_REVISION_INNER_BINDING_MISMATCH".to_owned());
        }
        let plaintext_len = usize::try_from(plaintext_len)
            .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH".to_owned())?;
        if inner.len().saturating_sub(cursor) != plaintext_len {
            return Err("DIRECT_REVISION_LENGTH_MISMATCH".to_owned());
        }
        let plaintext = inner[cursor..].to_vec();
        if sha256::digest(&plaintext) != expected_content_digest {
            return Err("DIRECT_REVISION_CONTENT_MISMATCH".to_owned());
        }
        Ok(plaintext)
    }
}

fn decode_digest(value: &str, error: &'static str) -> Result<[u8; 32], String> {
    sha256::decode_digest(value).ok_or_else(|| error.to_owned())
}

#[cfg(windows)]
fn take<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], String> {
    let end = cursor
        .checked_add(N)
        .ok_or_else(|| "DIRECT_REVISION_ENVELOPE_INVALID".to_owned())?;
    let value = bytes
        .get(*cursor..end)
        .ok_or_else(|| "DIRECT_REVISION_ENVELOPE_INVALID".to_owned())?;
    *cursor = end;
    value
        .try_into()
        .map_err(|_| "DIRECT_REVISION_ENVELOPE_INVALID".to_owned())
}

#[cfg(windows)]
fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(take::<4>(bytes, cursor)?))
}

#[cfg(windows)]
fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, String> {
    Ok(u64::from_be_bytes(take::<8>(bytes, cursor)?))
}

#[cfg(windows)]
fn zeroize(bytes: &mut [u8]) {
    bytes.fill(0);
    let _ = std::hint::black_box(bytes);
}

#[cfg(windows)]
#[path = "revision_protection_windows.rs"]
mod windows;
