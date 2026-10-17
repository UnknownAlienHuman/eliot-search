//! Canonical standalone-grant response body.
//!
//! The existing authenticated response envelope carries status, body digest and
//! keyed proof. This module defines only the exact bounded body associated with
//! that status; it does not create a second response envelope or transport mode.

use search_contracts::{ProtocolFailureCode, SearchReadGrantClaims};

use crate::error::ProtocolError;
use crate::grant::SearchReadGrantClaimsV2;
use crate::frame::{decode_grant_response_schema, encode_grant_response_schema};
use crate::frame::{decode_grant_response_schema_v2, encode_grant_response_schema_v2};
use crate::request::RequestStatus;

/// Version of the canonical standalone-grant response body.
pub const STANDALONE_GRANT_RESPONSE_VERSION: u16 = 1;
/// Version of the explicit evidence-grant response body.
pub const STANDALONE_EVIDENCE_GRANT_RESPONSE_VERSION: u16 = 2;
/// Independent response-body ceiling below the canonical transport frame cap.
pub const MAX_STANDALONE_GRANT_RESPONSE_BYTES: usize = 1024 * 1024;

/// Exact body returned by one authenticated standalone-grant command.
///
/// Claims appear only with [`RequestStatus::Ok`]. Every non-success terminal
/// carries one public content-free failure code instead of daemon-internal state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StandaloneGrantResponseBodyV1 {
    /// Exact server-minted claims produced by the original issuer ledger.
    Claims(SearchReadGrantClaims),
    /// Public protocol/search failure classification without private details.
    Failure(ProtocolFailureCode),
}

/// Exact body returned by one authenticated evidence-grant command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StandaloneGrantResponseBodyV2 {
    /// Exact server-issued claims, including explicit evidence recipe permissions.
    Claims(SearchReadGrantClaimsV2),
    /// Public protocol/search failure classification without private details.
    Failure(ProtocolFailureCode),
}

impl StandaloneGrantResponseBodyV2 {
    /// Validate that the authenticated response status and typed body agree.
    pub fn validate_for_status(&self, status: RequestStatus) -> Result<(), ProtocolError> {
        match (status, self) {
            (RequestStatus::Ok, Self::Claims(_))
            | (
                RequestStatus::Cancelled | RequestStatus::Failed | RequestStatus::OutcomeUnknown,
                Self::Failure(_),
            ) => Ok(()),
            (RequestStatus::Partial, _)
            | (RequestStatus::Ok, Self::Failure(_))
            | (
                RequestStatus::Cancelled | RequestStatus::Failed | RequestStatus::OutcomeUnknown,
                Self::Claims(_),
            ) => Err(ProtocolError::InvalidStatus),
        }
    }
}

impl StandaloneGrantResponseBodyV1 {
    /// Validate that the authenticated response status and typed body agree.
    ///
    /// Grant issuance has no partial-success response. A contradictory status
    /// and body fails before encoding or after decoding.
    pub fn validate_for_status(&self, status: RequestStatus) -> Result<(), ProtocolError> {
        match (status, self) {
            (RequestStatus::Ok, Self::Claims(_))
            | (
                RequestStatus::Cancelled | RequestStatus::Failed | RequestStatus::OutcomeUnknown,
                Self::Failure(_),
            ) => Ok(()),
            (RequestStatus::Partial, _)
            | (RequestStatus::Ok, Self::Failure(_))
            | (
                RequestStatus::Cancelled | RequestStatus::Failed | RequestStatus::OutcomeUnknown,
                Self::Claims(_),
            ) => Err(ProtocolError::InvalidStatus),
        }
    }
}

/// Encode one exact canonical UTF-8 JSON grant-response body.
///
/// The body is authenticated separately by the existing
/// [`crate::request::AuthenticatedResponse`] digest/proof envelope.
///
/// # Errors
///
/// Rejects a contradictory terminal status, invalid claims or a body above the
/// dedicated grant-response ceiling.
pub fn encode_standalone_grant_response_body(
    status: RequestStatus,
    body: &StandaloneGrantResponseBodyV1,
) -> Result<Vec<u8>, ProtocolError> {
    body.validate_for_status(status)?;
    encode_grant_response_schema(body)
}

/// Decode one exact canonical UTF-8 JSON grant-response body.
///
/// # Errors
///
/// Rejects malformed/noncanonical bytes, unsupported body versions, excessive
/// size and any status/body contradiction.
pub fn decode_standalone_grant_response_body(
    status: RequestStatus,
    bytes: &[u8],
) -> Result<StandaloneGrantResponseBodyV1, ProtocolError> {
    let body = decode_grant_response_schema(bytes)?;
    body.validate_for_status(status)?;
    Ok(body)
}

/// Encodes the exact V2 response associated with an authenticated terminal.
pub fn encode_standalone_grant_response_body_v2(
    status: RequestStatus,
    body: &StandaloneGrantResponseBodyV2,
) -> Result<Vec<u8>, ProtocolError> {
    body.validate_for_status(status)?;
    encode_grant_response_schema_v2(body)
}

/// Decodes the exact V2 response and rejects status/body disagreement.
pub fn decode_standalone_grant_response_body_v2(
    status: RequestStatus,
    bytes: &[u8],
) -> Result<StandaloneGrantResponseBodyV2, ProtocolError> {
    let body = decode_grant_response_schema_v2(bytes)?;
    body.validate_for_status(status)?;
    Ok(body)
}
