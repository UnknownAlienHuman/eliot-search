//! Explicit native authority and cooperative recipe ownership; no allow-all host.

use search_access::{AccessCheckpoint, AccessError, RequestSecurityFence};
use search_contracts::{
    BoundedBytes, MAX_FRAME_BYTES, ProviderBodyV1, RecipeBodyV1, RecipeIdV1,
    RecipeResultV1, RequestBody, ResultBody, SearchCandidateSet, SourceIdentity,
    SourceMembershipId, SourceRevisionRef,
};
use search_result_projector::{
    CandidateProjectionInput, CandidateSetEnvelope, ProjectionBudget, ProjectionError,
    project_candidate_set,
};
use search_provider_protocol::{
    AdmittedProviderRequest, BindingContext, FRAME_PREFIX_BYTES, MonotonicMillis,
    PreparedProviderDeliveryError, ProtocolLimits, ProviderEventPreparationV1,
    RequestGuard, ServerEnvelopeCodec, TerminalKind,
};
use std::task::Poll;

use crate::access_composition::{
    AuthoritativeGrantPolicy, NativeSecurityDomain, StandaloneGrantTemplate,
    StandalonePreRetrievalAdmission, VerifiedStandaloneGrant,
};
use super::{CanonicalServingError, CanonicalTcpConnection, CanonicalTcpError, monotonic_millis};

/// One owner-issued work turn with separate lifetime and scheduling bounds.
///
/// The hard deadline is the original request/cleanup deadline, tightened by
/// current authority. The earlier yield point only asks the task to return
/// Pending. Yielding does not expire a request, release resources or renew its
/// lifetime. Blocking backends must use an already-owned bounded worker and
/// poll its result; the cooperative quantum is not a preemption guarantee.
#[derive(Clone, Copy, Debug)]
pub struct CanonicalWorkBudget {
    started: MonotonicMillis,
    deadline: MonotonicMillis,
    yield_at: MonotonicMillis,
}

impl CanonicalWorkBudget {
    pub(super) fn new(
        deadline: MonotonicMillis,
        quantum_ms: u64,
    ) -> Result<Self, CanonicalServingError> {
        if !(1..=25).contains(&quantum_ms) {
            return Err(CanonicalServingError::InvalidConfiguration);
        }
        let started = monotonic_millis();
        let yield_at = started.get().checked_add(quantum_ms)
            .map(MonotonicMillis::new).unwrap_or(deadline).min(deadline);
        let budget = Self { started, deadline, yield_at };
        budget.check_at(started)?;
        Ok(budget)
    }

    /// Absolute lifetime ceiling for backend operations in this turn.
    /// This is not the scheduling quantum and cannot be extended by the task.
    #[must_use]
    pub const fn deadline(self) -> MonotonicMillis { self.deadline }

    /// Check the hard request/authority or cleanup lifetime before each backend
    /// operation and after returning from it. A completed work slice is not an
    /// error; use should_yield between bounded batches to return Pending.
    pub fn check(self) -> Result<(), CanonicalServingError> {
        self.check_at(monotonic_millis())
    }

    /// Whether the current turn should yield without starting another batch.
    /// Hard expiry or clock regression remains an error, even at the yield point.
    /// Pending must retain task state and all unfinished cleanup obligations.
    pub fn should_yield(self) -> Result<bool, CanonicalServingError> {
        let now = monotonic_millis();
        self.check_at(now)?;
        Ok(now >= self.yield_at)
    }

    // Tightening authority never widens either bound or changes the origin.
    // Only the serving owner can issue the next task slice; tasks cannot renew it.
    pub(super) fn bounded_by(self, valid_until: MonotonicMillis) -> Self {
        Self {
            started: self.started,
            deadline: self.deadline.min(valid_until),
            yield_at: self.yield_at.min(valid_until),
        }
    }

    fn check_at(self, now: MonotonicMillis) -> Result<(), CanonicalServingError> {
        if now < self.started || now >= self.deadline {
            return Err(CanonicalServingError::DeadlineExpired);
        }
        Ok(())
    }
}

/// Borrowed live authority held by the native host for one work/output turn.
/// None of these values may be reconstructed from client claims or capabilities.
pub struct CanonicalServingAuthority<'authority, 'grant> {
    /// Current active standalone policy borrowed from the held native lock.
    /// The standalone adapter requires Some; other peer roles may use None.
    /// Never supply a detached policy snapshot or reconstruct one from claims.
    pub standalone_policy: Option<&'authority AuthoritativeGrantPolicy>,
    /// Exact issuer-ledger grant verification retained for this authority turn.
    ///
    /// Only the standalone grant adapter supplies `Some`, after current policy,
    /// binding, request and lifetime validation inside the same host lock. Other
    /// roles use `None`; client claims or a transport MAC cannot construct it.
    pub(crate) standalone_grant: Option<VerifiedStandaloneGrant<'grant>>,
    /// Canonical pre-retrieval compiler borrowed from the same held authority
    /// lock as the policy, access snapshot, source resolver and native domain.
    ///
    /// It accepts only the exact admitted request plus the non-clonable issuer
    /// verification above. Tasks cannot obtain the template separately and must
    /// use this callback before source reads, provider dispatch, IDF or counts.
    pub(crate) standalone_admission: Option<
        &'authority mut dyn FnMut(
            &RequestBody,
            &StandaloneGrantTemplate,
        ) -> Result<StandalonePreRetrievalAdmission, AccessError>,
    >,
    /// Exact active C03 membership-to-source bindings captured with the current
    /// access snapshot under this same authority lock. Missing bindings deny
    /// source reads; request claims and object locators cannot supply them.
    pub(crate) standalone_sources:
        Option<&'authority std::collections::BTreeMap<SourceMembershipId, SourceIdentity>>,
    /// Restored native domain under the actual serving/mutation lock.
    pub domain: &'authority NativeSecurityDomain,
    /// Complete authoritative influence population, not only displayed hits.
    pub fence: &'authority RequestSecurityFence,
    /// Conservative monotonic expiry of the current grant/binding authorization.
    pub valid_until: MonotonicMillis,
    /// Current scope/disclosure/byte-limit validation of the actual output body.
    /// This callback must not write output or release the host's authority lock.
    pub validate_output: &'authority mut dyn FnMut(
        &RequestBody,
        &ProviderBodyV1,
    ) -> Result<(), AccessError>,
}

/// Native grant/plan owner injected into the serving loop. There is deliberately
/// no default implementation and no token-file or legacy-catalog fallback.
pub trait CanonicalRecipeHost {
    /// Task resources remain owned through cancellation and output completion.
    type Task: CanonicalRecipeTask;

    /// Prepare bounded local task state, without source/provider execution or
    /// publishing handles. Partial preparation must clean itself up on error.
    /// The actual source work starts only inside `with_current_authority`.
    /// Keep preparation bounded. If it consumes the scheduling quantum, the
    /// owner retains the returned task and schedules its first poll next turn.
    fn prepare(
        &mut self,
        binding: &BindingContext,
        request: &AdmittedProviderRequest,
        budget: CanonicalWorkBudget,
    ) -> Result<Self::Task, CanonicalServingError>;

    /// Revalidate server-issued grant, binding, scope and plan; keep the native
    /// authority/domain lock held across `operation`, including socket output.
    /// Supply the complete influence fence and a conservative expiry, never a
    /// cached permit. The operation performs the concrete native checkpoints.
    /// For standalone serving, also borrow the actual active binding policy and
    /// canonical admission compiler into the same turn; grant checks run inside
    /// this same lock scope. `new_standalone` supplies issuer verification
    /// without re-entering the policy source. Other roles retain their own grant
    /// and admission checks.
    ///
    /// The generic return and FnOnce callback cannot be replaced with a canned
    /// success. An error after invoking it still closes the serving connection.
    fn with_current_authority<R>(
        &mut self,
        binding: &BindingContext,
        request: &mut AdmittedProviderRequest,
        task: &mut Self::Task,
        operation: impl FnOnce(
            CanonicalServingAuthority<'_, '_>,
            &mut AdmittedProviderRequest,
            &mut Self::Task,
        ) -> Result<R, CanonicalServingError>,
    ) -> Result<R, CanonicalServingError>;
}

/// One cooperative executor with mandatory cancellation ownership.
/// Implementations call existing recipe/source owners, not a second query engine.
pub trait CanonicalRecipeTask {
    /// Perform at most one bounded work slice and emit at most one event.
    /// Ready requires a successfully emitted terminal; Pending permits either
    /// no output or one progress event. Standalone tasks must obtain and retain
    /// `output.admit_standalone_query()` before their first source read, provider
    /// dispatch, IDF, count or facet operation; the raw grant template is not
    /// exposed. Use `output.emit` inside existing prepared-handle/continuation
    /// delivery callbacks so their rollback and commit lifetimes span the actual
    /// write. Never dispatch detached work. Check budget.should_yield() between
    /// batches and return Pending when it requests a yield. Do not translate that
    /// yield into DeadlineExpired. budget.check() enforces the separate hard
    /// request/authority lifetime.
    fn poll(
        &mut self,
        output: &mut CanonicalWorkOutput<'_, '_, '_>,
        budget: CanonicalWorkBudget,
    ) -> Result<Poll<()>, CanonicalServingError>;

    /// Finish cancellation/resource cleanup without reading or emitting source
    /// content. Ready proves this task stopped and all request-local cleanup
    /// completed; it is not a claim that external mutations rolled back.
    /// Failure/Pending must preserve ownership and retryable cleanup obligations.
    /// Repeated polls after completed cleanup must remain safe and idempotent.
    /// Use should_yield for cooperative Pending and check for hard expiry; a
    /// slice boundary alone must not lose or fail unfinished cleanup.
    fn poll_cancel(&mut self, budget: CanonicalWorkBudget) -> Result<Poll<()>, CanonicalServingError>;

    /// Idempotent, infallible, non-panicking, nonblocking emergency signal. Do not discard
    /// unfinished cleanup. Task Drop must retain durable obligations with their
    /// real owner and must not detach workers; fallible cleanup uses poll_cancel.
    fn abort(&mut self);
}

/// Single-event output capability valid only inside the live host/domain borrow.
/// Executors never receive the socket or mutable protocol session.
pub struct CanonicalWorkOutput<'io, 'authority, 'grant> {
    pub(super) transport: &'io mut CanonicalTcpConnection,
    pub(super) request: &'io mut AdmittedProviderRequest,
    pub(super) authority: CanonicalServingAuthority<'authority, 'grant>,
    pub(super) budget: CanonicalWorkBudget,
    pub(super) emitted_terminal: Option<bool>,
    pub(super) failed: bool,
}

impl CanonicalWorkOutput<'_, '_, '_> {
    /// Exact admitted request, still subject to the host's live authorization.
    #[must_use]
    pub fn request(&self) -> &RequestBody { self.request.body() }

    /// Original guard: its cancellation and deadline are never renewed here.
    #[must_use]
    pub fn guard(&self) -> &RequestGuard { self.request.guard() }

    /// Original request deadline tightened by this turn's current authority.
    /// It is converted from the same process clock, never from a new timeout.
    pub(crate) fn work_deadline_instant(&self) -> Result<std::time::Instant, CanonicalServingError> {
        self.budget.check()?;
        super::super::super::super::router::monotonic_deadline_instant(self.budget.deadline())
            .ok_or(CanonicalServingError::DeadlineExpired)
    }

    /// Sequence to put in the next progress/result event.
    #[must_use]
    pub fn next_event_sequence(&self) -> Option<u64> { self.request.next_event_sequence() }

    /// Compile the exact standalone request under this turn's current policy,
    /// source/access snapshots and native-domain lock.
    ///
    /// The returned admission is not reusable authority: its request fence and
    /// safe legs must stay with the task and every later dispatch/readback/output
    /// boundary still requires the corresponding live checkpoint. The callback
    /// is intentionally one turn-scoped borrow; no grant template, resolver or
    /// authoritative snapshot escapes separately.
    pub(crate) fn admit_standalone_query(
        &mut self,
    ) -> Result<StandalonePreRetrievalAdmission, CanonicalServingError> {
        self.budget.check()?;
        let (issued, compiler) = match (
            self.authority.standalone_grant.as_ref(),
            self.authority.standalone_admission.as_mut(),
        ) {
            (Some(issued), Some(compiler)) => (issued, compiler),
            _ => return Err(CanonicalServingError::InvalidConfiguration),
        };
        let admission = compiler(self.request.body(), issued.template())
            .map_err(CanonicalServingError::Access)?;
        self.budget.check()?;
        Ok(admission)
    }

    /// Read an admitted membership under this turn's issuer and native authority.
    ///
    /// The callback must reopen the exact
    /// retained revision through its source owner; neither a handle nor storage
    /// custody substitutes for the issuer's source-read permission.
    pub(crate) fn with_standalone_source_read<T>(
        &mut self,
        admission: &StandalonePreRetrievalAdmission,
        membership: SourceMembershipId,
        expected: &SourceRevisionRef,
        read: impl FnOnce(
            &SourceRevisionRef, &AdmittedProviderRequest, CanonicalWorkBudget,
            &NativeSecurityDomain, &RequestSecurityFence,
        ) -> Result<T, CanonicalServingError>,
    ) -> Result<T, CanonicalServingError> {
        if self.failed || self.emitted_terminal.is_some() {
            return Err(CanonicalServingError::InvalidCompletion);
        }
        self.failed = true;
        self.budget.check()?;
        if self.request.guard().is_cancelled() {
            return Err(CanonicalServingError::GrantRefused(
                crate::access_composition::GrantUseError::RequestInactive,
            ));
        }
        let issued = self.authority.standalone_grant.as_ref()
            .ok_or(CanonicalServingError::InvalidConfiguration)?;
        issued.verify_current_claims(&self.request.body().grant)
            .map_err(CanonicalServingError::GrantRefused)?;
        if !issued.template().source_read_permission {
            return Err(CanonicalServingError::GrantRefused(
                crate::access_composition::GrantUseError::RequestDenied,
            ));
        }
        // Recompile under this turn's current owner instead of accepting a
        // retained admission as a reusable permit after policy/source drift.
        let current = self.admit_standalone_query()?;
        if current.grant_fence != admission.grant_fence
            || current.scope != admission.scope
            || current.request_fence != admission.request_fence
            || current.legs != admission.legs
            || current.predicates != admission.predicates
            || current.profile_digest != admission.profile_digest
            || current.request_fence != *self.authority.fence
            || !current.scope.memberships.contains_key(&membership)
        {
            return Err(CanonicalServingError::Access(AccessError::SnapshotStale));
        }
        let result = with_bound_source(self.authority.standalone_sources, membership, expected,
            |expected| self.authority.domain.with_live_checkpoint(
                &current.request_fence, AccessCheckpoint::BeforeSourceReadback, |_| read(
                    expected, self.request, self.budget, self.authority.domain, &current.request_fence,
                ),
            ).map_err(CanonicalServingError::Access))?;
        // Check even after a source error. Preserve that first error, but never
        // return successful bytes if cancellation, expiry or live deny won.
        let after = self.budget.check().and_then(|()| {
            if self.request.guard().is_cancelled() {
                return Err(CanonicalServingError::GrantRefused(
                    crate::access_composition::GrantUseError::RequestInactive,
                ));
            }
            self.authority.domain.with_live_checkpoint(
                &current.request_fence, AccessCheckpoint::BeforeSourceReadback, |_| (),
            ).map_err(CanonicalServingError::Access)
        });
        let value = result?;
        after?;
        self.failed = false;
        Ok(value)
    }

    /// Validate actual output and send it through the canonical transport.
    /// The native checkpoint and the host's lock span the complete frame/MAC
    /// write. Each write uses min(request deadline, current authority expiry).
    /// A swallowed error or second emission still poisons the owner turn.
    /// Once entered, output is governed by the hard lifetime, not the yield
    /// point. Do not roll back a prepared delivery merely because its work
    /// quantum elapsed; actual output failure still closes the connection.
    pub fn emit(
        &mut self,
        body: ProviderBodyV1,
        terminal: Option<TerminalKind>,
    ) -> Result<(), CanonicalServingError> {
        if self.failed || self.emitted_terminal.is_some() {
            self.failed = true;
            return Err(CanonicalServingError::InvalidCompletion);
        }
        self.failed = true;
        self.budget.check()?;
        (self.authority.validate_output)(self.request.body(), &body)
            .map_err(CanonicalServingError::Access)?;
        self.budget.check()?;
        let domain = self.authority.domain;
        let fence = self.authority.fence;
        let checkpoint = match &body {
            ProviderBodyV1::Result(value) if value.result.recipe_id() == RecipeIdV1::ExecuteExactScan => {
                AccessCheckpoint::BeforeExactEmission
            }
            _ => AccessCheckpoint::BeforeResultEmission,
        };
        domain.with_live_checkpoint(fence, checkpoint, |_| {
            self.transport.deliver_event_before(
                self.request, body, terminal, Some(self.authority.valid_until),
            ).map_err(CanonicalServingError::Transport)
        }).map_err(CanonicalServingError::Access)??;
        self.emitted_terminal = Some(terminal.is_some());
        self.failed = false;
        Ok(())
    }

    /// Project and emit a C26 candidate set as its exact prepared provider frame.
    ///
    /// The caller supplies C24-validated source-backed inputs with already
    /// minted public handles. Only the C00 `Locate` and `FindText` result
    /// families can carry `SearchCandidateSet`; all other request recipes fail
    /// closed. Projection, full-frame encoding, typed output validation and
    /// prepared protocol delivery remain inside the current C18 live checkpoint.
    pub fn emit_projected_candidate_set(
        &mut self,
        envelope: CandidateSetEnvelope,
        inputs: Vec<CandidateProjectionInput>,
        projection_budget: ProjectionBudget,
        terminal: TerminalKind,
    ) -> Result<(), CanonicalServingError> {
        if self.failed || self.emitted_terminal.is_some() {
            self.failed = true;
            return Err(CanonicalServingError::InvalidCompletion);
        }
        self.failed = true;
        self.budget.check()?;
        if !matches!(terminal, TerminalKind::Success | TerminalKind::Partial) {
            return Err(CanonicalServingError::InvalidCompletion);
        }

        let request_id = *self.request.guard().request_id();
        let request_body = self.request.body();
        if envelope.request_id != request_id
            || envelope.request_id != request_body.recipe_request.request_id
        {
            return Err(CanonicalServingError::InvalidCompletion);
        }
        let recipe = match (
            request_body.recipe_request.recipe,
            &request_body.recipe_request.body,
        ) {
            (RecipeIdV1::Locate, RecipeBodyV1::Locate(_)) => RecipeIdV1::Locate,
            (RecipeIdV1::FindText, RecipeBodyV1::FindText(_)) => RecipeIdV1::FindText,
            _ => return Err(CanonicalServingError::InvalidCompletion),
        };

        let domain = self.authority.domain;
        let fence = self.authority.fence;
        let valid_until = self.authority.valid_until;
        let budget = self.budget;
        let request_snapshot = self.request.body().clone();
        let request = &mut self.request;
        let transport = &mut self.transport;
        let validate_output = &mut self.authority.validate_output;
        domain.with_live_checkpoint(
            fence,
            AccessCheckpoint::BeforeResultEmission,
            |_| {
                budget.check()?;
                let delivery = transport.deliver_event_prepared_before(
                    request,
                    Some(terminal),
                    Some(valid_until),
                    |context: &ProviderEventPreparationV1| {
                        budget.check()?;
                        let mut encoded_body = None;
                        let projected = project_candidate_set(
                            envelope,
                            inputs,
                            projection_budget,
                            |candidate_set, byte_ceiling| {
                                let body = candidate_set_body(
                                    recipe,
                                    context.event_sequence(),
                                    candidate_set.clone(),
                                )
                                .ok_or(ProjectionError::OutputEncodingFailed)?;
                                let frame_limits = frame_limits_with_ceiling(
                                    context.limits(),
                                    byte_ceiling,
                                )?;
                                let provider_envelope = context.envelope(body.clone());
                                let frame = ServerEnvelopeCodec::encode(
                                    &provider_envelope,
                                    frame_limits,
                                    context.supported_versions(),
                                )
                                .map_err(|error| match error {
                                    search_provider_protocol::ProtocolError::FrameTooLarge => {
                                        ProjectionError::SerializedOutputBudgetExceeded
                                    }
                                    _ => ProjectionError::OutputEncodingFailed,
                                })?;
                                if frame.len() > byte_ceiling {
                                    return Err(ProjectionError::SerializedOutputBudgetExceeded);
                                }
                                encoded_body = Some(body);
                                Ok(frame)
                            },
                        )
                        .map_err(|error| CanonicalServingError::Backend(error.code()))?;

                        budget.check()?;
                        let body = candidate_set_body(
                            recipe,
                            context.event_sequence(),
                            projected.result().clone(),
                        )
                        .ok_or(CanonicalServingError::InvalidCompletion)?;
                        if encoded_body.as_ref() != Some(&body) {
                            return Err(CanonicalServingError::InvalidCompletion);
                        }
                        validate_output(&request_snapshot, &body)
                            .map_err(CanonicalServingError::Access)?;
                        budget.check()?;
                        let frame = BoundedBytes::<MAX_FRAME_BYTES>::new(
                            projected.serialized_output_bytes().to_vec(),
                        )
                        .map_err(|_| {
                            CanonicalServingError::Backend(
                                ProjectionError::SerializedOutputBudgetExceeded.code(),
                            )
                        })?;
                        Ok((body, frame))
                    },
                );
                match delivery {
                    Ok(()) => Ok(()),
                    Err(PreparedProviderDeliveryError::Protocol(error)) => {
                        Err(CanonicalServingError::Transport(CanonicalTcpError::Protocol(error)))
                    }
                    Err(PreparedProviderDeliveryError::Preparation(error)) => Err(error),
                    Err(PreparedProviderDeliveryError::Output(error)) => {
                        Err(CanonicalServingError::Transport(error))
                    }
                    Err(PreparedProviderDeliveryError::AfterOutput(error)) => {
                        Err(CanonicalServingError::Transport(
                            CanonicalTcpError::AfterOutput(error),
                        ))
                    }
                }
            },
        )
        .map_err(CanonicalServingError::Access)??;

        self.emitted_terminal = Some(true);
        self.failed = false;
        Ok(())
    }
}

fn candidate_set_body(
    recipe: RecipeIdV1,
    event_sequence: u64,
    candidate_set: SearchCandidateSet,
) -> Option<ProviderBodyV1> {
    let result = match recipe {
        RecipeIdV1::Locate => RecipeResultV1::Locate(candidate_set),
        RecipeIdV1::FindText => RecipeResultV1::FindText(candidate_set),
        _ => return None,
    };
    Some(ProviderBodyV1::Result(ResultBody { event_sequence, result }))
}

fn frame_limits_with_ceiling(
    limits: ProtocolLimits,
    byte_ceiling: usize,
) -> Result<ProtocolLimits, ProjectionError> {
    let max_frame_bytes = byte_ceiling.min(limits.max_frame_bytes);
    if max_frame_bytes <= FRAME_PREFIX_BYTES {
        return Err(ProjectionError::SerializedOutputBudgetExceeded);
    }
    let max_body_bytes = limits.max_body_bytes.min(max_frame_bytes - FRAME_PREFIX_BYTES);
    ProtocolLimits {
        max_frame_bytes,
        max_body_bytes,
        ..limits
    }
    .validate()
    .map_err(|_| ProjectionError::SerializedOutputBudgetExceeded)
}

// The binding comparison and backend dispatch are one boundary: no source work
// may run between the full grant/scope checks above and this exact reference.
fn with_bound_source<T>(
    sources: Option<&std::collections::BTreeMap<SourceMembershipId, SourceIdentity>>,
    membership: SourceMembershipId,
    expected: &SourceRevisionRef,
    read: impl FnOnce(&SourceRevisionRef) -> Result<T, CanonicalServingError>,
) -> Result<T, CanonicalServingError> {
    let source = sources.and_then(|sources| sources.get(&membership))
        .ok_or(CanonicalServingError::Access(AccessError::ScopeUnauthorized))?;
    if expected.source_namespace_id != source.source_namespace_id || expected.source_id != source.source_id {
        return Err(CanonicalServingError::Access(AccessError::ScopeUnauthorized));
    }
    read(expected)
}

#[cfg(test)]
#[path = "source_binding_tests.rs"]
mod source_binding_tests;
