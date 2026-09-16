//! Authenticated standalone-grant command execution over one bound session.
//!
//! This module owns the fixed composition order between the protocol envelope,
//! exact canonical body, session-bound authority and terminal lifecycle. It
//! performs no transport I/O and owns no policy persistence.

#![allow(clippy::module_name_repetitions)]

use search_contracts::{RequestId, SearchReadGrantClaims};
use search_provider_protocol::{
    AuthenticatedStandaloneGrantEnvelope, BoundSession, MonotonicMillis, ProofDigest,
    ProtocolError, RequestStatus, TerminalKind, decode_standalone_grant_request,
};

use super::grant::{GrantMintError, StandaloneGrantIssuer};
use super::grant_authority::{
    GrantAuthorityError, SessionBoundGrantAuthority, StandaloneGrantPolicySource,
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

/// Executes one exact authenticated standalone-grant command.
///
/// Fixed order: hash exact body bytes, authenticate the grant-specific envelope
/// and body digest through `BoundSession`, decode the one canonical request
/// representation, intersect it with two equal server policy reads, then record
/// exactly one terminal and release the in-flight slot. No request field can
/// choose binding or issuer operation identity.
///
/// # Errors
///
/// Returns [`GrantCommandFailure`] with the exact terminal status when one was
/// recorded. Admission and terminal-recording failures carry no recorded status.
pub fn execute_standalone_grant_command<P, I>(
    session: &mut BoundSession,
    authority: &mut SessionBoundGrantAuthority<P, I>,
    input: StandaloneGrantCommandInput<'_>,
) -> Result<SearchReadGrantClaims, GrantCommandFailure>
where
    P: StandaloneGrantPolicySource,
    I: StandaloneGrantIssuer,
{
    let observed_body_digest =
        ProofDigest::from_bytes(*blake3::hash(input.body_bytes).as_bytes());
    session
        .admit_standalone_grant_with_deadline(
            input.envelope,
            input.expected_proof,
            &observed_body_digest,
            input.sequence,
            input.now,
            input.relative_deadline_ms,
        )
        .map_err(|error| GrantCommandFailure {
            error: GrantCommandError::Protocol(error),
            status: None,
        })?;

    let request_id = *input.envelope.request_id();
    let body = match decode_standalone_grant_request(input.body_bytes) {
        Ok(body) => body,
        Err(error) => {
            return Err(finish_failure(
                session,
                &request_id,
                TerminalKind::Failed,
                GrantCommandError::Protocol(error),
            ));
        }
    };

    match authority.mint_protocol_request(session, request_id, body) {
        Ok(claims) => {
            if let Err(error) = session.complete_request(&request_id, TerminalKind::Success) {
                return Err(GrantCommandFailure {
                    error: GrantCommandError::TerminalOutcomeUnknown(error),
                    status: None,
                });
            }
            Ok(claims)
        }
        Err(error) => Err(finish_failure(
            session,
            &request_id,
            terminal_for_authority_error(error),
            GrantCommandError::Authority(error),
        )),
    }
}

fn finish_failure(
    session: &mut BoundSession,
    request_id: &RequestId,
    terminal: TerminalKind,
    error: GrantCommandError,
) -> GrantCommandFailure {
    match session.complete_request(request_id, terminal) {
        Ok(status) => GrantCommandFailure {
            error,
            status: Some(status),
        },
        Err(terminal_error) => GrantCommandFailure {
            error: GrantCommandError::TerminalOutcomeUnknown(terminal_error),
            status: None,
        },
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
