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
        // SAFETY: the complete allocation extent is established above. Parsing
        // below reads only checked headers/SIDs/ACEs, never unused ACL padding.
        let bytes = unsafe { core::slice::from_raw_parts(descriptor.cast::<u8>(), length) };
        let (sid, control, acl) = descriptor_parts(bytes, owner, dacl)?;
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
        qualify(sid, control, acl, expected)
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

type DescriptorParts<'a> = (&'a [u8], u16, &'a [u8]);

fn descriptor_parts<E>(
    bytes: &[u8],
    owner: *mut c_void,
    dacl: *mut c_void,
) -> Result<DescriptorParts<'_>, Error<E>> {
    if bytes.len() < size_of::<RelativeDescriptor>() {
        return Err(Error::InvalidDescriptor);
    }
    // SAFETY: this complete fixed SDK header is within the borrowed allocation;
    // no embedded pointer/relative offset has yet been dereferenced.
    let header = unsafe { read_unaligned(bytes.as_ptr().cast::<RelativeDescriptor>()) };
    if header.revision != 1 || header.reserved != 0 || header.control & 0x8000 == 0 {
        return Err(Error::InvalidDescriptor);
    }
    let base = bytes.as_ptr().cast_mut().cast();
    let owner_offset = checked_offset(base, bytes.len(), header.owner, owner)?;
    let acl_offset = checked_offset(base, bytes.len(), header.dacl, dacl)?;
    let sid_header = bytes
        .get(owner_offset..owner_offset + 8)
        .ok_or(Error::InvalidDescriptor)?;
    let sid_length = 8 + 4 * usize::from(sid_header[1]);
    if sid_header[0] != 1 || sid_length > MAX_SID {
        return Err(Error::InvalidDescriptor);
    }
    let owner_end = owner_offset
        .checked_add(sid_length)
        .ok_or(Error::InvalidDescriptor)?;
    let sid = bytes
        .get(owner_offset..owner_end)
        .ok_or(Error::InvalidDescriptor)?;
    let acl_header = bytes
        .get(acl_offset..acl_offset + 8)
        .ok_or(Error::InvalidDescriptor)?;
    let acl_length = usize::from(u16::from_le_bytes([acl_header[2], acl_header[3]]));
    let acl_end = acl_offset
        .checked_add(acl_length)
        .ok_or(Error::InvalidDescriptor)?;
    if acl_length < 8 || (owner_offset < acl_end && acl_offset < owner_end) {
        return Err(Error::InvalidDescriptor);
    }
    let acl = bytes
        .get(acl_offset..acl_end)
        .ok_or(Error::InvalidDescriptor)?;
    Ok((sid, header.control, acl))
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
