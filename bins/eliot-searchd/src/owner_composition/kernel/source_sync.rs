//! Closed effect-free sync custody; the durable owner stays RELEASED.

use std::fs::{self, Metadata};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use search_runtime_owner::OwnerError;

use super::super::native_request::{NativeRootRequest, NativeWork};
use super::super::spec::LifecycleState;
use super::read_child::registration_read::RegistrationPin;
use super::{NativeRootCapability, acquire_existing};

// Field order closes the registration child before native pins/exclusions on Drop.
struct SyncHolder {
    registration: RegistrationPin,
    bytes: Option<Vec<u8>>,
    consumed: bool,
    output_attempted: bool,
    owner: NativeRootCapability,
}

impl SyncHolder {
    const fn operation(&mut self) -> NativeSyncPreflightOperation<'_> {
        NativeSyncPreflightOperation {
            owner: &mut self.owner,
            registration: &self.registration,
            bytes: &mut self.bytes,
            consumed: &mut self.consumed,
        }
    }

    fn close(self) -> Result<(), OwnerError> {
        let Self {
            registration,
            bytes,
            owner,
            output_attempted,
            ..
        } = self;
        drop((registration, bytes));
        close_owner(owner).map_err(|error| {
            if output_attempted {
                OwnerError::OwnerAcquireOutcomeUnknown
            } else {
                error
            }
        })
    }

    fn terminal(&mut self, writer: &mut impl Write, code: &str) -> Result<(), OwnerError> {
        if self.output_attempted {
            return Err(OwnerError::OwnerAcquireOutcomeUnknown);
        }
        self.output_attempted = true;
        let result = emit(&self.owner.request, &mut self.owner.work, writer, code);
        if let Err(error) = result {
            self.owner.poisoned = Some(error);
        }
        result
    }
}

fn close_owner(mut owner: NativeRootCapability) -> Result<(), OwnerError> {
    let child = owner
        .work
        .arrive(&owner.request, "sync-registration-closed");
    let NativeRootCapability {
        pins,
        root,
        installation,
        record,
        request,
        mut work,
        bound_root,
        exclusions,
        ..
    } = owner;
    drop((pins, root, installation, record, bound_root));
    let pins = work.arrive(&request, "sync-pins-closed");
    drop(exclusions);
    let exclusions = work.arrive(&request, "sync-exclusions-released");
    child.and(pins).and(exclusions)
}

// Exactly one complete frame write; short/failed output is unknown, never retried.
fn emit(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    writer: &mut impl Write,
    code: &str,
) -> Result<(), OwnerError> {
    if code.len() > 128
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(OwnerError::OwnerInvalidTransition);
    }
    let frame = format!(
        "{{\"error\":\"{code}\",\"complete\":false,\"effects_may_have_committed\":false}}\n"
    );
    let mut attempted = false;
    let result = (|| {
        work.canonical(request, 0)?;
        work.write(request, frame.len())?;
        let count = work.call(request, || {
            attempted = true;
            writer
                .write(frame.as_bytes())
                .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
        })?;
        if count != frame.len() {
            return Err(OwnerError::OwnerAcquireOutcomeUnknown);
        }
        work.call(request, || {
            writer
                .flush()
                .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
        })
    })();
    if let Err(error) = result {
        if attempted {
            request
                .credential_context()
                .1
                .store(true, Ordering::Release);
            return Err(OwnerError::OwnerAcquireOutcomeUnknown);
        }
        return Err(error);
    }
    Ok(())
}

/// The real command acquires existing mutation custody without publishing lifecycle state.
pub fn run_source_sync_preflight(
    request: NativeRootRequest,
    output: &mut impl Write,
) -> Result<(), String> {
    let mut work = NativeWork::new();
    let acquired = match acquire_existing(&request, &mut work) {
        Ok(Ok(acquired)) => acquired,
        result => {
            let error = result.err().unwrap_or(OwnerError::OwnerRecoveryQuarantined);
            emit(&request, &mut work, output, error.code())
                .map_err(|error| error.code().to_owned())?;
            return Err(error.code().to_owned());
        }
    };
    let (mut owner, _unpublished_successor) = acquired.bind(request, work);
    let pinned = RegistrationPin::acquire(&owner.request, &mut owner.work, &owner.root);
    let (registration, bytes) = match pinned {
        Ok(Ok(pinned)) => pinned,
        result => {
            let code = match &result {
                Err(error) => error.code(),
                Ok(Err(code)) => code,
                _ => unreachable!(),
            };
            let emitted = emit(&owner.request, &mut owner.work, output, code);
            let closed = close_owner(owner);
            emitted
                .and(closed)
                .map_err(|error| error.code().to_owned())?;
            return Err(code.to_owned());
        }
    };
    let mut holder = SyncHolder {
        registration,
        bytes,
        consumed: false,
        output_attempted: false,
        owner,
    };
    let preflight = crate::source_roots::preflight_sync(&mut holder.operation());
    let checked = holder
        .operation()
        .verify()
        .map_err(|error| error.code().to_owned());
    let outcome = checked.and(preflight);
    let code = outcome
        .as_ref()
        .err()
        .map_or("SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE", String::as_str);
    let emitted = holder.terminal(output, code);
    let closing = holder.close();
    emitted
        .and(closing)
        .map_err(|error| error.code().to_owned())?;
    Err(code.to_owned())
}

/// Owner-private construction; callers borrow only bounded read/probe operations.
pub struct NativeSyncPreflightOperation<'owner> {
    owner: &'owner mut NativeRootCapability,
    registration: &'owner RegistrationPin,
    bytes: &'owner mut Option<Vec<u8>>,
    consumed: &'owner mut bool,
}

impl NativeSyncPreflightOperation<'_> {
    pub(crate) fn root(&self) -> &Path {
        &self.owner.root
    }
    pub(crate) fn checkpoint(&mut self) -> Result<(), OwnerError> {
        if !self.owner.mutation
            || self.owner.effect_observed
            || self.owner.record.lifecycle != LifecycleState::Released
        {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        self.owner.request.check()?;
        self.registration
            .check(&self.owner.request, &mut self.owner.work)
    }
    pub(crate) fn verify(&mut self) -> Result<(), OwnerError> {
        self.checkpoint()?;
        self.owner.verify()?;
        self.registration
            .verify(&self.owner.request, &mut self.owner.work)
    }
    pub(crate) fn registration_bytes(&mut self) -> Result<Option<Vec<u8>>, OwnerError> {
        self.verify()?;
        if *self.consumed {
            return Err(OwnerError::OwnerOperationConflict);
        }
        *self.consumed = true;
        Ok(self.bytes.take())
    }
    pub(crate) fn source_root_metadata(
        &mut self,
        path: &Path,
    ) -> Result<io::Result<Metadata>, OwnerError> {
        self.checkpoint()?;
        self.owner
            .work
            .call(&self.owner.request, || Ok(fs::symlink_metadata(path)))
    }
    pub(crate) fn source_root_canonicalize(
        &mut self,
        path: &Path,
    ) -> Result<io::Result<PathBuf>, OwnerError> {
        self.checkpoint()?;
        self.owner
            .work
            .call(&self.owner.request, || Ok(fs::canonicalize(path)))
    }
}
