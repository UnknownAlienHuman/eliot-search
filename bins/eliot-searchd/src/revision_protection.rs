//! Platform revision-object protection for the daemon composition root.
//!
//! Protected objects are never interpreted as plaintext. Legacy `.bin` reads
//! belong to the explicit migration/development paths, not to this decoder.
//! Both the outer envelope and authenticated inner envelope bind namespace,
//! key, revision, content digest and exact plaintext length.

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
#[cfg(windows)]
use zeroize::{Zeroize, Zeroizing};

use crate::sha256;

pub(crate) const PROTECTED_OBJECT_EXTENSION: &str = "dpapi";
const OUTER_MAGIC: [u8; 8] = *b"ELSRV2\0\0";
#[cfg(any(windows, test))]
const INNER_MAGIC: [u8; 8] = *b"ELSIN2\0\0";
const OBJECT_VERSION: u32 = 1;
const MAX_PLAINTEXT_BYTES: usize = 64 * 1024 * 1024;
const MAX_PROTECTED_OBJECT_BYTES: usize = 65 * 1024 * 1024;
const OUTER_HEADER_BYTES: usize = 8 + 4 + 32 + 32 + 32 + 32 + 8 + 8;
#[cfg(any(windows, test))]
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
        formatter.debug_struct("RevisionProtector")
            .field("namespace_id", &sha256::hex(&self.namespace_id))
            .field("backend", &self.backend_name())
            .field("secret", &"<redacted>")
            .finish()
    }
}

#[cfg(windows)]
impl Drop for RevisionProtector {
    fn drop(&mut self) {
        self.entropy.zeroize();
    }
}

impl RevisionProtector {
    /// Opens the platform protector. Existing protected objects require the
    /// original credential; a missing credential is not silently replaced.
    pub(crate) fn open(namespace_id: [u8; 32], revision_root: &Path) -> Result<Self, String> {
        #[cfg(windows)]
        {
            let root_secret = Zeroizing::new(windows::load_or_create_root_secret(
                namespace_id, revision_root,
            )?);
            let key_binding_digest = sha256::digest_parts(
                b"eliot-search/revision-key-binding/v1", &[&namespace_id, &root_secret[..]],
            );
            let entropy = sha256::digest_parts(
                b"eliot-search/revision-dpapi-entropy/v1", &[&namespace_id, &root_secret[..]],
            );
            Ok(Self { namespace_id, key_binding_digest, entropy })
        }
        #[cfg(not(windows))]
        {
            let _ = revision_root;
            Ok(Self { namespace_id })
        }
    }

    #[must_use]
    pub(crate) const fn backend_name(&self) -> &'static str {
        #[cfg(windows)]
        { "windows-dpapi-credential-manager-v1" }
        #[cfg(not(windows))]
        { "plaintext-development-v1" }
    }

    #[must_use]
    pub(crate) const fn encrypts_new_objects(&self) -> bool { cfg!(windows) }

    /// Format marker only, not proof that an object authenticates or decrypts.
    #[must_use]
    pub(crate) fn is_protected_object(bytes: &[u8]) -> bool {
        bytes.starts_with(&OUTER_MAGIC)
    }

    pub(crate) fn protect(
        &self, revision_id: &str, content_digest: &str, plaintext: &[u8],
    ) -> Result<Vec<u8>, String> {
        let revision_id = decode_digest(revision_id, "DIRECT_REVISION_ID_INVALID")?;
        let content_digest = decode_digest(content_digest, "DIRECT_CONTENT_DIGEST_INVALID")?;
        if plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err("DIRECT_REVISION_PLAINTEXT_TOO_LARGE".to_owned());
        }
        // Validate before invoking native encryption, including the explicit
        // development branch. A caller's digest is not evidence by itself.
        if sha256::digest(plaintext) != content_digest {
            return Err("DIRECT_REVISION_CONTENT_MISMATCH".to_owned());
        }
        #[cfg(windows)]
        { self.protect_windows(revision_id, content_digest, plaintext) }
        #[cfg(not(windows))]
        {
            let _ = revision_id;
            Ok(plaintext.to_vec())
        }
    }

    /// Opens a protected object only. There is deliberately no plaintext fallback,
    /// even if raw bytes have the expected digest and length.
    pub(crate) fn unprotect(
        &self, object: &[u8], revision_id: &str, content_digest: &str,
        expected_plaintext_len: u64,
    ) -> Result<Vec<u8>, String> {
        if object.len() > MAX_PROTECTED_OBJECT_BYTES {
            return Err("DIRECT_REVISION_OBJECT_TOO_LARGE".to_owned());
        }
        if expected_plaintext_len > MAX_PLAINTEXT_BYTES as u64 {
            return Err("DIRECT_REVISION_PLAINTEXT_TOO_LARGE".to_owned());
        }
        let revision_id = decode_digest(revision_id, "DIRECT_REVISION_ID_INVALID")?;
        let content_digest = decode_digest(content_digest, "DIRECT_CONTENT_DIGEST_INVALID")?;
        if !Self::is_protected_object(object) {
            return Err("DIRECT_REVISION_PROTECTED_FORMAT_REQUIRED".to_owned());
        }
        #[cfg(windows)]
        let expected_key = Some(self.key_binding_digest);
        #[cfg(not(windows))]
        let expected_key = None;
        let expected = ExpectedRevision {
            namespace_id: self.namespace_id, revision_id, content_digest,
            plaintext_len: expected_plaintext_len,
        };
        let (binding, ciphertext) = decode_outer(object, expected, expected_key)?;
        #[cfg(windows)]
        {
            let mut ciphertext = ciphertext.to_vec();
            let inner = Zeroizing::new(windows::unprotect_data(&mut ciphertext, &self.entropy)?);
            // Validate the borrowed plaintext before allocating a return value;
            // every rejected inner envelope stays inside the zeroizing owner.
            Ok(decode_inner(&inner, binding)?.to_vec())
        }
        #[cfg(not(windows))]
        {
            let _ = (binding, ciphertext);
            Err("DIRECT_REVISION_ENCRYPTION_UNAVAILABLE".to_owned())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedRevision {
    namespace_id: [u8; 32],
    revision_id: [u8; 32],
    content_digest: [u8; 32],
    plaintext_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Binding {
    revision: ExpectedRevision,
    key_binding_digest: [u8; 32],
}

fn decode_outer(
    object: &[u8], expected: ExpectedRevision, expected_key: Option<[u8; 32]>,
) -> Result<(Binding, &[u8]), String> {
    if object.len() < OUTER_HEADER_BYTES || object.len() > MAX_PROTECTED_OBJECT_BYTES
        || expected.plaintext_len > MAX_PLAINTEXT_BYTES as u64
    {
        return Err("DIRECT_REVISION_ENVELOPE_INVALID".to_owned());
    }
    let mut cursor = 0;
    if take::<8>(object, &mut cursor)? != OUTER_MAGIC
        || u32::from_be_bytes(take(object, &mut cursor)?) != OBJECT_VERSION
    {
        return Err("DIRECT_REVISION_ENVELOPE_INVALID".to_owned());
    }
    let binding = decode_binding(object, &mut cursor)?;
    if binding.revision.namespace_id != expected.namespace_id {
        return Err("DIRECT_REVISION_NAMESPACE_MISMATCH".to_owned());
    }
    if expected_key.is_some_and(|key| key != binding.key_binding_digest) {
        return Err("DIRECT_REVISION_KEY_BINDING_MISMATCH".to_owned());
    }
    if binding.revision != expected {
        return Err("DIRECT_REVISION_ENVELOPE_BINDING_MISMATCH".to_owned());
    }
    let ciphertext_len = usize::try_from(u64::from_be_bytes(take(object, &mut cursor)?))
        .map_err(|_| "DIRECT_REVISION_ENVELOPE_INVALID".to_owned())?;
    if ciphertext_len == 0 || object.len().checked_sub(cursor) != Some(ciphertext_len) {
        return Err("DIRECT_REVISION_ENVELOPE_INVALID".to_owned());
    }
    Ok((binding, &object[cursor..]))
}

#[cfg(any(windows, test))]
fn decode_inner(inner: &[u8], expected: Binding) -> Result<&[u8], String> {
    if inner.len() < INNER_HEADER_BYTES || inner.len() > INNER_HEADER_BYTES + MAX_PLAINTEXT_BYTES {
        return Err("DIRECT_REVISION_INNER_ENVELOPE_INVALID".to_owned());
    }
    let mut cursor = 0;
    if take::<8>(inner, &mut cursor)? != INNER_MAGIC
        || u32::from_be_bytes(take(inner, &mut cursor)?) != OBJECT_VERSION
    {
        return Err("DIRECT_REVISION_INNER_ENVELOPE_INVALID".to_owned());
    }
    if decode_binding(inner, &mut cursor)? != expected {
        return Err("DIRECT_REVISION_INNER_BINDING_MISMATCH".to_owned());
    }
    let plaintext_len = usize::try_from(expected.revision.plaintext_len)
        .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH".to_owned())?;
    if inner.len().checked_sub(cursor) != Some(plaintext_len) {
        return Err("DIRECT_REVISION_LENGTH_MISMATCH".to_owned());
    }
    let plaintext = &inner[cursor..];
    if sha256::digest(plaintext) != expected.revision.content_digest {
        return Err("DIRECT_REVISION_CONTENT_MISMATCH".to_owned());
    }
    Ok(plaintext)
}

fn decode_binding(bytes: &[u8], cursor: &mut usize) -> Result<Binding, String> {
    let namespace_id = take(bytes, cursor)?;
    let key_binding_digest = take(bytes, cursor)?;
    let revision_id = take(bytes, cursor)?;
    let content_digest = take(bytes, cursor)?;
    let plaintext_len = u64::from_be_bytes(take(bytes, cursor)?);
    Ok(Binding {
        revision: ExpectedRevision { namespace_id, revision_id, content_digest, plaintext_len },
        key_binding_digest,
    })
}

#[cfg(any(windows, test))]
fn encode_binding(output: &mut Vec<u8>, binding: Binding) {
    output.extend_from_slice(&binding.revision.namespace_id);
    output.extend_from_slice(&binding.key_binding_digest);
    output.extend_from_slice(&binding.revision.revision_id);
    output.extend_from_slice(&binding.revision.content_digest);
    output.extend_from_slice(&binding.revision.plaintext_len.to_be_bytes());
}

#[cfg(windows)]
impl RevisionProtector {
    fn protect_windows(
        &self, revision_id: [u8; 32], content_digest: [u8; 32], plaintext: &[u8],
    ) -> Result<Vec<u8>, String> {
        let binding = Binding {
            revision: ExpectedRevision {
                namespace_id: self.namespace_id, revision_id, content_digest,
                plaintext_len: u64::try_from(plaintext.len())
                    .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH".to_owned())?,
            },
            key_binding_digest: self.key_binding_digest,
        };
        let mut inner = Zeroizing::new(Vec::with_capacity(INNER_HEADER_BYTES + plaintext.len()));
        inner.extend_from_slice(&INNER_MAGIC);
        inner.extend_from_slice(&OBJECT_VERSION.to_be_bytes());
        encode_binding(&mut inner, binding);
        inner.extend_from_slice(plaintext);
        let protected = windows::protect_data(&mut inner, &self.entropy)?;
        if protected.is_empty() || protected.len() > MAX_PROTECTED_OBJECT_BYTES - OUTER_HEADER_BYTES {
            return Err("DIRECT_REVISION_OBJECT_TOO_LARGE".to_owned());
        }
        let mut object = Vec::with_capacity(OUTER_HEADER_BYTES + protected.len());
        object.extend_from_slice(&OUTER_MAGIC);
        object.extend_from_slice(&OBJECT_VERSION.to_be_bytes());
        encode_binding(&mut object, binding);
        object.extend_from_slice(&(protected.len() as u64).to_be_bytes());
        object.extend_from_slice(&protected);
        Ok(object)
    }
}

fn decode_digest(value: &str, error: &'static str) -> Result<[u8; 32], String> {
    sha256::decode_digest(value).ok_or_else(|| error.to_owned())
}

fn take<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], String> {
    let end = cursor.checked_add(N)
        .ok_or_else(|| "DIRECT_REVISION_ENVELOPE_INVALID".to_owned())?;
    let value = bytes.get(*cursor..end)
        .ok_or_else(|| "DIRECT_REVISION_ENVELOPE_INVALID".to_owned())?;
    *cursor = end;
    value.try_into().map_err(|_| "DIRECT_REVISION_ENVELOPE_INVALID".to_owned())
}

// Native allocation cleanup in the existing FFI adapter uses this same helper.
#[cfg(windows)]
fn zeroize(bytes: &mut [u8]) { bytes.zeroize(); }

#[cfg(windows)]
#[path = "revision_protection_windows.rs"]
mod windows;

#[cfg(test)]
#[path = "revision_protection_tests.rs"]
mod tests;
