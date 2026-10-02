//! Public entry module: the only cross-package entry point.
//!
//! All public operations enter through this module; the crate root
//! re-exports the same surface for intra-workspace callers. Layers, bottom
//! to top: [`frame`](crate::frame) (canonical `u32`-LE/JSON),
//! [`negotiation`](crate::negotiation) (exact major/minor),
//! [`pairing`](crate::pairing) (mutual-authentication ceremony),
//! [`pairing_wire`](crate::pairing_wire) (bounded pre-session proof records),
//! [`endpoint_name`](crate::endpoint_name) (bounded installation-scoped IPC name),
//! [`native_endpoint`](crate::native_endpoint) (authenticated local endpoint records),
//! [`request`](crate::request) (authenticated shell envelopes),
//! [`grant`](crate::grant) (bounded standalone-grant request/response bodies and
//! dedicated authenticated request envelope), [`binding`](crate::binding)
//! (session composition and admission order), plus [`session`](crate::session),
//! [`progress`](crate::progress), [`terminal`](crate::terminal),
//! [`cancel`](crate::cancel), [`cleanup`](crate::cleanup),
//! [`config`](crate::config), the private indexed-query framing layer, and
//! [`error`](crate::error).

pub use crate::binding::{
    AdmittedProviderRequest, BindingContext, BindingSession, BoundSession, NegotiatedHello,
    ProviderDeliveryError, ProviderFrameTranscript, SessionDrainHandle, TransportPeer,
    authenticate_binding, project_capability_descriptor,
};
pub use crate::cancel::{CancelOutcome, cancel_request};
pub use crate::cleanup::{DisconnectReceipt, disconnect_all};
pub use crate::config::{DEFAULT_PROTOCOL_LIMITS, FRAME_PREFIX_BYTES, ProtocolLimits};
pub use crate::endpoint_name::{NATIVE_ENDPOINT_NAME_BYTES, NativeEndpointNameV1};
pub use crate::error::ProtocolError;
pub use crate::frame::{
    ClientEnvelopeCodec, FrameCodec, ServerEnvelopeCodec, TypedRecordBuffer,
    TypedTransportProfileV1,
    decode_frame, encode_frame,
};
pub use crate::grant::{
    AuthenticatedStandaloneGrantEnvelope, MAX_STANDALONE_GRANT_ENVELOPE_JSON_BYTES,
    MAX_STANDALONE_GRANT_REQUEST_BYTES, MAX_STANDALONE_GRANT_RESPONSE_BYTES,
    STANDALONE_GRANT_ENVELOPE_DOMAIN, STANDALONE_GRANT_REQUEST_VERSION,
    STANDALONE_GRANT_RESPONSE_VERSION, StandaloneGrantRequestV1,
    StandaloneGrantResponseBodyV1, decode_standalone_grant_envelope,
    decode_standalone_grant_envelope_json, decode_standalone_grant_request,
    decode_standalone_grant_response_body, encode_standalone_grant_envelope,
    encode_standalone_grant_envelope_json, encode_standalone_grant_request,
    encode_standalone_grant_response_body, seal_standalone_grant_envelope,
    standalone_grant_envelope_transcript, verify_standalone_grant_envelope_proof,
};
pub use crate::indexed::{
    INDEXED_QUERY_MARKER, IndexedQueryFrameError, STRICT_QUERY_PREFIX,
    decode_indexed_query, encode_indexed_query,
};
pub use crate::negotiation::{negotiate_hello, negotiate_version, validate_envelope_version};
pub use crate::native_endpoint::{
    MAX_NATIVE_ENDPOINT_DESCRIPTOR_BYTES, NativeEndpointDescriptorV1,
    decode_native_endpoint_descriptor, encode_native_endpoint_descriptor,
    native_endpoint_descriptor_transcript, provider_pairing_credential_locator_material,
};
pub use crate::pairing::{
    BindingKey, ClientNonce, PAIRING_CLIENT_DOMAIN, PAIRING_SERVER_DOMAIN, PairingChallenge,
    PairingLedger, PairingMachine, PairingState, PairingTranscript, ProofDigest, ServerNonce,
    SessionId, VerifiedPairing, client_proof_transcript, server_proof_transcript, verify_proof,
};
pub use crate::pairing_wire::{
    PAIRING_CHALLENGE_BYTES, PAIRING_PROOF_BYTES, PAIRING_VERIFIED_BYTES,
    PairingChallengeFrame, PairingProofFrame, PairingVerifiedFrame,
    decode_pairing_challenge, decode_pairing_proof, decode_pairing_verified,
    encode_pairing_challenge, encode_pairing_proof, encode_pairing_verified,
};
pub use crate::progress::{ProgressState, emit_progress};
pub use crate::request::{
    AuthenticatedEnvelope, AuthenticatedResponse, ControlCommand, ENVELOPE_REQUEST_DOMAIN,
    ENVELOPE_RESPONSE_DOMAIN, InFlightEntry, InFlightRegistry, MonotonicMillis, RequestGuard,
    RequestStatus, decode_envelope, decode_envelope_json, decode_response, decode_response_json,
    encode_envelope, encode_envelope_json, encode_response, encode_response_json,
    envelope_transcript, response_transcript, seal_envelope, seal_response, verify_envelope_proof,
    verify_response_proof,
};
pub use crate::session::{
    BidirectionalSequence, SequenceObservation, SequenceTracker, SessionMachine, SessionState,
};
pub use crate::terminal::{TerminalKind, emit_terminal};
