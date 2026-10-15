use super::{
    ChildMembershipScratch, DirectoryIdentity, File, NativeLocationClass, NormalizedNameKind,
    Observation, ObservationError, SyncReadFile, NORMALIZED_NAME_MAX_UTF16, directory_batch_matches,
};
use core::ffi::c_void;
use std::fs::OpenOptions;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

const FILE_TYPE_DISK: u32 = 1;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_SHARE_READ: u32 = 1;
const FILE_NAME_NORMALIZED: u32 = 0;
const VOLUME_NAME_DOS: u32 = 0;
const VOLUME_NAME_GUID: u32 = 1;
// FS_INFORMATION_CLASS from ntifs.h/MS-FSCC: FileFsDeviceInformation is level 4.
// https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fscc/ee12042a-9352-46e3-9f67-c094b75fe6c3
const FILE_FS_DEVICE_INFORMATION_CLASS: i32 = 4;
// FILE_INFO_BY_HANDLE_CLASS from minwinbase.h: FileCaseSensitiveInfo follows
// FileRenameInfoEx and therefore has the documented value 23.
// https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ne-file_info_by_handle_class
const FILE_CASE_SENSITIVE_INFO_CLASS: i32 = 23;
// FILE_FS_DEVICE_INFORMATION / DEVICE_OBJECT characteristics from ntifs.h.
// https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fscc/616b66d5-b335-4e1c-8f87-b4a55e8d3e4a
const FILE_DEVICE_DISK: u32 = 0x0000_0007;
const FILE_REMOVABLE_MEDIA: u32 = 0x0000_0001;
const FILE_REMOTE_DEVICE: u32 = 0x0000_0010;
const FILE_VIRTUAL_VOLUME: u32 = 0x0000_0040;
const FILE_CHARACTERISTIC_WEBDAV_DEVICE: u32 = 0x0000_2000;
const FILE_PORTABLE_DEVICE: u32 = 0x0000_4000;
const FILE_CS_FLAG_CASE_SENSITIVE_DIR: u32 = 0x0000_0001;
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

// FILE_FS_DEVICE_INFORMATION is exactly two ULONG fields in the documented
// WDK type: DeviceType followed by Characteristics.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FileFsDeviceInformation {
    device_type: u32,
    characteristics: u32,
}

// FILE_CASE_SENSITIVE_INFORMATION contains one ULONG Flags field.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FileCaseSensitiveInformation {
    flags: u32,
}

// IO_STATUS_BLOCK is the documented Native API completion record. The union
// is pointer-sized; the x64 assertions below prevent ABI drift.
#[repr(C)]
union IoStatusValue {
    status: i32,
    pointer: *mut c_void,
}

#[repr(C)]
struct IoStatusBlock {
    status: IoStatusValue,
    information: usize,
}

const _: () = {
    let pointer = core::mem::size_of::<*mut c_void>();
    assert!(pointer == 4 || pointer == 8);
    assert!(core::mem::size_of::<IoStatusValue>() == pointer);
    assert!(core::mem::size_of::<IoStatusBlock>() == pointer * 2);
    assert!(core::mem::offset_of!(IoStatusBlock, information) == pointer);
    assert!(core::mem::size_of::<FileFsDeviceInformation>() == 8);
    assert!(core::mem::align_of::<FileFsDeviceInformation>() == 4);
    assert!(core::mem::size_of::<FileCaseSensitiveInformation>() == 4);
    assert!(core::mem::align_of::<FileCaseSensitiveInformation>() == 4);
};

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<IoStatusBlock>() == 16);
    assert!(core::mem::offset_of!(IoStatusBlock, information) == 8);
};

#[link(name = "Kernel32")]
unsafe extern "system" {
    fn GetFileType(file: *mut c_void) -> u32;
    fn GetFileInformationByHandle(file: *mut c_void, output: *mut FileInformation) -> i32;
    // winbase.h (not fileapi.h); FILE_INFO_BY_HANDLE_CLASS is a 32-bit C enum.
    fn GetFileInformationByHandleEx(
        file: *mut c_void, information_class: i32, output: *mut c_void, bytes: u32,
    ) -> i32;
    fn GetFinalPathNameByHandleW(
        file: *mut c_void, output: *mut u16, output_length: u32, flags: u32,
    ) -> u32;
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

#[link(name = "ntdll")]
unsafe extern "system" {
    // User mode uses the Nt name. The safe wrapper accepts only the private
    // synchronous leaf constructed below, never an arbitrary public File.
    fn NtQueryVolumeInformationFile(
        file: *mut c_void,
        io_status: *mut IoStatusBlock,
        fs_information: *mut c_void,
        length: u32,
        information_class: i32,
    ) -> i32;
}

pub(super) fn open_no_follow(
    path: &Path,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<SyncReadFile, ObservationError> {
    check(true)?;
    let opened = {
        use std::os::windows::fs::OpenOptionsExt;
        // No FILE_FLAG_OVERLAPPED is specified: this is the synchronous
        // physical leaf whose handle may be used by the Native volume query.
        OpenOptions::new()
            .read(true)
            .write(false)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .share_mode(FILE_SHARE_READ)
            .open(path)
    };
    let refusal = check(false).err();
    let file = match refusal {
        Some(error) => return Err(error),
        None => opened.map_err(|_| ObservationError::ObservationFailed)?,
    };
    let observed = observe_with_check(&file, check)?;
    if observed.attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
        return Err(ObservationError::UnsupportedHandle);
    }
    Ok(SyncReadFile { file })
}

pub(super) fn location_with_check(
    file: &SyncReadFile,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<NativeLocationClass, ObservationError> {
    let mut information = FileFsDeviceInformation::default();
    let mut io_status = IoStatusBlock {
        status: IoStatusValue { pointer: core::ptr::null_mut() },
        information: 0,
    };
    let length = u32::try_from(core::mem::size_of::<FileFsDeviceInformation>())
        .map_err(|_| ObservationError::ObservationFailed)?;
    check(true)?;
    // SAFETY: the private SyncReadFile owns a live synchronous handle; both
    // ABI outputs are initialized, correctly laid out and uniquely borrowed.
    // The synchronous handle keeps these stack outputs live until completion.
    let result = unsafe {
        NtQueryVolumeInformationFile(
            file.file.as_raw_handle(),
            &raw mut io_status,
            (&raw mut information).cast(),
            length,
            FILE_FS_DEVICE_INFORMATION_CLASS,
        )
    };
    let completion = unsafe { io_status.status.status };
    let bytes = io_status.information;
    let facts = information;
    check(false)?;
    if result != 0 || completion != 0 || bytes != core::mem::size_of::<FileFsDeviceInformation>() {
        return Err(ObservationError::ObservationFailed);
    }
    Ok(classify_location(facts))
}

const fn classify_location(information: FileFsDeviceInformation) -> NativeLocationClass {
    let characteristics = information.characteristics;
    let remote = characteristics & FILE_REMOTE_DEVICE != 0;
    let webdav = characteristics & FILE_CHARACTERISTIC_WEBDAV_DEVICE != 0;
    let removable = characteristics & (FILE_REMOVABLE_MEDIA | FILE_PORTABLE_DEVICE) != 0;
    if remote && webdav { return NativeLocationClass::Remote; }
    if remote { return if removable { NativeLocationClass::Unknown } else { NativeLocationClass::NetworkShare }; }
    if webdav { return NativeLocationClass::Remote; }
    if information.device_type != FILE_DEVICE_DISK { return NativeLocationClass::Unsupported; }
    if characteristics & FILE_VIRTUAL_VOLUME != 0 { return NativeLocationClass::Unsupported; }
    if removable { NativeLocationClass::LocalRemovable } else { NativeLocationClass::LocalFixed }
}

pub(super) fn normalized_name_with_check(
    file: &File,
    kind: NormalizedNameKind,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<String, ObservationError> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(NORMALIZED_NAME_MAX_UTF16).map_err(|_| ObservationError::ObservationFailed)?;
    buffer.resize(NORMALIZED_NAME_MAX_UTF16, 0);
    let capacity = u32::try_from(buffer.len()).map_err(|_| ObservationError::ObservationFailed)?;
    let flags = match kind { NormalizedNameKind::VolumeGuid => VOLUME_NAME_GUID | FILE_NAME_NORMALIZED, NormalizedNameKind::Dos => VOLUME_NAME_DOS | FILE_NAME_NORMALIZED };
    check(true)?;
    // SAFETY: the borrowed handle remains live; the fixed bounded UTF-16 buffer is writable.
    let result = unsafe { GetFinalPathNameByHandleW(file.as_raw_handle(), buffer.as_mut_ptr(), capacity, flags) };
    check(false)?;
    let length = usize::try_from(result).map_err(|_| ObservationError::ObservationFailed)?;
    if result == 0 || length >= buffer.len() || buffer[length] != 0 { return Err(ObservationError::ObservationFailed); }
    let units = &buffer[..length];
    if units.contains(&0) { return Err(ObservationError::ObservationFailed); }
    String::from_utf16(units).map_err(|_| ObservationError::ObservationFailed)
}

pub(super) fn case_sensitive_directory_with_check(
    file: &File,
    check: &dyn Fn(bool) -> Result<(), ObservationError>,
) -> Result<bool, ObservationError> {
    let observed = file_information_with_check(file, check)?;
    if observed.attributes & FILE_ATTRIBUTE_DIRECTORY == 0 { return Err(ObservationError::UnsupportedHandle); }
    let mut information = FileCaseSensitiveInformation::default();
    let length = u32::try_from(core::mem::size_of::<FileCaseSensitiveInformation>()).map_err(|_| ObservationError::ObservationFailed)?;
    check(true)?;
    // SAFETY: the borrowed handle remains live and the fixed output is initialized and uniquely borrowed.
    let result = unsafe { GetFileInformationByHandleEx(file.as_raw_handle(), FILE_CASE_SENSITIVE_INFO_CLASS, (&raw mut information).cast(), length) };
    check(false)?;
    if result == 0 || information.flags & !FILE_CS_FLAG_CASE_SENSITIVE_DIR != 0 { return Err(ObservationError::ObservationFailed); }
    Ok(information.flags & FILE_CS_FLAG_CASE_SENSITIVE_DIR != 0)
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
