//! Fixed catalog names resolved only beneath the retained native control handle.

use std::fs::File;

use search_runtime_owner::OwnerError;

#[derive(Clone, Copy)]
pub(super) enum CatalogName {
    Primary,
    Temporary,
    Backup,
}

impl CatalogName {
    pub(super) const fn text(self) -> &'static str {
        match self {
            Self::Primary => "source-roots.v1",
            Self::Temporary => "source-roots.tmp",
            Self::Backup => "source-roots.bak",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum CatalogOpen {
    ExistingMutation,
    ExistingRead,
    CreateNew,
}

pub(super) fn open(
    parent: &File,
    name: CatalogName,
    mode: CatalogOpen,
) -> Result<Option<File>, OwnerError> {
    platform::open(parent, name, mode)
}

pub(super) fn rename(file: &File, parent: &File, name: CatalogName) -> Result<(), OwnerError> {
    platform::rename(file, parent, name)
}

pub(super) fn mark_delete(file: &File) -> Result<(), OwnerError> {
    platform::mark_delete(file)
}

#[cfg(not(windows))]
mod platform {
    use super::{CatalogName, CatalogOpen, File, OwnerError};

    pub(super) fn open(
        _parent: &File,
        _name: CatalogName,
        _mode: CatalogOpen,
    ) -> Result<Option<File>, OwnerError> {
        Err(OwnerError::OwnerRecoveryQuarantined)
    }

    pub(super) fn rename(
        _file: &File,
        _parent: &File,
        _name: CatalogName,
    ) -> Result<(), OwnerError> {
        Err(OwnerError::OwnerRecoveryQuarantined)
    }

    pub(super) fn mark_delete(_file: &File) -> Result<(), OwnerError> {
        Err(OwnerError::OwnerRecoveryQuarantined)
    }
}

// Only the checked synchronous native calls below may use unsafe code.
#[cfg(windows)]
#[allow(unsafe_code)]
mod platform {
    use super::{CatalogName, CatalogOpen, File, OwnerError};
    use core::ffi::c_void;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};

    const FILE_GENERIC_READ: u32 = 0x0012_0089;
    const FILE_GENERIC_WRITE: u32 = 0x0012_0116;
    const DELETE: u32 = 0x0001_0000;
    const FILE_OPEN: u32 = 1;
    const FILE_CREATE: u32 = 2;
    const FILE_OPENED: usize = 1;
    const FILE_CREATED: usize = 2;
    const STATUS_OBJECT_NAME_NOT_FOUND: i32 = -1_073_741_772;

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }

    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root: *mut c_void,
        name: *mut UnicodeString,
        attributes: u32,
        security: *mut c_void,
        quality: *mut c_void,
    }

    #[repr(C)]
    union IoStatus {
        status: i32,
        pointer: *mut c_void,
    }

    #[repr(C)]
    struct IoStatusBlock {
        status: IoStatus,
        information: usize,
    }

    // FILE_RENAME_INFO's first DWORD union is zero: ReplaceIfExists is FALSE.
    #[repr(C)]
    struct RenameInfo {
        flags: u32,
        root: *mut c_void,
        length: u32,
        name: [u16; 17],
    }

    const _: () = {
        let pointer = core::mem::size_of::<*mut c_void>();
        assert!(pointer == 4 || pointer == 8);
        assert!(core::mem::size_of::<UnicodeString>() == pointer * 2);
        assert!(core::mem::offset_of!(UnicodeString, buffer) == pointer);
        assert!(core::mem::size_of::<ObjectAttributes>() == pointer * 6);
        assert!(core::mem::offset_of!(ObjectAttributes, root) == pointer);
        assert!(core::mem::offset_of!(ObjectAttributes, name) == pointer * 2);
        assert!(core::mem::offset_of!(ObjectAttributes, attributes) == pointer * 3);
        assert!(core::mem::offset_of!(ObjectAttributes, security) == pointer * 4);
        assert!(core::mem::offset_of!(ObjectAttributes, quality) == pointer * 5);
        assert!(core::mem::size_of::<IoStatusBlock>() == pointer * 2);
        assert!(core::mem::offset_of!(IoStatusBlock, information) == pointer);
        assert!(core::mem::offset_of!(RenameInfo, root) == pointer);
        assert!(core::mem::offset_of!(RenameInfo, length) == pointer * 2);
        assert!(core::mem::offset_of!(RenameInfo, name) == pointer * 2 + 4);
    };

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtCreateFile(
            file: *mut *mut c_void,
            access: u32,
            attributes: *mut ObjectAttributes,
            status: *mut IoStatusBlock,
            allocation: *const i64,
            file_attributes: u32,
            sharing: u32,
            disposition: u32,
            options: u32,
            extended_attributes: *const c_void,
            extended_length: u32,
        ) -> i32;
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn SetFileInformationByHandle(
            file: *mut c_void,
            class: i32,
            information: *mut c_void,
            bytes: u32,
        ) -> i32;
    }

    fn encoded_name(name: CatalogName) -> ([u16; 17], u16) {
        let mut units = [0; 17];
        let mut count = 0_u16;
        // The closed ASCII names above fit 16 UTF-16 units; no caller path enters.
        for (index, byte) in name.text().bytes().enumerate() {
            units[index] = u16::from(byte);
            count += 2;
        }
        (units, count)
    }

    pub(super) fn open(
        parent: &File,
        name: CatalogName,
        mode: CatalogOpen,
    ) -> Result<Option<File>, OwnerError> {
        let (mut units, length) = encoded_name(name);
        let mut unicode = UnicodeString {
            length,
            maximum_length: length,
            buffer: units.as_mut_ptr(),
        };
        let mut attributes = ObjectAttributes {
            length: u32::try_from(core::mem::size_of::<ObjectAttributes>())
                .map_err(|_| OwnerError::OwnerGuardMismatch)?,
            root: parent.as_raw_handle(),
            name: &raw mut unicode,
            attributes: 0x40, // OBJ_CASE_INSENSITIVE, matching the existing Windows locator.
            security: core::ptr::null_mut(),
            quality: core::ptr::null_mut(),
        };
        let mut status = IoStatusBlock {
            status: IoStatus {
                pointer: core::ptr::null_mut(),
            },
            information: 0,
        };
        let mut raw = core::ptr::null_mut();
        let (access, sharing, disposition, expected) = match mode {
            CatalogOpen::ExistingMutation => (
                FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE,
                1,
                FILE_OPEN,
                FILE_OPENED,
            ),
            CatalogOpen::ExistingRead => (FILE_GENERIC_READ, 7, FILE_OPEN, FILE_OPENED),
            CatalogOpen::CreateNew => (
                FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE,
                1,
                FILE_CREATE,
                FILE_CREATED,
            ),
        };
        // SAFETY: all counted strings, ABI-asserted structures and outputs remain
        // live and uniquely borrowed for this synchronous call. The retained parent
        // handle is borrowed. No absolute name, inheritance or delete-on-close flag.
        let result = unsafe {
            NtCreateFile(
                &raw mut raw,
                access,
                &raw mut attributes,
                &raw mut status,
                core::ptr::null(),
                0x80,
                sharing,
                disposition,
                0x0020_0060, // OPEN_REPARSE_POINT | NON_DIRECTORY_FILE | SYNCHRONOUS_IO_NONALERT.
                core::ptr::null(),
                0,
            )
        };
        if matches!(
            mode,
            CatalogOpen::ExistingMutation | CatalogOpen::ExistingRead
        ) && result == STATUS_OBJECT_NAME_NOT_FOUND
        {
            return Ok(None);
        }
        // A failed output parameter does not establish ownership of a handle.
        if result != 0 || raw.is_null() || raw as isize == -1 {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        // SAFETY: completed successful NtCreateFile returned this newly owned
        // valid handle. Adopt it exactly once, before checking completion details.
        let file = unsafe { File::from_raw_handle(raw) };
        // A pending/informational or inconsistent completion is never accepted.
        // SAFETY: synchronous NtCreateFile initialized the asserted status union.
        if unsafe { status.status.status } != 0 || status.information != expected {
            drop(file); // No delete-on-close; uncertain created artifacts remain.
            return Err(OwnerError::OwnerGuardMismatch);
        }
        Ok(Some(file))
    }

    pub(super) fn rename(file: &File, parent: &File, name: CatalogName) -> Result<(), OwnerError> {
        let (units, length) = encoded_name(name);
        let mut info = RenameInfo {
            flags: 0,
            root: parent.as_raw_handle(),
            length: u32::from(length),
            name: units,
        };
        // SAFETY: checked ABI, live borrowed original/parent handles, bounded
        // counted fixed name and synchronous call; no replacement is requested.
        let info_length = u32::try_from(core::mem::size_of::<RenameInfo>())
            .map_err(|_| OwnerError::OwnerGuardMismatch)?;
        let success = unsafe {
            SetFileInformationByHandle(file.as_raw_handle(), 3, (&raw mut info).cast(), info_length)
        };
        if success == 0 {
            Err(OwnerError::OwnerGuardMismatch)
        } else {
            Ok(())
        }
    }

    pub(super) fn mark_delete(file: &File) -> Result<(), OwnerError> {
        let mut delete = 1_u8; // FILE_DISPOSITION_INFO::DeleteFile is BOOLEAN.
        // SAFETY: the live original handle has DELETE access. The one-byte
        // BOOLEAN remains live and uniquely borrowed; the call retains no pointer.
        let success = unsafe {
            SetFileInformationByHandle(file.as_raw_handle(), 4, (&raw mut delete).cast(), 1)
        };
        if success == 0 {
            Err(OwnerError::OwnerGuardMismatch)
        } else {
            Ok(())
        }
    }
}
