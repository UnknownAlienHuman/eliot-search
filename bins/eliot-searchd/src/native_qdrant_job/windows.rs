//! Checked Win32 Qdrant Job Object spawn boundary (Windows SDK 10.0.26100).
//!
//! The child is created suspended with stable `CommandExt::creation_flags`,
//! assigned and verified in a non-breakaway `KILL_ON_JOB_CLOSE` Job, then the
//! single exact suspended primary thread is resumed. No PID is opened or adopted.

use core::ffi::c_void;
use std::cell::RefCell;
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io;
use std::mem::{offset_of, size_of, zeroed};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command};
use std::ptr::null;
use std::sync::Arc;

use search_contracts::{Blake3Digest32, ReceiptRef};
use search_qdrant_supervisor::{
    ChildContainmentCustody, ContainedLaunchError, ContainedLaunchWork, ContainedSpawn,
    ContainedSpawnPort, ContainedSpawnPortFailure, LaunchPlan, SecretMaterial, SpawnedChild,
    spawn_qualified_contained,
};

use crate::native_file::{
    DirectoryIdentity, Observation, QualifiedDirectory, StorageAclError, StorageSecurityContext,
    StorageSecurityWork, observe_owner_private_acl, observe_with_check,
};

use super::{NativeQdrantJobError, original_suspend_count_is_single, record_thread_candidate};

const CREATE_SUSPENDED: u32 = 0x0000_0004;
const JOBOBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: u32 = 9;
const JOBOBJECT_BASIC_ACCOUNTING_INFORMATION_CLASS: u32 = 1;
const JOB_OBJECT_LIMIT_BREAKAWAY_OK: u32 = 0x0000_0800;
const JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK: u32 = 0x0000_1000;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
const THREAD_SUSPEND_RESUME: u32 = 0x0000_0002;
const THREAD_QUERY_LIMITED_INFORMATION: u32 = 0x0000_0800;
const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const QUALIFIED_DIRECTORY_ACCESS: u32 = 0x0012_00a9;
const FILE_SHARE_READ: u32 = 1;
const ERROR_NO_MORE_FILES: i32 = 18;
const MAX_THREAD_SNAPSHOT_ENTRIES: usize = 65_536;
const JOB_POLICY_DOMAIN: &[u8] = b"eliot-search/qdrant-job-policy/v1\0";
const INVALID_HANDLE_VALUE: RawHandle = (-1_isize) as RawHandle;

#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}

#[repr(C, align(8))]
struct BasicLimitInformation {
    _per_process_user_time_limit: i64,
    _per_job_user_time_limit: i64,
    limit_flags: u32,
    _minimum_working_set_size: usize,
    _maximum_working_set_size: usize,
    _active_process_limit: u32,
    _affinity: usize,
    _priority_class: u32,
    _scheduling_class: u32,
}

#[repr(C, align(8))]
struct IoCounters {
    _read_ops: u64,
    _write_ops: u64,
    _other_ops: u64,
    _read_bytes: u64,
    _write_bytes: u64,
    _other_bytes: u64,
}

#[repr(C, align(8))]
struct ExtendedLimitInformation {
    basic_limit_information: BasicLimitInformation,
    io_info: IoCounters,
    _process_memory_limit: usize,
    _job_memory_limit: usize,
    _peak_process_memory_used: usize,
    _peak_job_memory_used: usize,
}

#[repr(C, align(8))]
struct BasicAccountingInformation {
    _total_user_time: i64,
    _total_kernel_time: i64,
    _this_period_total_user_time: i64,
    _this_period_total_kernel_time: i64,
    _total_page_fault_count: u32,
    _total_processes: u32,
    active_processes: u32,
    _total_terminated_processes: u32,
}

#[repr(C)]
struct ThreadEntry32 {
    size: u32,
    _usage: u32,
    thread_id: u32,
    owner_process_id: u32,
    _base_priority: i32,
    _delta_priority: i32,
    _flags: u32,
}

const _: () = {
    assert!(size_of::<BasicLimitInformation>() == if size_of::<usize>() == 8 { 64 } else { 48 });
    assert!(size_of::<IoCounters>() == 48);
    assert!(
        size_of::<ExtendedLimitInformation>() == if size_of::<usize>() == 8 { 144 } else { 112 }
    );
    assert!(offset_of!(ExtendedLimitInformation, io_info) == size_of::<BasicLimitInformation>());
    assert!(
        offset_of!(ExtendedLimitInformation, _process_memory_limit)
            == size_of::<BasicLimitInformation>() + size_of::<IoCounters>()
    );
    assert!(size_of::<BasicAccountingInformation>() == 48);
    assert!(size_of::<ThreadEntry32>() == 28);
};

#[link(name = "Kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> RawHandle;
    fn SetInformationJobObject(
        job: RawHandle,
        information_class: u32,
        information: *const c_void,
        information_length: u32,
    ) -> i32;
    fn QueryInformationJobObject(
        job: RawHandle,
        information_class: u32,
        information: *mut c_void,
        information_length: u32,
        returned_length: *mut u32,
    ) -> i32;
    fn AssignProcessToJobObject(job: RawHandle, process: RawHandle) -> i32;
    fn IsProcessInJob(process: RawHandle, job: RawHandle, result: *mut i32) -> i32;
    fn TerminateJobObject(job: RawHandle, exit_code: u32) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> RawHandle;
    fn Thread32First(snapshot: RawHandle, entry: *mut ThreadEntry32) -> i32;
    fn Thread32Next(snapshot: RawHandle, entry: *mut ThreadEntry32) -> i32;
    fn OpenThread(access: u32, inherit: i32, thread_id: u32) -> RawHandle;
    fn GetProcessIdOfThread(thread: RawHandle) -> u32;
    fn ResumeThread(thread: RawHandle) -> u32;
    fn GetProcessTimes(
        process: RawHandle,
        creation: *mut FileTime,
        exit: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
}

struct JobCustody {
    handle: OwnedHandle,
    directory_pin: QualifiedDirectory,
    directory_identity: DirectoryIdentity,
}

impl ChildContainmentCustody for JobCustody {
    fn terminate(&mut self) -> Result<(), search_qdrant_supervisor::SupervisorError> {
        // SAFETY: `handle` is the retained valid Job Object handle owned here.
        let terminated = unsafe { TerminateJobObject(self.handle.as_raw_handle(), 1) };
        if terminated == 0 {
            return Err(search_qdrant_supervisor::SupervisorError::ShutdownOutcomeUnknown);
        }
        Ok(())
    }

    fn active_processes(&mut self) -> Result<u32, search_qdrant_supervisor::SupervisorError> {
        let mut information: BasicAccountingInformation =
            // SAFETY: every field is an integer and the all-zero value is valid.
            unsafe { zeroed() };
        let length = u32::try_from(size_of::<BasicAccountingInformation>())
            .map_err(|_| search_qdrant_supervisor::SupervisorError::ShutdownOutcomeUnknown)?;
        let mut returned = 0_u32;
        // SAFETY: the retained Job handle is valid and `information`/`returned`
        // are writable for exactly the supplied SDK structure sizes.
        let queried = unsafe {
            QueryInformationJobObject(
                self.handle.as_raw_handle(),
                JOBOBJECT_BASIC_ACCOUNTING_INFORMATION_CLASS,
                (&raw mut information).cast(),
                length,
                &raw mut returned,
            )
        };
        if queried == 0 || returned < length {
            return Err(search_qdrant_supervisor::SupervisorError::ShutdownOutcomeUnknown);
        }
        Ok(information.active_processes)
    }
}

struct WindowsPort<'a, C: StorageSecurityContext> {
    data_directory: &'a File,
    expected_current_user_digest: [u8; 32],
    context: &'a mut C,
}

pub(super) fn spawn_qualified<C: StorageSecurityContext>(
    plan: &LaunchPlan,
    secret: &SecretMaterial,
    data_directory: &File,
    expected_current_user_digest: [u8; 32],
    context: &mut C,
) -> Result<SpawnedChild, ContainedLaunchError<NativeQdrantJobError<C::Error>>> {
    let mut port = WindowsPort {
        data_directory,
        expected_current_user_digest,
        context,
    };
    spawn_qualified_contained(plan, secret, &mut port)
}

impl<C: StorageSecurityContext> ContainedSpawnPort for WindowsPort<'_, C> {
    type Error = NativeQdrantJobError<C::Error>;

    fn absolute_deadline(&self) -> std::time::Instant {
        self.context.absolute_deadline()
    }

    fn admit(&mut self, work: ContainedLaunchWork) -> Result<(), Self::Error> {
        self.context
            .checkpoint()
            .map_err(NativeQdrantJobError::Context)?;
        let charge = match work {
            ContainedLaunchWork::Checkpoint => None,
            ContainedLaunchWork::NativeCall => Some(StorageSecurityWork::NativeCall),
            ContainedLaunchWork::Open => Some(StorageSecurityWork::Open),
            ContainedLaunchWork::Allocate(bytes) => Some(StorageSecurityWork::Allocate(bytes)),
            ContainedLaunchWork::Process(bytes) => Some(StorageSecurityWork::Process(bytes)),
        };
        if let Some(charge) = charge {
            self.context
                .charge(charge)
                .map_err(NativeQdrantJobError::Context)?;
        }
        self.context
            .checkpoint()
            .map_err(NativeQdrantJobError::Context)
    }

    fn spawn_contained(
        &mut self,
        mut command: Command,
        canonical_data_dir: &Path,
    ) -> Result<ContainedSpawn, ContainedSpawnPortFailure<Self::Error>> {
        let (directory_pin, directory_identity) =
            qualify_command_directory(canonical_data_dir, self.data_directory, self.context)
                .map_err(ContainedSpawnPortFailure::before_spawn)?;
        observe_owner_private_acl(
            directory_pin.file(),
            self.expected_current_user_digest,
            self.context,
        )
        .map_err(|error| ContainedSpawnPortFailure::before_spawn(map_acl_error(error)))?;
        let job_digest =
            job_policy_digest(self.context).map_err(ContainedSpawnPortFailure::before_spawn)?;
        let job = create_job(self.context, directory_pin, directory_identity)
            .map_err(ContainedSpawnPortFailure::before_spawn)?;
        command.creation_flags(CREATE_SUSPENDED);

        if let Err(error) = before_native(self.context) {
            return Err(ContainedSpawnPortFailure::before_spawn(error));
        }
        let spawn_result = command.spawn();
        if let Ok(child) = spawn_result {
            if let Err(error) = after_native(self.context) {
                return Err(ContainedSpawnPortFailure::after_spawn(error, child, job));
            }
            self.assign_verify_resume(child, job, canonical_data_dir, job_digest)
        } else {
            let context_error = after_native(self.context).err();
            Err(ContainedSpawnPortFailure::before_spawn(
                context_error.unwrap_or(NativeQdrantJobError::NativeOperationFailed),
            ))
        }
    }
}

impl<C: StorageSecurityContext> WindowsPort<'_, C> {
    fn assign_verify_resume(
        &mut self,
        child: Child,
        job: JobCustody,
        canonical_data_dir: &Path,
        job_digest: Blake3Digest32,
    ) -> Result<ContainedSpawn, ContainedSpawnPortFailure<NativeQdrantJobError<C::Error>>> {
        let pid = child.id();
        let process = child.as_raw_handle();
        let creation_marker = match observe_child_creation(self.context, process) {
            Ok(marker) => marker,
            Err(error) => return Err(ContainedSpawnPortFailure::after_spawn(error, child, job)),
        };
        if let Err(error) = assign_to_job(self.context, job.handle.as_raw_handle(), process)
            .and_then(|()| verify_job_membership(self.context, job.handle.as_raw_handle(), process))
        {
            return Err(ContainedSpawnPortFailure::after_spawn(error, child, job));
        }
        let acl_receipt = match revalidate_command_directory(
            canonical_data_dir,
            self.data_directory,
            &job,
            self.expected_current_user_digest,
            self.context,
        ) {
            Ok(receipt) => receipt,
            Err(error) => return Err(ContainedSpawnPortFailure::after_spawn(error, child, job)),
        };
        let thread = match open_primary_thread(pid, self.context) {
            Ok(thread) => thread,
            Err(error) => return Err(ContainedSpawnPortFailure::after_spawn(error, child, job)),
        };
        if let Err(error) = resume_primary_thread(self.context, thread.as_raw_handle()) {
            return Err(ContainedSpawnPortFailure::after_spawn(error, child, job));
        }

        Ok(ContainedSpawn::new(child, job, job_digest, acl_receipt)
            .with_native_creation_marker(creation_marker))
    }
}

fn observe_child_creation<C: StorageSecurityContext>(
    context: &mut C,
    process: RawHandle,
) -> Result<core::num::NonZeroU64, NativeQdrantJobError<C::Error>> {
    before_native(context)?;
    let (mut creation, mut exit, mut kernel, mut user) = (
        FileTime::default(),
        FileTime::default(),
        FileTime::default(),
        FileTime::default(),
    );
    // SAFETY: the exact owned Child keeps this process handle valid; all four
    // output structures have the Win32 FILETIME layout and live for the call.
    let observed = unsafe {
        GetProcessTimes(process, &raw mut creation, &raw mut exit, &raw mut kernel, &raw mut user)
    };
    after_native(context)?;
    if observed == 0 {
        return Err(NativeQdrantJobError::NativeOperationFailed);
    }
    core::num::NonZeroU64::new((u64::from(creation.high) << 32) | u64::from(creation.low))
        .ok_or(NativeQdrantJobError::NativeOperationFailed)
}

fn assign_to_job<C: StorageSecurityContext>(
    context: &mut C,
    job: RawHandle,
    process: RawHandle,
) -> Result<(), NativeQdrantJobError<C::Error>> {
    before_native(context)?;
    // SAFETY: both handles refer to the exact newly created Job and Child.
    let assigned = unsafe { AssignProcessToJobObject(job, process) };
    after_native(context)?;
    if assigned == 0 {
        return Err(NativeQdrantJobError::NativeOperationFailed);
    }
    Ok(())
}

fn verify_job_membership<C: StorageSecurityContext>(
    context: &mut C,
    job: RawHandle,
    process: RawHandle,
) -> Result<(), NativeQdrantJobError<C::Error>> {
    before_native(context)?;
    let mut in_job = 0_i32;
    // SAFETY: both handles are the exact live child and owned Job; `in_job` is writable.
    let checked = unsafe { IsProcessInJob(process, job, &raw mut in_job) };
    after_native(context)?;
    if checked == 0 || in_job == 0 {
        return Err(NativeQdrantJobError::NativeOperationFailed);
    }
    Ok(())
}

fn open_primary_thread<C: StorageSecurityContext>(
    process_id: u32,
    context: &mut C,
) -> Result<OwnedHandle, NativeQdrantJobError<C::Error>> {
    let thread_id = primary_thread_id(process_id, context)?;
    before_native(context)?;
    // SAFETY: the candidate came from a bounded snapshot of this suspended child.
    let raw_thread = unsafe {
        OpenThread(
            THREAD_SUSPEND_RESUME | THREAD_QUERY_LIMITED_INFORMATION,
            0,
            thread_id,
        )
    };
    let thread = if raw_thread.is_null() {
        None
    } else {
        // SAFETY: successful OpenThread transfers one CloseHandle-compatible handle.
        Some(unsafe { OwnedHandle::from_raw_handle(raw_thread) })
    };
    after_native(context)?;
    let thread = thread.ok_or(NativeQdrantJobError::NativeOperationFailed)?;

    before_native(context)?;
    // SAFETY: `thread` is the live owned handle opened for the selected ID.
    let owner_pid = unsafe { GetProcessIdOfThread(thread.as_raw_handle()) };
    after_native(context)?;
    if owner_pid != process_id {
        return Err(NativeQdrantJobError::PrimaryThreadAmbiguous);
    }
    Ok(thread)
}

fn resume_primary_thread<C: StorageSecurityContext>(
    context: &mut C,
    thread: RawHandle,
) -> Result<(), NativeQdrantJobError<C::Error>> {
    before_native(context)?;
    // SAFETY: only the verified exact primary thread is resumed, once.
    let previous_suspend_count = unsafe { ResumeThread(thread) };
    after_native(context)?;
    if previous_suspend_count == u32::MAX {
        return Err(NativeQdrantJobError::NativeOperationFailed);
    }
    if !original_suspend_count_is_single(previous_suspend_count) {
        return Err(NativeQdrantJobError::PrimaryThreadResumeRefused);
    }
    Ok(())
}

fn qualify_command_directory<C: StorageSecurityContext>(
    path: &Path,
    held_data_directory: &File,
    context: &mut C,
) -> Result<(QualifiedDirectory, DirectoryIdentity), NativeQdrantJobError<C::Error>> {
    let (directory, identity) = open_command_directory(path, context)?;
    let held_observation = observe_native_handle(held_data_directory, context)?;
    compare_directory_identities(identity, held_observation)
        .map_err(|_| NativeQdrantJobError::DataDirectoryMismatch)?;
    Ok((directory, identity))
}

fn revalidate_command_directory<C: StorageSecurityContext>(
    path: &Path,
    held_data_directory: &File,
    custody: &JobCustody,
    expected_user: [u8; 32],
    context: &mut C,
) -> Result<ReceiptRef, NativeQdrantJobError<C::Error>> {
    let held = observe_native_handle(held_data_directory, context)?;
    compare_directory_identities(custody.directory_identity, held)
        .map_err(|_| NativeQdrantJobError::DataDirectoryMismatch)?;

    let pinned = observe_pinned_directory(&custody.directory_pin, context)?;
    if pinned != custody.directory_identity {
        return Err(NativeQdrantJobError::DataDirectoryMismatch);
    }

    let (path_pin, path_identity) = open_command_directory(path, context)?;
    drop(path_pin);
    if path_identity != custody.directory_identity {
        return Err(NativeQdrantJobError::DataDirectoryMismatch);
    }

    let acl = observe_owner_private_acl(custody.directory_pin.file(), expected_user, context)
        .map_err(map_acl_error)?;
    let receipt = acl_receipt(acl.semantic_fingerprint(), context)?;

    let held = observe_native_handle(held_data_directory, context)?;
    compare_directory_identities(custody.directory_identity, held)
        .map_err(|_| NativeQdrantJobError::DataDirectoryMismatch)?;
    let pinned = observe_pinned_directory(&custody.directory_pin, context)?;
    if pinned != custody.directory_identity {
        return Err(NativeQdrantJobError::DataDirectoryMismatch);
    }
    Ok(receipt)
}

fn open_command_directory<C: StorageSecurityContext>(
    path: &Path,
    context: &mut C,
) -> Result<(QualifiedDirectory, DirectoryIdentity), NativeQdrantJobError<C::Error>> {
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    context
        .charge(StorageSecurityWork::Open)
        .map_err(NativeQdrantJobError::Context)?;
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    let opened = OpenOptions::new()
        .access_mode(QUALIFIED_DIRECTORY_ACCESS)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path);
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    let file = opened.map_err(|_| NativeQdrantJobError::NativeOperationFailed)?;
    context
        .charge(StorageSecurityWork::Allocate(
            size_of::<File>() + 2 * size_of::<usize>(),
        ))
        .map_err(NativeQdrantJobError::Context)?;
    let qualified = observe_native_file(context, |check| {
        QualifiedDirectory::qualify_with_check(Arc::new(file), check)
    })?
    .ok_or(NativeQdrantJobError::NativeOperationFailed)?;
    let identity = identity_from_native_observation(qualified.1)
        .map_err(|_| NativeQdrantJobError::DataDirectoryMismatch)?;
    Ok((qualified.0, identity))
}

fn observe_native_handle<C: StorageSecurityContext>(
    file: &File,
    context: &mut C,
) -> Result<DirectoryIdentity, NativeQdrantJobError<C::Error>> {
    let observation = observe_native_file(context, |check| observe_with_check(file, check))?;
    identity_from_native_observation(observation)
        .map_err(|_| NativeQdrantJobError::DataDirectoryMismatch)
}

fn observe_pinned_directory<C: StorageSecurityContext>(
    directory: &QualifiedDirectory,
    context: &mut C,
) -> Result<DirectoryIdentity, NativeQdrantJobError<C::Error>> {
    observe_native_file(context, |check| directory.identity_with_check(check))
}

fn observe_native_file<C, T>(
    context: &mut C,
    operation: impl FnOnce(
        &dyn Fn(bool) -> Result<(), crate::native_file::ObservationError>,
    ) -> Result<T, crate::native_file::ObservationError>,
) -> Result<T, NativeQdrantJobError<C::Error>>
where
    C: StorageSecurityContext,
{
    let first = RefCell::new(None);
    let result = {
        let context_cell = RefCell::new(&mut *context);
        let check = |before| {
            let result = {
                let mut context = context_cell.borrow_mut();
                context.checkpoint().and_then(|()| {
                    if before {
                        context.charge(StorageSecurityWork::NativeCall)
                    } else {
                        Ok(())
                    }
                })
            };
            result.map_err(|error| {
                if first.borrow().is_none() {
                    *first.borrow_mut() = Some(error);
                }
                crate::native_file::ObservationError::ObservationFailed
            })
        };
        operation(&check)
    };
    if let Some(error) = first.into_inner() {
        return Err(NativeQdrantJobError::Context(error));
    }
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    result.map_err(|_| NativeQdrantJobError::NativeOperationFailed)
}

const fn identity_from_native_observation(
    observation: Observation,
) -> Result<DirectoryIdentity, crate::native_qdrant_job::DirectoryBindingError> {
    super::identity_from_observation(observation)
}

fn compare_directory_identities(
    expected: DirectoryIdentity,
    observed: DirectoryIdentity,
) -> Result<(), crate::native_qdrant_job::DirectoryBindingError> {
    super::compare_directory_identities(expected, observed)
}

fn create_job<C: StorageSecurityContext>(
    context: &mut C,
    directory_pin: QualifiedDirectory,
    directory_identity: DirectoryIdentity,
) -> Result<JobCustody, NativeQdrantJobError<C::Error>> {
    before_native(context)?;
    // SAFETY: null security attributes create a private unnamed Job handle;
    // the returned handle is immediately adopted when valid.
    let raw = unsafe { CreateJobObjectW(null(), null()) };
    let job = if raw.is_null() {
        None
    } else {
        // SAFETY: successful CreateJobObjectW transfers one owned kernel handle.
        Some(unsafe { OwnedHandle::from_raw_handle(raw) })
    };
    after_native(context)?;
    let handle = job.ok_or(NativeQdrantJobError::NativeOperationFailed)?;

    let mut limits: ExtendedLimitInformation =
        // SAFETY: every field is an integer and zero is a valid unset limit.
        unsafe { zeroed() };
    limits.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let length = u32::try_from(size_of::<ExtendedLimitInformation>())
        .map_err(|_| NativeQdrantJobError::NativeOperationFailed)?;
    before_native(context)?;
    // SAFETY: `limits` is fully initialized and matches JobObjectExtendedLimitInformation.
    let configured = unsafe {
        SetInformationJobObject(
            handle.as_raw_handle(),
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
            (&raw const limits).cast(),
            length,
        )
    };
    after_native(context)?;
    if configured == 0 {
        return Err(NativeQdrantJobError::NativeOperationFailed);
    }

    let mut observed: ExtendedLimitInformation =
        // SAFETY: integer-only SDK structure; zero initializes all output storage.
        unsafe { zeroed() };
    let mut returned = 0_u32;
    before_native(context)?;
    // SAFETY: the retained Job handle is valid and both outputs have exact bounded storage.
    let queried = unsafe {
        QueryInformationJobObject(
            handle.as_raw_handle(),
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
            (&raw mut observed).cast(),
            length,
            &raw mut returned,
        )
    };
    after_native(context)?;
    if queried == 0
        || returned != length
        || observed.basic_limit_information.limit_flags != JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        || observed.basic_limit_information.limit_flags & JOB_OBJECT_LIMIT_BREAKAWAY_OK != 0
        || observed.basic_limit_information.limit_flags & JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK != 0
    {
        return Err(NativeQdrantJobError::NativeOperationFailed);
    }
    Ok(JobCustody {
        handle,
        directory_pin,
        directory_identity,
    })
}

fn primary_thread_id<C: StorageSecurityContext>(
    process_id: u32,
    context: &mut C,
) -> Result<u32, NativeQdrantJobError<C::Error>> {
    before_native(context)?;
    // SAFETY: snapshot is read-only and scoped to thread metadata; its handle is
    // immediately adopted when valid. PID reuse is excluded by the retained Child.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    let snapshot = if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        None
    } else {
        // SAFETY: successful Toolhelp snapshot transfers one CloseHandle-compatible handle.
        Some(unsafe { OwnedHandle::from_raw_handle(raw) })
    };
    after_native(context)?;
    let snapshot = snapshot.ok_or(NativeQdrantJobError::NativeOperationFailed)?;

    let mut entry: ThreadEntry32 =
        // SAFETY: integer-only THREADENTRY32; size is set before the API writes it.
        unsafe { zeroed() };
    entry.size = u32::try_from(size_of::<ThreadEntry32>())
        .map_err(|_| NativeQdrantJobError::NativeOperationFailed)?;
    before_native(context)?;
    // SAFETY: `entry` is writable for the exact SDK structure size.
    let first = unsafe { Thread32First(snapshot.as_raw_handle(), &raw mut entry) };
    after_native(context)?;
    if first == 0 {
        return Err(NativeQdrantJobError::NativeOperationFailed);
    }

    let mut selected = None;
    let mut entries = 0_usize;
    loop {
        if entries >= MAX_THREAD_SNAPSHOT_ENTRIES {
            return Err(NativeQdrantJobError::NativeOperationFailed);
        }
        context
            .checkpoint()
            .map_err(NativeQdrantJobError::Context)?;
        context
            .charge(StorageSecurityWork::Process(size_of::<ThreadEntry32>()))
            .map_err(NativeQdrantJobError::Context)?;
        context
            .checkpoint()
            .map_err(NativeQdrantJobError::Context)?;
        entries += 1;
        if entry.owner_process_id == process_id {
            record_thread_candidate(&mut selected, entry.thread_id)
                .map_err(|_| NativeQdrantJobError::PrimaryThreadAmbiguous)?;
        }

        before_native(context)?;
        // SAFETY: `entry` remains writable for the same validated SDK extent.
        let next = unsafe { Thread32Next(snapshot.as_raw_handle(), &raw mut entry) };
        let next_error = (next == 0).then(io::Error::last_os_error);
        after_native(context)?;
        if next != 0 {
            continue;
        }
        if next_error.and_then(|error| error.raw_os_error()) != Some(ERROR_NO_MORE_FILES) {
            return Err(NativeQdrantJobError::NativeOperationFailed);
        }
        break;
    }
    selected.ok_or(NativeQdrantJobError::PrimaryThreadAmbiguous)
}

fn before_native<C: StorageSecurityContext>(
    context: &mut C,
) -> Result<(), NativeQdrantJobError<C::Error>> {
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    context
        .charge(StorageSecurityWork::NativeCall)
        .map_err(NativeQdrantJobError::Context)
}

fn after_native<C: StorageSecurityContext>(
    context: &mut C,
) -> Result<(), NativeQdrantJobError<C::Error>> {
    context.checkpoint().map_err(NativeQdrantJobError::Context)
}

fn map_acl_error<E>(error: StorageAclError<E>) -> NativeQdrantJobError<E> {
    match error {
        StorageAclError::Context(error) => NativeQdrantJobError::Context(error),
        _ => NativeQdrantJobError::DataRootAclUnqualified,
    }
}

fn acl_receipt<C: StorageSecurityContext>(
    fingerprint: [u8; 32],
    context: &mut C,
) -> Result<ReceiptRef, NativeQdrantJobError<C::Error>> {
    const PREFIX: &str = "qdrant-held-data-acl-v1:";
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    context
        .charge(StorageSecurityWork::Allocate(96))
        .map_err(NativeQdrantJobError::Context)?;
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    context
        .charge(StorageSecurityWork::Process(PREFIX.len() + 64))
        .map_err(NativeQdrantJobError::Context)?;
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    let mut text = String::with_capacity(96);
    text.push_str(PREFIX);
    for byte in fingerprint {
        write!(&mut text, "{byte:02x}")
            .map_err(|_| NativeQdrantJobError::DataRootAclUnqualified)?;
    }
    let receipt =
        ReceiptRef::new(text).map_err(|_| NativeQdrantJobError::DataRootAclUnqualified)?;
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    Ok(receipt)
}

fn job_policy_digest<C: StorageSecurityContext>(
    context: &mut C,
) -> Result<Blake3Digest32, NativeQdrantJobError<C::Error>> {
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    context
        .charge(StorageSecurityWork::Process(
            JOB_POLICY_DOMAIN.len() + size_of::<u32>(),
        ))
        .map_err(NativeQdrantJobError::Context)?;
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(JOB_POLICY_DOMAIN);
    hasher.update(&JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.to_le_bytes());
    let digest = Blake3Digest32::from_bytes(*hasher.finalize().as_bytes());
    context
        .checkpoint()
        .map_err(NativeQdrantJobError::Context)?;
    Ok(digest)
}
