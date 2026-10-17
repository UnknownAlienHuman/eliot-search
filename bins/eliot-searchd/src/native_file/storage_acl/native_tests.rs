//! Real ACL changes are confined to exact newly created, empty fixture directories.
use super::*;
use core::ffi::c_void;
use search_os_secrets_windows::{
    self as secrets, LegacyRevisionDpapiContext, LegacyRevisionDpapiWork, RevisionDekContext,
};
use std::{
    fs::OpenOptions,
    os::windows::fs::OpenOptionsExt,
    os::windows::io::AsRawHandle,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

struct Context {
    deadline: Instant,
    refused: bool,
    calls: usize,
    opens: usize,
    bytes: usize,
}
impl Context {
    fn new() -> Self {
        Self {
            deadline: Instant::now() + Duration::from_secs(30),
            refused: false,
            calls: 0,
            opens: 0,
            bytes: 0,
        }
    }
    fn poll(&self) -> Result<(), &'static str> {
        if self.refused || Instant::now() >= self.deadline {
            Err("fixture-original-refusal")
        } else {
            Ok(())
        }
    }
    fn work(&mut self, work: StorageSecurityWork) -> Result<(), &'static str> {
        self.poll()?;
        match work {
            StorageSecurityWork::NativeCall => {
                self.calls += 1;
                if self.calls > 16384 {
                    return Err("fixture-call-bound");
                }
            }
            StorageSecurityWork::Open => {
                self.opens += 1;
                if self.opens > 256 {
                    return Err("fixture-open-bound");
                }
            }
            StorageSecurityWork::Allocate(bytes) | StorageSecurityWork::Process(bytes) => {
                self.bytes = self
                    .bytes
                    .checked_add(bytes)
                    .ok_or("fixture-byte-overflow")?;
                if self.bytes > 1024 * 1024 * 1024 {
                    return Err("fixture-byte-bound");
                }
            }
        }
        Ok(())
    }
}
impl StorageSecurityContext for Context {
    type Error = &'static str;
    fn absolute_deadline(&self) -> Instant {
        self.deadline
    }
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        self.poll()
    }
    fn charge(&mut self, work: StorageSecurityWork) -> Result<(), Self::Error> {
        self.work(work)
    }
}
impl LegacyRevisionDpapiContext for Context {
    type Error = &'static str;
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        self.poll()
    }
    fn charge(&mut self, work: LegacyRevisionDpapiWork) -> Result<(), Self::Error> {
        self.work(match work {
            LegacyRevisionDpapiWork::NativeCall => StorageSecurityWork::NativeCall,
            LegacyRevisionDpapiWork::Allocate(n) => StorageSecurityWork::Allocate(n),
            LegacyRevisionDpapiWork::Process(n) => StorageSecurityWork::Process(n),
        })
    }
    fn account_native_clear(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or("fixture-clear-overflow")?;
        Ok(())
    }
}
impl RevisionDekContext for Context {
    fn deadline(&self) -> Instant {
        self.deadline
    }
    fn charge_open(&mut self) -> Result<(), Self::Error> {
        self.work(StorageSecurityWork::Open)
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch {
    root: PathBuf,
    owned: bool,
}
impl Scratch {
    fn new() -> Self {
        let mut scratch = Self {
            root: std::env::temp_dir().join(format!(
                "eliot-search-594-acl-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
            owned: false,
        };
        assert!(scratch.root.is_absolute());
        std::fs::create_dir(&scratch.root).expect("create exact empty ACL fixture");
        scratch.owned = true;
        // Test preparation only: this host's Temp inherits foreign trustees.
        // Protect only our newly created empty fixture; production never repairs roots.
        let file = directory(&scratch.root);
        let mut acl = trusted_fixture_acl();
        assert_eq!(
            unsafe {
                SetSecurityInfo(
                    file.as_raw_handle(),
                    1,
                    4 | 0x8000_0000,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    acl.as_mut_ptr().cast(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        scratch
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if !self.owned {
            return;
        }
        for name in ["private-child", "refused-child"] {
            let child = self.root.join(name);
            if child.exists() && std::fs::remove_dir(&child).is_err() {
                if !std::thread::panicking() {
                    panic!("exact empty ACL child cleanup unresolved");
                }
                return;
            }
        }
        if std::fs::remove_dir(&self.root).is_err() && !std::thread::panicking() {
            panic!("exact empty ACL root cleanup unresolved");
        }
    }
}
fn directory(path: &std::path::Path) -> std::fs::File {
    OpenOptions::new()
        .access_mode(0x0006_0080)
        .custom_flags(0x0220_0000)
        .share_mode(7)
        .open(path)
        .expect("open owned fixture with READ_CONTROL and WRITE_DAC")
}

fn trusted_fixture_acl() -> Vec<u32> {
    // Owner Rights grants the already-observed current object owner full control.
    // These exact new fixtures have no foreign creator or retained user contents.
    let sids: [&[u8]; 3] = [
        &[1, 1, 0, 0, 0, 0, 0, 3, 4, 0, 0, 0],
        &[1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0],
        &[1, 2, 0, 0, 0, 0, 0, 5, 32, 0, 0, 0, 32, 2, 0, 0],
    ];
    let length = 8 + sids.iter().map(|sid| 8 + sid.len()).sum::<usize>();
    let mut bytes = vec![2, 0];
    bytes.extend_from_slice(&u16::try_from(length).unwrap().to_le_bytes());
    bytes.extend_from_slice(&[3, 0, 0, 0]);
    for sid in sids {
        bytes.extend_from_slice(&[0, 3]);
        bytes.extend_from_slice(&u16::try_from(8 + sid.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&0x001f_01ff_u32.to_le_bytes());
        bytes.extend_from_slice(sid);
    }
    bytes
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect()
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn GetSecurityInfo(
        handle: *mut c_void,
        kind: u32,
        info: u32,
        owner: *mut *mut c_void,
        group: *mut *mut c_void,
        dacl: *mut *mut c_void,
        sacl: *mut *mut c_void,
        descriptor: *mut *mut c_void,
    ) -> u32;
    fn SetSecurityInfo(
        handle: *mut c_void,
        kind: u32,
        info: u32,
        owner: *mut c_void,
        group: *mut c_void,
        dacl: *mut c_void,
        sacl: *mut c_void,
    ) -> u32;
    fn GetSecurityDescriptorControl(
        descriptor: *const c_void,
        control: *mut u16,
        revision: *mut u32,
    ) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
}

struct Restore<'a> {
    file: &'a std::fs::File,
    descriptor: *mut c_void,
    dacl: *mut c_void,
    flags: u32,
    active: bool,
}
impl<'a> Restore<'a> {
    fn capture(file: &'a std::fs::File) -> Self {
        let mut descriptor = std::ptr::null_mut();
        let mut dacl = std::ptr::null_mut();
        let code = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                1,
                4,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        assert_eq!(code, 0, "capture fixture DACL before mutation");
        assert!(!descriptor.is_null());
        let mut guard = Self {
            file,
            descriptor,
            dacl,
            flags: 4 | 0x8000_0000,
            active: true,
        };
        let (mut control, mut revision) = (0, 0);
        assert_ne!(
            unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) },
            0
        );
        guard.flags = 4 | if control & 0x1000 != 0 {
            0x8000_0000
        } else {
            0x2000_0000
        };
        guard
    }
    fn replace(&self, dacl: *mut c_void) {
        assert_eq!(
            unsafe {
                SetSecurityInfo(
                    self.file.as_raw_handle(),
                    1,
                    4 | 0x8000_0000,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    dacl,
                    std::ptr::null_mut(),
                )
            },
            0,
            "change only the exact empty fixture DACL"
        );
    }
    fn restore(&mut self) {
        if self.active {
            assert_eq!(
                unsafe {
                    SetSecurityInfo(
                        self.file.as_raw_handle(),
                        1,
                        self.flags,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        self.dacl,
                        std::ptr::null_mut(),
                    )
                },
                0,
                "restore exact fixture DACL"
            );
            self.active = false;
        }
    }
}
impl Drop for Restore<'_> {
    fn drop(&mut self) {
        if self.active {
            let status = unsafe {
                SetSecurityInfo(
                    self.file.as_raw_handle(),
                    1,
                    self.flags,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    self.dacl,
                    std::ptr::null_mut(),
                )
            };
            if status != 0 && !std::thread::panicking() {
                panic!("exact fixture DACL restoration unresolved");
            }
        }
        let failed = !unsafe { LocalFree(self.descriptor) }.is_null();
        if failed && !std::thread::panicking() {
            panic!("fixture descriptor release unresolved");
        }
    }
}

#[test]
fn native_private_child_is_atomic_and_null_foreign_changes_refuse() {
    let scratch = Scratch::new();
    let root = directory(&scratch.root);
    let mut context = Context::new();
    let user = secrets::observe_revision_user(&mut context).expect("observe actual fixture user");
    let expected = user.user_scope_digest();
    let parent = observe_owner_private_acl(&root, expected, &mut context)
        .expect("qualify actual private parent");
    let path = scratch.root.join("private-child");
    create_owner_private_directory(&path, &parent, &mut context).expect("atomic private child");
    let child = directory(&path);
    let first =
        observe_owner_private_acl(&child, expected, &mut context).expect("qualify created child");
    assert_eq!(
        parent,
        observe_owner_private_acl(&root, expected, &mut context).unwrap(),
        "parent ACL was rewritten"
    );
    assert!(
        create_owner_private_directory(&path, &parent, &mut context).is_err(),
        "existing child must never be adopted"
    );
    assert_eq!(
        first,
        observe_owner_private_acl(&child, expected, &mut context).unwrap()
    );

    let mut restore = Restore::capture(&child);
    restore.replace(std::ptr::null_mut());
    assert!(
        observe_owner_private_acl(&child, expected, &mut context).is_err(),
        "null DACL is unrestricted"
    );
    restore.restore();
    observe_owner_private_acl(&child, expected, &mut context).expect("restored private child");

    let mut restore = Restore::capture(&child);
    // ACL revision2, one ordinary allow ACE: Everyone receives FILE_ALL_ACCESS.
    let bytes: [u8; 28] = [
        2, 0, 28, 0, 1, 0, 0, 0, 0, 0, 20, 0, 255, 1, 31, 0, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
    ];
    let mut words = [0_u32; 7];
    for (target, source) in words.iter_mut().zip(bytes.chunks_exact(4)) {
        *target = u32::from_le_bytes(source.try_into().unwrap());
    }
    restore.replace(words.as_mut_ptr().cast());
    assert!(
        observe_owner_private_acl(&child, expected, &mut context).is_err(),
        "foreign trustee must refuse"
    );
    restore.restore();
    observe_owner_private_acl(&child, expected, &mut context).expect("restored private child");
    assert!(context.opens <= 256 && context.calls <= 16384);
}

#[test]
fn native_original_refusal_prevents_creation_and_preserves_parent() {
    let scratch = Scratch::new();
    let root = directory(&scratch.root);
    let mut context = Context::new();
    let expected = secrets::observe_revision_user(&mut context)
        .unwrap()
        .user_scope_digest();
    let parent = observe_owner_private_acl(&root, expected, &mut context).unwrap();
    let calls = context.calls;
    context.refused = true;
    let path = scratch.root.join("refused-child");
    assert!(create_owner_private_directory(&path, &parent, &mut context).is_err());
    assert!(!path.exists());
    assert_eq!(
        context.calls, calls,
        "refused context must not call Windows"
    );
}
