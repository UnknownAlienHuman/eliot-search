//! Actual NTFS ACL changes over component reads, not NativeCtx/SourceGrant proof.
//! The external ACL actor is outside the component's source work accounting.

use super::*;
use std::cell::Cell;
use std::os::windows::process::CommandExt;
use std::process::{Command, Output};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const BODY: &[u8] = b"owned scratch leaf; never a source grant\n";
const PREFIX: &str = "search-scoped-read-acl-";
const CAPTURE: &str = "$ErrorActionPreference='Stop'; $a=Get-Acl -LiteralPath $env:ELIOT_SCOPED_ACL_PATH; [Console]::Out.Write($a.GetSecurityDescriptorSddlForm([System.Security.AccessControl.AccessControlSections]::Access))";
const RESTORE: &str = "$ErrorActionPreference='Stop'; $a=Get-Acl -LiteralPath $env:ELIOT_SCOPED_ACL_PATH; $a.SetSecurityDescriptorSddlForm($env:ELIOT_SCOPED_ACL_SDDL,[System.Security.AccessControl.AccessControlSections]::Access); Set-Acl -LiteralPath $env:ELIOT_SCOPED_ACL_PATH -AclObject $a";

fn hidden(program: &str) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW, no shell interpolation.
    command
}

fn checked(output: std::io::Result<Output>) -> Result<Vec<u8>, &'static str> {
    let output = output.map_err(|_| "ACL_TOOL_UNAVAILABLE")?;
    if !output.status.success() {
        return Err("ACL_TOOL_REFUSED");
    }
    if output.stdout.len() > 64 * 1024 {
        return Err("ACL_OUTPUT_BOUND");
    }
    Ok(output.stdout)
}

fn powershell(script: &str, path: &Path, sddl: Option<&str>) -> Result<String, &'static str> {
    let mut command = hidden("powershell.exe");
    // Let Windows PowerShell build its own module path instead of inheriting
    // incompatible PowerShell 7 modules from the test runner's parent shell.
    command.env_remove("PSModulePath");
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        script,
    ]);
    // Paths and captured descriptors are data, never command-language text.
    command.env("ELIOT_SCOPED_ACL_PATH", path);
    if let Some(sddl) = sddl {
        command.env("ELIOT_SCOPED_ACL_SDDL", sddl);
    }
    String::from_utf8(checked(command.output())?).map_err(|_| "ACL_OUTPUT_ENCODING")
}

struct Scratch {
    base: PathBuf,
    temp: PathBuf,
    cleanup_ok: Rc<Cell<bool>>,
}
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temp = fs::canonicalize(std::env::temp_dir()).expect("canonical temp");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = temp.join(format!(
            "{PREFIX}{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).expect("new owned scratch");
        fs::create_dir_all(base.join("data/known")).unwrap();
        fs::write(base.join("data/known/leaf.bin"), BODY).unwrap();
        Self {
            base,
            temp,
            cleanup_ok: Rc::new(Cell::new(true)),
        }
    }
    fn store(&self) -> crate::plaintext_direct_store::DirectStore {
        crate::plaintext_direct_store::DirectStore::open(&self.base.join("data"))
            .expect("component catalog only")
    }
    fn require_ntfs(&self) {
        let file = File::open(self.base.join("data/known/leaf.bin")).unwrap();
        if eliot_searchd::native_file::observe(&file).is_err() {
            panic!("NOT_RUN: qualified NTFS observation unavailable");
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if !self.cleanup_ok.get() {
            eprintln!("SCOPED_READ_ACL_SCRATCH_RETAINED restore_failed=1");
            return;
        }
        // Resolve and fence the exact fresh fixture before recursive removal.
        let current = fs::canonicalize(&self.base).expect("owned scratch still exists");
        assert_eq!(current, self.base);
        assert_eq!(current.parent(), Some(self.temp.as_path()));
        assert!(
            current
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(PREFIX)
        );
        assert!(!is_reparse(&fs::symlink_metadata(&current).unwrap()));
        fs::remove_dir_all(current).expect("remove owned scratch after ACLs and pins");
    }
}

struct OriginalAcl {
    path: PathBuf,
    sddl: String,
    armed: bool,
    cleanup_ok: Rc<Cell<bool>>,
}
impl OriginalAcl {
    fn capture(path: &Path, scratch: &Scratch) -> Self {
        assert!(path.starts_with(&scratch.base));
        let sddl =
            powershell(CAPTURE, path, None).unwrap_or_else(|cause| panic!("NOT_RUN: {cause}"));
        assert!(sddl.starts_with("D:"), "DACL-only captured descriptor");
        Self {
            path: path.to_owned(),
            sddl,
            armed: false,
            cleanup_ok: Rc::clone(&scratch.cleanup_ok),
        }
    }
    fn deny_read_data_only(&mut self, sid: &str) {
        assert!(
            sid.starts_with("S-1-")
                && sid
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'S' | b'-'))
        );
        // RD is FILE_LIST_DIRECTORY on a directory, FILE_READ_DATA on a file.
        // No OI/CI, inheritance change, /T, /reset or recursive denial exists.
        self.armed = true; // Also restore after a possible partial command failure.
        let denied = format!("*{sid}:(RD)");
        checked(
            hidden("icacls.exe")
                .arg(&self.path)
                .arg("/deny")
                .arg(denied)
                .arg("/q")
                .output(),
        )
        .unwrap_or_else(|cause| panic!("NOT_RUN: {cause}"));
    }
    fn restore(&mut self) -> Result<(), &'static str> {
        if !self.armed {
            return Ok(());
        }
        powershell(RESTORE, &self.path, Some(&self.sddl))?;
        if powershell(CAPTURE, &self.path, None)? != self.sddl {
            return Err("ACL_RESTORE_MISMATCH");
        }
        self.armed = false;
        Ok(())
    }
}
impl Drop for OriginalAcl {
    fn drop(&mut self) {
        if self.restore().is_err() {
            self.cleanup_ok.set(false);
            eprintln!("SCOPED_READ_ACL_RESTORE_FAILED");
            if !std::thread::panicking() {
                panic!("captured scratch DACL restore failed");
            }
        }
    }
}

#[test]
fn cached_parents_keep_leaf_access_fresh_under_list_only_and_leaf_read_denials() {
    let scratch = Scratch::new();
    scratch.require_ntfs();
    let store = scratch.store();
    let source = MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
    assert!(
        source.native().is_none(),
        "component context grants no NativeCtx"
    );
    let parent = source.inventory_root().join("known");
    let leaf = parent.join("leaf.bin");
    let mut parent_acl = OriginalAcl::capture(&parent, &scratch);
    let mut leaf_acl = OriginalAcl::capture(&leaf, &scratch);
    let sid = powershell("[Console]::Out.Write([System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value)",
        &parent, None).unwrap_or_else(|cause| panic!("NOT_RUN: {cause}"));
    let mut scope = ParentReadScope::new();
    let first = read_detailed_scoped(&source, &leaf, 4096, &mut scope)
        .expect("original component context")
        .unwrap_or_else(|_| panic!("first qualified read"));
    assert_eq!(&*first.bytes, BODY);

    parent_acl.deny_read_data_only(&sid);
    let denied = match fs::read_dir(&parent) {
        Err(error) => error,
        Ok(_) => panic!("NOT_RUN: directory LIST denial was not effective"),
    };
    assert_eq!(denied.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(
        powershell(CAPTURE, &leaf, None).unwrap(),
        leaf_acl.sddl,
        "directory-only ACE must not alter or inherit onto the leaf DACL"
    );
    // A fresh full-path GENERIC_READ/traverse still succeeds without parent LIST.
    assert_eq!(fs::read(&leaf).expect("fresh full-path leaf access"), BODY);
    let second = read_detailed_scoped(&source, &leaf, 4096, &mut scope)
        .expect("retained original directories")
        .unwrap_or_else(|_| panic!("LIST is not leaf read authority"));
    assert_eq!(&*second.bytes, BODY);
    assert_eq!(second.proof, first.proof);
    match read_detailed(&source, &leaf, 4096).expect("unscoped context") {
        Err(Status::Unreadable) => eprintln!("SCOPED_READ_ACL unscoped_parent_reopen=unreadable"),
        Ok(read) => {
            assert_eq!(&*read.bytes, BODY);
            eprintln!("SCOPED_READ_ACL unscoped_parent_reopen=readable");
        }
        Err(_) => panic!("unexpected unscoped observation"),
    }

    leaf_acl.deny_read_data_only(&sid);
    let denied =
        File::open(&leaf).expect_err("fresh GENERIC_READ must observe leaf READ_DATA denial");
    assert_eq!(denied.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(
        matches!(
            read_detailed_scoped(&source, &leaf, 4096, &mut scope),
            Ok(Err(Status::Unreadable))
        ),
        "cached parents must return no body for the denied leaf"
    );
    // Explicit success-path finally, plus both guards' Drop on every early exit.
    parent_acl
        .restore()
        .expect("restore original directory DACL");
    leaf_acl.restore().expect("restore original leaf DACL");
    assert_eq!(fs::read(&leaf).expect("restored leaf"), BODY);
    eprintln!(
        "SCOPED_READ_ACL list_denied=1 fresh_leaf_read=1 leaf_read_denied=1 scoped_unreadable=1 dacl_restored=1 native_ctx=0 source_grant=0"
    );
}

#[test]
fn foreign_root_locator_is_refused_by_the_original_source_scope() {
    let scratch = Scratch::new();
    let foreign = Scratch::new();
    scratch.require_ntfs();
    foreign.require_ntfs();
    let store = scratch.store();
    let source = MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
    assert!(source.native().is_none());
    let mut scope = ParentReadScope::new();
    let own = source.inventory_root().join("known/leaf.bin");
    assert!(
        read_detailed_scoped(&source, &own, 4096, &mut scope)
            .unwrap()
            .is_ok()
    );
    let other = foreign.base.join("data/known/leaf.bin");
    assert!(
        matches!(read_detailed_scoped(&source, &other, 4096, &mut scope),
        Err(error) if error == changed()),
        "foreign root cannot use the original source context"
    );
    assert_eq!(fs::read(other).unwrap(), BODY);
    eprintln!(
        "SCOPED_READ_ACL foreign_root_refused=1 original_source_context=1 native_ctx=0 source_grant=0"
    );
}
