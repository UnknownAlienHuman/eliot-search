//! Checked Win32 Qdrant Job Object spawn boundary (Windows SDK 10.0.26100).
//!
//! The child is created suspended with stable `CommandExt::creation_flags`,
//! assigned and verified in a non-breakaway `KILL_ON_JOB_CLOSE` Job, then the
//! single exact suspended primary thread is resumed. No PID is opened or adopted.

use core::ffi::c_void;
use std::fmt::Write as _;
use std::fs::File;
use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use std::ptr::null;

use search_contracts::{Blake3Digest32, ReceiptRef};
use search_qdrant_supervisor::{
    ChildContainmentCustody, ContainedLaunchError, ContainedSpawn, ContainedSpawnPort,
    ContainedSpawnPortFailure, LaunchPlan, SecretMaterial, SpawnedChild, spawn_qualified_contained,
};

use crate::native_file::{
    StorageAclError, StorageSecurityContext, StorageSecurityWork, observe_owner_private_acl,
};

use super::{NativeQdrantJobError, original_suspend_count_is_single, record_thread_candidate};

const CREATE_SUSPENDED: u32 = 0x0000_0004;
const JOBOBJECT_BASIC_LIMIT_INFORMATION_CLASS: u32 = 2;
const JOBOBJECT_BASIC_ACCOUNTING_INFORMATION_CLASS: u32 = 1;
const JOB_OBJECT_LIMIT_BREAKAWAY_OK: u32 = 0x0000_0800;
const JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK: u32 = 0x0000_1000;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
const THREAD_SUSPEND_RESUME: u32 = 0x0000_0002;
const THREAD_QUERY_LIMITED_INFORMATION: u32 = 0x0000_0800;
const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
const ERROR_NO_MORE_FILES: i32 = 18;
const MAX_THREAD_SNAPSHOT_ENTRIES: usize = 65_536;
const JOB_POLICY_DOMAIN: &[u8] = b"eliot-search/qdrant-job-policy/v1\0";
const INVALID_HANDLE_VALUE: RawHandle = (-1_isize) as RawHandle;

#[repr(C)]
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
}

struct JobCustody {
    handle: OwnedHandle,
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

    fn spawn_contained(
        &mut self,
        mut command: Command,
    ) -> Result<ContainedSpawn, ContainedSpawnPortFailure<Self::Error>> {
        let acl = observe_owner_private_acl(
            self.data_directory,
            self.expected_current_user_digest,
            self.context,
        )
        .map_err(|error| ContainedSpawnPortFailure::before_spawn(map_acl_error(error)))?;
        let acl_receipt = acl_receipt(acl.semantic_fingerprint(), self.context)
            .map_err(ContainedSpawnPortFailure::before_spawn)?;
        let job_digest =
            job_policy_digest(self.context).map_err(ContainedSpawnPortFailure::before_spawn)?;
        let job = create_job(self.context).map_err(ContainedSpawnPortFailure::before_spawn)?;
        command.creation_flags(CREATE_SUSPENDED);

        if let Err(error) = before_native(self.context) {
            return Err(ContainedSpawnPortFailure::before_spawn(error));
        }
        let spawn_result = command.spawn();
        if let Ok(child) = spawn_result {
            if let Err(error) = after_native(self.context) {
                return Err(ContainedSpawnPortFailure::after_spawn(error, child, job));
            }
            self.assign_verify_resume(child, job, acl_receipt, job_digest)
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
        acl_receipt: ReceiptRef,
        job_digest: Blake3Digest32,
    ) -> Result<ContainedSpawn, ContainedSpawnPortFailure<NativeQdrantJobError<C::Error>>> {
        let pid = child.id();
        let process = child.as_raw_handle();
        if let Err(error) = assign_to_job(self.context, job.handle.as_raw_handle(), process)
            .and_then(|()| verify_job_membership(self.context, job.handle.as_raw_handle(), process))
        {
            return Err(ContainedSpawnPortFailure::after_spawn(error, child, job));
        }
        let thread = match open_primary_thread(pid, self.context) {
            Ok(thread) => thread,
            Err(error) => return Err(ContainedSpawnPortFailure::after_spawn(error, child, job)),
        };
        if let Err(error) = resume_primary_thread(self.context, thread.as_raw_handle()) {
            return Err(ContainedSpawnPortFailure::after_spawn(error, child, job));
        }

        Ok(ContainedSpawn::new(child, job, job_digest, acl_receipt))
    }
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

fn create_job<C: StorageSecurityContext>(
    context: &mut C,
) -> Result<JobCustody, NativeQdrantJobError<C::Error>> {
    before_native(context)?;
    // SAFETY: null security attributes create a private unnamed Job handle;
    // the returned handle is immediately adopted when valid.
    let raw = unsafe { CreateJobObjectW(null(), null()) };
    let job = if raw.is_null() {
        None
    } else {
        // SAFETY: successful CreateJobObjectW transfers one owned kernel handle.
        Some(JobCustody {
            handle: unsafe { OwnedHandle::from_raw_handle(raw) },
        })
    };
    after_native(context)?;
    let job = job.ok_or(NativeQdrantJobError::NativeOperationFailed)?;

    let mut limits: BasicLimitInformation =
        // SAFETY: every field is an integer and zero is a valid unset limit.
        unsafe { zeroed() };
    limits.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let length = u32::try_from(size_of::<BasicLimitInformation>())
        .map_err(|_| NativeQdrantJobError::NativeOperationFailed)?;
    before_native(context)?;
    // SAFETY: `limits` is fully initialized and matches JobObjectBasicLimitInformation.
    let configured = unsafe {
        SetInformationJobObject(
            job.handle.as_raw_handle(),
            JOBOBJECT_BASIC_LIMIT_INFORMATION_CLASS,
            (&raw const limits).cast(),
            length,
        )
    };
    after_native(context)?;
    if configured == 0 {
        return Err(NativeQdrantJobError::NativeOperationFailed);
    }

    let mut observed: BasicLimitInformation =
        // SAFETY: integer-only SDK structure; zero initializes all output storage.
        unsafe { zeroed() };
    let mut returned = 0_u32;
    before_native(context)?;
    // SAFETY: the retained Job handle is valid and both outputs have exact bounded storage.
    let queried = unsafe {
        QueryInformationJobObject(
            job.handle.as_raw_handle(),
            JOBOBJECT_BASIC_LIMIT_INFORMATION_CLASS,
            (&raw mut observed).cast(),
            length,
            &raw mut returned,
        )
    };
    after_native(context)?;
    if queried == 0
        || returned < length
        || observed.limit_flags != JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        || observed.limit_flags & JOB_OBJECT_LIMIT_BREAKAWAY_OK != 0
        || observed.limit_flags & JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK != 0
    {
        return Err(NativeQdrantJobError::NativeOperationFailed);
    }
    Ok(job)
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
