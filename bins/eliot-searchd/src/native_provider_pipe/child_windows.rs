//! Checked Win32 boundary, SDK 10.0.26100.0; consulted 2026-10-16.
//!
//! Microsoft CreateNamedPipeW/ConnectNamedPipe (updated 2024-11-20), named-pipe
//! security (2025-04-15), token APIs (2025-07-01), descriptor layouts (2024-02-22):
//! <https://learn.microsoft.com/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew>
//! <https://learn.microsoft.com/windows/win32/api/namedpipeapi/nf-namedpipeapi-connectnamedpipe>
//! <https://learn.microsoft.com/windows/win32/ipc/named-pipe-security-and-access-rights>
//! Uses the absolute descriptor/ACL construction of `native_file/storage_acl`,
//! with separate SDK-aligned buffers and no token or descriptor escape.

use super::{PIPE_PATH_UNITS, redact};
use core::{
    ffi::c_void,
    mem::{MaybeUninit, align_of, offset_of, size_of},
    ptr,
};
use std::{
    fs::File,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
};

const SYSTEM: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0];
const ADMIN: &[u8] = &[1, 2, 0, 0, 0, 0, 0, 5, 32, 0, 0, 0, 32, 2, 0, 0];
const FILE_ALL_ACCESS: u32 = 0x001f_01ff;
const TOKEN_QUERY: u32 = 8;
const TOKEN_USER: u32 = 1;
const ERROR_NO_TOKEN: i32 = 1008;
const PIPE_ACCESS_DUPLEX: u32 = 3;
const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
const PIPE_TYPE_BYTE: u32 = 0;
const PIPE_READMODE_BYTE: u32 = 0;
const PIPE_NOWAIT: u32 = 1;
const PIPE_REJECT_REMOTE_CLIENTS: u32 = 8;
const PIPE_MODE: u32 =
    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS;
const MAX_INSTANCES: u32 = 2;
const TOKEN_BUFFER_BYTES: usize = 1024;
const MAX_SID_BYTES: usize = 68;

#[repr(C)]
struct SecurityAttributes {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}
#[repr(C)]
struct Descriptor {
    revision: u8,
    reserved: u8,
    control: u16,
    owner: *mut c_void,
    group: *mut c_void,
    sacl: *mut c_void,
    dacl: *mut c_void,
}
#[repr(C)]
struct TokenUser {
    sid: *mut c_void,
    attributes: u32,
}
const _: () = {
    assert!(size_of::<SecurityAttributes>() == if size_of::<usize>() == 8 { 24 } else { 12 });
    assert!(offset_of!(SecurityAttributes, descriptor) == size_of::<usize>());
    assert!(size_of::<Descriptor>() == if size_of::<usize>() == 8 { 40 } else { 20 });
    assert!(offset_of!(Descriptor, dacl) == 4 * size_of::<usize>());
    assert!(align_of::<Descriptor>() == align_of::<usize>());
    assert!(size_of::<TokenUser>() == 2 * size_of::<usize>());
    assert!(align_of::<TokenUser>() == align_of::<usize>());
    assert!(PIPE_MODE & PIPE_REJECT_REMOTE_CLIENTS != 0 && MAX_INSTANCES == 2);
};

#[link(name = "Kernel32")]
unsafe extern "system" {
    fn CreateNamedPipeW(
        name: *const u16,
        open: u32,
        mode: u32,
        instances: u32,
        output: u32,
        input: u32,
        timeout: u32,
        attributes: *const SecurityAttributes,
    ) -> RawHandle;
    fn ConnectNamedPipe(pipe: RawHandle, overlapped: *mut c_void) -> i32;
    fn DisconnectNamedPipe(pipe: RawHandle) -> i32;
    fn GetCurrentProcess() -> RawHandle;
    fn GetCurrentThread() -> RawHandle;
}
#[link(name = "Advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(process: RawHandle, access: u32, token: *mut RawHandle) -> i32;
    fn OpenThreadToken(thread: RawHandle, access: u32, as_self: i32, token: *mut RawHandle) -> i32;
    fn GetTokenInformation(
        token: RawHandle,
        class: u32,
        buffer: *mut c_void,
        length: u32,
        returned: *mut u32,
    ) -> i32;
    fn InitializeSecurityDescriptor(sd: *mut c_void, revision: u32) -> i32;
    fn SetSecurityDescriptorOwner(sd: *mut c_void, owner: *mut c_void, defaulted: i32) -> i32;
    fn InitializeAcl(acl: *mut c_void, length: u32, revision: u32) -> i32;
    fn AddAccessAllowedAceEx(
        acl: *mut c_void,
        revision: u32,
        flags: u32,
        mask: u32,
        sid: *mut c_void,
    ) -> i32;
    fn SetSecurityDescriptorDacl(
        sd: *mut c_void,
        present: i32,
        acl: *mut c_void,
        defaulted: i32,
    ) -> i32;
    fn SetSecurityDescriptorControl(sd: *mut c_void, bits: u16, set: u16) -> i32;
}

struct Sid {
    words: [u32; MAX_SID_BYTES / 4],
}
impl Sid {
    fn copy(bytes: &[u8]) -> io::Result<Self> {
        if !valid_sid(bytes) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let mut words = [0; MAX_SID_BYTES / 4];
        for (slot, bytes) in words.iter_mut().zip(bytes.as_chunks::<4>().0) {
            *slot = u32::from_ne_bytes(*bytes);
        }
        Ok(Self { words })
    }
    const fn pointer(&self) -> *mut c_void {
        self.words.as_ptr().cast_mut().cast()
    }
}

/// Native fields stay private; contains no self-referential pointers.
pub(super) struct PipeSecurity {
    owner: Sid,
    acl: [u32; 32],
}
impl PipeSecurity {
    pub(super) fn new(expected: [u8; 32], check: &dyn Fn() -> io::Result<()>) -> io::Result<Self> {
        let owner = current_user_sid(expected, check)?;
        let system = Sid::copy(SYSTEM)?;
        let admin = Sid::copy(ADMIN)?;
        let mut acl = [0_u32; 32]; // 128 bytes >= header + three ACEs with maximum owner SID
        let acl_ptr = acl.as_mut_ptr().cast();
        // SAFETY: distinct DWORD-aligned 128-byte ACL. Each ACE copies one
        // validated DWORD-aligned SID; maximum total is 8+76+20+24=128 bytes.
        checked(check, || unsafe { InitializeAcl(acl_ptr, 128, 2) })?;
        for sid in [&owner, &system, &admin] {
            checked(check, || unsafe {
                AddAccessAllowedAceEx(acl_ptr, 2, 0, FILE_ALL_ACCESS, sid.pointer())
            })?;
        }
        Ok(Self { owner, acl })
    }
}

fn current_user_sid(expected: [u8; 32], check: &dyn Fn() -> io::Result<()>) -> io::Result<Sid> {
    check().map_err(redact)?;
    let mut thread_raw = ptr::null_mut();
    // SAFETY: pseudo handle is borrowed; writable result slot. Never owned/closed.
    let thread_open =
        unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &raw mut thread_raw) };
    let thread_error = if thread_open == 0 {
        Some(io::Error::last_os_error())
    } else {
        None
    };
    if thread_open != 0 {
        // SAFETY: successful OpenThreadToken transfers a CloseHandle-compatible
        // token. Guard is installed before any checkpoint or fallible work.
        let thread = unsafe { OwnedHandle::from_raw_handle(thread_raw) };
        check().map_err(redact)?;
        drop(thread);
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    check().map_err(redact)?;
    if thread_error.and_then(|error| error.raw_os_error()) != Some(ERROR_NO_TOKEN) {
        return Err(io::ErrorKind::PermissionDenied.into());
    }

    let mut raw = ptr::null_mut();
    check().map_err(redact)?;
    // SAFETY: current process pseudo handle is borrowed, output is writable.
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut raw) };
    if opened == 0 {
        let error = io::Error::last_os_error();
        check().map_err(redact)?;
        return Err(redact(error));
    }
    // SAFETY: successful token acquisition; sole CloseHandle owner immediately.
    // No File conversion and no return/checkpoint can bypass this guard.
    let token = unsafe { OwnedHandle::from_raw_handle(raw) };
    check().map_err(redact)?;
    let mut buffer = [0_usize; TOKEN_BUFFER_BYTES / size_of::<usize>()];
    let mut returned = 0_u32;
    check().map_err(redact)?;
    // SAFETY: fixed pointer-aligned, zero-initialized 1024-byte output with a
    // separate writable count. No OS-requested allocation or size retry.
    let observed = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TOKEN_USER,
            buffer.as_mut_ptr().cast(),
            u32::try_from(TOKEN_BUFFER_BYTES).map_err(|_| io::ErrorKind::InvalidInput)?,
            &raw mut returned,
        )
    };
    let error = if observed == 0 {
        Some(io::Error::last_os_error())
    } else {
        None
    };
    check().map_err(redact)?;
    if let Some(error) = error {
        return Err(redact(error));
    }
    let returned = usize::try_from(returned).map_err(|_| io::ErrorKind::InvalidData)?;
    if !(size_of::<TokenUser>()..=TOKEN_BUFFER_BYTES).contains(&returned) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    // SAFETY: read only the initialized first pointer field, not struct padding.
    // The returned SID pointer is converted to a checked offset, never dereferenced.
    let sid_pointer = unsafe { ptr::read_unaligned(buffer.as_ptr().cast::<*mut c_void>()) };
    // SAFETY: all buffer bytes were initialized before the call; extent is fixed.
    let bytes =
        unsafe { core::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), TOKEN_BUFFER_BYTES) };
    let sid = checked_sid(bytes, returned, sid_pointer.addr())?;
    if search_os_secrets::hash_native_revision_user_sid(sid) != expected {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    check().map_err(redact)?;
    Sid::copy(sid)
}

fn checked_sid(buffer: &[u8], returned: usize, pointer: usize) -> io::Result<&[u8]> {
    let invalid = || io::Error::from(io::ErrorKind::InvalidData);
    if returned < size_of::<TokenUser>() || returned > buffer.len() {
        return Err(invalid());
    }
    let offset = pointer
        .checked_sub(buffer.as_ptr().addr())
        .ok_or_else(invalid)?;
    if offset < size_of::<TokenUser>() || !offset.is_multiple_of(4) {
        return Err(invalid());
    }
    let header_end = offset
        .checked_add(8)
        .filter(|end| *end <= returned)
        .ok_or_else(invalid)?;
    let header = buffer.get(offset..header_end).ok_or_else(invalid)?;
    if header[0] != 1 || header[1] > 15 {
        return Err(invalid());
    }
    let end = offset
        .checked_add(8 + 4 * usize::from(header[1]))
        .filter(|end| *end <= returned)
        .ok_or_else(invalid)?;
    buffer.get(offset..end).ok_or_else(invalid)
}
fn valid_sid(sid: &[u8]) -> bool {
    (8..=MAX_SID_BYTES).contains(&sid.len())
        && sid[0] == 1
        && sid[1] <= 15
        && sid.len() == 8 + 4 * usize::from(sid[1])
}

fn checked(check: &dyn Fn() -> io::Result<()>, call: impl FnOnce() -> i32) -> io::Result<()> {
    check().map_err(redact)?;
    let result = call();
    let error = if result == 0 {
        Some(io::Error::last_os_error())
    } else {
        None
    };
    check().map_err(redact)?;
    error.map_or(Ok(()), |error| Err(redact(error)))
}

pub(super) fn create_pipe_instance(
    name: &[u16; PIPE_PATH_UNITS],
    security: &PipeSecurity,
    first: bool,
    check: &dyn Fn() -> io::Result<()>,
) -> io::Result<File> {
    // Absolute SECURITY_DESCRIPTOR is separate from ACL and SID backing. Its
    // native pointers are set and consumed while every referent remains live.
    let mut descriptor = MaybeUninit::<Descriptor>::uninit();
    let sd = descriptor.as_mut_ptr().cast();
    // SAFETY: SDK-sized pointer-aligned storage, initialized before setters.
    checked(check, || unsafe { InitializeSecurityDescriptor(sd, 1) })?;
    checked(check, || unsafe {
        SetSecurityDescriptorOwner(sd, security.owner.pointer(), 0)
    })?;
    checked(check, || unsafe {
        SetSecurityDescriptorDacl(sd, 1, security.acl.as_ptr().cast_mut().cast(), 0)
    })?;
    checked(check, || unsafe {
        SetSecurityDescriptorControl(sd, 0x1000, 0x1000)
    })?;
    let attributes = SecurityAttributes {
        length: u32::try_from(size_of::<SecurityAttributes>())
            .map_err(|_| io::ErrorKind::InvalidInput)?,
        descriptor: sd,
        inherit: 0,
    };
    let open_mode = PIPE_ACCESS_DUPLEX
        | if first {
            FILE_FLAG_FIRST_PIPE_INSTANCE
        } else {
            0
        };
    check().map_err(redact)?;
    // SAFETY: fixed canonical UTF-16 array includes terminal NUL; explicit
    // protected absolute descriptor, ACL and SIDs are live through this call.
    // No overlapped flag; nonblocking/remote rejection are in dwPipeMode.
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            open_mode,
            PIPE_MODE,
            MAX_INSTANCES,
            64 * 1024,
            64 * 1024,
            0,
            &raw const attributes,
        )
    };
    if handle == (-1_isize as RawHandle) {
        let error = io::Error::last_os_error();
        check().map_err(redact)?;
        return Err(redact(error));
    }
    // SAFETY: successful CreateNamedPipeW transfers sole ownership of this file
    // handle. Install custody before post-call refusal can exit.
    let pipe = unsafe { File::from_raw_handle(handle) };
    check().map_err(redact)?;
    Ok(pipe)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConnectResult {
    Pending,
    Connected,
    Disconnected,
}

fn connect_result(success: bool, code: Option<i32>) -> io::Result<ConnectResult> {
    match (success, code) {
        // In PIPE_NOWAIT, TRUE only makes a disconnected instance available.
        (true, _) | (false, Some(536)) => Ok(ConnectResult::Pending),
        (false, Some(535)) => Ok(ConnectResult::Connected),
        (false, Some(232)) => Ok(ConnectResult::Disconnected),
        _ => Err(io::ErrorKind::Other.into()),
    }
}
pub(super) fn try_connect(
    pipe: &File,
    check: &dyn Fn() -> io::Result<()>,
) -> io::Result<ConnectResult> {
    check().map_err(redact)?;
    // SAFETY: owned synchronous PIPE_NOWAIT server handle; no OVERLAPPED retained.
    let result = unsafe { ConnectNamedPipe(pipe.as_raw_handle(), ptr::null_mut()) };
    let error = if result == 0 {
        Some(io::Error::last_os_error())
    } else {
        None
    };
    check().map_err(redact)?;
    connect_result(result != 0, error.and_then(|error| error.raw_os_error()))
}
pub(super) fn arm(pipe: &File, check: &dyn Fn() -> io::Result<()>) -> io::Result<()> {
    match try_connect(pipe, check)? {
        ConnectResult::Pending | ConnectResult::Connected => Ok(()),
        ConnectResult::Disconnected => Err(io::ErrorKind::BrokenPipe.into()),
    }
}
pub(super) fn disconnect(pipe: &File, check: &dyn Fn() -> io::Result<()>) -> io::Result<()> {
    // SAFETY: live server pipe; immediate disconnect, never a blocking flush.
    checked(check, || unsafe {
        DisconnectNamedPipe(pipe.as_raw_handle())
    })
}
pub(super) fn disconnect_on_drop(pipe: &File) {
    // SAFETY: File remains sole live server handle during drop; best-effort
    // disconnect cannot be vetoed by expired caller context. File then closes.
    let _ = unsafe { DisconnectNamedPipe(pipe.as_raw_handle()) };
}

#[cfg(test)]
mod fixtures {
    use super::*;
    #[test]
    fn nonblocking_true_is_not_a_connected_client() {
        assert_eq!(connect_result(true, None).unwrap(), ConnectResult::Pending);
        assert_eq!(
            connect_result(false, Some(535)).unwrap(),
            ConnectResult::Connected
        );
        assert_eq!(
            connect_result(false, Some(536)).unwrap(),
            ConnectResult::Pending
        );
        assert_eq!(
            connect_result(false, Some(232)).unwrap(),
            ConnectResult::Disconnected
        );
        assert!(connect_result(false, Some(5)).is_err());
    }
    #[test]
    fn sid_pointer_and_header_are_bounded_before_length_is_read() {
        let mut buffer = [0_u8; 128];
        let offset = size_of::<TokenUser>();
        buffer[offset..offset + SYSTEM.len()].copy_from_slice(SYSTEM);
        let base = buffer.as_ptr().addr();
        assert_eq!(
            checked_sid(&buffer, offset + SYSTEM.len(), base + offset).unwrap(),
            SYSTEM
        );
        for pointer in [
            0,
            base.saturating_sub(1),
            base,
            base + offset + 1,
            base + 127,
            usize::MAX,
        ] {
            assert!(checked_sid(&buffer, buffer.len(), pointer).is_err());
        }
        assert!(checked_sid(&buffer, offset + 7, base + offset).is_err());
        assert!(checked_sid(&buffer, offset + SYSTEM.len() - 1, base + offset).is_err());
        buffer[offset + 1] = 16;
        assert!(checked_sid(&buffer, buffer.len(), base + offset).is_err());
    }
}
