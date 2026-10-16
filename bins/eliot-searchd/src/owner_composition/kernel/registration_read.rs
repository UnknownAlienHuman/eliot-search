//! Optional registration custody is separate from persisted native10/native11 pins.

use std::fs::{self, File, Metadata};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
#[cfg(not(windows))]
use std::time::SystemTime;

use search_runtime_owner::OwnerError;

use super::super::super::native_objects::{
    metadata, native_identity, open_initialization, read_bytes,
};
use super::{NativeReadOperation, NativeRootCapability, NativeRootRequest, NativeWork};

pub(in super::super) struct RegistrationPin {
    path: PathBuf,
    file: Option<File>,
    identity: Vec<u8>,
    #[cfg(windows)]
    observation: Option<eliot_searchd::native_file::Observation>,
    #[cfg(not(windows))]
    length: u64,
    #[cfg(not(windows))]
    modified: Option<SystemTime>,
}

type RegistrationAcquisition = Result<(RegistrationPin, Option<Vec<u8>>), &'static str>;

impl RegistrationPin {
    fn pending(
        request: &NativeRootRequest,
        work: &mut NativeWork,
        path: &Path,
    ) -> Result<bool, OwnerError> {
        for pending in [path.with_extension("tmp"), path.with_extension("bak")] {
            if metadata(request, work, &pending)?.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(in super::super) fn acquire(
        request: &NativeRootRequest,
        work: &mut NativeWork,
        root: &Path,
    ) -> Result<RegistrationAcquisition, OwnerError> {
        let path = root.join("control/source-roots.v1");
        if Self::pending(request, work, &path)? {
            return Ok(Err("SOURCE_ROOT_UPDATE_OUTCOME_UNKNOWN"));
        }
        let mut pin = Self {
            path,
            file: None,
            identity: Vec::new(),
            #[cfg(windows)]
            observation: None,
            #[cfg(not(windows))]
            length: 0,
            #[cfg(not(windows))]
            modified: None,
        };
        let bytes = if let Some(found) = metadata(request, work, &pin.path)? {
            if found.len() > crate::source_roots::MAX_SOURCE_ROOT_FILE_BYTES as u64 {
                return Ok(Err("SOURCE_ROOT_CONFIG_TOO_LARGE"));
            }
            let mut file = open_initialization(request, work, &pin.path, false)?;
            let bytes = read_bytes(
                request,
                work,
                &mut file,
                crate::source_roots::MAX_SOURCE_ROOT_FILE_BYTES,
            )?;
            #[cfg(windows)]
            {
                let observed = work.call(request, || eliot_searchd::native_file::observe(&file)
                    .map_err(|_| OwnerError::OwnerGuardMismatch))?;
                pin.identity.extend_from_slice(&observed.volume_serial.to_be_bytes());
                pin.identity.extend_from_slice(&observed.file_index.to_be_bytes());
                pin.identity.extend_from_slice(&observed.creation_time.to_be_bytes());
                pin.observation = Some(observed);
            }
            #[cfg(not(windows))]
            {
                let held = work.call(request, || {
                    file.metadata().map_err(|_| OwnerError::OwnerGuardMismatch)
                })?;
                pin.identity = work.call(request, || native_identity(&file))?;
                pin.length = held.len();
                pin.modified = held.modified().ok();
            }
            pin.file = Some(file);
            Some(bytes)
        } else {
            None
        };
        pin.verify(request, work)?;
        Ok(Ok((pin, bytes)))
    }

    /// Retained read-only/no-delete handles keep the catalog and pinned parents
    /// stable between full locator checkpoints. Every seam polls pending state.
    pub(in super::super) fn check(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
    ) -> Result<(), OwnerError> {
        if Self::pending(request, work, &self.path)? {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let Some(file) = &self.file else {
            return if metadata(request, work, &self.path)?.is_none() {
                Ok(())
            } else {
                Err(OwnerError::OwnerGuardMismatch)
            };
        };
        #[cfg(windows)]
        {
            let observed = work.call(request, || eliot_searchd::native_file::observe(file)
                .map_err(|_| OwnerError::OwnerGuardMismatch))?;
            if Some(observed) != self.observation { return Err(OwnerError::OwnerGuardMismatch); }
        }
        #[cfg(not(windows))]
        {
            if work.call(request, || native_identity(file))? != self.identity {
                return Err(OwnerError::OwnerGuardMismatch);
            }
            let held = work.call(request, || {
                file.metadata().map_err(|_| OwnerError::OwnerGuardMismatch)
            })?;
            if held.len() != self.length || held.modified().ok() != self.modified {
                return Err(OwnerError::OwnerGuardMismatch);
            }
        }
        request.check()
    }

    pub(in super::super) fn verify(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
    ) -> Result<(), OwnerError> {
        self.check(request, work)?;
        if self.file.is_some() {
            let current = open_initialization(request, work, &self.path, false)?;
            if work.call(request, || native_identity(&current))? != self.identity {
                return Err(OwnerError::OwnerGuardMismatch);
            }
        }
        request.check()
    }
}

impl NativeReadOperation<'_> {
    pub(super) fn verify_registration(&mut self) -> Result<(), OwnerError> {
        let Some(pin) = self.registration.take() else {
            return Ok(());
        };
        let result = self.with_binding(|_, request, work, _| pin.verify(request, work));
        self.registration = Some(pin);
        result
    }

    pub(crate) fn registration_bytes(&mut self) -> Result<Option<Vec<u8>>, String> {
        if self.registration.is_some() {
            return Err(OwnerError::OwnerOperationConflict.code().to_owned());
        }
        let result = self
            .with_binding(|binding, request, work, _| {
                RegistrationPin::acquire(request, work, binding.root)
            })
            .map_err(|error| error.code().to_owned())?;
        let (pin, bytes) = result.map_err(str::to_owned)?;
        self.registration = Some(pin);
        Ok(bytes)
    }

    /// Named observation seams; I/O failure is a gap, context refusal is terminal.
    pub(crate) fn source_root_metadata(
        &mut self,
        path: &Path,
    ) -> Result<io::Result<Metadata>, OwnerError> {
        self.with_binding(|_, request, work, _| {
            work.call(request, || Ok(fs::symlink_metadata(path)))
        })
    }

    pub(crate) fn source_root_canonicalize(
        &mut self,
        path: &Path,
    ) -> Result<io::Result<PathBuf>, OwnerError> {
        self.with_binding(|_, request, work, _| work.call(request, || Ok(fs::canonicalize(path))))
    }

    /// The complete bounded result is prepared before the first fallible output.
    pub(crate) fn emit_registration(
        &mut self,
        writer: &mut impl Write,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.verify().map_err(|error| error.code().to_owned())?;
        let mut remaining = bytes;
        while !remaining.is_empty() {
            let count = self
                .write_output(writer, remaining)
                .map_err(|error| error.code().to_owned())?;
            if count == 0 || count > remaining.len() {
                self.owner.poisoned = Some(OwnerError::OwnerAcquireOutcomeUnknown);
                self.cancel_output();
                return Err(OwnerError::OwnerAcquireOutcomeUnknown.code().to_owned());
            }
            remaining = &remaining[count..];
        }
        self.flush_output(writer)
            .map_err(|error| error.code().to_owned())?;
        self.verify().map_err(|error| error.code().to_owned())
    }
}

impl NativeRootCapability {
    /// Catalog-only observation never opens DIRECT, credentials or lifecycle state.
    pub(crate) fn run_registration_one_shot<T>(
        mut self: Box<Self>,
        execute: impl FnOnce(&mut NativeReadOperation<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut operation = NativeReadOperation {
            owner: &mut self,
            context: None,
            output_attempted: false,
            registration: None, first_refusal: None,
        };
        operation
            .verify()
            .map_err(|error| error.code().to_owned())?;
        let result = execute(&mut operation);
        let checkpoint = operation.verify();
        let attempted = operation.output_attempted;
        drop(operation); // Optional registration handle closes before native custody.
        let closing = self.verify();
        let Self {
            pins,
            root,
            installation,
            record,
            request,
            mut work,
            bound_root,
            exclusions,
            ..
        } = *self;
        let children_closed = work.arrive(&request, "registration-children-closed");
        drop((pins, root, installation, record, bound_root, exclusions));
        let released = work.arrive(&request, "registration-custody-released");
        checkpoint
            .and(closing)
            .and(children_closed)
            .and(released)
            .map_err(|error| {
                if attempted {
                    OwnerError::OwnerAcquireOutcomeUnknown
                } else {
                    error
                }
                .code()
                .to_owned()
            })?;
        if attempted && result.is_err() {
            return Err(OwnerError::OwnerAcquireOutcomeUnknown.code().to_owned());
        }
        result
    }
}
