//! SDK 10.0.26100.0 ABI only; no token lookup or existing-object ACL mutation.

use super::{
    ADMIN, MAX_SID, OwnerSid, SYSTEM, StorageAclError as Error, StorageAclObservation,
    StorageSecurityContext, StorageSecurityWork as Work, creation_result, qualify,
};
use core::{
    ffi::c_void,
    mem::{MaybeUninit, offset_of, size_of},
    ptr::{null_mut, read_unaligned},
};
use std::{
    fs::File,
    os::windows::{ffi::OsStrExt, io::AsRawHandle},
    path::Path,
};

const MAX_DESCRIPTOR_BYTES: usize = 128 * 1024; // bounded OWNER plus the SDK's <=64KiB DACL
const FILE_ALL_ACCESS: u32 = 0x001f_01ff;

#[repr(C)]
struct RelativeDescriptor {
    revision: u8,
    reserved: u8,
    control: u16,
    owner: u32,
    group: u32,
    sacl: u32,
    dacl: u32,
}
#[repr(C)]
struct Descriptor {
    // Opaque pointer-aligned SDK storage, initialized only by Advapi32.
    _words: [usize; 5],
}
#[repr(C)]
struct SecurityAttributes {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}
#[repr(C)]
struct AclHeader {
    revision: u8,
    reserved: u8,
    length: u16,
    count: u16,
    reserved2: u16,
}
#[repr(C)]
struct AceHeader {
    kind: u8,
    flags: u8,
    length: u16,
}
#[repr(C)]
struct AllowedAce {
    header: AceHeader,
    mask: u32,
    sid_start: u32,
}

// Matching SDK header sizes/offsets, including both supported pointer widths.
const _: () = {
    assert!(size_of::<RelativeDescriptor>() == 20 && offset_of!(RelativeDescriptor, dacl) == 16);
    assert!(size_of::<AclHeader>() == 8 && offset_of!(AclHeader, count) == 4);
    assert!(size_of::<AceHeader>() == 4);
    assert!(size_of::<AllowedAce>() == 12 && offset_of!(AllowedAce, sid_start) == 8);
    assert!(size_of::<Descriptor>() == if size_of::<usize>() == 8 { 40 } else { 20 });
    assert!(size_of::<SecurityAttributes>() == if size_of::<usize>() == 8 { 24 } else { 12 });
};

#[link(name = "Advapi32")]
unsafe extern "system" {
    fn GetSecurityInfo(
        handle: *mut c_void,
        kind: u32,
        information: u32,
        owner: *mut *mut c_void,
        group: *mut *mut c_void,
        dacl: *mut *mut c_void,
        sacl: *mut *mut c_void,
        descriptor: *mut *mut c_void,
    ) -> u32;
    fn InitializeSecurityDescriptor(descriptor: *mut c_void, revision: u32) -> i32;
    fn SetSecurityDescriptorOwner(
        descriptor: *mut c_void,
        owner: *mut c_void,
        defaulted: i32,
    ) -> i32;
    fn InitializeAcl(acl: *mut c_void, length: u32, revision: u32) -> i32;
    fn AddAccessAllowedAceEx(
        acl: *mut c_void,
        revision: u32,
        flags: u32,
        mask: u32,
        sid: *mut c_void,
    ) -> i32;
    fn SetSecurityDescriptorDacl(
        descriptor: *mut c_void,
        present: i32,
        acl: *mut c_void,
        defaulted: i32,
    ) -> i32;
    fn SetSecurityDescriptorControl(descriptor: *mut c_void, bits: u16, set: u16) -> i32;
}
#[link(name = "Kernel32")]
unsafe extern "system" {
    fn LocalSize(memory: *mut c_void) -> usize;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
    fn CreateDirectoryW(path: *const u16, attributes: *const SecurityAttributes) -> i32;
    fn GetLastError() -> u32;
}

struct LocalDescriptor(*mut c_void);
impl LocalDescriptor {
    fn release(&mut self) -> bool {
        if self.0.is_null() {
            return true;
        }
        // SAFETY: sole allocation returned by GetSecurityInfo; release/error
        // capture and one failure fallback were reserved before acquisition.
        let remaining = unsafe { LocalFree(self.0) };
        if remaining.is_null() {
            self.0 = null_mut();
            true
        } else {
            let _error = unsafe { GetLastError() };
            false
        }
    }
}
impl Drop for LocalDescriptor {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

pub(super) fn observe<C: StorageSecurityContext>(
    file: &File,
    expected: [u8; 32],
    context: &mut C,
) -> Result<StorageAclObservation, Error<C::Error>> {
    // GetSecurityInfo, LocalSize/error, release/error and one failed-release
    // Drop fallback/error. No callbacks can prevent release of owned memory.
    for _ in 0..7 {
        context.charge(Work::NativeCall).map_err(Error::Context)?;
    }
    context
        .charge(Work::Allocate(MAX_DESCRIPTOR_BYTES))
        .map_err(Error::Context)?;
    context
        .charge(Work::Process(MAX_DESCRIPTOR_BYTES))
        .map_err(Error::Context)?;
    context.checkpoint().map_err(Error::Context)?;
    let (mut owner, mut dacl, mut descriptor) = (null_mut(), null_mut(), null_mut());
    // SAFETY: the borrowed FD stays live; all output slots are writable, and
    // only OWNER|DACL (READ_CONTROL) are requested, never privileged SACL access.
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            1,
            5,
            &raw mut owner,
            null_mut(),
            &raw mut dacl,
            null_mut(),
            &raw mut descriptor,
        )
    };
    let mut allocation = LocalDescriptor(descriptor);
    let result = (|| {
        let after = context.checkpoint();
        if status != 0 {
            return Err(Error::ObservationFailed);
        }
        after.map_err(Error::Context)?;
        if descriptor.is_null() {
            return Err(Error::InvalidDescriptor);
        }
        // SAFETY: GetSecurityInfo returned this LocalFree-compatible allocation.
        // LocalSize establishes the allocation extent before any dereference.
        let length = unsafe { LocalSize(descriptor) };
        if length == 0 {
            let _error = unsafe { GetLastError() };
        }
        context.checkpoint().map_err(Error::Context)?;
        if !(size_of::<RelativeDescriptor>()..=MAX_DESCRIPTOR_BYTES).contains(&length) {
            return Err(Error::InvalidDescriptor);
        }
        // SAFETY: successful GetSecurityInfo initializes its documented fields,
        // SIDs and counted ACE components, but not necessarily allocation/ACL slack.
        // Copy those components separately; never form a slice over native slack.
        let (sid, control, acl) =
            unsafe { descriptor_parts(descriptor, length, owner, dacl, context)? };
        let count = usize::from(u16::from_le_bytes([acl[4], acl[5]]));
        if count > (acl.len() - 8) / 16 {
            return Err(Error::InvalidDescriptor);
        }
        let allocation =
            1024 + count * (4 * size_of::<search_contracts::CanonicalValue>() + MAX_SID);
        context
            .charge(Work::Allocate(allocation))
            .map_err(Error::Context)?;
        context
            .charge(Work::Process(allocation + 4 * acl.len()))
            .map_err(Error::Context)?;
        qualify(&sid, control, &acl, expected)
    })();
    let released = allocation.release();
    let after = context.checkpoint();
    match result {
        Err(first) => Err(first),
        Ok(_) if !released => Err(Error::ObservationFailed),
        Ok(observation) => {
            after.map_err(Error::Context)?;
            Ok(observation)
        }
    }
}

type DescriptorParts = (Vec<u8>, u16, Vec<u8>);

fn extent<E>(length: usize, offset: usize, count: usize) -> Result<(), Error<E>> {
    if offset.checked_add(count).is_none_or(|end| end > length) {
        return Err(Error::InvalidDescriptor);
    }
    Ok(())
}

// SAFETY: caller promises this exact scalar is an initialized SDK field, not
// padding. All call sites use u8/u16/u32, with no Rust padding or invalid bit pattern.
unsafe fn scalar<T: Copy, E>(
    base: *mut c_void,
    length: usize,
    offset: usize,
) -> Result<T, Error<E>> {
    extent(length, offset, size_of::<T>())?;
    Ok(unsafe { read_unaligned(base.cast::<u8>().add(offset).cast::<T>()) })
}

// SAFETY: caller promises the SDK initialized exactly this SID component.
unsafe fn sid_length<E>(
    base: *mut c_void,
    length: usize,
    offset: usize,
) -> Result<usize, Error<E>> {
    let revision = unsafe { scalar::<u8, E>(base, length, offset)? };
    if revision != 1 {
        return Err(Error::InvalidDescriptor);
    }
    let count = unsafe { scalar::<u8, E>(base, length, offset + 1)? };
    let size = 8 + 4 * usize::from(count);
    if size > MAX_SID {
        return Err(Error::InvalidDescriptor);
    }
    extent(length, offset, size)?;
    Ok(size)
}

// SAFETY: the successful SDK query initialized its meaningful header fields,
// complete owner SID and each counted ACE's defined fields/SID. Allocation slack,
// reserved fields and unused ACL capacity may remain uninitialized and are not read.
unsafe fn descriptor_parts<C: StorageSecurityContext>(
    base: *mut c_void,
    length: usize,
    owner: *mut c_void,
    dacl: *mut c_void,
    context: &mut C,
) -> Result<DescriptorParts, Error<C::Error>> {
    if length < size_of::<RelativeDescriptor>() {
        return Err(Error::InvalidDescriptor);
    }
    let revision = unsafe { scalar::<u8, C::Error>(base, length, 0)? };
    if revision != 1 {
        return Err(Error::InvalidDescriptor);
    }
    let control = unsafe { scalar::<u16, C::Error>(base, length, 2)? };
    if control & 0x8000 == 0 {
        return Err(Error::InvalidDescriptor);
    }
    let owner_relative = unsafe { scalar::<u32, C::Error>(base, length, 4)? };
    let acl_relative = unsafe { scalar::<u32, C::Error>(base, length, 16)? };
    let owner_offset = checked_offset(base, length, owner_relative, owner)?;
    let acl_offset = checked_offset(base, length, acl_relative, dacl)?;
    let owner_size = unsafe { sid_length::<C::Error>(base, length, owner_offset)? };
    let owner_end = owner_offset
        .checked_add(owner_size)
        .ok_or(Error::InvalidDescriptor)?;
    extent(length, acl_offset, 8)?;
    let acl_revision = unsafe { scalar::<u8, C::Error>(base, length, acl_offset)? };
    if !matches!(acl_revision, 2 | 4) {
        return Err(Error::InvalidDescriptor);
    }
    let acl_size = unsafe { scalar::<u16, C::Error>(base, length, acl_offset + 2)? };
    let count = unsafe { scalar::<u16, C::Error>(base, length, acl_offset + 4)? };
    let acl_length = usize::from(acl_size);
    let acl_end = acl_offset
        .checked_add(acl_length)
        .ok_or(Error::InvalidDescriptor)?;
    if acl_length < 8 || (owner_offset < acl_end && acl_offset < owner_end) {
        return Err(Error::InvalidDescriptor);
    }
    extent(length, acl_offset, acl_length)?;
    if usize::from(count) > (acl_length - 8) / 16 {
        return Err(Error::InvalidDescriptor);
    }
    let copied = owner_size
        .checked_add(acl_length)
        .ok_or(Error::InvalidDescriptor)?;
    context
        .charge(Work::Allocate(copied))
        .map_err(Error::Context)?;
    context
        .charge(Work::Process(copied))
        .map_err(Error::Context)?;
    let mut sid = vec![0; owner_size];
    let mut acl = vec![0; acl_length];
    unsafe {
        core::ptr::copy_nonoverlapping(
            base.cast::<u8>().add(owner_offset),
            sid.as_mut_ptr(),
            owner_size,
        );
    };
    acl[0] = acl_revision;
    acl[2..4].copy_from_slice(&acl_size.to_le_bytes());
    acl[4..6].copy_from_slice(&count.to_le_bytes());
    let mut offset = 8_usize;
    for _ in 0..count {
        extent(acl_length, offset, 8)?;
        let at = acl_offset + offset;
        let kind = unsafe { scalar::<u8, C::Error>(base, length, at)? };
        let flags = unsafe { scalar::<u8, C::Error>(base, length, at + 1)? };
        let size = unsafe { scalar::<u16, C::Error>(base, length, at + 2)? };
        let size_usize = usize::from(size);
        if kind != 0 || size_usize < 16 || size_usize % 4 != 0 {
            return Err(Error::InvalidDescriptor);
        }
        extent(acl_length, offset, size_usize)?;
        let mask = unsafe { scalar::<u32, C::Error>(base, length, at + 4)? };
        let trustee_size = unsafe { sid_length::<C::Error>(base, length, at + 8)? };
        if trustee_size != size_usize - 8 {
            return Err(Error::InvalidDescriptor);
        }
        acl[offset] = kind;
        acl[offset + 1] = flags;
        acl[offset + 2..offset + 4].copy_from_slice(&size.to_le_bytes());
        acl[offset + 4..offset + 8].copy_from_slice(&mask.to_le_bytes());
        unsafe {
            core::ptr::copy_nonoverlapping(
                base.cast::<u8>().add(at + 8),
                acl.as_mut_ptr().add(offset + 8),
                trustee_size,
            );
        };
        offset += size_usize;
    }
    Ok((sid, control, acl))
}

fn checked_offset<E>(
    base: *mut c_void,
    length: usize,
    offset: u32,
    pointer: *mut c_void,
) -> Result<usize, Error<E>> {
    let offset = usize::try_from(offset).map_err(|_| Error::InvalidDescriptor)?;
    if offset < size_of::<RelativeDescriptor>()
        || offset % 4 != 0
        || offset >= length
        || (base as usize).checked_add(offset) != Some(pointer as usize)
    {
        return Err(Error::InvalidDescriptor);
    }
    Ok(offset)
}

fn checked<C: StorageSecurityContext>(
    context: &mut C,
    call: impl FnOnce() -> i32,
) -> Result<(), Error<C::Error>> {
    context.checkpoint().map_err(Error::Context)?;
    let success = call();
    if success == 0 {
        let _error = unsafe { GetLastError() };
    }
    let after = context.checkpoint();
    if success == 0 {
        return Err(Error::InvalidDescriptor);
    }
    after.map_err(Error::Context)
}

pub(super) fn create<C: StorageSecurityContext>(
    path: &Path,
    root: &StorageAclObservation,
    context: &mut C,
) -> Result<(), Error<C::Error>> {
    let input_bytes = path.as_os_str().as_encoded_bytes().len();
    context
        .charge(Work::Process(
            input_bytes.checked_mul(2).ok_or(Error::InvalidDescriptor)?,
        ))
        .map_err(Error::Context)?;
    let units = path.as_os_str().encode_wide().count();
    if units == 0 || units >= 32768 {
        return Err(Error::InvalidDescriptor);
    }
    context
        .charge(Work::Allocate((units + 1) * 2 + 256))
        .map_err(Error::Context)?;
    context
        .charge(Work::Process((units + 1) * 2 + 256))
        .map_err(Error::Context)?;
    context.charge(Work::Open).map_err(Error::Context)?;
    // Eight descriptor/ACL setup calls plus CreateDirectory, each with possible
    // immediate GetLastError. All reservations precede dependent preparation.
    for _ in 0..18 {
        context.charge(Work::NativeCall).map_err(Error::Context)?;
    }
    let mut wide = Vec::with_capacity(units + 1);
    wide.extend(path.as_os_str().encode_wide());
    if wide.contains(&0) {
        return Err(Error::InvalidDescriptor);
    }
    wide.push(0);
    let system = OwnerSid::copy(SYSTEM)?;
    let admin = OwnerSid::copy(ADMIN)?;
    let mut acl = [0_u32; 32]; // maximum three ACEs with the admitted <=68-byte owner
    let acl_ptr = acl.as_mut_ptr().cast::<c_void>();
    let mut descriptor = MaybeUninit::<Descriptor>::uninit();
    let sd = descriptor.as_mut_ptr().cast::<c_void>();
    // SAFETY: SDK-sized/aligned descriptor and 128-byte ACL buffers remain live.
    // Initializers run before setters. SID backing is DWORD aligned and private.
    checked(context, || unsafe { InitializeSecurityDescriptor(sd, 1) })?;
    checked(context, || unsafe {
        SetSecurityDescriptorOwner(sd, root.owner.words.as_ptr().cast_mut().cast(), 0)
    })?;
    checked(context, || unsafe { InitializeAcl(acl_ptr, 128, 2) })?;
    for sid in [&root.owner, &system, &admin] {
        checked(context, || unsafe {
            AddAccessAllowedAceEx(
                acl_ptr,
                2,
                3,
                FILE_ALL_ACCESS,
                sid.words.as_ptr().cast_mut().cast(),
            )
        })?;
    }
    checked(context, || unsafe {
        SetSecurityDescriptorDacl(sd, 1, acl_ptr, 0)
    })?;
    checked(context, || unsafe {
        SetSecurityDescriptorControl(sd, 0x1000, 0x1000)
    })?;
    let attributes = SecurityAttributes {
        length: u32::try_from(size_of::<SecurityAttributes>())
            .map_err(|_| Error::InvalidDescriptor)?,
        descriptor: sd,
        inherit: 0,
    };
    context.checkpoint().map_err(Error::Context)?;
    // SAFETY: NUL-terminated path, initialized absolute descriptor, ACL and SIDs
    // remain live for this synchronous atomic creation. No existing ACL is set.
    let created = unsafe { CreateDirectoryW(wide.as_ptr(), &raw const attributes) };
    let error = if created == 0 {
        unsafe { GetLastError() }
    } else {
        0
    };
    let after = context.checkpoint();
    creation_result(created != 0, error == 183, after)
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
