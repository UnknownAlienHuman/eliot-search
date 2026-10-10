//! A separate consumer crate using only the public typed client boundary.
//!
//! Second independent consumer for the prototype `search-provider-client` crate.
//! This is a standalone example target compiled against the real library with
//! default features. It declares no library shell of its own, names no private
//! wire format, and performs no cryptography.

use std::task::Poll;
use std::time::{Duration, Instant};

use search_contracts::{
    Coverage, ProviderEnvelope, RequestBody, RequestId, SearchCandidateSet,
    SearchReadGrantClaims, SearchSourceHandle,
};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{BindingKey, ProtocolLimits, StandaloneGrantRequestV1};
use search_provider_transport_windows::NativeLocalConnection;

use search_provider_client::{
    NativeClientExpectations, OutcomeClass, TypedClientError, TypedProviderSession,
};

/// Open one typed local provider session.
///
/// The only generic parameter is `C`, the process-local cancellation capability
/// that `OperationContext` requires. Every other argument is concrete. No
/// private wire or crypto type appears in this signature, and the transport is
/// consumed rather than reopened.
///
/// # Errors
/// Returns [`TypedClientError`] on an expired original deadline, observed
/// cancellation, pairing or profile failure. A peer-visible write that then
/// fails is [`OutcomeClass::Unknown`], never a fabricated success.
pub fn open_session<C: CancellationProbe>(
    connection: NativeLocalConnection,
    expectations: &NativeClientExpectations,
    key: BindingKey,
    limits: ProtocolLimits,
    context: &OperationContext<C>,
    original_deadline: Instant,
) -> Result<TypedProviderSession, TypedClientError> {
    TypedProviderSession::open_native(
        connection,
        expectations,
        key,
        limits,
        context,
        original_deadline,
    )
}

/// Request and retain the one server-issued standalone grant.
///
/// The claims are returned by value, matching the library signature at
/// `typed/state/grant.rs:42`. They are comparison evidence for this session, not
/// authority: the daemon still decided every capability they describe.
///
/// # Errors
/// Returns [`TypedClientError::TrustedBindingRequired`] when no independently
/// trusted installation expectation was supplied, and `GrantRejected` when the
/// daemon authenticates a refusal. Both are refusals before any grant is
/// retained.
pub fn acquire_grant(
    session: &mut TypedProviderSession,
    request_id: RequestId,
    request: StandaloneGrantRequestV1,
    timeout: Duration,
) -> Result<SearchReadGrantClaims, TypedClientError> {
    session.request_standalone_grant(request_id, request, timeout)
}

/// Send one typed recipe and return the assigned request identity.
///
/// The typed body already carries the request identity; no separate argument
/// is passed. The grant must be the exact retained claims, since the library rejects
/// any substitution.
///
/// # Errors
/// Returns [`TypedClientError::GrantRequired`] when no grant was retained and
/// `GrantMismatch` when the body does not carry the exact issued claims.
pub fn submit(
    session: &mut TypedProviderSession,
    body: RequestBody,
    timeout: Duration,
) -> Result<RequestId, TypedClientError> {
    session.send_request(body, timeout)
}

/// Ask the server to cancel one in-flight request identity.
///
/// A cancellation receipt is a request, not a revert: the target may still
/// complete. This consumer does not infer rollback from the acknowledgement.
///
/// # Errors
/// Returns [`TypedClientError::CancellationPending`] when another cancellation is
/// already outstanding, and `NothingPending` when no operation is in flight.
pub fn cancel(
    session: &mut TypedProviderSession,
    control_id: RequestId,
    target: RequestId,
    timeout: Duration,
) -> Result<(), TypedClientError> {
    session.send_cancel(control_id, target, timeout)
}

/// Collect ready events for one bounded quantum, up to `max_events`.
///
/// Returns the number of events pulled. Each poll is bounded by the quantum
/// and the original request deadline. Stops at Pending or the event ceiling;
/// neither means the operation finished.
///
/// # Errors
/// Returns [`TypedClientError`] on protocol rejection, correlation mismatch, or
/// an expired request deadline.
pub fn drain(
    session: &mut TypedProviderSession,
    quantum: Duration,
    max_events: usize,
) -> Result<usize, TypedClientError> {
    let mut collected = 0_usize;
    while collected < max_events {
        match session.poll_receive(quantum) {
            Ok(Poll::Ready(_)) => collected += 1,
            Ok(Poll::Pending) => break,
            Err(error) => return Err(error),
        }
    }
    Ok(collected)
}

/// Read one whole authenticated event, blocking until it arrives.
///
/// # Errors
/// Returns [`TypedClientError`] on authentication failure, correlation mismatch
/// or an expired deadline.
pub fn next_event(
    session: &mut TypedProviderSession,
) -> Result<ProviderEnvelope, TypedClientError> {
    session.receive()
}

/// Report the correlation and timing fields of one event, unchanged.
///
/// Borrowing only. This returns the identity the caller must use to correlate,
/// plus the optional peer-declared relative deadline. A `Some` deadline is peer
/// metadata; the typed client already rejects envelopes that carry one, so this
/// is informational.
#[must_use]
pub fn event_summary(event: &ProviderEnvelope) -> (RequestId, u64, Option<u64>) {
    (
        event.request_id,
        event.connection_sequence,
        event.relative_deadline_ms,
    )
}

/// Borrow actual result coverage without interpreting it as a complete proof.
#[must_use]
pub const fn result_coverage(result: &SearchCandidateSet) -> &Coverage {
    &result.coverage
}

/// Borrow exact result handles; current authorization is checked by the server
/// on expansion, never inferred here from handle shape or expiry metadata.
pub fn candidate_handles(result: &SearchCandidateSet) -> impl Iterator<Item = &SearchSourceHandle> {
    result.candidates.iter().map(|candidate| &candidate.source_handle)
}

/// Classify one failure conservatively.
///
/// `true` means a local guard proved this failed call submitted nothing to the
/// peer. `false` is the unknown case and must be treated as unresolved. Neither
/// result proves rollback or authorizes replay.
#[must_use]
pub fn definitely_not_submitted(error: &TypedClientError) -> bool {
    matches!(error.outcome_class(), OutcomeClass::DefinitelyNotSubmitted)
}

// Compilation-only consumer: no credential, endpoint or synthetic authority.
fn main() {}
