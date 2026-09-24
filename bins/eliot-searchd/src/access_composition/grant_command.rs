//! Authenticated standalone-grant command execution over one bound session.
//!
//! This module owns the fixed composition order between the protocol envelope,
//! exact canonical body, session-bound authority and terminal lifecycle. It
//! performs no transport I/O and owns no policy persistence.

#![allow(clippy::module_name_repetitions)]

use std::time::Instant;

use search_contracts::SearchReadGrantClaims;
use search_provider_protocol::{
    AuthenticatedStandaloneGrantEnvelope, BoundSession, MonotonicMillis, ProofDigest,
    ProtocolError, RequestGuard, RequestStatus, TerminalKind, decode_standalone_grant_request,
};

use super::grant::{GrantMintError, StandaloneGrantIssuer};
use super::grant_authority::{
    GrantAuthorityError, GrantExecutionError, GrantRequestInterruption,
    SessionBoundGrantAuthority, StandaloneGrantPolicySource,
};

/// Exact protocol inputs for one standalone-grant command.
#[derive(Clone, Copy, Debug)]
pub struct StandaloneGrantCommandInput<'a> {
    /// Dedicated authenticated grant envelope.
    pub envelope: &'a AuthenticatedStandaloneGrantEnvelope,
    /// Expected keyed proof computed by the secret-owning adapter.
    pub expected_proof: &'a ProofDigest,
    /// Exact canonical standalone-grant body bytes.
    pub body_bytes: &'a [u8],
    /// Exact next client-to-provider sequence.
    pub sequence: u64,
    /// Adapter-observed monotonic admission instant.
    pub now: MonotonicMillis,
    /// Optional finite relative request deadline.
    pub relative_deadline_ms: Option<u64>,
}

/// Closed failure for one authenticated standalone-grant command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantCommandError {
    /// Envelope, exact body or session lifecycle failed.
    Protocol(ProtocolError),
    /// Server-owned grant authority denied or could not complete safely.
    Authority(GrantAuthorityError),
    /// Admission, cancellation or deadline failed around actual issuance.
    Execution(GrantExecutionError),
    /// Grant effects may exist but the canonical terminal could not be recorded.
    TerminalOutcomeUnknown(ProtocolError),
}

impl GrantCommandError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Protocol(error) => error.code(),
            Self::Authority(error) => error.code(),
            Self::Execution(error) => error.code(),
            Self::TerminalOutcomeUnknown(_) => "DAEMON_GRANT_TERMINAL_OUTCOME_UNKNOWN",
        }
    }
}

/// Failure plus the terminal status actually recorded for the request.
///
/// `status == None` means no terminal was recorded: either admission failed
/// before mutable session state, or terminal recording itself became outcome
/// unknown. The exact [`GrantCommandError`] distinguishes those cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrantCommandFailure {
    error: GrantCommandError,
    status: Option<RequestStatus>,
}

impl GrantCommandFailure {
    /// Exact closed failure.
    #[must_use]
    pub const fn error(self) -> GrantCommandError {
        self.error
    }

    /// Terminal status actually recorded, or `None` when none was recorded.
    #[must_use]
    pub const fn status(self) -> Option<RequestStatus> {
        self.status
    }

    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        self.error.code()
    }
}

/// Transport delivery failure without a fabricated recorded terminal.
#[derive(Debug)]
pub enum GrantDeliveryFailure<E> {
    /// Admission, command execution or terminal preparation failed.
    Command(GrantCommandFailure),
    /// Output failed; the session was disconnected, and no terminal was committed.
    Output {
        /// Original transport/encoding failure.
        error: E,
        /// Selected status, not a recorded or delivered-status claim.
        attempted_status: RequestStatus,
        /// An issued grant or unresolved issuer effects may already exist.
        /// False is not proof of rollback of effects outside this command.
        grant_may_exist: bool,
    },
}

/// Executes a command and returns its result directly to an in-process caller.
///
/// Admission/body checks, issuer identity and the original non-widening policy
/// path are preserved. Request liveness is checked around actual policy/issuer
/// calls. This compatibility method records completion before returning claims;
/// it is not a transport-delivery API. Transports must use
/// [`execute_standalone_grant_command_with_delivery`] instead.
///
/// # Errors
///
/// Returns the exact command failure and recorded terminal when available.
/// Failure to record a terminal closes the session; unwind also disconnects.
pub fn execute_standalone_grant_command<P, I>(
    session: &mut BoundSession,
    authority: &mut SessionBoundGrantAuthority<P, I>,
    input: StandaloneGrantCommandInput<'_>,
) -> Result<SearchReadGrantClaims, GrantCommandFailure>
where
    P: StandaloneGrantPolicySource,
    I: StandaloneGrantIssuer,
{
    let (mut pending, outcome) = execute_pending(session, authority, input)?;
    let terminal = terminal_for_outcome(&outcome);
    let status = pending.session.complete_request(pending.request.request_id(), terminal)
        .map_err(terminal_failure)?;
    pending.finished = true;
    finish_outcome(outcome, status)
}

/// Executes and delivers one canonical standalone-grant command before completion.
///
/// Admission failures invoke no output callback. Once admitted, the callback
/// receives the actual guard, selected terminal and claims or a closed failure.
/// It must encode/write/flush the whole response and required acknowledgement
/// under the original deadline and live security/output barrier. The guard's
/// cancellation signal must not suppress `Cancelled` or `OutcomeUnknown` output.
/// Only callback success records completion and releases the in-flight slot.
///
/// The callback must not report success after partial output. Returned error or
/// unwind disconnects the session and signals its requests. Issuance receipts
/// remain with their owner: delivery failure is neither rollback nor permission
/// to remint with a new operation ID. This adapter does not install a wire route.
///
/// # Errors
///
/// Returns admission/execution/terminal failure, or the exact delivery error and
/// whether grant effects may exist. An output error has no recorded terminal.
pub fn execute_standalone_grant_command_with_delivery<P, I, E>(
    session: &mut BoundSession,
    authority: &mut SessionBoundGrantAuthority<P, I>,
    input: StandaloneGrantCommandInput<'_>,
    output: impl FnOnce(
        &RequestGuard,
        RequestStatus,
        Result<&SearchReadGrantClaims, &GrantCommandError>,
    ) -> Result<(), E>,
) -> Result<SearchReadGrantClaims, GrantDeliveryFailure<E>>
where
    P: StandaloneGrantPolicySource,
    I: StandaloneGrantIssuer,
{
    let (mut pending, outcome) = execute_pending(session, authority, input)
        .map_err(GrantDeliveryFailure::Command)?;
    let terminal = terminal_for_outcome(&outcome);
    let attempted_status = RequestStatus::from_terminal(terminal);
    let grant_may_exist = outcome.is_ok() || terminal == TerminalKind::OutcomeUnknown;
    let prepared = pending.session
        .prepare_request_terminal(pending.request.request_id(), terminal)
        .map_err(|error| GrantDeliveryFailure::Command(terminal_failure(error)))?;
    let status = prepared.deliver(|status| output(&pending.request, status, outcome.as_ref()))
        .map_err(|error| GrantDeliveryFailure::Output {
            error,
            attempted_status,
            grant_may_exist,
        })?;
    pending.finished = true;
    finish_outcome(outcome, status).map_err(GrantDeliveryFailure::Command)
}

type GrantCommandOutcome = Result<SearchReadGrantClaims, GrantCommandError>;

// Installed immediately after successful admission. External callbacks may
// unwind, so dropping the borrowed owner without terminal completion must
// disconnect rather than strand an admitted request or allow a blind retry.
struct PendingGrantCommand<'a> {
    session: &'a mut BoundSession,
    request: RequestGuard,
    finished: bool,
}

impl Drop for PendingGrantCommand<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.session.disconnect();
        }
    }
}

fn execute_pending<'a, P, I>(
    session: &'a mut BoundSession,
    authority: &mut SessionBoundGrantAuthority<P, I>,
    input: StandaloneGrantCommandInput<'_>,
) -> Result<(PendingGrantCommand<'a>, GrantCommandOutcome), GrantCommandFailure>
where
    P: StandaloneGrantPolicySource,
    I: StandaloneGrantIssuer,
{
    // Advance from the adapter's admission origin with real monotonic elapsed
    // time. Hashing, decoding and policy reads cannot restart the work budget.
    let started = Instant::now();
    let observed_body_digest =
        ProofDigest::from_bytes(*blake3::hash(input.body_bytes).as_bytes());
    let request = session.admit_standalone_grant_with_deadline(
        input.envelope,
        input.expected_proof,
        &observed_body_digest,
        input.sequence,
        input.now,
        input.relative_deadline_ms,
    ).map_err(|error| GrantCommandFailure {
        error: GrantCommandError::Protocol(error),
        status: None,
    })?;
    let pending = PendingGrantCommand { session, request, finished: false };
    let outcome = match decode_standalone_grant_request(input.body_bytes) {
        Ok(body) => authority.mint_admitted_protocol_request(
            pending.session,
            &pending.request,
            body,
            || input.now.plus(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
        ).map_err(|error| match error {
            GrantExecutionError::Authority(error) => GrantCommandError::Authority(error),
            interruption => GrantCommandError::Execution(interruption),
        }),
        Err(error) => Err(GrantCommandError::Protocol(error)),
    };
    Ok((pending, outcome))
}

fn finish_outcome(
    outcome: GrantCommandOutcome,
    status: RequestStatus,
) -> Result<SearchReadGrantClaims, GrantCommandFailure> {
    outcome.map_err(|error| GrantCommandFailure { error, status: Some(status) })
}

fn terminal_failure(error: ProtocolError) -> GrantCommandFailure {
    GrantCommandFailure {
        error: GrantCommandError::TerminalOutcomeUnknown(error),
        status: None,
    }
}

fn terminal_for_outcome(
    outcome: &GrantCommandOutcome,
) -> TerminalKind {
    match outcome {
        Ok(_) => TerminalKind::Success,
        Err(GrantCommandError::Authority(error))
        | Err(GrantCommandError::Execution(GrantExecutionError::Authority(error))) => {
            terminal_for_authority_error(*error)
        }
        Err(GrantCommandError::Execution(GrantExecutionError::OutcomeUnknown(_)))
        | Err(GrantCommandError::TerminalOutcomeUnknown(_)) => TerminalKind::OutcomeUnknown,
        Err(GrantCommandError::Execution(GrantExecutionError::Interrupted(
            GrantRequestInterruption::Cancelled,
        ))) => TerminalKind::Cancelled,
        Err(GrantCommandError::Execution(GrantExecutionError::Interrupted(_)))
        | Err(GrantCommandError::Protocol(_)) => TerminalKind::Failed,
    }
}

const fn terminal_for_authority_error(error: GrantAuthorityError) -> TerminalKind {
    match error {
        GrantAuthorityError::PolicyChangedDuringIssuance
        | GrantAuthorityError::Mint(GrantMintError::IssuerOutcomeUnknown)
        | GrantAuthorityError::Mint(GrantMintError::IssuerReceiptMismatch)
        | GrantAuthorityError::Mint(GrantMintError::IssuerReturnedInvalidGrant) => {
            TerminalKind::OutcomeUnknown
        }
        GrantAuthorityError::SessionInactive
        | GrantAuthorityError::PeerRoleDenied
        | GrantAuthorityError::PolicyUnavailable
        | GrantAuthorityError::PolicyBindingMismatch
        | GrantAuthorityError::ProtocolRequestInvalid
        | GrantAuthorityError::Mint(_) => TerminalKind::Failed,
    }
}

#[cfg(test)]
mod tests;
