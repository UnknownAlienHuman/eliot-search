//! Private native Windows process, Job Object, and file-security adapter.
//!
//! All unsafe operations are isolated here. Public types never contain
//! handles, environment blocks, or secret bytes.

#[cfg(windows)]
use crate::SupervisorError;

#[cfg(windows)]
mod windows_impl {

    use super::NativeSpawnError;

    use core::mem::{size_of, zeroed};
    use core::ptr::{null, null_mut};
    use std::collections::BTreeMap;
    use std::ffi::{OsStr, OsString};
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::num::{NonZeroU32, NonZeroU64};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::path::{Path, PathBuf};
    use std::ptr::NonNull;
    use std::time::{Duration, Instant};

    use search_contracts::{ArtifactDigest, Blake3Digest32, Sha256Digest32};
    use windows_sys::Win32::Foundation::{
        CloseHandle, FILETIME, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
        LocalFree, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SE_FILE_OBJECT,
        SE_KERNEL_OBJECT, SE_OBJECT_TYPE, SetSecurityInfo,
    };
    use windows_sys::Win32::Security::{
        ACL, ACL_SIZE_INFORMATION, AclSizeInformation, DACL_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES,
        SECURITY_DESCRIPTOR_CONTROL,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
        GetFinalPathNameByHandleW, OPEN_ALWAYS, OPEN_EXISTING, READ_CONTROL, WRITE_DAC,
    };
    use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
    use windows_sys::Win32::System::JobObjects::{
        CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
        QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{
        CREATE_NEW_PROCESS_GROUP, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
        DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess,
        GetProcessId, GetProcessTimes, InitializeProcThreadAttributeList,
        PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST, PROCESS_INFORMATION,
        QueryFullProcessImageNameW, STARTF_USESTDHANDLES, STARTUPINFOEXW,
        UpdateProcThreadAttribute, WaitForSingleObject,
    };

    use crate::launch::MAX_DATA_ROOT_PATH_UTF16_UNITS;
    use crate::secret::SecretMaterial;
    use crate::{
        LoopbackHost, ProcessIdentity, QdrantEndpointIdentity, QdrantOwnerFence, ShutdownReceipt,
        StartRecoveryReceipt, SupervisorError,
    };

    const SECURITY_DESCRIPTOR_REVISION: u32 = 1;
    const MAX_DIRECTORY_ENTRIES: usize = 100_000;
    const MAX_DIRECTORY_DEPTH: usize = 64;
    const MAX_WIN32_PATH_UNITS: usize = 32_768;
    const EXTENDED_PATH_PREFIX_UNITS: usize = 4;
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const SAFE_DIRECTORY_SDDL: &str = "D:P(A;OICI;FA;;;OW)(A;OICI;FA;;;SY)";
    const SAFE_FILE_SDDL: &str = "D:P(A;;FA;;;OW)(A;;FA;;;SY)";
    const SAFE_KERNEL_OBJECT_SDDL: &str = "D:P(A;;GA;;;OW)(A;;GA;;;SY)";
    const CHILD_API_KEY_NAME: &str = "QDRANT__SERVICE__API_KEY";

    /// Native process plus the retained resources that establish ownership.
    pub(crate) struct NativeProcess {
        process: Option<OwnedHandle>,
        job: OwnedHandle,
        _exe_file: File,
        _data_root: Vec<OwnedHandle>,
        config_file: File,
        config_path: PathBuf,
        config_bytes: Vec<u8>,
        pid: Option<NonZeroU32>,
        exe_sha256: Sha256Digest32,
        exe_bytes: u64,
        artifact_digest: ArtifactDigest,
        config_digest: Blake3Digest32,
        owner_fence: QdrantOwnerFence,
        endpoint: QdrantEndpointIdentity,
        expected_secret_purpose: Blake3Digest32,
        operation_id: search_contracts::OpaqueId,
        host: LoopbackHost,
        http_port: u16,
        grpc_port: u16,
        secret: SecretMaterial,
        process_group_id: u32,
    }

    impl NativeProcess {
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn spawn(
            executable_path: &Path,
            expected_sha256: Sha256Digest32,
            expected_bytes: u64,
            expected_version: &str,
            operation_id: search_contracts::OpaqueId,
            artifact_digest: ArtifactDigest,
            owner_fence: QdrantOwnerFence,
            endpoint: QdrantEndpointIdentity,
            config_digest: Blake3Digest32,
            expected_secret_purpose: Blake3Digest32,
            data_dir: &Path,
            config_path: &Path,
            config_bytes: &[u8],
            secret: SecretMaterial,
        ) -> Result<Self, NativeSpawnError> {
            let host = endpoint.host();
            let http_port = endpoint.http_port().get();
            let grpc_port = endpoint.grpc_port().get();
            let mut exe_file = open_pinned_executable(executable_path)?;
            let observed_bytes = file_size(&exe_file)?;
            if observed_bytes != expected_bytes {
                return Err(NativeSpawnError::Definite(
                    SupervisorError::ArtifactDigestMismatch,
                ));
            }
            let observed_sha = hash_open_file(&mut exe_file)?;
            if observed_sha != expected_sha256 {
                return Err(NativeSpawnError::Definite(
                    SupervisorError::ArtifactDigestMismatch,
                ));
            }
            if expected_version.is_empty() || expected_version.len() > 128 {
                return Err(NativeSpawnError::Definite(
                    SupervisorError::ArtifactVersionMismatch,
                ));
            }

            let (mut retained_dirs, canonical_data_dir) = secure_data_tree(data_dir)?;
            let canonical_config_path = canonical_data_dir.join(
                config_path
                    .file_name()
                    .ok_or(SupervisorError::InvalidProcessConfig)?,
            );
            if config_path.file_name() != Some(OsStr::new("config.yaml")) {
                return Err(NativeSpawnError::Definite(
                    SupervisorError::DataRootMismatch,
                ));
            }
            let config_file = write_secure_config(&canonical_config_path, config_bytes)?;

            let temp_dir = canonical_data_dir.join("tmp");
            fs::create_dir_all(&temp_dir).map_err(|_| SupervisorError::ContainmentUnavailable)?;
            let temp_handles = secure_data_tree(&temp_dir)?;
            if !is_beneath(&canonical_data_dir, &temp_handles.1) {
                return Err(NativeSpawnError::Definite(
                    SupervisorError::DataRootMismatch,
                ));
            }
            retained_dirs.extend(temp_handles.0);

            let exe_final_path = final_path(exe_file.as_raw_handle() as HANDLE)?;
            let config_final_path = final_path(config_file.as_raw_handle() as HANDLE)?;
            let current_dir = wide_path(&canonical_data_dir)?;
            let application_name = wide_path(&exe_final_path)?;
            let mut command_line = make_command_line(&exe_final_path, &config_final_path)?;
            let mut environment = child_environment_block(&temp_dir, secret.secret_bytes())?;
            let retained_config_bytes = config_bytes.to_vec();
            let retained_operation_id = operation_id.clone();
            let retained_config_path = canonical_config_path.clone();

            let object_security = PrivateSecurityDescriptor::new(SAFE_KERNEL_OBJECT_SDDL)?;
            let job = create_kill_on_close_job(&object_security)?;
            let null_handle = open_inheritable_nul()?;
            let mut attribute_list = AttributeList::new(2)?;
            let job_handles = [job.raw()];
            let inherited_handles = [null_handle.raw()];
            attribute_list.update(
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                job_handles.as_ptr().cast(),
                size_of::<[HANDLE; 1]>(),
            )?;
            attribute_list.update(
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                inherited_handles.as_ptr().cast(),
                size_of::<[HANDLE; 1]>(),
            )?;

            // SAFETY: `startup` is initialized, its attribute list and referenced
            // job/handle arrays outlive CreateProcessW, and command/environment
            // buffers are NUL-terminated and writable for the duration of the call.
            let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
            startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
            startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            startup.StartupInfo.hStdInput = null_handle.raw();
            startup.StartupInfo.hStdOutput = null_handle.raw();
            startup.StartupInfo.hStdError = null_handle.raw();
            startup.lpAttributeList = attribute_list.as_ptr();
            let mut information: PROCESS_INFORMATION = unsafe { zeroed() };
            let process_attributes = object_security.attributes();
            let thread_attributes = object_security.attributes();
            let creation_flags = CREATE_UNICODE_ENVIRONMENT
                | EXTENDED_STARTUPINFO_PRESENT
                | CREATE_NEW_PROCESS_GROUP;
            // SAFETY: all pointer arguments reference live, correctly terminated
            // UTF-16 buffers or initialized Win32 structs. JOB_LIST makes the
            // process a member before its first instruction; HANDLE_LIST limits
            // inheritance to the NUL standard stream handle.
            let created = unsafe {
                CreateProcessW(
                    application_name.as_ptr(),
                    command_line.as_mut_ptr(),
                    &process_attributes,
                    &thread_attributes,
                    1,
                    creation_flags,
                    environment.as_ptr().cast(),
                    current_dir.as_ptr(),
                    &startup.StartupInfo,
                    &mut information,
                )
            };
            command_line.fill(0);
            drop(environment);
            if created == 0 {
                let guard = Self {
                    process: None,
                    job,
                    _exe_file: exe_file,
                    _data_root: retained_dirs,
                    config_file,
                    config_path: retained_config_path,
                    config_bytes: retained_config_bytes,
                    pid: None,
                    exe_sha256: expected_sha256,
                    exe_bytes: expected_bytes,
                    artifact_digest,
                    config_digest,
                    owner_fence,
                    endpoint,
                    expected_secret_purpose,
                    operation_id: retained_operation_id,
                    host,
                    http_port,
                    grpc_port,
                    secret,
                    process_group_id: 0,
                };
                match guard.job_accounting() {
                    Ok(accounting) if accounting.TotalProcesses == 0 => {
                        return Err(NativeSpawnError::Definite(SupervisorError::StartFailed));
                    }
                    _ => {
                        return Err(NativeSpawnError::Unknown {
                            reason: SupervisorError::StartFailed,
                            guard,
                            containment: crate::ContainmentReport::unverified(),
                        });
                    }
                }
            }
            // SAFETY: CreateProcessW returned success, which guarantees both
            // PROCESS_INFORMATION handles are valid, non-null owned handles.
            let process = unsafe { OwnedHandle::from_success(information.hProcess) };
            // SAFETY: same successful CreateProcessW contract as above.
            let thread = unsafe { OwnedHandle::from_success(information.hThread) };
            drop(null_handle);
            // SAFETY: the CreateProcessW contract returns a nonzero process ID on
            // success; this branch is reached only after its success result.
            let pid = unsafe { NonZeroU32::new_unchecked(information.dwProcessId) };
            let process_acl =
                verify_private_acl(process.raw(), object_security.dacl(), SE_KERNEL_OBJECT);
            let thread_acl =
                verify_private_acl(thread.raw(), object_security.dacl(), SE_KERNEL_OBJECT);
            let child = Self {
                process: Some(process),
                job,
                _exe_file: exe_file,
                _data_root: retained_dirs,
                config_file,
                config_path: canonical_config_path,
                config_bytes: retained_config_bytes,
                pid: Some(pid),
                exe_sha256: expected_sha256,
                exe_bytes: expected_bytes,
                artifact_digest,
                config_digest,
                owner_fence,
                endpoint,
                expected_secret_purpose,
                operation_id,
                host,
                http_port,
                grpc_port,
                secret,
                process_group_id: information.dwProcessId,
            };
            drop(thread);
            if process_acl.is_err() || thread_acl.is_err() {
                return Err(NativeSpawnError::Unknown {
                    reason: SupervisorError::ContainmentUnavailable,
                    guard: child,
                    containment: crate::ContainmentReport::unverified(),
                });
            }
            // If verification fails, return the process-owning guard anyway.
            // The caller can retain it and perform bounded cleanup; no successful
            // readiness or shutdown receipt is synthesized here.
            Ok(child)
        }

        pub(crate) const fn config_digest(&self) -> Blake3Digest32 {
            self.config_digest
        }

        pub(crate) const fn expected_secret_purpose(&self) -> Blake3Digest32 {
            self.expected_secret_purpose
        }

        pub(crate) const fn has_process_handle(&self) -> bool {
            self.process.is_some()
        }

        pub(crate) fn identity(&mut self) -> Result<ProcessIdentity, SupervisorError> {
            let pid = self.pid.ok_or(SupervisorError::StartupOutcomeUnknown)?;
            let created_at = {
                // SAFETY: process is a live owned process handle returned by
                // CreateProcessW and remains open for this call.
                let process = self
                    .process
                    .as_ref()
                    .ok_or(SupervisorError::StartupOutcomeUnknown)?;
                if unsafe { GetProcessId(process.raw()) } != pid.get() {
                    return Err(SupervisorError::ProcessIdentityMismatch);
                }
                let mut created = FILETIME::default();
                let mut exited = FILETIME::default();
                let mut kernel = FILETIME::default();
                let mut user = FILETIME::default();
                // SAFETY: each FILETIME pointer is writable and the process handle
                // is valid for query access until the guard is dropped.
                if unsafe {
                    GetProcessTimes(
                        process.raw(),
                        &mut created,
                        &mut exited,
                        &mut kernel,
                        &mut user,
                    )
                } == 0
                {
                    return Err(SupervisorError::ProcessIdentityMismatch);
                }
                let created_at =
                    (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
                NonZeroU64::new(created_at).ok_or(SupervisorError::ProcessIdentityMismatch)?
            };
            self.verify_image_identity()?;
            let process = self
                .process
                .as_ref()
                .ok_or(SupervisorError::StartupOutcomeUnknown)?;
            let mut in_job = 0;
            // SAFETY: both handles are valid, owned handles and `in_job` is a
            // writable BOOL output.
            if unsafe { IsProcessInJob(process.raw(), self.job.raw(), &mut in_job) } == 0
                || in_job == 0
            {
                return Err(SupervisorError::ContainmentUnavailable);
            }
            Ok(ProcessIdentity::from_platform(
                pid,
                created_at,
                self.exe_sha256,
                self.artifact_digest,
                self.owner_fence,
                self.endpoint,
                self.secret.binding(),
            ))
        }

        fn verify_image_identity(&mut self) -> Result<(), SupervisorError> {
            let mut buffer = vec![0_u16; MAX_WIN32_PATH_UNITS];
            let mut length = u32::try_from(buffer.len())
                .map_err(|_| SupervisorError::ExecutableIdentityMismatch)?;
            // SAFETY: process is a valid owned process handle and the UTF-16 buffer
            // is writable with its initialized capacity expressed by `length`.
            let process = self
                .process
                .as_ref()
                .ok_or(SupervisorError::StartupOutcomeUnknown)?;
            if unsafe {
                QueryFullProcessImageNameW(process.raw(), 0, buffer.as_mut_ptr(), &mut length)
            } == 0
            {
                return Err(SupervisorError::ExecutableIdentityMismatch);
            }
            buffer.truncate(length as usize);
            let process_image = PathBuf::from(OsString::from_wide(&buffer));
            let process_file = open_pinned_executable(&process_image)?;
            let process_info = file_info(process_file.as_raw_handle() as HANDLE)?;
            let pinned_info = file_info(self._exe_file.as_raw_handle() as HANDLE)?;
            if process_info.dwVolumeSerialNumber != pinned_info.dwVolumeSerialNumber
                || process_info.nFileIndexHigh != pinned_info.nFileIndexHigh
                || process_info.nFileIndexLow != pinned_info.nFileIndexLow
            {
                return Err(SupervisorError::ExecutableIdentityMismatch);
            }
            let observed_size = file_size(&process_file)?;
            let mut process_file = process_file;
            if observed_size != self.exe_bytes
                || hash_open_file(&mut process_file)? != self.exe_sha256
            {
                return Err(SupervisorError::ArtifactDigestMismatch);
            }
            let mut config = Vec::with_capacity(self.config_bytes.len());
            (&mut self.config_file)
                .seek(SeekFrom::Start(0))
                .and_then(|_| (&mut self.config_file).read_to_end(&mut config))
                .map_err(|_| SupervisorError::InvalidProcessConfig)?;
            if config != self.config_bytes {
                return Err(SupervisorError::InvalidProcessConfig);
            }
            Ok(())
        }

        pub(crate) fn is_running(&self) -> Result<bool, SupervisorError> {
            let process = self
                .process
                .as_ref()
                .ok_or(SupervisorError::StartupOutcomeUnknown)?;
            // SAFETY: the process handle is valid for the full function call.
            match unsafe { WaitForSingleObject(process.raw(), 0) } {
                WAIT_TIMEOUT => Ok(true),
                WAIT_OBJECT_0 => Ok(false),
                WAIT_FAILED => Err(SupervisorError::ProcessIdentityMismatch),
                _ => Err(SupervisorError::ProcessIdentityMismatch),
            }
        }

        pub(crate) fn try_exit_code(&self) -> Result<Option<i32>, SupervisorError> {
            if self.is_running()? {
                return Ok(None);
            }
            let process = self
                .process
                .as_ref()
                .ok_or(SupervisorError::StartupOutcomeUnknown)?;
            let mut code = 0_u32;
            // SAFETY: the process is signaled and its owned handle remains valid;
            // `code` is a writable output.
            if unsafe { GetExitCodeProcess(process.raw(), &mut code) } == 0 {
                return Err(SupervisorError::ProcessIdentityMismatch);
            }
            Ok(Some(code as i32))
        }

        pub(crate) fn job_empty(&self) -> Result<bool, SupervisorError> {
            Ok(self.job_accounting()?.ActiveProcesses == 0)
        }

        fn job_accounting(
            &self,
        ) -> Result<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, SupervisorError> {
            let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            // SAFETY: `accounting` is writable and exactly sized for the selected
            // information class; the job handle remains open.
            if unsafe {
                QueryInformationJobObject(
                    self.job.raw(),
                    JobObjectBasicAccountingInformation,
                    (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    null_mut(),
                )
            } == 0
            {
                return Err(SupervisorError::ContainmentUnavailable);
            }
            Ok(accounting)
        }

        pub(crate) fn shutdown_tree(
            &mut self,
            expected_identity: ProcessIdentity,
            deadline: Instant,
        ) -> Result<ShutdownReceipt, SupervisorError> {
            if self.process.is_some() {
                match self.identity() {
                    Ok(current) if current == expected_identity => {}
                    Ok(_) => return Err(SupervisorError::ProcessIdentityMismatch),
                    Err(_)
                        if !self.is_running()?
                            && self.basic_identity_matches(expected_identity)? => {}
                    Err(_) => return Err(SupervisorError::ProcessIdentityMismatch),
                }
            } else {
                return Err(SupervisorError::StartupOutcomeUnknown);
            }
            let graceful_signal_sent =
            // SAFETY: the process group ID came from this CreateProcessW call;
            // a false result is retained as an honest forced-shutdown path.
            unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, self.process_group_id) } != 0;
            let graceful_deadline = (Instant::now() + Duration::from_secs(5)).min(deadline);
            let mut forced = false;
            while Instant::now() < graceful_deadline {
                if self.job_empty()? && !self.is_running()? && self.endpoints_absent() {
                    return Ok(ShutdownReceipt::from_process_handle(
                        expected_identity,
                        false,
                        graceful_signal_sent,
                    ));
                }
                std::thread::sleep(
                    POLL_INTERVAL.min(graceful_deadline.saturating_duration_since(Instant::now())),
                );
            }
            if !self.job_empty()? {
                // SAFETY: the job handle was created privately for this exact
                // process tree and remains open; no external job can be affected.
                if unsafe { TerminateJobObject(self.job.raw(), 1) } == 0 {
                    return Err(SupervisorError::ShutdownOutcomeUnknown);
                }
                forced = true;
            }
            loop {
                if self.job_empty()? && !self.is_running()? && self.endpoints_absent() {
                    return Ok(ShutdownReceipt::from_process_handle(
                        expected_identity,
                        forced,
                        graceful_signal_sent,
                    ));
                }
                if Instant::now() >= deadline {
                    return Err(SupervisorError::ShutdownOutcomeUnknown);
                }
                std::thread::sleep(
                    POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                );
            }
        }

        /// Kills and reads back an owned Job tree when OS identity verification
        /// failed. Cleanup can be proven, but this path deliberately emits no
        /// shutdown receipt because the root identity could not be established.
        pub(crate) fn terminate_unverified_tree(
            &mut self,
            deadline: Instant,
        ) -> Result<(), SupervisorError> {
            if self.process.is_none() {
                return Err(SupervisorError::StartupOutcomeUnknown);
            }
            if self.job_accounting()?.ActiveProcesses != 0 {
                // SAFETY: this private Job Object was created for this launch and
                // contains only descendants admitted with JOB_LIST assignment.
                if unsafe { TerminateJobObject(self.job.raw(), 1) } == 0 {
                    return Err(SupervisorError::ShutdownOutcomeUnknown);
                }
            }
            loop {
                if self.job_accounting()?.ActiveProcesses == 0
                    && !self.is_running()?
                    && self.endpoints_absent()
                {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(SupervisorError::ShutdownOutcomeUnknown);
                }
                std::thread::sleep(
                    POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                );
            }
        }

        fn basic_identity_matches(
            &self,
            expected: ProcessIdentity,
        ) -> Result<bool, SupervisorError> {
            let process = self
                .process
                .as_ref()
                .ok_or(SupervisorError::StartupOutcomeUnknown)?;
            let Some(pid) = self.pid else {
                return Ok(false);
            };
            if pid != expected.process_id()
            // SAFETY: this is the same retained process handle returned by
            // CreateProcessW and expected contains its verified PID.
            || unsafe { GetProcessId(process.raw()) } != expected.process_id().get()
            {
                return Ok(false);
            }
            let mut created = FILETIME::default();
            let mut exited = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            // SAFETY: process handle is valid; each FILETIME output is writable.
            if unsafe {
                GetProcessTimes(
                    process.raw(),
                    &mut created,
                    &mut exited,
                    &mut kernel,
                    &mut user,
                )
            } == 0
            {
                return Err(SupervisorError::ProcessIdentityMismatch);
            }
            let created_at =
                (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
            Ok(NonZeroU64::new(created_at) == Some(expected.creation_marker()))
        }

        pub(crate) fn verify_root_handle(
            &self,
            expected: ProcessIdentity,
        ) -> Result<(), SupervisorError> {
            if self.basic_identity_matches(expected)? {
                Ok(())
            } else {
                Err(SupervisorError::ProcessIdentityMismatch)
            }
        }

        pub(crate) fn recover_ambiguous_start(
            &mut self,
            deadline: Instant,
        ) -> Result<StartRecoveryReceipt, SupervisorError> {
            if self.process.is_some() {
                return Err(SupervisorError::InvalidLifecycleTransition);
            }
            let mut forced = false;
            let before = self.job_accounting()?;
            if before.ActiveProcesses != 0 {
                // SAFETY: this private unnamed job was created for this launch;
                // the handle cannot identify or affect any foreign process.
                if unsafe { TerminateJobObject(self.job.raw(), 1) } == 0 {
                    return Err(SupervisorError::StartupOutcomeUnknown);
                }
                forced = true;
            }
            loop {
                let accounting = self.job_accounting()?;
                if accounting.ActiveProcesses == 0 && self.endpoints_absent() {
                    return Ok(StartRecoveryReceipt::from_empty_job(
                        self.operation_id.clone(),
                        accounting.TotalProcesses != 0,
                        forced,
                    ));
                }
                if Instant::now() >= deadline {
                    return Err(SupervisorError::StartupOutcomeUnknown);
                }
                std::thread::sleep(
                    POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                );
            }
        }

        fn endpoints_absent(&self) -> bool {
            let http = socket_for(self.host, self.http_port);
            let grpc = socket_for(self.host, self.grpc_port);
            endpoint_absent(http) && endpoint_absent(grpc)
        }

        pub(crate) fn secret(&self) -> &SecretMaterial {
            &self.secret
        }

        pub(crate) fn secret_appears_in(&self, value: &str) -> bool {
            self.secret.appears_in(value)
        }

        pub(crate) fn config_path(&self) -> &Path {
            &self.config_path
        }
    }

    fn endpoint_absent(endpoint: SocketAddr) -> bool {
        matches!(
            TcpStream::connect_timeout(&endpoint, Duration::from_millis(300)),
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused
        )
    }

    fn socket_for(host: LoopbackHost, port: u16) -> SocketAddr {
        match host {
            LoopbackHost::V4 => SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)),
            LoopbackHost::V6 => SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port)),
        }
    }

    struct OwnedHandle(NonNull<core::ffi::c_void>);

    impl OwnedHandle {
        fn new(raw: HANDLE) -> Result<Self, SupervisorError> {
            if raw.is_null() || raw == INVALID_HANDLE_VALUE {
                return Err(SupervisorError::ContainmentUnavailable);
            }
            NonNull::new(raw)
                .map(Self)
                .ok_or(SupervisorError::ContainmentUnavailable)
        }

        const fn raw(&self) -> HANDLE {
            self.0.as_ptr()
        }

        /// Wraps a handle that Win32 guarantees valid after a successful call.
        ///
        /// # Safety
        /// `raw` must be a unique, valid handle owned by the caller.
        unsafe fn from_success(raw: HANDLE) -> Self {
            // SAFETY: upheld by this function's caller contract.
            Self(unsafe { NonNull::new_unchecked(raw) })
        }
    }

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: this wrapper uniquely owns the handle returned by Win32;
            // Drop runs exactly once before the pointer is discarded.
            unsafe { CloseHandle(self.raw()) };
        }
    }

    struct AttributeList {
        storage: Vec<usize>,
        initialized: bool,
    }

    impl AttributeList {
        fn new(attribute_count: u32) -> Result<Self, SupervisorError> {
            let mut bytes = 0_usize;
            // SAFETY: the first call is the documented size query; null is the
            // required probe pointer and `bytes` is writable.
            let first = unsafe {
                InitializeProcThreadAttributeList(null_mut(), attribute_count, 0, &mut bytes)
            };
            if first != 0 || bytes == 0 {
                return Err(SupervisorError::ContainmentUnavailable);
            }
            let words = bytes.saturating_add(size_of::<usize>() - 1) / size_of::<usize>();
            let mut storage = vec![0_usize; words];
            // SAFETY: Vec<usize> provides the alignment and at least the byte
            // length requested by the preceding size query.
            if unsafe {
                InitializeProcThreadAttributeList(
                    storage.as_mut_ptr().cast(),
                    attribute_count,
                    0,
                    &mut bytes,
                )
            } == 0
            {
                return Err(SupervisorError::ContainmentUnavailable);
            }
            Ok(Self {
                storage,
                initialized: true,
            })
        }

        fn as_ptr(&mut self) -> *mut core::ffi::c_void {
            self.storage.as_mut_ptr().cast()
        }

        fn update(
            &mut self,
            attribute: usize,
            value: *const core::ffi::c_void,
            value_size: usize,
        ) -> Result<(), SupervisorError> {
            // SAFETY: the list has been initialized, and caller keeps `value`
            // alive until CreateProcessW returns; the API copies attribute data.
            if unsafe {
                UpdateProcThreadAttribute(
                    self.as_ptr(),
                    0,
                    attribute,
                    value,
                    value_size,
                    null_mut(),
                    null(),
                )
            } == 0
            {
                return Err(SupervisorError::ContainmentUnavailable);
            }
            Ok(())
        }
    }

    impl Drop for AttributeList {
        fn drop(&mut self) {
            if self.initialized {
                // SAFETY: the attribute list was successfully initialized in new
                // and its backing storage is still alive here.
                unsafe { DeleteProcThreadAttributeList(self.storage.as_mut_ptr().cast()) };
            }
        }
    }

    struct PrivateSecurityDescriptor {
        descriptor: *mut core::ffi::c_void,
        dacl: *mut ACL,
    }

    impl PrivateSecurityDescriptor {
        fn new(sddl: &str) -> Result<Self, SupervisorError> {
            let sddl_wide = wide_path(Path::new(sddl))?;
            let mut descriptor = null_mut();
            // SAFETY: SDDL is NUL-terminated for this call and descriptor is a
            // writable output pointer; successful output is freed by Drop.
            if unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl_wide.as_ptr(),
                    SECURITY_DESCRIPTOR_REVISION,
                    &mut descriptor,
                    null_mut(),
                )
            } == 0
            {
                return Err(SupervisorError::ContainmentUnavailable);
            }
            let mut dacl = null_mut();
            let mut present = 0;
            let mut defaulted = 0;
            // SAFETY: descriptor came from the SDDL conversion and remains live;
            // output pointers are writable for the documented values.
            if unsafe {
                windows_sys::Win32::Security::GetSecurityDescriptorDacl(
                    descriptor,
                    &mut present,
                    &mut dacl,
                    &mut defaulted,
                )
            } == 0
                || present == 0
                || dacl.is_null()
            {
                // SAFETY: descriptor is the LocalAlloc result from above.
                unsafe { LocalFree(descriptor) };
                return Err(SupervisorError::ContainmentUnavailable);
            }
            Ok(Self { descriptor, dacl })
        }

        fn attributes(&self) -> SECURITY_ATTRIBUTES {
            SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: self.descriptor,
                bInheritHandle: 0,
            }
        }

        const fn dacl(&self) -> *mut ACL {
            self.dacl
        }
    }

    impl Drop for PrivateSecurityDescriptor {
        fn drop(&mut self) {
            // SAFETY: this unique LocalAlloc descriptor is freed once after all
            // SECURITY_ATTRIBUTES and DACL references have left scope.
            unsafe { LocalFree(self.descriptor) };
        }
    }

    fn create_kill_on_close_job(
        security: &PrivateSecurityDescriptor,
    ) -> Result<OwnedHandle, SupervisorError> {
        let attributes = security.attributes();
        // SAFETY: attributes points to the live private descriptor; null name
        // requests an unnamed job object whose DACL is checked immediately.
        let job = OwnedHandle::new(unsafe { CreateJobObjectW(&attributes, null()) })?;
        verify_private_acl(job.raw(), security.dacl(), SE_KERNEL_OBJECT)?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `limits` is fully initialized and its size matches the selected
        // JobObjectExtendedLimitInformation class.
        if unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(SupervisorError::ContainmentUnavailable);
        }
        Ok(job)
    }

    fn open_inheritable_nul() -> Result<OwnedHandle, SupervisorError> {
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        };
        let name = wide_path(Path::new("NUL"))?;
        // SAFETY: the name and SECURITY_ATTRIBUTES remain alive for the call; the
        // returned handle is explicitly inheritable and later constrained by the
        // process attribute handle list.
        OwnedHandle::new(unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &mut attributes,
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        })
    }

    fn open_pinned_executable(path: &Path) -> Result<File, SupervisorError> {
        let mut options = OpenOptions::new();
        options
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        let file = options
            .open(path)
            .map_err(|_| SupervisorError::InvalidArtifact)?;
        let info = file_info(file.as_raw_handle() as HANDLE)?;
        if info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0 {
            return Err(SupervisorError::InvalidArtifact);
        }
        Ok(file)
    }

    fn file_info(handle: HANDLE) -> Result<BY_HANDLE_FILE_INFORMATION, SupervisorError> {
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: handle is a live file handle and information is writable.
        if unsafe { GetFileInformationByHandle(handle, &mut information) } == 0 {
            return Err(SupervisorError::ContainmentUnavailable);
        }
        Ok(information)
    }

    fn file_size(file: &File) -> Result<u64, SupervisorError> {
        let info = file_info(file.as_raw_handle() as HANDLE)?;
        Ok((u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow))
    }

    fn hash_open_file(file: &mut File) -> Result<Sha256Digest32, SupervisorError> {
        file.seek(SeekFrom::Start(0))
            .map_err(|_| SupervisorError::ExecutableProbeFailed)?;
        let digest = crate::sha256::sha256_reader(&mut *file)
            .map_err(|_| SupervisorError::ExecutableProbeFailed)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| SupervisorError::ExecutableProbeFailed)?;
        Ok(Sha256Digest32::from_bytes(digest))
    }

    fn secure_data_tree(root: &Path) -> Result<(Vec<OwnedHandle>, PathBuf), SupervisorError> {
        let canonical_root = root
            .canonicalize()
            .map_err(|_| SupervisorError::ContainmentUnavailable)?;
        if canonical_root.as_os_str().encode_wide().count()
            > MAX_DATA_ROOT_PATH_UTF16_UNITS + EXTENDED_PATH_PREFIX_UNITS
        {
            return Err(SupervisorError::DataRootMismatch);
        }
        // Open the requested final path with OPEN_REPARSE_POINT before resolving
        // it; canonicalize is used only for equality checking, never as the open
        // operation that silently follows a final reparse point.
        let root_handle = open_path_handle(root)?;
        ensure_expected_kind(&root_handle, true)?;
        set_and_verify_private_acl(root_handle.raw(), true)?;
        let root_final = final_path(root_handle.raw())?;
        let opened_final = root_final
            .canonicalize()
            .map_err(|_| SupervisorError::ContainmentUnavailable)?;
        if !same_windows_path(&canonical_root, &opened_final) {
            return Err(SupervisorError::DataRootMismatch);
        }
        let mut handles = vec![root_handle];
        let mut count = 0_usize;
        secure_descendants(&opened_final, &root_final, 0, &mut count, &mut handles)?;
        Ok((handles, opened_final))
    }

    fn secure_descendants(
        directory: &Path,
        root_final: &Path,
        depth: usize,
        count: &mut usize,
        retained: &mut Vec<OwnedHandle>,
    ) -> Result<(), SupervisorError> {
        if depth > MAX_DIRECTORY_DEPTH {
            return Err(SupervisorError::ContainmentUnavailable);
        }
        for entry in fs::read_dir(directory).map_err(|_| SupervisorError::ContainmentUnavailable)? {
            *count = count.saturating_add(1);
            if *count > MAX_DIRECTORY_ENTRIES {
                return Err(SupervisorError::ContainmentUnavailable);
            }
            let entry = entry.map_err(|_| SupervisorError::ContainmentUnavailable)?;
            let child_path = entry.path();
            let child_handle = open_path_handle(&child_path)?;
            let is_directory = ensure_expected_kind(&child_handle, false)?;
            let child_final = final_path(child_handle.raw())?;
            if !is_beneath(root_final, &child_final) {
                return Err(SupervisorError::DataRootMismatch);
            }
            set_and_verify_private_acl(child_handle.raw(), is_directory)?;
            if is_directory {
                retained.push(child_handle);
                secure_descendants(&child_path, root_final, depth + 1, count, retained)?;
            }
        }
        Ok(())
    }

    fn open_path_handle(path: &Path) -> Result<OwnedHandle, SupervisorError> {
        let name = wide_path(path)?;
        // BACKUP_SEMANTICS is required to open directory handles; it is also
        // valid for files, whose attributes are checked after the handle opens.
        let flags = FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS;
        // SAFETY: the path buffer is NUL terminated for the duration of the call;
        // OPEN_REPARSE_POINT prevents following the final symlink/reparse object.
        OwnedHandle::new(unsafe {
            CreateFileW(
                name.as_ptr(),
                FILE_READ_ATTRIBUTES | READ_CONTROL | WRITE_DAC,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                flags,
                null_mut(),
            )
        })
    }

    fn ensure_expected_kind(
        handle: &OwnedHandle,
        expected_directory: bool,
    ) -> Result<bool, SupervisorError> {
        let info = file_info(handle.raw())?;
        if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(SupervisorError::ContainmentUnavailable);
        }
        let is_directory = info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
        if expected_directory && !is_directory {
            return Err(SupervisorError::ContainmentUnavailable);
        }
        if !is_directory && info.nNumberOfLinks > 1 {
            return Err(SupervisorError::ContainmentUnavailable);
        }
        Ok(is_directory)
    }

    fn set_and_verify_private_acl(handle: HANDLE, directory: bool) -> Result<(), SupervisorError> {
        let sddl = if directory {
            SAFE_DIRECTORY_SDDL
        } else {
            SAFE_FILE_SDDL
        };
        let sddl_wide = wide_path(Path::new(sddl))?;
        let mut expected_descriptor = null_mut();
        // SAFETY: SDDL input is a valid, NUL-terminated constant and output pointer
        // is writable. The resulting LocalAlloc descriptor is freed below.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl_wide.as_ptr(),
                SECURITY_DESCRIPTOR_REVISION,
                &mut expected_descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(SupervisorError::ContainmentUnavailable);
        }
        let mut expected_dacl: *mut ACL = null_mut();
        let mut present = 0;
        let mut defaulted = 0;
        // SAFETY: descriptor is returned by the conversion API and remains live;
        // all outputs are writable pointers.
        if unsafe {
            windows_sys::Win32::Security::GetSecurityDescriptorDacl(
                expected_descriptor,
                &mut present,
                &mut expected_dacl,
                &mut defaulted,
            )
        } == 0
            || present == 0
            || expected_dacl.is_null()
        {
            // SAFETY: descriptor is a LocalAlloc allocation returned above.
            unsafe { LocalFree(expected_descriptor.cast()) };
            return Err(SupervisorError::ContainmentUnavailable);
        }
        // SAFETY: handle is an opened filesystem object and expected_dacl points
        // into the still-live descriptor; SetSecurityInfo copies the ACL.
        let set_status = unsafe {
            SetSecurityInfo(
                handle,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                expected_dacl,
                null_mut(),
            )
        };
        let result = if set_status != 0 {
            Err(SupervisorError::ContainmentUnavailable)
        } else {
            verify_private_acl(handle, expected_dacl, SE_FILE_OBJECT)
        };
        // SAFETY: descriptor is a LocalAlloc allocation returned by the SDDL API.
        unsafe { LocalFree(expected_descriptor.cast()) };
        result
    }

    fn verify_private_acl(
        handle: HANDLE,
        expected_dacl: *mut ACL,
        object_type: SE_OBJECT_TYPE,
    ) -> Result<(), SupervisorError> {
        let mut actual_descriptor = null_mut();
        let mut actual_dacl: *mut ACL = null_mut();
        // SAFETY: handle is a valid open file handle; output pointers are writable.
        let status = unsafe {
            GetSecurityInfo(
                handle,
                object_type,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut actual_dacl,
                null_mut(),
                &mut actual_descriptor,
            )
        };
        if status != 0 || actual_descriptor.is_null() || actual_dacl.is_null() {
            if !actual_descriptor.is_null() {
                // SAFETY: GetSecurityInfo returns LocalAlloc memory on success.
                unsafe { LocalFree(actual_descriptor.cast()) };
            }
            return Err(SupervisorError::ContainmentUnavailable);
        }
        let mut control: SECURITY_DESCRIPTOR_CONTROL = 0;
        let mut revision = 0;
        // SAFETY: both descriptors are valid until the LocalFree below; outputs
        // are writable and match their declared scalar types.
        let control_ok = unsafe {
            windows_sys::Win32::Security::GetSecurityDescriptorControl(
                actual_descriptor,
                &mut control,
                &mut revision,
            )
        } != 0;
        let actual_size = acl_bytes_in_use(actual_dacl);
        let expected_size = acl_bytes_in_use(expected_dacl);
        let exact_acl = match (actual_size, expected_size) {
            (Some(actual_size), Some(expected_size)) if actual_size == expected_size => {
                // SAFETY: each ACL pointer came from a valid security descriptor;
                // GetAclInformation supplied the byte lengths bounded by AclSize.
                let actual =
                    unsafe { core::slice::from_raw_parts(actual_dacl.cast::<u8>(), actual_size) };
                // SAFETY: same as above; expected descriptor remains live.
                let expected = unsafe {
                    core::slice::from_raw_parts(expected_dacl.cast::<u8>(), expected_size)
                };
                actual == expected
            }
            _ => false,
        };
        // SAFETY: the descriptor was allocated by GetSecurityInfo.
        unsafe { LocalFree(actual_descriptor.cast()) };
        if control_ok && control & SE_DACL_PROTECTED != 0 && exact_acl {
            Ok(())
        } else {
            Err(SupervisorError::ContainmentUnavailable)
        }
    }

    fn acl_bytes_in_use(acl: *mut ACL) -> Option<usize> {
        let mut information = ACL_SIZE_INFORMATION::default();
        // SAFETY: ACL is returned by a live security descriptor and information
        // is writable with the exact AclSizeInformation layout.
        if unsafe {
            windows_sys::Win32::Security::GetAclInformation(
                acl,
                (&mut information as *mut ACL_SIZE_INFORMATION).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } == 0
        {
            None
        } else {
            Some(information.AclBytesInUse as usize)
        }
    }

    fn write_secure_config(path: &Path, contents: &[u8]) -> Result<File, SupervisorError> {
        let name = wide_path(path)?;
        // SAFETY: the UTF-16 path buffer and null security attributes remain live
        // for the call; OPEN_ALWAYS does not follow the final reparse point.
        let raw = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE | FILE_READ_ATTRIBUTES | READ_CONTROL | WRITE_DAC,
                FILE_SHARE_READ,
                null(),
                OPEN_ALWAYS,
                FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
            )
        };
        let mut file = if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            return Err(SupervisorError::StartFailed);
        } else {
            // SAFETY: CreateFileW succeeded and ownership of this unique handle is
            // transferred to the File for its entire remaining lifetime.
            unsafe { File::from_raw_handle(raw) }
        };
        let information = file_info(file.as_raw_handle() as HANDLE)?;
        if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0
        {
            return Err(SupervisorError::ContainmentUnavailable);
        }
        set_and_verify_private_acl(file.as_raw_handle() as HANDLE, false)?;
        file.set_len(0).map_err(|_| SupervisorError::StartFailed)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| SupervisorError::StartFailed)?;
        file.write_all(contents)
            .map_err(|_| SupervisorError::StartFailed)?;
        file.sync_data().map_err(|_| SupervisorError::StartFailed)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| SupervisorError::StartFailed)?;
        let mut observed = Vec::with_capacity(contents.len());
        file.read_to_end(&mut observed)
            .map_err(|_| SupervisorError::StartFailed)?;
        if observed != contents {
            return Err(SupervisorError::InvalidProcessConfig);
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|_| SupervisorError::StartFailed)?;
        Ok(file)
    }

    fn final_path(handle: HANDLE) -> Result<PathBuf, SupervisorError> {
        let mut buffer = vec![0_u16; MAX_WIN32_PATH_UNITS];
        // SAFETY: handle is valid and buffer has writable capacity as expressed
        // by the character count; flags zero request DOS-device path form.
        let length = unsafe {
            GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, 0)
        };
        if length == 0 || length as usize >= buffer.len() {
            return Err(SupervisorError::ContainmentUnavailable);
        }
        buffer.truncate(length as usize);
        Ok(PathBuf::from(OsString::from_wide(&buffer)))
    }

    fn is_beneath(root: &Path, candidate: &Path) -> bool {
        let root = normalize_windows_path(root);
        let candidate = normalize_windows_path(candidate);
        candidate.starts_with(&format!("{root}\\"))
    }

    fn same_windows_path(left: &Path, right: &Path) -> bool {
        normalize_windows_path(left) == normalize_windows_path(right)
    }

    fn normalize_windows_path(path: &Path) -> String {
        let value = path.to_string_lossy().replace('/', "\\");
        value
            .strip_prefix("\\\\?\\")
            .unwrap_or(&value)
            .trim_end_matches('\\')
            .to_lowercase()
    }

    fn wide_path(path: &Path) -> Result<Vec<u16>, SupervisorError> {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.is_empty() || wide.contains(&0) || wide.len() >= MAX_WIN32_PATH_UNITS {
            return Err(SupervisorError::InvalidProcessConfig);
        }
        wide.push(0);
        Ok(wide)
    }

    fn quote_windows_argument(
        argument: &OsStr,
        output: &mut Vec<u16>,
    ) -> Result<(), SupervisorError> {
        let input: Vec<u16> = argument.encode_wide().collect();
        if input.contains(&0) {
            return Err(SupervisorError::InvalidProcessConfig);
        }
        output.push(b'"' as u16);
        let mut backslashes = 0_usize;
        for unit in input {
            if unit == b'\\' as u16 {
                backslashes += 1;
            } else if unit == b'"' as u16 {
                output.extend(core::iter::repeat_n(b'\\' as u16, backslashes * 2 + 1));
                output.push(unit);
                backslashes = 0;
            } else {
                output.extend(core::iter::repeat_n(b'\\' as u16, backslashes));
                output.push(unit);
                backslashes = 0;
            }
        }
        output.extend(core::iter::repeat_n(b'\\' as u16, backslashes * 2));
        output.push(b'"' as u16);
        Ok(())
    }

    fn make_command_line(executable: &Path, config: &Path) -> Result<Vec<u16>, SupervisorError> {
        let mut command = Vec::new();
        quote_windows_argument(executable.as_os_str(), &mut command)?;
        command.extend(" --config-path ".encode_utf16());
        quote_windows_argument(config.as_os_str(), &mut command)?;
        if command.len() >= 32_767 {
            return Err(SupervisorError::InvalidProcessConfig);
        }
        command.push(0);
        Ok(command)
    }

    struct SensitiveEnvironment(Vec<u16>);

    impl SensitiveEnvironment {
        fn as_ptr(&self) -> *const u16 {
            self.0.as_ptr()
        }
    }

    impl Drop for SensitiveEnvironment {
        fn drop(&mut self) {
            self.0.fill(0);
            core::hint::black_box(self.0.as_mut_ptr());
        }
    }

    fn child_environment_block(
        temp_dir: &Path,
        secret: &[u8],
    ) -> Result<SensitiveEnvironment, SupervisorError> {
        if secret.is_empty() || !secret.iter().all(|byte| (0x21..=0x7e).contains(byte)) {
            return Err(SupervisorError::SecretLeaseInvalid);
        }
        let mut values: BTreeMap<String, Vec<u16>> = BTreeMap::new();
        for name in ["SystemRoot", "WINDIR"] {
            if let Some(value) = std::env::var_os(name) {
                let wide: Vec<u16> = value.encode_wide().collect();
                if wide.contains(&0) {
                    return Err(SupervisorError::StartFailed);
                }
                values.insert(name.to_ascii_lowercase(), wide);
            }
        }
        if !values.contains_key("systemroot") {
            return Err(SupervisorError::PlatformUnavailable);
        }
        let temp = temp_dir.as_os_str().encode_wide().collect::<Vec<_>>();
        values.insert("temp".to_owned(), temp.clone());
        values.insert("tmp".to_owned(), temp);
        let secret_name = CHILD_API_KEY_NAME.to_ascii_lowercase();
        let mut names = values.keys().cloned().collect::<Vec<_>>();
        names.push(secret_name.clone());
        names.sort();
        let mut block = Vec::new();
        for name in names {
            if name == secret_name {
                block.extend(CHILD_API_KEY_NAME.encode_utf16());
                block.push(b'=' as u16);
                block.extend(secret.iter().map(|byte| u16::from(*byte)));
            } else {
                let value = values.get(&name).ok_or(SupervisorError::StartFailed)?;
                block.extend(name.encode_utf16());
                block.push(b'=' as u16);
                block.extend(value);
            }
            block.push(0);
        }
        block.push(0);
        Ok(SensitiveEnvironment(block))
    }
} // mod windows_impl

#[cfg(windows)]
pub(crate) use windows_impl::NativeProcess;

pub(crate) enum NativeSpawnError {
    Definite(SupervisorError),
    Unknown {
        reason: SupervisorError,
        guard: NativeProcess,
        containment: crate::ContainmentReport,
    },
}

impl From<SupervisorError> for NativeSpawnError {
    fn from(error: SupervisorError) -> Self {
        Self::Definite(error)
    }
}

#[cfg(not(windows))]
use crate::secret::SecretMaterial;
#[cfg(not(windows))]
use crate::{
    LoopbackHost, ProcessIdentity, QdrantEndpointIdentity, QdrantOwnerFence, ShutdownReceipt,
    StartRecoveryReceipt, SupervisorError,
};
#[cfg(not(windows))]
use search_contracts::{ArtifactDigest, Blake3Digest32, Sha256Digest32};
#[cfg(not(windows))]
use std::net::SocketAddr;
#[cfg(not(windows))]
use std::path::Path;
#[cfg(not(windows))]
use std::time::Instant;

#[cfg(not(windows))]
pub(crate) struct NativeProcess;

#[cfg(not(windows))]
impl NativeProcess {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spawn(
        _: &Path,
        _: Sha256Digest32,
        _: u64,
        _: &str,
        _: search_contracts::OpaqueId,
        _: ArtifactDigest,
        _: QdrantOwnerFence,
        _: QdrantEndpointIdentity,
        _: Blake3Digest32,
        _: Blake3Digest32,
        _: &Path,
        _: &Path,
        _: &[u8],
        _: SecretMaterial,
    ) -> Result<Self, NativeSpawnError> {
        Err(NativeSpawnError::Definite(
            SupervisorError::PlatformUnavailable,
        ))
    }

    pub(crate) const fn config_digest(&self) -> Blake3Digest32 {
        unreachable!("unsupported platform")
    }
    pub(crate) const fn expected_secret_purpose(&self) -> Blake3Digest32 {
        unreachable!("unsupported platform")
    }
    pub(crate) const fn has_process_handle(&self) -> bool {
        false
    }
    pub(crate) fn identity(&mut self) -> Result<ProcessIdentity, SupervisorError> {
        Err(SupervisorError::PlatformUnavailable)
    }
    pub(crate) fn is_running(&self) -> Result<bool, SupervisorError> {
        Err(SupervisorError::PlatformUnavailable)
    }
    pub(crate) fn try_exit_code(&self) -> Result<Option<i32>, SupervisorError> {
        Err(SupervisorError::PlatformUnavailable)
    }
    pub(crate) fn job_empty(&self) -> Result<bool, SupervisorError> {
        Err(SupervisorError::PlatformUnavailable)
    }
    pub(crate) fn shutdown_tree(
        &mut self,
        _: ProcessIdentity,
        _: Instant,
    ) -> Result<ShutdownReceipt, SupervisorError> {
        Err(SupervisorError::PlatformUnavailable)
    }
    pub(crate) fn verify_root_handle(&self, _: ProcessIdentity) -> Result<(), SupervisorError> {
        Err(SupervisorError::PlatformUnavailable)
    }
    pub(crate) fn terminate_unverified_tree(&mut self, _: Instant) -> Result<(), SupervisorError> {
        Err(SupervisorError::PlatformUnavailable)
    }
    pub(crate) fn recover_ambiguous_start(
        &mut self,
        _: Instant,
    ) -> Result<StartRecoveryReceipt, SupervisorError> {
        Err(SupervisorError::PlatformUnavailable)
    }
    pub(crate) fn secret(&self) -> &SecretMaterial {
        unreachable!("unsupported platform")
    }
    pub(crate) fn secret_appears_in(&self, _: &str) -> bool {
        unreachable!("unsupported platform")
    }
    pub(crate) fn config_path(&self) -> &Path {
        unreachable!("unsupported platform")
    }
}
