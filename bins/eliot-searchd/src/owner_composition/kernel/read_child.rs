//! Borrowed read operations over the retained native owner, without lifecycle writes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_runtime_owner::OwnerError;

use super::super::native_objects::read_bytes;
use super::super::spec::LifecycleState;
use super::{NativeBindingView, NativeRootCapability, NativeRootRequest, NativeWork};

// None borrows the startup request/meter; Some owns a distinct admitted command.
type ReadContext = Option<(NativeRootRequest, NativeWork)>;

/// No constructor, clone, lifecycle transition or unbound path admission.
pub struct NativeReadOperation<'owner> {
    owner: &'owner mut NativeRootCapability,
    context: ReadContext,
    output_attempted: bool,
    registration: Option<registration_read::RegistrationPin>,
}

impl NativeReadOperation<'_> {
    fn with_binding<T>(
        &mut self,
        operation: impl FnOnce(
            NativeBindingView<'_>,
            &NativeRootRequest,
            &mut NativeWork,
            &mut bool,
        ) -> Result<T, OwnerError>,
    ) -> Result<T, OwnerError> {
        if self.owner.mutation || self.owner.record.lifecycle != LifecycleState::Released {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        if let Some(error) = self.owner.poisoned {
            return Err(error);
        }
        let NativeRootCapability {
            pins,
            root,
            installation,
            record,
            selected_key_binding,
            bound_root,
            exclusions,
            request,
            work,
            ..
        } = &mut *self.owner;
        let (request, work) = match &mut self.context {
            None => (&*request, work),
            Some((request, work)) => (&*request, work),
        };
        let registration = &self.registration;
        let result = request.check().and_then(|()| {
            if let Some(pin) = registration {
                pin.check(request, work)?;
            }
            let result = operation(
                NativeBindingView {
                    pins,
                    root,
                    installation,
                    record,
                    selected_key_binding: *selected_key_binding,
                    bound_root,
                    exclusions,
                },
                request,
                work,
                &mut self.output_attempted,
            );
            result.and_then(|value| {
                if let Some(pin) = registration {
                    pin.check(request, work)?;
                }
                Ok(value)
            })
        });
        let checkpoint = request.check();
        let result = result
            .and_then(|value| checkpoint.map(|()| value))
            .map_err(|error| {
                if self.output_attempted {
                    OwnerError::OwnerAcquireOutcomeUnknown
                } else {
                    error
                }
            });
        if let Err(error) = result.as_ref() {
            self.owner.poisoned = Some(*error);
        }
        result
    }

    pub(crate) fn verify(&mut self) -> Result<(), OwnerError> {
        self.with_binding(|binding, request, work, _| binding.verify(request, work).map(|_| ()))?;
        self.verify_registration()
    }

    pub(crate) fn root(&self) -> &std::path::Path {
        &self.owner.root
    }

    pub(crate) const fn selected_binding(&self) -> Option<[u8; 32]> {
        self.owner.selected_key_binding
    }

    pub(crate) fn namespace_bytes(&mut self) -> Result<Vec<u8>, OwnerError> {
        self.with_binding(|binding, request, work, _| {
            read_bytes(request, work, &mut binding.pins.files[8], 256)
        })
    }

    pub(crate) fn catalog_bytes(&mut self) -> Result<Vec<u8>, OwnerError> {
        self.with_binding(|binding, request, work, _| {
            read_bytes(request, work, &mut binding.pins.files[9], 512 * 1024 * 1024)
        })
    }

    pub(crate) fn credential_call<T>(
        &mut self,
        read: impl FnOnce(Instant, &AtomicBool) -> Result<T, OwnerError>,
    ) -> Result<T, OwnerError> {
        self.with_binding(|_, request, work, _| {
            let (deadline, cancellation) = request.credential_context();
            work.call(request, || read(deadline, cancellation))
        })
    }

    pub(crate) fn checkpoint(&mut self) -> Result<(), OwnerError> {
        self.verify()
    }

    // Per-row polling keeps the retained registration checks and original
    // request. Full native locator verification brackets the migration child.
    pub(crate) fn migration_checkpoint(&mut self) -> Result<(), OwnerError> {
        self.with_binding(|_, request, work, _| work.arrive(request, "migration-checkpoint"))
    }

    /// Reserve one migration seam against the original invocation's allowance.
    /// Reservations grant no source mutation or new root/locator authority.
    pub(crate) fn migration_charge(
        &mut self,
        attempt: search_revision_store::LegacyRevisionObjectWork,
    ) -> Result<(), OwnerError> {
        use search_revision_store::LegacyRevisionObjectWork as Work;
        self.with_binding(|_, request, work, _| match attempt {
            Work::Observe => work.call(request, || Ok(())),
            Work::Open => {
                work.open(request)?;
                work.call(request, || Ok(()))
            }
            Work::Read(bytes) => {
                work.read(request, bytes)?;
                work.call(request, || Ok(()))
            }
            Work::Write(bytes) => {
                work.write(request, bytes)?;
                work.call(request, || Ok(()))
            }
            // Buffer capacity consumes the unchanged byte allowance too.
            Work::Allocate(bytes) | Work::Compare(bytes) => work.read(request, bytes),
            Work::AllocateLocator(bytes) => work.canonical(request, bytes),
            Work::Effect => Err(OwnerError::OwnerInvalidTransition),
        })
    }

    /// Mandatory cleanup accounting bypasses sticky checkpoints but reuses the
    /// admitted request and meter. It never admits further ordinary work.
    pub(crate) fn migration_account_native_clear(
        &mut self, actual_bytes: usize,
    ) -> Result<(), (OwnerError, bool)> {
        let first_refusal = self.owner.poisoned;
        let Some((request, work)) = self.context.as_mut() else {
            return Err((first_refusal.unwrap_or(OwnerError::OwnerInvalidTransition), true));
        };
        let accounted = work.account_native_clear(request, actual_bytes);
        if let Some(first) = first_refusal { return Err((first, accounted.is_err())); }
        match accounted {
            Ok(()) => Ok(()),
            Err(error) => {
                self.owner.poisoned = Some(error);
                Err((error, true))
            }
        }
    }

    pub(crate) fn migration_deadline(&self) -> Instant {
        let request = match &self.context {
            None => &self.owner.request,
            Some((request, _)) => request,
        };
        request.credential_context().0
    }

    /// Observe the actual child request under this retained custody, never a
    /// caller-selected identity or another operation's startup request.
    pub(crate) fn migration_inventory_owner(
        &mut self,
    ) -> Result<search_control_redb::migration::LegacyInventoryOwner, OwnerError> {
        self.verify()?;
        self.with_binding(|binding, request, _, _| {
            let (invocation, request_digest) = request.binding();
            Ok(search_control_redb::migration::LegacyInventoryOwner {
                installation: binding.record.installation_id,
                incarnation: binding.record.installation_incarnation_id,
                root: binding.record.data_root_id,
                invocation: *invocation.as_bytes(),
                request_blake3: request_digest,
                record_blake3: binding.record.record_digest,
                physical_blake3: binding.record.volume_identity_digest,
                native_limits: request.inventory_limits(),
                epoch: binding.record.epoch,
                generation: binding.record.generation,
            })
        })
    }

    pub(crate) const fn output_attempted(&self) -> bool {
        self.output_attempted
    }

    pub(crate) fn cancel_output(&self) {
        let request = match &self.context {
            None => &self.owner.request,
            Some((request, _)) => request,
        };
        request
            .credential_context()
            .1
            .store(true, Ordering::Release);
    }

    fn output_call<T>(
        &mut self,
        bytes: Option<usize>,
        output: impl FnOnce() -> std::io::Result<T>,
    ) -> Result<T, OwnerError> {
        self.with_binding(|_, request, work, attempted| {
            if let Some(bytes) = bytes {
                work.write(request, bytes)?;
            }
            work.call(request, || {
                *attempted = true;
                output().map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })
        })
    }

    pub(crate) fn write_output(
        &mut self,
        writer: &mut impl std::io::Write,
        bytes: &[u8],
    ) -> Result<usize, OwnerError> {
        self.output_call(Some(bytes.len()), || writer.write(bytes))
    }

    pub(crate) fn flush_output(
        &mut self,
        writer: &mut impl std::io::Write,
    ) -> Result<(), OwnerError> {
        self.output_call(None, || writer.flush())
    }

    pub(crate) fn retain_shutdown(mut self) -> Result<RetainedShutdown, OwnerError> {
        self.verify()?;
        let binding = self.owner.record.record_digest;
        match self.context {
            Some((request, work)) => Ok(RetainedShutdown {
                request,
                work,
                binding,
                output_attempted: self.output_attempted,
            }),
            None => Err(OwnerError::OwnerInvalidTransition),
        }
    }
}

/// Retains the exact admitted shutdown operation across child teardown.
/// Neither a renewed deadline/flag nor a fresh operation meter is accepted.
pub struct RetainedShutdown {
    request: NativeRootRequest,
    work: NativeWork,
    binding: [u8; 32],
    output_attempted: bool,
}

// Store/protector fields precede custody so every terminal/error/Drop path
// closes children before the retained native exclusions. No Drop publishes.
pub struct NativeReadStartup {
    store: crate::direct_store::ExistingReadStore,
    owner: Box<NativeRootCapability>,
}

pub struct NativeReadService {
    store: crate::direct_store::ExistingReadStore,
    owner: Box<NativeRootCapability>,
}

impl NativeReadStartup {
    pub(super) fn open(mut owner: Box<NativeRootCapability>) -> Result<Self, OwnerError> {
        let mut operation = NativeReadOperation {
            owner: &mut owner,
            context: None,
            output_attempted: false,
            registration: None,
        };
        operation.verify()?;
        let store = crate::direct_store::open_existing_read(&mut operation)?;
        operation.verify()?;
        drop(operation);
        Ok(Self { store, owner })
    }

    pub(crate) fn read(&mut self) -> crate::direct_store::ReadOnlyStore<'_> {
        crate::direct_store::ReadOnlyStore::new(
            &self.store,
            NativeReadOperation {
                owner: &mut self.owner,
                context: None,
                output_attempted: false,
                registration: None,
            },
        )
    }

    /// One invocation retains its original startup request and cumulative meter.
    /// All children close before exclusions, without a new shutdown request.
    pub(crate) fn run_one_shot<T>(
        mut self,
        execute: impl FnOnce(&crate::direct_store::ReadOnlyStore<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let child = self.read();
        let result = execute(&child);
        let checkpoint = child.native_checkpoint();
        let operation = child.into_operation();
        let attempted = operation.output_attempted;
        drop(operation);
        let Self { store, mut owner } = self;
        drop(store);
        let closing = NativeReadOperation {
            owner: &mut owner,
            context: None,
            output_attempted: attempted,
            registration: None,
        }
        .verify();
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
        } = *owner;
        let children_closed = work.arrive(&request, "one-shot-children-closed");
        drop((pins, root, installation, record, bound_root, exclusions));
        let released = work.arrive(&request, "one-shot-custody-released");
        checkpoint
            .and(closing)
            .and(children_closed)
            .and(released)
            .map_err(|error| {
                let error = if attempted {
                    OwnerError::OwnerAcquireOutcomeUnknown
                } else {
                    error
                };
                error.code().to_owned()
            })?;
        if attempted && result.is_err() {
            return Err(OwnerError::OwnerAcquireOutcomeUnknown.code().to_owned());
        }
        result
    }

    pub(crate) fn identity(&mut self) -> Result<ReadServiceIdentity, OwnerError> {
        self.owner.verify()?;
        Ok(ReadServiceIdentity {
            epoch: self.owner.record.epoch,
            installation_incarnation: search_contracts::InstallationIncarnationId::from_bytes(
                self.owner.record.installation_incarnation_id,
            )
            .to_string(),
            root: search_contracts::DataRootId::from_bytes(self.owner.record.data_root_id)
                .to_string(),
        })
    }

    /// Consumes startup exactly once, after its final complete acknowledgement.
    /// Custody persists; only a newly admitted operation can perform later work.
    pub(crate) fn finish(mut self) -> Result<NativeReadService, OwnerError> {
        // This handoff is after the complete ready acknowledgement. A late
        // refusal cannot certify that the peer observed no startup response.
        self.owner
            .verify()
            .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)?;
        let Self { store, owner } = self;
        Ok(NativeReadService { store, owner })
    }
}

impl NativeReadService {
    /// Emits a closed validation refusal without native/store admission work.
    /// It retains the exact rejected command/context and performs no root I/O.
    pub(crate) fn refuse_command(
        &mut self,
        command: &str,
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
        writer: &mut impl std::io::Write,
        code: &'static str,
    ) -> Result<(), OwnerError> {
        if let Some(error) = self.owner.poisoned {
            return Err(error);
        }
        if !matches!(
            code,
            "SERVICE_MUTATION_UNAVAILABLE" | "SERVICE_COMMAND_INVALID"
        ) {
            return Err(OwnerError::OwnerOperationConflict);
        }
        let request = self
            .owner
            .request
            .service_refusal(command, deadline, cancellation)?;
        let mut work = NativeWork::new();
        work.canonical(&request, 0)?;
        let response = NativeReadTerminal {
            request,
            work,
            output_attempted: false,
        };
        let result = response.emit_line(
            writer,
            &format!("{{\"event\":\"error\",\"error\":\"{code}\"}}"),
            ["read-refusal", "read-refusal-complete"],
        );
        if let Err(error) = result.as_ref() {
            self.owner.poisoned = Some(*error);
        }
        result
    }

    pub(crate) fn read_command(
        &mut self,
        command: &str,
        deadline: Instant,
        cancellation: Arc<AtomicBool>,
    ) -> Result<crate::direct_store::ReadOnlyStore<'_>, OwnerError> {
        let request = self
            .owner
            .request
            .service_command(command, deadline, cancellation)?;
        let mut operation = NativeReadOperation {
            owner: &mut self.owner,
            context: Some((request, NativeWork::new())),
            output_attempted: false,
            registration: None,
        };
        operation.verify()?;
        Ok(crate::direct_store::ReadOnlyStore::new(
            &self.store,
            operation,
        ))
    }

    /// Closes the store before returning custody for the caller's bounded
    /// shutdown operation. This does not produce a durable release receipt.
    pub(crate) fn close(self) -> NativeReadClosed {
        let Self { store, owner } = self;
        drop(store);
        NativeReadClosed { owner }
    }
}

pub struct NativeReadClosed {
    owner: Box<NativeRootCapability>,
}

pub struct ReadServiceIdentity {
    pub(crate) epoch: u64,
    pub(crate) installation_incarnation: String,
    pub(crate) root: String,
}

/// Bounded refusal/terminal context. This is never a lifecycle receipt.
pub struct NativeReadTerminal {
    request: NativeRootRequest,
    work: NativeWork,
    output_attempted: bool,
}

impl NativeReadTerminal {
    pub(crate) fn emit(self, writer: &mut impl std::io::Write) -> Result<(), OwnerError> {
        let line = concat!(
            "{\"event\":\"data_root_stopped\",\"clean\":true,",
            "\"read_only\":true,\"owner_state_written\":false,",
            "\"native_exclusions_released\":true}"
        );
        self.emit_line(
            writer,
            line,
            ["read-terminal-after-release", "read-terminal-complete"],
        )
    }

    fn emit_line(
        self,
        writer: &mut impl std::io::Write,
        line: &str,
        phases: [&'static str; 2],
    ) -> Result<(), OwnerError> {
        let Self {
            request,
            mut work,
            output_attempted,
        } = self;
        let mut attempted = output_attempted;
        let result = (|| {
            work.arrive(&request, phases[0])?;
            for part in [line.as_bytes(), b"\n"] {
                let mut remaining = part;
                while !remaining.is_empty() {
                    work.write(&request, remaining.len())?;
                    let count = work.call(&request, || {
                        attempted = true;
                        writer
                            .write(remaining)
                            .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
                    })?;
                    if count == 0 || count > remaining.len() {
                        return Err(OwnerError::OwnerAcquireOutcomeUnknown);
                    }
                    remaining = &remaining[count..];
                }
            }
            work.call(&request, || {
                attempted = true;
                writer
                    .flush()
                    .map_err(|_| OwnerError::OwnerAcquireOutcomeUnknown)
            })?;
            work.arrive(&request, phases[1])
        })();
        if let Err(error) = result {
            request
                .credential_context()
                .1
                .store(true, Ordering::Release);
            return Err(if attempted {
                OwnerError::OwnerAcquireOutcomeUnknown
            } else {
                error
            });
        }
        Ok(())
    }
}

impl NativeReadClosed {
    pub(crate) fn finish(
        self,
        retained: RetainedShutdown,
    ) -> Result<NativeReadTerminal, OwnerError> {
        let Self { mut owner } = self;
        let RetainedShutdown {
            request,
            work,
            binding,
            output_attempted,
        } = retained;
        let result = (|| {
            if binding != owner.record.record_digest {
                return Err(OwnerError::OwnerGuardMismatch);
            }
            let mut operation = NativeReadOperation {
                owner: &mut owner,
                context: Some((request, work)),
                output_attempted,
                registration: None,
            };
            operation.verify()?;
            let Some((request, mut work)) = operation.context else {
                return Err(OwnerError::OwnerInvalidTransition);
            };
            work.arrive(&request, "read-children-closed")?;
            drop(owner);
            work.arrive(&request, "read-custody-released")?;
            Ok(NativeReadTerminal {
                request,
                work,
                output_attempted,
            })
        })();
        result.map_err(|error| {
            if output_attempted {
                OwnerError::OwnerAcquireOutcomeUnknown
            } else {
                error
            }
        })
    }
}

#[cfg(test)]
#[path = "read_child_tests.rs"]
mod tests;

#[path = "registration_read.rs"]
pub(super) mod registration_read;
