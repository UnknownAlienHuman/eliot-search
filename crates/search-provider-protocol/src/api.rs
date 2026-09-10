//! Public entry module: the only cross-package entry point.
//!
//! All public operations enter through this module; the crate root
//! re-exports the same surface for intra-workspace callers. Layers, bottom
//! to top: [`frame`](crate::frame) (canonical `u32`-LE/JSON),
//! [`negotiation`](crate::negotiation) (exact major/minor),
//! [`pairing`](crate::pairing) (mutual-authentication ceremony),
//! [`request`](crate::request) (authenticated envelopes),
//! [`binding`](crate::binding) (session composition and admission order),
//! plus [`session`](crate::session), [`progress`](crate::progress),
//! [`terminal`](crate::terminal), [`cancel`](crate::cancel),
//! [`cleanup`](crate::cleanup), [`config`](crate::config) and
//! [`error`](crate::error).

pub use crate::binding::{
    BindingContext, BindingSession, BoundSession, NegotiatedHello, TransportPeer,
    authenticate_binding, project_capability_descriptor,
};
pub use crate::cancel::{CancelOutcome, cancel_request};
pub use crate::cleanup::{DisconnectReceipt, disconnect_all};
pub use crate::config::{DEFAULT_PROTOCOL_LIMITS, FRAME_PREFIX_BYTES, ProtocolLimits};
pub use crate::error::ProtocolError;
pub use crate::frame::{FrameCodec, decode_frame, encode_frame};
pub use crate::negotiation::{negotiate_hello, negotiate_version, validate_envelope_version};
pub use crate::pairing::{
    BindingKey, ClientNonce, PAIRING_CLIENT_DOMAIN, PAIRING_SERVER_DOMAIN, PairingChallenge,
    PairingLedger, PairingMachine, PairingState, PairingTranscript, ProofDigest, ServerNonce,
    SessionId, VerifiedPairing, client_proof_transcript, server_proof_transcript, verify_proof,
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
