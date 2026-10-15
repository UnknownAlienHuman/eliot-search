use super::{ChildMembershipScratch, DirectoryIdentity, File, Observation, ObservationError, directory_batch_matches};
use core::ffi::c_void;
use std::os::windows::io::AsRawHandle;

const FILE_TYPE_DISK: u32 = 1;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ID_BOTH_DIRECTORY_INFO: i32 = 10;
const FILE_ID_BOTH_DIRECTORY_RESTART_INFO: i32 = 11;
const ERROR_NO_MORE_FILES: u32 = 18;

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
    // winbase.h (not fileapi.h); FILE_INFO_BY_HANDLE_CLASS is a 32-bit C enum.
    fn GetFileInformationByHandleEx(
        file: *mut c_void, information_class: i32, output: *mut c_void, bytes: u32,
    ) -> i32;
    fn GetLastError() -> u32;
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

pub(super) fn observe_with_check(file: &File, check: &dyn Fn(bool) -> Result<(), ObservationError>) -> Result<Observation, ObservationError> {
    observe_with_links_with_check(file, check).map(|(observation, _)| observation)
}

pub(super) fn observe_with_links_with_check(file: &File,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<(Observation, u32), ObservationError> {
    let info = file_information_with_check(file, check)?;
    Ok((Observation {
        volume_serial: info.volume_serial,
        file_index: (u64::from(info.index_high) << 32) | u64::from(info.index_low),
        attributes: info.attributes,
        length: (u64::from(info.size_high) << 32) | u64::from(info.size_low),
        creation_time: info.creation.value(),
        last_write_time: info.write.value(),
    }, info.links))
}

pub(super) fn hardlink_count(file: &File) -> Result<u32, ObservationError> {
    Ok(file_information_with_check(file, &|_| Ok(()))?.links)
}

pub(super) fn qualified_directory_identity_with_check(
    file: &File, volume_serial: u32, check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<DirectoryIdentity, ObservationError> {
    let mut info = FileInformation::default();
    check(true)?;
    // SAFETY: the opaque owner retains this exact qualified FD throughout the
    // call; the initialized ABI output is correctly sized and uniquely borrowed.
    let result = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &raw mut info) };
    check(false)?;
    if result == 0 || info.volume_serial != volume_serial { return Err(ObservationError::ObservationFailed); }
    if info.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 { return Err(ObservationError::ReparsePointDenied); }
    if info.attributes & FILE_ATTRIBUTE_DIRECTORY == 0 { return Err(ObservationError::UnsupportedHandle); }
    Ok(DirectoryIdentity {
        volume_serial: info.volume_serial,
        file_index: (u64::from(info.index_high) << 32) | u64::from(info.index_low),
        creation_time: info.creation.value(),
        attributes: info.attributes,
    })
}

pub(super) fn verify_direct_child_with_check(
    parent: &File, child: &File, scratch: &mut ChildMembershipScratch, max_entries: usize,
    directory: bool,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
    process: &dyn Fn(usize) -> Result<(), ObservationError>,
) -> Result<Observation, ObservationError> {
    let parent_before = observe_with_check(parent, check)?;
    if parent_before.attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(ObservationError::UnsupportedHandle);
    }
    let child_before = observe_with_check(child, check)?;
    if (child_before.attributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory {
        return Err(ObservationError::UnsupportedHandle);
    }
    if child_before.volume_serial != parent_before.volume_serial
        || child_before.file_index == parent_before.file_index
    { return Err(ObservationError::NotDirectChild); }
    let mut entries = 0;
    let mut found = false;
    let mut restart = true;
    loop {
        process(scratch.bytes.len())?;
        scratch.bytes.fill(0);
        if !query_directory(parent, scratch, restart, check)? { break; }
        found |= directory_batch_matches(&scratch.bytes, child_before, &mut entries, max_entries, process)?;
        restart = false;
    }
    // Membership is never issued after a partial/limited enumeration or when
    // either actual handle observation drifted across the complete traversal.
    if observe_with_check(parent, check)? != parent_before
        || observe_with_check(child, check)? != child_before
    { return Err(ObservationError::ObservationFailed); }
    if !found { return Err(ObservationError::NotDirectChild); }
    Ok(child_before)
}

fn query_directory(parent: &File, scratch: &mut ChildMembershipScratch, restart: bool,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<bool, ObservationError> {
    let class = if restart { FILE_ID_BOTH_DIRECTORY_RESTART_INFO } else { FILE_ID_BOTH_DIRECTORY_INFO };
    let bytes = u32::try_from(scratch.bytes.len()).map_err(|_| ObservationError::ObservationFailed)?;
    // The official sample uses 11 for the first query, 10 thereafter and accepts
    // only ERROR_NO_MORE_FILES as exhaustion. Both ABI calls are admitted first:
    // a caller checkpoint between the query and GetLastError could overwrite it.
    // https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/Win7Samples/winbase/io/extendedfileapis/ExtendedFileAPIs.cpp
    check(true)?;
    check(true)?;
    // SAFETY: both handles stay borrowed; scratch is initialized, exclusively
    // borrowed, eight-byte aligned and exactly the supplied bounded capacity.
    let result = unsafe { GetFileInformationByHandleEx(parent.as_raw_handle(), class,
        scratch.bytes.as_mut_ptr().cast(), bytes) };
    // SAFETY: this thread-local error capture immediately follows the query;
    // it has no pointer arguments or retained memory. No callback intervenes.
    let error = unsafe { GetLastError() };
    check(false)?;
    check(false)?;
    if result != 0 { return Ok(true); }
    if error == ERROR_NO_MORE_FILES { return Ok(false); }
    Err(ObservationError::ObservationFailed)
}

fn file_information_with_check(file: &File, check: &dyn Fn(bool) -> Result<(), ObservationError>) -> Result<FileInformation, ObservationError> {
    let handle = file.as_raw_handle();
    check(true)?;
    // SAFETY: `file` owns this live handle throughout the synchronous call.
    let kind = unsafe { GetFileType(handle) };
    check(false)?;
    if kind != FILE_TYPE_DISK {
        return Err(ObservationError::UnsupportedHandle);
    }
    let mut filesystem_name = [0_u16; 32];
    let mut volume_serial = 0_u32;
    check(true)?;
    // SAFETY: optional buffers are null with zero lengths; supplied pointers
    // reference live, correctly sized writable values. Nothing is retained.
    let volume_ok = unsafe {
        GetVolumeInformationByHandleW(
            handle,
            core::ptr::null_mut(),
            0,
            &raw mut volume_serial,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            filesystem_name.as_mut_ptr(),
            32,
        )
    };
    check(false)?;
    if volume_ok == 0 {
        return Err(ObservationError::ObservationFailed);
    }
    // No 128-bit ReFS identifier is silently shortened to the old u64 layout.
    if filesystem_name[..5] != [78, 84, 70, 83, 0] {
        return Err(ObservationError::UnsupportedFileSystem);
    }
    let mut info = FileInformation::default();
    check(true)?;
    // SAFETY: layout is asserted above; output is initialized and uniquely
    // borrowed. The borrowed File keeps the handle valid; failure is checked.
    let result = unsafe { GetFileInformationByHandle(handle, &raw mut info) };
    check(false)?;
    if result == 0 {
        return Err(ObservationError::ObservationFailed);
    }
    if info.volume_serial != volume_serial {
        return Err(ObservationError::ObservationFailed);
    }
    if info.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(ObservationError::ReparsePointDenied);
    }
    Ok(info)
}
