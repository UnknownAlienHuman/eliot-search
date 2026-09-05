use super::{File, Observation, ObservationError};
use core::ffi::c_void;
use std::os::windows::io::AsRawHandle;

const FILE_TYPE_DISK: u32 = 1;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}

impl FileTime {
    fn value(&self) -> u64 {
        (u64::from(self.high) << 32) | u64::from(self.low)
    }
}

// BY_HANDLE_FILE_INFORMATION, including fields unused by the safe projection.
#[repr(C)]
#[derive(Default)]
struct FileInformation {
    attributes: u32,
    creation: FileTime,
    access: FileTime,
    write: FileTime,
    volume_serial: u32,
    size_high: u32,
    size_low: u32,
    links: u32,
    index_high: u32,
    index_low: u32,
}

// These are Windows ABI assertions, not estimates based on pointer width.
const _: () = assert!(core::mem::size_of::<FileTime>() == 8);
const _: () = assert!(core::mem::size_of::<FileInformation>() == 52);
const _: () = assert!(core::mem::align_of::<FileInformation>() == 4);
const _: () = assert!(core::mem::offset_of!(FileInformation, volume_serial) == 28);
const _: () = assert!(core::mem::offset_of!(FileInformation, index_high) == 44);

#[link(name = "Kernel32")]
unsafe extern "system" {
    fn GetFileType(file: *mut c_void) -> u32;
    fn GetFileInformationByHandle(file: *mut c_void, output: *mut FileInformation) -> i32;
    fn GetVolumeInformationByHandleW(
        file: *mut c_void,
        volume_name: *mut u16,
        volume_name_length: u32,
        volume_serial: *mut u32,
        maximum_component_length: *mut u32,
        filesystem_flags: *mut u32,
        filesystem_name: *mut u16,
        filesystem_name_length: u32,
    ) -> i32;
}

pub(super) fn observe(file: &File) -> Result<Observation, ObservationError> {
    let handle = file.as_raw_handle();
    // SAFETY: `file` owns this live handle throughout the synchronous call.
    if unsafe { GetFileType(handle) } != FILE_TYPE_DISK {
        return Err(ObservationError::UnsupportedHandle);
    }
    let mut filesystem_name = [0_u16; 32];
    let mut volume_serial = 0_u32;
    // SAFETY: optional buffers are null with zero lengths; supplied pointers
    // reference live, correctly sized writable values. Nothing is retained.
    let volume_ok = unsafe {
        GetVolumeInformationByHandleW(
            handle,
            core::ptr::null_mut(),
            0,
            &mut volume_serial,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            filesystem_name.as_mut_ptr(),
            32,
        )
    };
    if volume_ok == 0 {
        return Err(ObservationError::ObservationFailed);
    }
    // No 128-bit ReFS identifier is silently shortened to the old u64 layout.
    if filesystem_name[..5] != [78, 84, 70, 83, 0] {
        return Err(ObservationError::UnsupportedFileSystem);
    }
    let mut info = FileInformation::default();
    // SAFETY: layout is asserted above; output is initialized and uniquely
    // borrowed. The borrowed File keeps the handle valid; failure is checked.
    if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 {
        return Err(ObservationError::ObservationFailed);
    }
    if info.volume_serial != volume_serial {
        return Err(ObservationError::ObservationFailed);
    }
    if info.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(ObservationError::ReparsePointDenied);
    }
    Ok(Observation {
        volume_serial: info.volume_serial,
        file_index: (u64::from(info.index_high) << 32) | u64::from(info.index_low),
        attributes: info.attributes,
        length: (u64::from(info.size_high) << 32) | u64::from(info.size_low),
        creation_time: info.creation.value(),
        last_write_time: info.write.value(),
    })
}
