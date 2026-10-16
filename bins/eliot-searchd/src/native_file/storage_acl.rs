//! Closed Windows owner/SYSTEM/Administrators storage policy, never an access grant.
//!
//! SDK 10.0.26100.0 layouts and Microsoft GetSecurityInfo (2021-10-13),
//! ACCESS_ALLOWED_ACE (2024-02-22), and ACE inheritance rules (2025-02-06).
//! Fingerprints describe copied semantics only; callers must reobserve live FDs.

use core::fmt;
use search_contracts::{self as c, CanonicalValue as V};
use std::{fs::File, path::Path};

/// Work charged to the original Native invocation before the corresponding action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageSecurityWork {
    /// One possible ABI call, including error capture and release.
    NativeCall,
    /// One directory creation/open attempt.
    Open,
    /// Requested owned-buffer capacity.
    Allocate(usize),
    /// Bounded validation, copying, encoding and hashing work.
    Process(usize),
}

/// Original caller deadline, cancellation and cumulative allowance; no default renewal.
pub trait StorageSecurityContext {
    /// Original typed refusal, retained without formatting it.
    type Error;
    /// Poll original custody, cancellation and deadline.
    fn checkpoint(&mut self) -> Result<(), Self::Error>;
    /// Reserve cumulative work before its action; refusal remains sticky.
    fn charge(&mut self, work: StorageSecurityWork) -> Result<(), Self::Error>;
}

/// Closed failures with no SID, path, descriptor or numeric OS diagnostic.
pub enum StorageAclError<E> {
    /// This platform cannot perform the qualified Windows observation.
    UnsupportedPlatform,
    /// Original caller refusal before any possible creation effect.
    Context(E),
    /// The held-handle query or descriptor release failed.
    ObservationFailed,
    /// Descriptor, ACL, ACE or SID framing is unsupported or malformed.
    InvalidDescriptor,
    /// Owner or trustees do not meet the closed private policy.
    PolicyRefused,
    /// Shared bounded canonical encoding/hashing refused.
    Encoding,
    /// The exact creation call reported an existing target; nothing was adopted.
    AlreadyExists,
    /// The exact creation call failed; no usable directory result was issued.
    CreateFailed,
    /// Creation succeeded but its original post-call checkpoint refused.
    CreateOutcomeUnknown(E),
}
impl<E> StorageAclError<E> {
    /// Stable redacted reason code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "STORAGE_ACL_UNSUPPORTED_PLATFORM",
            Self::Context(_) => "STORAGE_ACL_CONTEXT_REFUSED",
            Self::ObservationFailed => "STORAGE_ACL_OBSERVATION_FAILED",
            Self::InvalidDescriptor => "STORAGE_ACL_DESCRIPTOR_INVALID",
            Self::PolicyRefused => "STORAGE_ACL_POLICY_REFUSED",
            Self::Encoding => "STORAGE_ACL_ENCODING_REFUSED",
            Self::AlreadyExists => "STORAGE_ACL_ALREADY_EXISTS",
            Self::CreateFailed => "STORAGE_ACL_CREATE_FAILED",
            Self::CreateOutcomeUnknown(_) => "STORAGE_ACL_CREATE_OUTCOME_UNKNOWN",
        }
    }
}
impl<E> fmt::Debug for StorageAclError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl<E> fmt::Display for StorageAclError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl<E> std::error::Error for StorageAclError<E> {}

const MAX_SID: usize = 68;
const SYSTEM: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0];
const ADMIN: &[u8] = &[1, 2, 0, 0, 0, 0, 0, 5, 32, 0, 0, 0, 32, 2, 0, 0];
const OWNER_RIGHTS: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 3, 4, 0, 0, 0];
const CREATOR_OWNER: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0];
const DACL_POLICY_BITS: u16 = 0x150d; // owner-defaulted, present/defaulted, auto-inherit, protected
const UNCLAIMED_SD_BITS: u16 = 0x2a32; // group-defaulted and SACL control bits, not requested/grants
const SUPPORTED_ACCESS_MASK: u32 = 0x001f_01ff | 0xf000_0000;
const SELF_RELATIVE: u16 = 0x8000;

// Fixed DWORD-aligned copied storage, never exposed through the public facade.
#[derive(Clone, Eq, PartialEq)]
struct OwnerSid {
    words: [u32; MAX_SID / 4],
}
impl OwnerSid {
    fn copy<E>(sid: &[u8]) -> Result<Self, StorageAclError<E>> {
        if !valid_sid(sid) {
            return Err(StorageAclError::InvalidDescriptor);
        }
        let mut words = [0; MAX_SID / 4];
        for (destination, bytes) in words.iter_mut().zip(sid.as_chunks::<4>().0) {
            *destination = u32::from_ne_bytes(*bytes);
        }
        Ok(Self { words })
    }
}

/// Fresh qualified copied owner/DACL semantics; possession is not current permission proof.
#[derive(Clone, Eq, PartialEq)]
pub struct StorageAclObservation {
    owner: OwnerSid,
    fingerprint: [u8; 32],
}
impl StorageAclObservation {
    /// Canonical semantic equality only; never a replacement for fresh observation.
    #[must_use]
    pub const fn semantic_fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }
}
impl fmt::Debug for StorageAclObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StorageAclObservation(<private owner and ACL>)")
    }
}

/// Queries OWNER/DACL on the exact held FD without opening a path or a user token.
pub fn observe_owner_private_acl<C: StorageSecurityContext>(
    file: &File,
    expected_current_user_digest: [u8; 32],
    context: &mut C,
) -> Result<StorageAclObservation, StorageAclError<C::Error>> {
    context.checkpoint().map_err(StorageAclError::Context)?;
    #[cfg(windows)]
    {
        windows::observe(file, expected_current_user_digest, context)
    }
    #[cfg(not(windows))]
    {
        let _ = (file, expected_current_user_digest);
        Err(StorageAclError::UnsupportedPlatform)
    }
}

/// Attempts one atomic creation with an explicit protected inheritable three-ACE DACL.
///
/// The qualified root's privately copied owner supplies the SID. Existing targets
/// are never changed or adopted. Success describes creation only; Root must open,
/// qualify and fence the created object under its original Native custody.
pub fn create_owner_private_directory<C: StorageSecurityContext>(
    path: &Path,
    root_owner: &StorageAclObservation,
    context: &mut C,
) -> Result<(), StorageAclError<C::Error>> {
    context.checkpoint().map_err(StorageAclError::Context)?;
    #[cfg(windows)]
    {
        windows::create(path, root_owner, context)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, root_owner);
        Err(StorageAclError::UnsupportedPlatform)
    }
}

#[cfg(any(windows, test))]
fn creation_result<E>(
    created: bool,
    already_exists: bool,
    after: Result<(), E>,
) -> Result<(), StorageAclError<E>> {
    if !created {
        return Err(if already_exists {
            StorageAclError::AlreadyExists
        } else {
            StorageAclError::CreateFailed
        });
    }
    after.map_err(StorageAclError::CreateOutcomeUnknown)
}

fn valid_sid(sid: &[u8]) -> bool {
    sid.len() >= 8
        && sid.len() <= MAX_SID
        && sid[0] == 1
        && sid[1] <= 15
        && sid.len() == 8 + 4 * usize::from(sid[1])
}
fn word<E>(bytes: &[u8], offset: usize) -> Result<u16, StorageAclError<E>> {
    let pair = bytes
        .get(offset..offset + 2)
        .ok_or(StorageAclError::InvalidDescriptor)?;
    Ok(u16::from_le_bytes([pair[0], pair[1]]))
}
fn byte_value<E>(bytes: &[u8]) -> Result<V, StorageAclError<E>> {
    c::BoundedBytes::new(bytes.to_vec())
        .map(V::Bytes)
        .map_err(|_| StorageAclError::Encoding)
}
fn array<E>(values: Vec<V>) -> Result<V, StorageAclError<E>> {
    c::BoundedList::new(values)
        .map(V::Array)
        .map_err(|_| StorageAclError::Encoding)
}

// Pure parser sees only already bounded descriptor slices; no native pointer API.
fn qualify<E>(
    owner: &[u8],
    control: u16,
    acl: &[u8],
    expected: [u8; 32],
) -> Result<StorageAclObservation, StorageAclError<E>> {
    if !valid_sid(owner)
        || acl.len() < 8
        || !matches!(acl[0], 2 | 4)
        || acl[1] != 0
        || word(acl, 6)? != 0
        || usize::from(word(acl, 2)?) != acl.len()
        || control & SELF_RELATIVE == 0
        || control & 4 == 0
        || control & !(SELF_RELATIVE | DACL_POLICY_BITS | UNCLAIMED_SD_BITS) != 0
    {
        return Err(StorageAclError::InvalidDescriptor);
    }
    if search_os_secrets::hash_native_revision_user_sid(owner) != expected {
        return Err(StorageAclError::PolicyRefused);
    }
    let count = usize::from(word(acl, 4)?);
    if count == 0 {
        return Err(StorageAclError::PolicyRefused);
    }
    if count > (acl.len() - 8) / 16 {
        return Err(StorageAclError::InvalidDescriptor);
    }
    let mut rows = Vec::with_capacity(count);
    let mut offset = 8;
    for _ in 0..count {
        let length = usize::from(word(acl, offset + 2)?);
        let end = offset
            .checked_add(length)
            .ok_or(StorageAclError::InvalidDescriptor)?;
        let ace = acl
            .get(offset..end)
            .ok_or(StorageAclError::InvalidDescriptor)?;
        if length < 16 || length % 4 != 0 || ace[0] != 0 || ace[1] & !0x1f != 0 {
            return Err(StorageAclError::InvalidDescriptor);
        }
        let sid = &ace[8..];
        if !valid_sid(sid) {
            return Err(StorageAclError::InvalidDescriptor);
        }
        if sid != owner
            && sid != SYSTEM
            && sid != ADMIN
            && sid != OWNER_RIGHTS
            && !(sid == CREATOR_OWNER && ace[1] & 8 != 0 && ace[1] & 3 != 0)
        {
            return Err(StorageAclError::PolicyRefused);
        }
        let mask = u32::from_le_bytes([ace[4], ace[5], ace[6], ace[7]]);
        if mask & !SUPPORTED_ACCESS_MASK != 0 {
            return Err(StorageAclError::PolicyRefused);
        }
        rows.push(array(vec![
            V::U64(u64::from(ace[1])),
            V::U64(u64::from(mask)),
            byte_value(sid)?,
        ])?);
        offset = end;
    }
    // AclSize includes unused capacity. Padding is not access semantics.
    let value = array(vec![
        byte_value(owner)?,
        V::U64(u64::from(control & DACL_POLICY_BITS)),
        V::U64(u64::from(acl[0])),
        array(rows)?,
    ])?;
    let domain = c::CanonicalDigestDomain::parse("eliot/cbor/windows-storage-acl/v1")
        .map_err(|_| StorageAclError::Encoding)?;
    let limit = c::DigestInputLimit::new(128 * 1024).map_err(|_| StorageAclError::Encoding)?;
    let fingerprint = *c::blake3_canonical(&domain, &value, limit)
        .map_err(|_| StorageAclError::Encoding)?
        .as_bytes();
    Ok(StorageAclObservation {
        owner: OwnerSid::copy(owner)?,
        fingerprint,
    })
}

#[cfg(windows)]
#[allow(unsafe_code)]
#[path = "storage_acl/windows.rs"]
mod windows;

#[cfg(test)]
#[path = "storage_acl/tests.rs"]
mod tests;

#[cfg(all(test, windows))]
#[path = "storage_acl/native_tests.rs"]
#[allow(unsafe_code)]
mod native_tests;
