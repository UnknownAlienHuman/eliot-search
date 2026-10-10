//! Mandatory first-command standalone grant exchange for one typed session.
//!
//! The grant request and response reuse the single paired byte stream, binding
//! key, sequence/replay ledger and original absolute deadline. No second reader,
//! transport, grant cache, retry path or caller-minted claim is introduced.

use std::task::Poll;
use std::time::{Duration, Instant};

use search_contracts::{
    BoundedSet, CorpusOrPortfolioId, InstallationId, MAX_SET_ITEMS, RequestId,
    SearchReadGrantClaims,
};
use search_provider_protocol::{
    FrameCodec, JsonFramePayload, ProofDigest, ProtocolError,
    ProviderFrameTranscript, StandaloneGrantRequestV1,
    StandaloneGrantResponseBodyV1, decode_response,
    decode_standalone_grant_response_body, encode_standalone_grant_envelope,
    encode_standalone_grant_request, response_transcript,
    seal_standalone_grant_envelope, standalone_grant_envelope_transcript,
    verify_proof, verify_response_proof,
};

use super::super::io::{POLL_INTERVAL, budget, remaining};
use super::super::{TypedClientError, TypedProviderSession, keyed_parts};
use super::State;

const INITIAL_STANDALONE_GRANT_SEQUENCE: u64 = 1;

#[cfg(test)]
mod tests;

impl TypedProviderSession {
    /// Request and retain the one server-issued standalone read grant.
    ///
    /// This must be the first post-profile command and requires an independently
    /// trusted registration installation identity;
    /// pairing-only compatibility sessions fail before sending any grant bytes.
    /// The canonical grant envelope and body use two typed/MAC records, then two
    /// authenticated response records are read under the same absolute deadline.
    /// Any refusal, partial I/O, mismatch or unwind drops the entire session.
    pub fn request_standalone_grant(
        &mut self,
        request_id: RequestId,
        request: StandaloneGrantRequestV1,
        timeout: Duration,
    ) -> Result<SearchReadGrantClaims, TypedClientError> {
        self.with_state(|state| {
            state.request_standalone_grant(request_id, request, timeout)
        })
    }
}

impl State {
    fn request_standalone_grant(
        &mut self,
        request_id: RequestId,
        request: StandaloneGrantRequestV1,
        timeout: Duration,
    ) -> Result<SearchReadGrantClaims, TypedClientError> {
        // A paired compatibility peer cannot supply its own installation anchor.
        // Reject before encoding, spending sequence state or writing any record.
        let trusted = self.trusted_installation_id.ok_or(TypedClientError::TrustedBindingRequired)?;
        request.validate()?;
        let (deadline, _) = budget(timeout)?;
        if self.grant.is_some()
            || !self.requests.is_empty()
            || self.cancel.is_some()
            || self.next_client_sequence()? != INITIAL_STANDALONE_GRANT_SEQUENCE
        {
            return Err(TypedClientError::GrantAlreadyIssued);
        }

        let body_bytes = encode_standalone_grant_request(&request)?;
        let body_payload = JsonFramePayload::new(body_bytes)
            .map_err(|_| ProtocolError::FrameTooLarge)?;
        let body_digest =
            ProofDigest::from_bytes(*blake3::hash(body_payload.as_slice()).as_bytes());
        let stub = seal_standalone_grant_envelope(
            self.binding.version(),
            self.nonce,
            request_id,
            body_digest,
            ProofDigest::from_bytes([0_u8; 32]),
        );
        let request_proof = keyed_bytes(
            &self.key,
            &standalone_grant_envelope_transcript(&stub),
        );
        let envelope = seal_standalone_grant_envelope(
            self.binding.version(),
            self.nonce,
            request_id,
            body_digest,
            request_proof,
        );
        let envelope_frame =
            encode_standalone_grant_envelope(&envelope, self.limits)?;
        let body_frame = FrameCodec::encode(&body_payload, self.limits)?;
        let envelope_record_proof = self.request_proof(envelope_frame.as_slice());
        let body_record_proof = self.request_proof(body_frame.as_slice());

        remaining(deadline)?;
        // Spend sequence/replay state before the first possible peer-visible
        // byte. Any later error closes this sole owner and is never replayed.
        self.session
            .admit_request(request_id, INITIAL_STANDALONE_GRANT_SEQUENCE)?;
        self.socket.write_record(
            envelope_frame.as_slice(),
            &envelope_record_proof,
            self.limits,
            deadline,
        )?;
        self.socket.write_record(
            body_frame.as_slice(),
            &body_record_proof,
            self.limits,
            deadline,
        )?;

        let (response_frame, response_record_proof) = self.read_record(deadline)?;
        self.verify_response_record(&response_frame, &response_record_proof)?;
        let response = decode_response(
            &response_frame,
            self.limits,
            self.versions(),
        )?;
        if response.version() != self.binding.version()
            || response.request_id() != &request_id
        {
            return Err(TypedClientError::ResponseMismatch);
        }
        let expected_response_proof =
            keyed_bytes(&self.key, &response_transcript(&response));
        verify_response_proof(&response, &expected_response_proof)?;

        let (response_body_frame, response_body_record_proof) =
            self.read_record(deadline)?;
        self.verify_response_record(
            &response_body_frame,
            &response_body_record_proof,
        )?;
        let response_body_payload =
            FrameCodec::decode(&response_body_frame, self.limits)?;
        let observed_body_digest = ProofDigest::from_bytes(
            *blake3::hash(response_body_payload.as_slice()).as_bytes(),
        );
        if !verify_proof(response.body_digest(), &observed_body_digest) {
            return Err(ProtocolError::AuthenticationFailed.into());
        }
        let response_body = decode_standalone_grant_response_body(
            response.status(),
            response_body_payload.as_slice(),
        )?;
        remaining(deadline)?;

        match response_body {
            StandaloneGrantResponseBodyV1::Claims(claims) => {
                validate_claims(self, &request, &claims, trusted)?;
                self.grant = Some(claims.clone());
                Ok(claims)
            }
            StandaloneGrantResponseBodyV1::Failure(failure) => {
                Err(TypedClientError::GrantRejected {
                    status: response.status(),
                    failure,
                })
            }
        }
    }

    fn read_record(
        &mut self,
        deadline: Instant,
    ) -> Result<(Vec<u8>, ProofDigest), TypedClientError> {
        loop {
            if let Poll::Ready(record) = self.socket.poll_record(
                self.limits,
                deadline,
                POLL_INTERVAL,
            )? {
                return Ok(record);
            }
        }
    }

    fn verify_response_record(
        &self,
        frame: &[u8],
        observed: &ProofDigest,
    ) -> Result<(), TypedClientError> {
        let transcript = ProviderFrameTranscript::response(
            self.pairing.session(),
            self.nonce,
            frame,
        );
        let expected = keyed_parts(&self.key, transcript.parts());
        if verify_proof(&expected, observed) {
            Ok(())
        } else {
            Err(ProtocolError::AuthenticationFailed.into())
        }
    }
}

fn validate_claims(
    state: &State,
    request: &StandaloneGrantRequestV1,
    claims: &SearchReadGrantClaims,
    trusted_installation_id: InstallationId,
) -> Result<(), TypedClientError> {
    claims
        .validate_shape()
        .map_err(|_| TypedClientError::ResponseMismatch)?;
    if claims.installation_id != trusted_installation_id
        || claims.binding_id != state.binding.binding_id()
        || claims.installation_incarnation_id != state.binding.incarnation()
        || claims.allowed_membership_ids.is_empty()
        || claims.allowed_modalities.is_empty()
        || claims.permitted_recipe_families.is_empty()
        || !is_subset(
            &claims.allowed_membership_ids,
            &request.requested_membership_ids,
        )
        || !is_subset(
            &claims.allowed_corpus_or_portfolio_ids,
            &request.requested_corpus_or_portfolio_ids,
        )
        || !is_subset(
            &claims.allowed_access_partitions,
            &request.requested_access_partitions,
        )
        || !is_subset(
            &claims.allowed_modalities,
            &request.requested_modalities,
        )
        || !is_subset(
            &claims.permitted_recipe_families,
            &request.requested_recipe_families,
        )
        || claims.maximum_budget_class != request.requested_budget_class
        || claims.sensitivity_ceiling > request.requested_sensitivity_ceiling
        || claims.disclosure_ceiling > request.requested_disclosure_ceiling
        || (claims.source_read_permission
            && !request.requested_source_read_permission)
        || (claims.exact_scan_permission
            && !request.requested_exact_scan_permission)
        || (claims.exact_scan_permission && !claims.source_read_permission)
        || (contains_portfolio(&claims.allowed_corpus_or_portfolio_ids)
            != claims.reference_portfolio_revision.is_some())
    {
        return Err(TypedClientError::GrantMismatch);
    }
    Ok(())
}

fn keyed_bytes(key: &search_provider_protocol::BindingKey, bytes: &[u8]) -> ProofDigest {
    key.with_bytes(|key_bytes| {
        ProofDigest::from_bytes(*blake3::keyed_hash(key_bytes, bytes).as_bytes())
    })
}

fn is_subset<T: Ord, const LIMIT: usize>(
    values: &BoundedSet<T, LIMIT>,
    ceiling: &BoundedSet<T, LIMIT>,
) -> bool {
    values.iter().all(|value| ceiling.contains(value))
}

fn contains_portfolio(
    values: &BoundedSet<CorpusOrPortfolioId, MAX_SET_ITEMS>,
) -> bool {
    values
        .iter()
        .any(|value| matches!(value, CorpusOrPortfolioId::Portfolio(_)))
}
