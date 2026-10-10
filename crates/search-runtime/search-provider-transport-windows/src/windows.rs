use core::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};

use super::{NativeEndpointNameV1, NativeTransportError, read_result, write_result};

const PIPE_SERVER_END: u32 = 1;
const PIPE_TYPE_MESSAGE: u32 = 4;
const PIPE_READMODE_BYTE: u32 = 0;
const PIPE_NOWAIT: u32 = 1;
const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;

#[link(name = "Kernel32")]
unsafe extern "system" {
    fn GetNamedPipeInfo(
        pipe: *mut c_void,
        flags: *mut u32,
        out_buffer_size: *mut u32,
        in_buffer_size: *mut u32,
        max_instances: *mut u32,
    ) -> i32;
    fn SetNamedPipeHandleState(
        pipe: *mut c_void,
        mode: *const u32,
        max_collection_count: *const u32,
        collect_data_timeout: *const u32,
    ) -> i32;
}

pub struct Pipe {
    file: File,
}

pub fn open(endpoint: NativeEndpointNameV1) -> Result<Pipe, NativeTransportError> {
    // The typed name has fixed canonical ASCII and no separators or suffixes.
    // OpenOptions defaults to synchronous OPEN_EXISTING and non-inherited.
    // SQOS prevents granting the server impersonation/delegation privileges.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .security_qos_flags(SECURITY_IDENTIFICATION)
        .open(endpoint_path(endpoint))
        .map_err(|_| NativeTransportError::Unavailable)?;
    let handle = file.as_raw_handle();
    let mut flags = 0;
    // SAFETY: File owns a live handle for this synchronous call. flags is a
    // uniquely borrowed u32; all optional outputs are null. No pointer retained.
    let observed = unsafe {
        GetNamedPipeInfo(
            handle,
            &raw mut flags,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
        )
    };
    if observed == 0 {
        return Err(NativeTransportError::Unavailable);
    }
    if flags & (PIPE_SERVER_END | PIPE_TYPE_MESSAGE) != 0 {
        return Err(NativeTransportError::UnsupportedPipe);
    }
    let mode = PIPE_READMODE_BYTE | PIPE_NOWAIT;
    // SAFETY: File remains live; mode is a readable u32. Optional remote
    // collection parameters are null for this local pipe. No pointer retained.
    if unsafe {
        SetNamedPipeHandleState(
            handle,
            &raw const mode,
            core::ptr::null(),
            core::ptr::null(),
        )
    } == 0
    {
        return Err(NativeTransportError::Unavailable);
    }
    Ok(Pipe { file })
}

pub fn endpoint_path(endpoint: NativeEndpointNameV1) -> String {
    format!(r"\\.\pipe\{endpoint}")
}

impl Pipe {
    pub(super) fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        read_result(self.file.read(buffer))
    }
    pub(super) fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        write_result(self.file.write(buffer), buffer.is_empty())
    }
}
