//! Borrowed read operations over the retained native owner, without lifecycle writes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[cfg(feature = "wave4-query")]
use crate::provider_composition::{CanonicalWorkBudget, monotonic_millis};
#[cfg(feature = "wave4-query")]
use search_provider_protocol::{AdmittedProviderRequest, MonotonicMillis, RequestGuard};
#[cfg(feature = "wave4-query")]
use search_provider_protocol::request::RequestCancellation;
#[cfg(feature = "wave4-query")]
use search_contracts::RequestId;
use search_runtime_owner::OwnerError;

use super::super::native_objects::read_bytes;
use super::super::spec::LifecycleState;
use super::{NativeBindingView, NativeRootCapability, NativeRootRequest, NativeWork};

// None borrows the startup request/meter; Some owns a distinct admitted command.
type ReadContext = Option<(NativeRootRequest, NativeWork)>;

#[path = "fresh_root.rs"]
pub(super) mod fresh_root;
pub(super) use fresh_root::native_retention_object_io::{
    NativeDerivedArtifactFailure, OriginalObjectContext,
};

#[cfg(test)]
pub(super) fn source_body_attempts_for_test() -> usize {
    fresh_root::FreshRootContext::source_body_attempts_for_test()
}

pub(super) const fn direct_operation(owner: &mut NativeRootCapability) -> Result<NativeReadOperation<'_>, OwnerError> {
    if owner.pins.direct.is_none() { return Err(OwnerError::OwnerInvalidTransition); }
    // Closed caller immediately consumes this into fresh_root_context's FULL gate.
    // No source read, output or capability accessor occurs in between.
    Ok(NativeReadOperation { owner, context: None, output_attempted: false, registration: None,
        first_refusal: None })
}

/// One non-cloneable context retained by the canonical task across all read turns.
#[cfg(feature = "wave4-query")]
pub(super) struct NativeRequestReadState {
    context: ReadContext,
    monotonic_deadline: MonotonicMillis,
    owner_record: [u8; 32],
}

#[cfg(feature = "wave4-query")]
#[allow(dead_code)] // The parent composition branch owns task/facade wiring.
impl NativeRequestReadState {
    pub(super) fn new(owner: &NativeRootCapability, admitted: &AdmittedProviderRequest,
        original_budget: CanonicalWorkBudget,
    ) -> Result<Self, OwnerError> {
        Self::from_guard(owner, admitted.guard(), original_budget)
    }

    pub(super) fn from_guard(owner: &NativeRootCapability, guard: &RequestGuard,
        original_budget: CanonicalWorkBudget,
    ) -> Result<Self, OwnerError> {
        original_budget.check().map_err(|_| OwnerError::OwnerDeadlineInvalid)?;
        let monotonic_deadline = guard.deadline().ok_or(OwnerError::OwnerDeadlineInvalid)?
            .min(original_budget.deadline());
        let anchor = Instant::now();
        let observed = monotonic_millis();
        let deadline = provider_read_deadline_at(guard.deadline(), original_budget.deadline(), anchor, observed)?;
        let request = owner.request.provider_source_read(*guard.request_id(), deadline)?;
        let work = NativeWork::for_provider_source_read(guard.cancellation());
        work.check(&request)?;
        Ok(Self { context: Some((request, work)), monotonic_deadline,
            owner_record: owner.record.record_digest })
    }

    // Identity only: the caller still supplies the canonical admitted request and grant gate.
    fn check(&mut self, id: &RequestId, observer: &RequestCancellation,
        current_deadline: MonotonicMillis,
    ) -> Result<(), OwnerError> {
        let (request, work) = self.context.as_mut().ok_or(OwnerError::OwnerInvalidTransition)?;
        if request.binding().0 != *id
            || !work.provider_cancellation().is_some_and(|original| original.same_request(observer))
        { return Err(OwnerError::OwnerGuardMismatch); }
        let deadline = self.monotonic_deadline.min(current_deadline);
        request.withhold_provider_millis(self.monotonic_deadline.get() - deadline.get())?;
        self.monotonic_deadline = deadline;
        work.check(request)
    }
}

#[cfg(feature = "wave4-query")]
struct RestoreOnDrop<T, F: FnMut(&mut T)> { value: T, restore: F }
#[cfg(feature = "wave4-query")]
impl<T, F: FnMut(&mut T)> Drop for RestoreOnDrop<T, F> {
    fn drop(&mut self) { (self.restore)(&mut self.value); }
}

/// Loans the task's same meter synchronously; no operation or grant escapes.
/// Call only inside the canonical standalone-read gate, using the full fresh-root checks.
#[cfg(feature = "wave4-query")]
#[allow(dead_code)]
pub(super) fn with_direct_read_operation<T>(
    owner: &mut NativeRootCapability,
    admitted: &AdmittedProviderRequest,
    state: &mut NativeRequestReadState,
    current_budget: CanonicalWorkBudget,
    callback: impl for<'operation, 'root> FnOnce(&'operation mut NativeReadOperation<'root>) -> T,
) -> Result<T, OwnerError> {
    with_guarded_read_operation(owner, admitted.guard(), state, current_budget, callback)
}

/// The V1 and V2 admissions share the same opaque session guard and native meter.
/// Only the live issuer/source boundary may call this operation.
#[cfg(feature = "wave4-query")]
pub(super) fn with_guarded_read_operation<T>(
    owner: &mut NativeRootCapability,
    guard: &RequestGuard,
    state: &mut NativeRequestReadState,
    current_budget: CanonicalWorkBudget,
    callback: impl for<'operation, 'root> FnOnce(&'operation mut NativeReadOperation<'root>) -> T,
) -> Result<T, OwnerError> {
    if owner.pins.direct.is_none()
        || !owner.mutation
        || owner.record.lifecycle != LifecycleState::Active
    {
        return Err(OwnerError::OwnerInvalidTransition);
    }
    if let Some(error) = owner.poisoned {
        return Err(error);
    }
    if state.owner_record != owner.record.record_digest { return Err(OwnerError::OwnerGuardMismatch); }
    current_budget.check().map_err(|_| OwnerError::OwnerDeadlineInvalid)?;
    state.check(guard.request_id(), &guard.cancellation(), current_budget.deadline())?;
    let context = state.context.take().ok_or(OwnerError::OwnerInvalidTransition)?;
    let mut frame = RestoreOnDrop {
        value: NativeReadOperation { owner, context: Some(context), output_attempted: false,
            registration: None, first_refusal: None },
        restore: |operation: &mut NativeReadOperation<'_>| { state.context = operation.context.take(); },
    };
    let value = callback(&mut frame.value);
    current_budget.check().map_err(|_| OwnerError::OwnerDeadlineInvalid)?;
    frame.value.with_binding(|_, request, work, _| work.check(request))?;
    Ok(value)
}

#[cfg(feature = "wave4-query")]
fn provider_read_deadline_at(
    request_deadline: Option<MonotonicMillis>,
    work_deadline: MonotonicMillis,
    anchor: Instant,
    observed: MonotonicMillis,
) -> Result<Instant, OwnerError> {
    let deadline = request_deadline
        .ok_or(OwnerError::OwnerDeadlineInvalid)?
        .min(work_deadline);
    // MonotonicMillis truncates fractional milliseconds; reserve one full tick.
    let remaining = deadline
        .get()
        .checked_sub(observed.get())
        .and_then(|millis| millis.checked_sub(1))
        .filter(|millis| *millis > 0)
        .ok_or(OwnerError::OwnerDeadlineInvalid)?;
    anchor
        .checked_add(std::time::Duration::from_millis(remaining))
        .ok_or(OwnerError::OwnerDeadlineInvalid)
}

const fn provider_read_local_refusal(error: OwnerError) -> bool {
    matches!(error, OwnerError::OwnerCancelledBeforeMutation
        | OwnerError::OwnerDeadlineInvalid | OwnerError::ContractExhausted)
}

#[derive(Clone, Copy)]
enum RegistrationCheck {
    Both,
    Before,
    After,
    None,
}

/// No constructor, clone, lifecycle transition or unbound path admission.
pub struct NativeReadOperation<'owner> {
    owner: &'owner mut NativeRootCapability,
    context: ReadContext,
    output_attempted: bool,
    registration: Option<registration_read::RegistrationPin>,
    // Preserve the original cause before possible output/effects require the
    // outward unknown-outcome classification. This is invocation-local only.
    first_refusal: Option<OwnerError>,
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
        self.with_registration_check(RegistrationCheck::Both, operation)
    }

    fn with_registration_check<T>(
        &mut self,
        check_registration: RegistrationCheck,
        operation: impl FnOnce(
            NativeBindingView<'_>,
            &NativeRootRequest,
            &mut NativeWork,
            &mut bool,
        ) -> Result<T, OwnerError>,
    ) -> Result<T, OwnerError> {
        let provider_source_read = match &self.context {
            Some((_, work)) => work.is_provider_source_read(),
            None => false,
        };
        let direct = self.owner.pins.direct.is_some()
            && self.registration.is_none()
            && self.owner.mutation
            && self.owner.record.lifecycle == LifecycleState::Active
            && ((self.context.is_none() && !provider_source_read)
                || (self.context.is_some() && provider_source_read));
        if provider_source_read && !direct {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        if !direct && (self.owner.mutation || self.owner.record.lifecycle != LifecycleState::Released) {
            return Err(OwnerError::OwnerInvalidTransition);
        }
        if let Some(error) = self.owner.poisoned {
            return Err(error);
        }
        let NativeRootCapability {
            pins,
            storage_acl,
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
        let registration = self.registration.as_ref();
        let result = work.check(request).and_then(|()| {
            if let Some(pin) = registration.filter(|_| {
                matches!(check_registration, RegistrationCheck::Both | RegistrationCheck::Before)
            }) {
                pin.check(request, work)?;
            }
            let result = operation(
                NativeBindingView {
                    pins,
                    storage_acl: storage_acl.as_ref(),
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
                if let Some(pin) = registration.filter(|_| {
                    matches!(check_registration, RegistrationCheck::Both | RegistrationCheck::After)
                }) {
                    pin.check(request, work)?;
                }
                Ok(value)
            })
        });
        let checkpoint = work.check(request);
        let result = result
            .and_then(|value| checkpoint.map(|()| value))
            .map_err(|error| {
                self.first_refusal.get_or_insert(error);
                if self.output_attempted {
                    OwnerError::OwnerAcquireOutcomeUnknown
                } else {
                    error
                }
            });
        if let Err(error) = result.as_ref()
            && (!provider_source_read || !provider_read_local_refusal(*error))
        {
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
        if self.owner.pins.direct.is_some() { return Err(OwnerError::OwnerInvalidTransition); }
        self.with_binding(|binding, request, work, _| {
            read_bytes(request, work, &mut binding.pins.files[8], 256)
        })
    }

    pub(crate) fn catalog_bytes(&mut self) -> Result<Vec<u8>, OwnerError> {
        if self.owner.pins.direct.is_some() { return Err(OwnerError::OwnerInvalidTransition); }
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
        // The arrive body only polls/accounts. This single post-check still
        // observes registration/pending state after the preceding external I/O.
        self.with_registration_check(RegistrationCheck::After, |_, request, work, _| {
            work.arrive(request, "migration-checkpoint")
        })
    }

    /// One internal ABI attempt inside a closed borrowed-FD metadata observer.
    /// Its caller retains full registration checkpoints around the whole
    /// observer. Path opens, body I/O and effects must use `migration_charge`.
    pub(crate) fn migration_held_observer_dispatch(&mut self) -> Result<(), OwnerError> {
        self.with_registration_check(RegistrationCheck::None, |_, request, work, _| {
            #[cfg(test)]
            {
                work.call(request, || Ok(()))?;
                work.arrive(request, "migration-held-observer-dispatch")
            }
            #[cfg(not(test))]
            work.call(request, || Ok(()))
        })
    }

    /// Reserve one migration seam against the original invocation's allowance.
    /// Reservations grant no source mutation or new root/locator authority.
    pub(crate) fn migration_charge(
        &mut self,
        attempt: search_revision_store::LegacyRevisionObjectWork,
    ) -> Result<(), OwnerError> {
        use search_revision_store::LegacyRevisionObjectWork as Work;
        // Pure accounting cannot observe or mutate an external object. Keep
        // every original-context/poison/output guard, and leave registration
        // observation at checkpoints and actual I/O seams. An I/O reservation
        // returns before dispatch; its pre-check is paired with the owner's
        // post-call checkpoint, not a second check around the reservation NOOP.
        let check_registration = match attempt {
            Work::Allocate(_) | Work::Compare(_) | Work::AllocateLocator(_) => RegistrationCheck::None,
            Work::Observe | Work::Open | Work::Read(_) | Work::Write(_) => RegistrationCheck::Before,
            Work::Effect => RegistrationCheck::Both,
        };
        self.with_registration_check(check_registration, |_, request, work, _| match attempt {
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
    pub(crate) const fn migration_account_native_clear(
        &mut self, actual_bytes: usize,
    ) -> Result<(), (OwnerError, bool)> {
        let first_refusal = self.owner.poisoned;
        let (request, work) = match self.context.as_mut() {
            None => (&self.owner.request, &mut self.owner.work),
            Some((request, work)) => (&*request, work),
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

    /// Original retained root identity for a private read-scope containment join.
    /// Observation metadata is not a current-policy or source-serving grant.
    pub(crate) fn migration_root_observation(
        &mut self,
    ) -> Result<eliot_searchd::native_file::Observation, OwnerError> {
        self.with_binding(|binding, request, work, _| {
            work.call(request, || {
                eliot_searchd::native_file::observe(binding.bound_root)
                    .map_err(|_| OwnerError::OwnerGuardMismatch)
            })
        })
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

#[cfg(all(test, feature = "wave4-query"))]
mod provider_read_deadline_tests {
    use super::*;
    use search_provider_protocol::{DEFAULT_PROTOCOL_LIMITS, RequestGuard};
    use std::time::{Duration, Instant};

    fn guard() -> RequestGuard {
        RequestGuard::new(RequestId::from_bytes([0x71; 16]), 1, MonotonicMillis::new(1),
            Some(10_000), DEFAULT_PROTOCOL_LIMITS).unwrap()
    }

    fn state(guard: &RequestGuard, deadline: Instant) -> NativeRequestReadState {
        NativeRequestReadState {
            context: Some((NativeRootRequest::provider_read_fixture(*guard.request_id(), deadline),
                NativeWork::for_provider_source_read(guard.cancellation()))),
            monotonic_deadline: MonotonicMillis::new(10_000), owner_record: [0; 32],
        }
    }

    fn loan<T>(state: &mut NativeRequestReadState,
        callback: impl FnOnce(&NativeRootRequest, &mut NativeWork) -> T,
    ) -> T {
        let context = state.context.take();
        let mut frame = RestoreOnDrop { value: context,
            restore: |context: &mut ReadContext| { state.context = context.take(); } };
        let (request, work) = frame.value.as_mut().unwrap();
        callback(request, work)
    }

    #[test]
    fn provider_read_context_meter_survives_success_error_and_multiple_loans() {
        let guard = guard();
        let mut state = state(&guard, Instant::now() + Duration::from_secs(30));
        for fail in [true, false] {
            let result = loan(&mut state, |request, work| {
                work.read(request, 4).unwrap();
                work.open(request).unwrap();
                work.call(request, || Ok(())).unwrap();
                if fail { Err("consumer refusal") } else { Ok(()) }
            });
            assert_eq!(result.is_err(), fail);
        }
        loan(&mut state, |request, work| {
            assert!(matches!(work.read(request, 1), Err(OwnerError::ContractExhausted)));
            assert!(matches!(work.open(request), Err(OwnerError::ContractExhausted)));
            assert!(matches!(work.call(request, || Ok(())), Err(OwnerError::ContractExhausted)));
        });
    }

    #[test]
    fn provider_read_context_meter_is_restored_during_unwind() {
        let guard = guard();
        let mut state = state(&guard, Instant::now() + Duration::from_secs(30));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            loan(&mut state, |request, work| {
                work.read(request, 1).unwrap();
                panic!("pure callback unwind");
            });
        }));
        assert!(result.is_err());
        loan(&mut state, |request, work| {
            assert!(matches!(work.read(request, 8), Err(OwnerError::ContractExhausted)));
            work.read(request, 7).unwrap();
        });
    }

    #[test]
    fn provider_read_context_refuses_equal_id_foreign_observer_and_never_renews_deadline() {
        let guard = guard();
        let foreign = self::guard();
        let original_deadline = Instant::now() + Duration::from_secs(30);
        let mut state = state(&guard, original_deadline);
        assert!(matches!(state.check(guard.request_id(), &foreign.cancellation(),
            MonotonicMillis::new(8_000)), Err(OwnerError::OwnerGuardMismatch)));
        let observer = guard.cancellation();
        state.check(guard.request_id(), &observer, MonotonicMillis::new(9_000)).unwrap();
        state.check(guard.request_id(), &observer, MonotonicMillis::new(9_500)).unwrap();
        assert_eq!(state.context.as_ref().unwrap().0.credential_context().0,
            original_deadline - Duration::from_millis(1_000));
        assert!(matches!(state.check(&RequestId::from_bytes([0x72; 16]), &observer,
            MonotonicMillis::new(9_000)), Err(OwnerError::OwnerGuardMismatch)));
    }

    #[test]
    fn provider_read_context_retains_cancel_and_expiry_across_loans() {
        let mut guard = guard();
        let mut active = state(&guard, Instant::now() + Duration::from_secs(30));
        loan(&mut active, |request, work| work.read(request, 1).unwrap());
        guard.mark_cancelled();
        assert!(matches!(active.check(guard.request_id(), &guard.cancellation(),
            MonotonicMillis::new(10_000)), Err(OwnerError::OwnerCancelledBeforeMutation)));
        let guard = self::guard();
        let mut expired = state(&guard, Instant::now());
        assert!(matches!(expired.check(guard.request_id(), &guard.cancellation(),
            MonotonicMillis::new(10_000)), Err(OwnerError::OwnerDeadlineInvalid)));
    }

    #[test]
    fn provider_read_context_local_limits_do_not_mask_corruption_poisoning() {
        for error in [OwnerError::OwnerCancelledBeforeMutation, OwnerError::OwnerDeadlineInvalid,
            OwnerError::ContractExhausted] { assert!(provider_read_local_refusal(error)); }
        assert!(!provider_read_local_refusal(OwnerError::OwnerGuardMismatch));
    }

    #[test]
    fn provider_read_context_deadline_uses_earliest_guard_or_budget() {
        let anchor = Instant::now();
        let actual = provider_read_deadline_at(
            Some(MonotonicMillis::new(900)),
            MonotonicMillis::new(700),
            anchor,
            MonotonicMillis::new(500),
        )
        .unwrap();
        assert_eq!(actual, anchor + Duration::from_millis(199));
    }

    #[test]
    fn provider_read_context_deadline_refuses_missing_expired_or_one_millisecond_guard() {
        let anchor = Instant::now();
        let budget = MonotonicMillis::new(900);
        let now = MonotonicMillis::new(500);
        assert!(provider_read_deadline_at(None, budget, anchor, now).is_err());
        assert!(provider_read_deadline_at(
            Some(MonotonicMillis::new(500)),
            budget,
            anchor,
            now,
        )
        .is_err());
        assert!(provider_read_deadline_at(
            Some(MonotonicMillis::new(501)),
            budget,
            anchor,
            now,
        )
        .is_err());
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
            registration: None, first_refusal: None,
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
                registration: None, first_refusal: None,
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
            registration: None, first_refusal: None,
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
            registration: None, first_refusal: None,
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
                registration: None, first_refusal: None,
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

#[cfg(all(test, windows))]
impl NativeReadOperation<'_> {
    /// Retain the original meter and observer when adding a test adversary.
    pub(crate) fn chain_migration_phase_hook_for_test(
        &mut self,
        mut hook: Box<dyn FnMut(&'static str)>,
    ) {
        let work = match self.context.as_mut() {
            None => &mut self.owner.work,
            Some((_, work)) => work,
        };
        let mut original = work.phase_hook.take();
        work.phase_hook = Some(Box::new(move |phase| {
            if let Some(observer) = original.as_mut() {
                observer(phase);
            }
            hook(phase);
        }));
    }
}
