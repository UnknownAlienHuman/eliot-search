//! Authenticated, bounded live-probe session used before operational admission.

use core::fmt;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CollectionExistsRequest, CollectionOperationResponse, CountPoints, CountResponse,
    CreateCollection, CreateFieldIndexCollection, DeleteCollection, DeletePoints,
    GetCollectionInfoRequest, GetCollectionInfoResponse, GetPoints, GetResponse, HealthCheckReply,
    HealthCheckRequest, ListCollectionsRequest, PointsOperationResponse, QueryPoints,
    QueryResponse, UpsertPoints,
};
use search_contracts::OpaqueId;
use sha2::{Digest, Sha256};
use tonic::{Request, Response, Status};

use crate::live::LiveEndpoint;
use crate::qualified::QualifiedGate;
use crate::{BridgeError, BridgeLimits, QdrantApiKeyLease, QdrantConnectionBinding};

use super::{
    CanonicalRequestContext, MAX_SINGLE_ATTEMPT_DECODED_MESSAGE_BYTES, RealDataPlane,
    SingleAttemptTransport, canonical,
};

type ProbeReadFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<Response<T>, Status>> + Send + 'a>>;

const MAX_PROBE_RPC_CALLS: usize = 64;
/// Each issued RPC reserves one full decoder slot from this shared total.
/// This bounds aggregate response decoding without trusting SDK retries or headers.
const MAX_PROBE_DECODE_RESERVATION_BYTES: usize = 24 * 1024 * 1024;
static NEXT_PROBE_ATTEMPT: AtomicU64 = AtomicU64::new(1);

/// Opaque identity of one fresh temporary capability-probe collection.
///
/// The name is generated from the current supervisor binding and a unique
/// single-process attempt nonce. A pre-existing name is always refused.
#[derive(Clone, Eq, PartialEq)]
pub struct QdrantProbeAttemptId {
    id: OpaqueId,
    collection_name: String,
}

impl QdrantProbeAttemptId {
    /// Opaque owner-bound identity retained for exact reconciliation.
    #[must_use]
    pub const fn id(&self) -> &OpaqueId {
        &self.id
    }

    /// Exact temporary collection locator for this attempt.
    #[must_use]
    pub fn collection_name(&self) -> &str {
        &self.collection_name
    }
}

impl fmt::Debug for QdrantProbeAttemptId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QdrantProbeAttemptId")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Exact identity of one mutation issued during a live-probe attempt.
#[derive(Clone, Eq, PartialEq)]
pub struct QdrantProbeMutationId(OpaqueId);

impl QdrantProbeMutationId {
    /// Opaque effect identity retained when acknowledgement or cleanup is unknown.
    #[must_use]
    pub const fn id(&self) -> &OpaqueId {
        &self.0
    }
}

impl fmt::Debug for QdrantProbeMutationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QdrantProbeMutationId([OPAQUE])")
    }
}

/// Failed live admission with the temporary owner scope needed for safe reconciliation.
#[derive(Clone, Eq, PartialEq)]
pub struct OperationalProbeFailure {
    error: BridgeError,
    attempt: Option<QdrantProbeAttemptId>,
    unknown_mutation: Option<QdrantProbeMutationId>,
}

impl OperationalProbeFailure {
    pub(super) const fn new(
        error: BridgeError,
        attempt: Option<QdrantProbeAttemptId>,
        unknown_mutation: Option<QdrantProbeMutationId>,
    ) -> Self {
        Self {
            error,
            attempt,
            unknown_mutation,
        }
    }

    pub(super) const fn before_attempt(error: BridgeError) -> Self {
        Self::new(error, None, None)
    }

    /// Stable Eliot-owned reason; vendor status text never escapes the bridge.
    #[must_use]
    pub const fn error(&self) -> BridgeError {
        self.error
    }

    /// Temporary collection identity, when an attempt scope was generated.
    #[must_use]
    pub const fn attempt(&self) -> Option<&QdrantProbeAttemptId> {
        self.attempt.as_ref()
    }

    /// Exact possibly-applied mutation. It must be resolved by its owner before cleanup/retry.
    #[must_use]
    pub const fn unknown_mutation(&self) -> Option<&QdrantProbeMutationId> {
        self.unknown_mutation.as_ref()
    }
}

impl fmt::Debug for OperationalProbeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationalProbeFailure")
            .field("error", &self.error)
            .field("attempt", &self.attempt)
            .field("has_unknown_mutation", &self.unknown_mutation.is_some())
            .finish()
    }
}

impl fmt::Display for OperationalProbeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.error.code())
    }
}

impl std::error::Error for OperationalProbeFailure {}

struct MutationLedger {
    next: u64,
    pending: Option<QdrantProbeMutationId>,
    last: Option<QdrantProbeMutationId>,
    unknown: Option<QdrantProbeMutationId>,
}

/// Existing authenticated transport plus one bounded attempt scope.
///
/// Vendor calls and collection lifecycle remain private. All probes share the
/// same original request context and the same finite RPC-work allowance.
pub struct LiveProbeClient {
    // Retained for the real-plane handoff; probe RPCs use generated clients below.
    client: Qdrant,
    single_attempt: SingleAttemptTransport,
    auth_lease: QdrantApiKeyLease,
    context: CanonicalRequestContext,
    limits: BridgeLimits,
    rpc_timeout: Duration,
    attempt: QdrantProbeAttemptId,
    rpc_calls: AtomicUsize,
    response_decode_bytes_reserved: AtomicUsize,
    collection_owned: AtomicBool,
    mutations: Mutex<MutationLedger>,
}

impl LiveProbeClient {
    pub(crate) fn connect(
        endpoint: &LiveEndpoint,
        supervisor: QdrantConnectionBinding,
        auth_lease: QdrantApiKeyLease,
        context: CanonicalRequestContext,
        limits: BridgeLimits,
        rpc_timeout: Duration,
    ) -> Result<Self, OperationalProbeFailure> {
        let attempt = probe_attempt_id(supervisor)
            .map_err(|error| OperationalProbeFailure::new(error, None, None))?;
        let fail = |error| OperationalProbeFailure::new(error, Some(attempt.clone()), None);
        if rpc_timeout.is_zero() || rpc_timeout > Duration::from_secs(600) {
            return Err(fail(BridgeError::QueryBudgetExceeded));
        }
        let limits = limits.validate().map_err(fail)?;
        auth_lease.validate_binding(supervisor).map_err(fail)?;
        if !supervisor.matches_live_endpoint(endpoint) {
            return Err(fail(BridgeError::SupervisorReceiptMismatch));
        }
        context.check().map_err(fail)?;

        let mut built_client = None;
        auth_lease
            .with_secret(|api_key| {
                built_client = Some(
                    Qdrant::from_url(&endpoint.grpc_url())
                        .api_key(api_key)
                        .timeout(Duration::from_secs(10))
                        .connect_timeout(Duration::from_secs(5))
                        .skip_compatibility_check()
                        .build()
                        .map_err(|_| BridgeError::TransportFailed)?,
                );
                Ok(())
            })
            .map_err(fail)?;
        auth_lease.validate().map_err(fail)?;
        let single_attempt = SingleAttemptTransport::new(endpoint).map_err(fail)?;
        Ok(Self {
            client: built_client.ok_or_else(|| fail(BridgeError::AuthenticationInvalid))?,
            single_attempt,
            auth_lease,
            context,
            limits,
            rpc_timeout,
            attempt,
            rpc_calls: AtomicUsize::new(0),
            response_decode_bytes_reserved: AtomicUsize::new(0),
            collection_owned: AtomicBool::new(false),
            mutations: Mutex::new(MutationLedger {
                next: 1,
                pending: None,
                last: None,
                unknown: None,
            }),
        })
    }

    pub(crate) const fn attempt(&self) -> &QdrantProbeAttemptId {
        &self.attempt
    }

    pub(crate) fn collection_name(&self) -> &str {
        self.attempt.collection_name()
    }

    pub(crate) async fn refuse_existing_collection(&self) -> Result<(), BridgeError> {
        refuse_preexisting_collection(self.collection_exists(self.collection_name()).await?)
    }

    pub(crate) async fn health_check(&self) -> Result<HealthCheckReply, BridgeError> {
        // A newly resumed owned child can precede its listener. Retry only this
        // read, before any probe mutation, with the SAME attempt, absolute
        // deadline and shared RPC/response budget. Each actual attempt goes
        // through read_rpc; SDK mutation retries remain disabled.
        let mut backoff = Duration::from_millis(50);
        for attempt in 0..8 {
            let request = self.rpc_request(HealthCheckRequest::default())?;
            let mut client = self.single_attempt.health.clone();
            match self.read_rpc(Box::pin(client.health_check(request))).await {
                Err(BridgeError::TransportFailed) if attempt < 7 => {
                    let remaining = self.context.remaining()?;
                    self.auth_lease.validate()?;
                    tokio::time::sleep(backoff.min(remaining)).await;
                    self.context.check()?;
                    backoff = backoff.saturating_mul(2).min(Duration::from_millis(250));
                }
                result => return result,
            }
        }
        Err(BridgeError::TransportFailed)
    }

    pub(crate) async fn verify_list_collections_access(&self) -> Result<(), BridgeError> {
        let request = self.rpc_request(ListCollectionsRequest::default())?;
        let mut client = self.single_attempt.collections.clone();
        self.read_rpc(Box::pin(client.list(request))).await?;
        Ok(())
    }

    pub(crate) async fn collection_exists(&self, name: &str) -> Result<bool, BridgeError> {
        self.require_attempt_collection(name)?;
        let request = self.rpc_request(CollectionExistsRequest {
            collection_name: name.to_owned(),
        })?;
        let mut client = self.single_attempt.collections.clone();
        self.read_rpc(Box::pin(client.collection_exists(request)))
            .await?
            .result
            .map(|result| result.exists)
            .ok_or(BridgeError::MalformedResponse)
    }

    pub(crate) async fn collection_info(
        &self,
        name: &str,
    ) -> Result<GetCollectionInfoResponse, BridgeError> {
        self.require_attempt_collection(name)?;
        let request = self.rpc_request(GetCollectionInfoRequest {
            collection_name: name.to_owned(),
        })?;
        let mut client = self.single_attempt.collections.clone();
        self.read_rpc(Box::pin(client.get(request))).await
    }

    pub(crate) async fn count(&self, message: CountPoints) -> Result<CountResponse, BridgeError> {
        self.require_attempt_collection(&message.collection_name)?;
        let request = self.rpc_request(message)?;
        let mut client = self.single_attempt.points.clone();
        self.read_rpc(Box::pin(client.count(request))).await
    }

    pub(crate) async fn get_points(&self, message: GetPoints) -> Result<GetResponse, BridgeError> {
        self.require_attempt_collection(&message.collection_name)?;
        let request = self.rpc_request(message)?;
        let mut client = self.single_attempt.points.clone();
        self.read_rpc(Box::pin(client.get(request))).await
    }

    pub(crate) async fn query(&self, message: QueryPoints) -> Result<QueryResponse, BridgeError> {
        self.require_attempt_collection(&message.collection_name)?;
        let request = self.rpc_request(message)?;
        let mut client = self.single_attempt.points.clone();
        self.read_rpc(Box::pin(client.query(request))).await
    }
    pub(crate) async fn create_collection(
        &self,
        request: CreateCollection,
    ) -> Result<CollectionOperationResponse, BridgeError> {
        self.require_attempt_collection(&request.collection_name)?;
        let (mutation, request) = self.prepare_mutation("create_collection", request)?;
        let mut collections = self.single_attempt.collections.clone();
        let reply = self
            .finish_mutation_dispatch(mutation, collections.create(request), map_create_status)
            .await?;
        if reply.result {
            self.collection_owned.store(true, Ordering::SeqCst);
        }
        Ok(reply)
    }

    pub(crate) async fn create_field_index(
        &self,
        request: CreateFieldIndexCollection,
    ) -> Result<PointsOperationResponse, BridgeError> {
        self.require_attempt_collection(&request.collection_name)?;
        let (mutation, request) = self.prepare_mutation("create_payload_index", request)?;
        let mut points = self.single_attempt.points.clone();
        self.finish_mutation_dispatch(
            mutation,
            points.create_field_index(request),
            map_create_status,
        )
        .await
    }

    pub(crate) async fn upsert_points(
        &self,
        request: UpsertPoints,
    ) -> Result<PointsOperationResponse, BridgeError> {
        self.require_attempt_collection(&request.collection_name)?;
        let (mutation, request) = self.prepare_mutation("upsert_probe_points", request)?;
        let mut points = self.single_attempt.points.clone();
        self.finish_mutation_dispatch(mutation, points.upsert(request), map_mutation_status)
            .await
    }

    pub(crate) async fn delete_points(
        &self,
        request: DeletePoints,
    ) -> Result<PointsOperationResponse, BridgeError> {
        self.require_attempt_collection(&request.collection_name)?;
        let unindexed_filter = request.points.as_ref().is_some_and(|selector| {
            matches!(
                selector.points_selector_one_of.as_ref(),
                Some(qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Filter(_))
            )
        });
        let (mutation, request) = self.prepare_mutation("delete_probe_points", request)?;
        let mut points = self.single_attempt.points.clone();
        self.finish_mutation_dispatch(mutation, points.delete(request), move |status| {
            if unindexed_filter && is_strict_filter_rejection(&status) {
                BridgeError::UnindexedFilter
            } else {
                map_mutation_status(status)
            }
        })
        .await
    }

    fn acknowledge_mutation(&self, expected: &QdrantProbeMutationId) -> Result<(), BridgeError> {
        let mut state = self
            .mutations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.pending.as_ref() != Some(expected) {
            state.unknown = state.pending.clone().or_else(|| Some(expected.clone()));
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        state.pending = None;
        drop(state);
        Ok(())
    }

    pub(crate) fn unknown_mutation(&self) -> Option<QdrantProbeMutationId> {
        self.mutations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .unknown
            .clone()
    }

    fn unresolved_mutation(&self) -> Option<QdrantProbeMutationId> {
        let state = self
            .mutations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.unknown.clone().or_else(|| state.pending.clone())
    }

    pub(crate) async fn cleanup_owned_collection(&self) -> Result<(), BridgeError> {
        if self.unresolved_mutation().is_some() {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        if !self.collection_owned.load(Ordering::SeqCst) {
            return Ok(());
        }
        let (mutation, request) = self.prepare_mutation(
            "delete_probe_collection",
            DeleteCollection {
                collection_name: self.collection_name().to_owned(),
                ..Default::default()
            },
        )?;
        let mut collections = self.single_attempt.collections.clone();
        let reply = self
            .finish_mutation_dispatch(mutation, collections.delete(request), map_create_status)
            .await?;
        if !reply.result {
            return Err(BridgeError::CapabilityProbeFailed);
        }
        self.collection_owned.store(false, Ordering::SeqCst);
        if self.collection_exists(self.collection_name()).await? {
            self.mark_last_mutation_unknown();
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        Ok(())
    }

    pub(crate) fn into_data_plane(
        self,
        gate: QualifiedGate,
    ) -> Result<RealDataPlane, OperationalProbeFailure> {
        let state = self
            .mutations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let handoff = validate_probe_handoff(
            self.collection_owned.load(Ordering::SeqCst),
            state.pending.is_some(),
            state.unknown.is_some(),
        );
        drop(state);
        if let Err(error) = handoff {
            return Err(self.failure(error));
        }
        if let Err(error) = self
            .context
            .check()
            .and_then(|()| self.auth_lease.validate())
        {
            return Err(self.failure(error));
        }
        Ok(RealDataPlane {
            client: self.client,
            single_attempt: self.single_attempt,
            auth_lease: self.auth_lease,
            gate,
            limits: self.limits,
            schemas: BTreeMap::default(),
            operations: BTreeMap::default(),
            canonical: canonical::CanonicalState::default(),
        })
    }

    pub(crate) fn failure(&self, error: BridgeError) -> OperationalProbeFailure {
        OperationalProbeFailure::new(
            error,
            Some(self.attempt.clone()),
            self.unresolved_mutation(),
        )
    }

    fn require_attempt_collection(&self, name: &str) -> Result<(), BridgeError> {
        require_probe_collection(self.collection_name(), name)
    }

    fn reserve_rpc(&self) -> Result<(), BridgeError> {
        self.context.remaining()?;
        let has_pending = self
            .mutations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pending
            .is_some();
        if has_pending {
            self.mark_last_mutation_unknown();
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        self.rpc_calls
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |calls| {
                (calls < MAX_PROBE_RPC_CALLS).then_some(calls + 1)
            })
            .map_err(|_| BridgeError::QueryBudgetExceeded)?;
        self.response_decode_bytes_reserved
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, next_decode_reservation)
            .map(|_| ())
            .map_err(|_| BridgeError::QueryBudgetExceeded)
    }

    async fn read_rpc<T>(&self, call: ProbeReadFuture<'_, T>) -> Result<T, BridgeError> {
        self.reserve_rpc()?;
        self.auth_lease.validate()?;
        let remaining = self.context.remaining()?.min(self.rpc_timeout);
        let response = tokio::time::timeout(remaining, call)
            .await
            .map_err(|_| BridgeError::TransportFailed)?
            .map_err(|status| {
                super::map_read_error(qdrant_client::QdrantError::ResponseError { status })
            })?;
        self.context.check()?;
        self.auth_lease.validate()?;
        Ok(response.into_inner())
    }

    fn rpc_request<T>(&self, message: T) -> Result<Request<T>, BridgeError> {
        let remaining = self.context.remaining()?;
        let mut request = Request::new(message);
        self.auth_lease.with_secret(|key| {
            let value = key
                .parse()
                .map_err(|_| BridgeError::AuthenticationInvalid)?;
            request.metadata_mut().insert("api-key", value);
            Ok(())
        })?;
        request.set_timeout(remaining.min(self.rpc_timeout));
        Ok(request)
    }

    fn prepare_mutation<T>(
        &self,
        effect: &str,
        message: T,
    ) -> Result<(QdrantProbeMutationId, Request<T>), BridgeError> {
        self.reserve_rpc()?;
        let request = self.rpc_request(message)?;
        let mutation = self.next_mutation_id(effect)?;
        self.begin_mutation(mutation.clone())?;
        Ok((mutation, request))
    }

    async fn finish_mutation_dispatch<T, Fut>(
        &self,
        mutation: QdrantProbeMutationId,
        dispatch: Fut,
        map_status: impl FnOnce(Status) -> BridgeError,
    ) -> Result<T, BridgeError>
    where
        Fut: Future<Output = Result<Response<T>, Status>>,
    {
        if let Err(error) = self.auth_lease.validate() {
            if !self.clear_pending_mutation(&mutation) {
                return Err(BridgeError::MutationOutcomeUnknown);
            }
            return Err(error);
        }
        let remaining = match self.context.remaining() {
            Ok(remaining) => remaining.min(self.rpc_timeout),
            Err(error) => {
                if !self.clear_pending_mutation(&mutation) {
                    return Err(BridgeError::MutationOutcomeUnknown);
                }
                return Err(error);
            }
        };
        match tokio::time::timeout(remaining, dispatch).await {
            Err(_) => {
                self.mark_mutation_unknown(mutation);
                Err(BridgeError::MutationOutcomeUnknown)
            }
            Ok(Err(status)) => {
                let error = map_status(status);
                if error == BridgeError::MutationOutcomeUnknown {
                    self.mark_mutation_unknown(mutation);
                } else if !self.clear_pending_mutation(&mutation) {
                    return Err(BridgeError::MutationOutcomeUnknown);
                }
                Err(error)
            }
            Ok(Ok(response)) => {
                if self.context.remaining_after_dispatch().is_err() {
                    self.mark_mutation_unknown(mutation);
                    return Err(BridgeError::MutationOutcomeUnknown);
                }
                self.acknowledge_mutation(&mutation)?;
                Ok(response.into_inner())
            }
        }
    }

    fn next_mutation_id(&self, effect: &str) -> Result<QdrantProbeMutationId, BridgeError> {
        let mut state = self
            .mutations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sequence = state.next;
        state.next = sequence
            .checked_add(1)
            .ok_or(BridgeError::OperationConflict)?;
        drop(state);
        let id = OpaqueId::new(format!(
            "qdrant-probe-mutation:{}:{effect}:{sequence}",
            self.attempt.id.as_str()
        ))
        .map_err(|_| BridgeError::OperationConflict)?;
        Ok(QdrantProbeMutationId(id))
    }

    fn begin_mutation(&self, mutation: QdrantProbeMutationId) -> Result<(), BridgeError> {
        let mut state = self
            .mutations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.pending.is_some() || state.unknown.is_some() {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        state.pending = Some(mutation.clone());
        state.last = Some(mutation);
        drop(state);
        Ok(())
    }

    fn clear_pending_mutation(&self, expected: &QdrantProbeMutationId) -> bool {
        let mut state = self
            .mutations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.pending.as_ref() != Some(expected) {
            state.unknown = state.pending.clone().or_else(|| Some(expected.clone()));
            return false;
        }
        state.pending = None;
        drop(state);
        true
    }

    fn mark_mutation_unknown(&self, mutation: QdrantProbeMutationId) {
        let mut state = self
            .mutations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.pending = None;
        state.unknown = Some(mutation);
        drop(state);
    }

    fn mark_last_mutation_unknown(&self) {
        let mut state = self
            .mutations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(mutation) = state.pending.take() {
            state.unknown = Some(mutation);
        } else if state.last.is_some() {
            state.unknown = state.last.take();
        }
        drop(state);
    }
}

fn next_decode_reservation(used: usize) -> Option<usize> {
    let next = used.checked_add(MAX_SINGLE_ATTEMPT_DECODED_MESSAGE_BYTES)?;
    (next <= MAX_PROBE_DECODE_RESERVATION_BYTES).then_some(next)
}
fn probe_attempt_id(
    supervisor: QdrantConnectionBinding,
) -> Result<QdrantProbeAttemptId, BridgeError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| BridgeError::TransportFailed)?
        .as_nanos();
    let nonce = NEXT_PROBE_ATTEMPT
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(1)
        })
        .map_err(|_| BridgeError::OperationConflict)?;
    let mut hash = Sha256::new();
    hash.update(b"eliot:qdrant-live-probe-attempt:v1\0");
    hash.update(now.to_be_bytes());
    hash.update(nonce.to_be_bytes());
    hash.update(supervisor.process_id().get().to_be_bytes());
    hash.update(supervisor.creation_marker().get().to_be_bytes());
    hash.update(supervisor.data_root_id().as_bytes());
    hash.update(supervisor.installation_incarnation_id().as_bytes());
    hash.update(supervisor.owner_epoch().get().to_be_bytes());
    hash.update(supervisor.endpoint().endpoint_digest().as_bytes());
    hash.update(supervisor.artifact_digest().as_bytes());
    hash.update(supervisor.secret_reference_digest().as_bytes());
    hash.update(supervisor.secret_purpose_digest().as_bytes());
    hash.update(supervisor.secret_expires_at_tick().get().to_be_bytes());
    let digest = hash.finalize();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest);
    let hex = super::hex_from_32(&bytes);
    let id = OpaqueId::new(format!("qdrant-probe:v1:{hex}"))
        .map_err(|_| BridgeError::OperationConflict)?;
    Ok(QdrantProbeAttemptId {
        id,
        collection_name: format!("elprobe_{hex}"),
    })
}

const fn refuse_preexisting_collection(exists: bool) -> Result<(), BridgeError> {
    if exists {
        Err(BridgeError::CollectionAlreadyExists)
    } else {
        Ok(())
    }
}

fn require_probe_collection(expected: &str, requested: &str) -> Result<(), BridgeError> {
    if expected == requested {
        Ok(())
    } else {
        Err(BridgeError::OperationConflict)
    }
}

const fn validate_probe_handoff(
    collection_owned: bool,
    mutation_pending: bool,
    mutation_unknown: bool,
) -> Result<(), BridgeError> {
    if mutation_pending || mutation_unknown {
        return Err(BridgeError::MutationOutcomeUnknown);
    }
    if collection_owned {
        return Err(BridgeError::CapabilityProbeFailed);
    }
    Ok(())
}

fn map_create_status(status: Status) -> BridgeError {
    if status.code() == tonic::Code::ResourceExhausted {
        BridgeError::MutationOutcomeUnknown
    } else {
        super::map_create_error(qdrant_client::QdrantError::ResponseError { status })
    }
}

fn map_mutation_status(status: Status) -> BridgeError {
    if status.code() == tonic::Code::ResourceExhausted {
        BridgeError::MutationOutcomeUnknown
    } else {
        super::map_mutation_error(qdrant_client::QdrantError::ResponseError { status })
    }
}

fn is_strict_filter_rejection(status: &Status) -> bool {
    matches!(
        status.code(),
        tonic::Code::InvalidArgument | tonic::Code::FailedPrecondition | tonic::Code::OutOfRange
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_mutation_response_preserves_unknown_outcome() {
        let status = Status::resource_exhausted("bounded generated response decoder limit");
        assert_eq!(
            map_create_status(status.clone()),
            BridgeError::MutationOutcomeUnknown
        );
        assert_eq!(
            map_mutation_status(status),
            BridgeError::MutationOutcomeUnknown
        );
    }
    #[test]
    fn shared_decode_budget_reserves_each_rpc_before_dispatch() {
        let last_slot =
            MAX_PROBE_DECODE_RESERVATION_BYTES - MAX_SINGLE_ATTEMPT_DECODED_MESSAGE_BYTES;
        assert_eq!(
            next_decode_reservation(last_slot),
            Some(MAX_PROBE_DECODE_RESERVATION_BYTES)
        );
        assert_eq!(
            next_decode_reservation(MAX_PROBE_DECODE_RESERVATION_BYTES),
            None
        );
    }
    #[test]
    fn preexisting_attempt_collection_is_refused() {
        assert_eq!(
            refuse_preexisting_collection(true),
            Err(BridgeError::CollectionAlreadyExists)
        );
        assert_eq!(refuse_preexisting_collection(false), Ok(()));
    }

    #[test]
    fn probe_rpc_scope_rejects_every_other_collection() {
        assert_eq!(
            require_probe_collection("elprobe_owned", "elprobe_foreign"),
            Err(BridgeError::OperationConflict)
        );
        assert_eq!(
            require_probe_collection("elprobe_owned", "elprobe_owned"),
            Ok(())
        );
    }

    #[test]
    fn handoff_requires_cleanup_and_resolved_mutations() {
        assert_eq!(
            validate_probe_handoff(true, false, false),
            Err(BridgeError::CapabilityProbeFailed)
        );
        assert_eq!(
            validate_probe_handoff(false, true, false),
            Err(BridgeError::MutationOutcomeUnknown)
        );
        assert_eq!(
            validate_probe_handoff(false, false, true),
            Err(BridgeError::MutationOutcomeUnknown)
        );
        assert_eq!(validate_probe_handoff(false, false, false), Ok(()));
    }
}
