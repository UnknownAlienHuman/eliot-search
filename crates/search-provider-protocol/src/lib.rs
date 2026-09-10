//! Bounded authenticated local-provider protocol semantics.
//!
//! This package performs no socket, pipe, filesystem, process, or secret-store
//! I/O. Transport adapters supply complete finite frames and cryptographic proof
//! digests; this package validates limits, sequencing, replay, progress, and
//! session lifecycle before a daemon admits work.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

use core::fmt;
use std::collections::BTreeSet;

use search_contracts::protocol::{decode_json_frame, encode_json_frame};
use search_contracts::{BoundedBytes, ContractErrorKind, MAX_FRAME_BYTES, ProtocolErrorCode};

/// Canonical transport payload owned by `search-contracts`.
pub use search_contracts::protocol::{JsonFramePayload, ProviderEnvelope};
/// Canonical protocol identity owned by `search-contracts`.
///
/// Norm #89 forbids duplicate canonical `ProtocolVersion` / `ProtocolRange`
/// definitions: this package re-exports the canonical types instead of
/// defining its own single-`u16` versions.
pub use search_contracts::{ProtocolRange, ProtocolVersion, RequestId};

/// Length-prefix size in bytes for canonical framing (`u32` little-endian).
pub const FRAME_PREFIX_BYTES: usize = 4;
/// Conservative default protocol limits.
pub const DEFAULT_PROTOCOL_LIMITS: ProtocolLimits = ProtocolLimits {
    max_frame_bytes: MAX_FRAME_BYTES,
    max_body_bytes: MAX_FRAME_BYTES - FRAME_PREFIX_BYTES,
    max_replay_entries: 4_096,
    max_progress_total: 1_000_000_000,
};

/// Closed protocol failure registry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProtocolError {
    /// Complete frame exceeds the configured byte ceiling.
    FrameTooLarge,
    /// Length-prefixed JSON frame is malformed (truncated prefix, declared
    /// length mismatch, non-UTF-8 or non-JSON body).
    InvalidEnvelope,
    /// Version value or range is malformed.
    InvalidVersion,
    /// Client and provider version ranges do not overlap.
    NoCompatibleVersion,
    /// Immediately preceding sequence was repeated.
    DuplicateSequence,
    /// Sequence moved behind accepted history.
    SequenceRegression,
    /// Sequence skipped one or more expected values.
    SequenceGap,
    /// Direction-local sequence space is exhausted.
    SequenceExhausted,
    /// Request identity was already admitted.
    ReplayDetected,
    /// Finite replay ledger is full.
    ReplayCapacityExceeded,
    /// Progress moved backwards.
    ProgressRegression,
    /// Progress exceeded its declared total or configured ceiling.
    ProgressExceededTotal,
    /// More than one terminal response was attempted.
    DuplicateTerminal,
    /// Terminal success contradicts incomplete progress.
    IncompleteTerminalSuccess,
    /// Session transition is invalid.
    InvalidSessionTransition,
    /// Pairing or binding proof is absent or invalid.
    AuthenticationRequired,
    /// New requests were attempted after draining began.
    SessionDraining,
    /// Session is closed.
    SessionClosed,
    /// Contradictory protocol state is quarantined.
    Quarantined,
    /// Protocol limits are internally inconsistent.
    InvalidLimits,
}

impl ProtocolError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::FrameTooLarge => "PROTOCOL_FRAME_TOO_LARGE",
            Self::InvalidEnvelope => "PROTOCOL_INVALID_ENVELOPE",
            Self::InvalidVersion => "PROTOCOL_INVALID_VERSION",
            Self::NoCompatibleVersion => "PROTOCOL_NO_COMPATIBLE_VERSION",
            Self::DuplicateSequence => "PROTOCOL_DUPLICATE_SEQUENCE",
            Self::SequenceRegression => "PROTOCOL_SEQUENCE_REGRESSION",
            Self::SequenceGap => "PROTOCOL_SEQUENCE_GAP",
            Self::SequenceExhausted => "PROTOCOL_SEQUENCE_EXHAUSTED",
            Self::ReplayDetected => "PROTOCOL_REPLAY_DETECTED",
            Self::ReplayCapacityExceeded => "PROTOCOL_REPLAY_CAPACITY_EXCEEDED",
            Self::ProgressRegression => "PROTOCOL_PROGRESS_REGRESSION",
            Self::ProgressExceededTotal => "PROTOCOL_PROGRESS_EXCEEDED_TOTAL",
            Self::DuplicateTerminal => "PROTOCOL_DUPLICATE_TERMINAL",
            Self::IncompleteTerminalSuccess => "PROTOCOL_INCOMPLETE_TERMINAL_SUCCESS",
            Self::InvalidSessionTransition => "PROTOCOL_INVALID_SESSION_TRANSITION",
            Self::AuthenticationRequired => "PROTOCOL_AUTHENTICATION_REQUIRED",
            Self::SessionDraining => "PROTOCOL_SESSION_DRAINING",
            Self::SessionClosed => "PROTOCOL_SESSION_CLOSED",
            Self::Quarantined => "PROTOCOL_QUARANTINED",
            Self::InvalidLimits => "PROTOCOL_INVALID_LIMITS",
        }
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProtocolError {}

/// Finite protocol limits checked before body allocation or request admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolLimits {
    /// Maximum complete frame size.
    pub max_frame_bytes: usize,
    /// Maximum frame body size.
    pub max_body_bytes: usize,
    /// Maximum retained request identities in one session.
    pub max_replay_entries: usize,
    /// Maximum declared progress denominator.
    pub max_progress_total: u64,
}

impl ProtocolLimits {
    /// Validates finite internally consistent limits.
    pub const fn validate(self) -> Result<Self, ProtocolError> {
        if self.max_frame_bytes < FRAME_PREFIX_BYTES
            || self.max_frame_bytes > MAX_FRAME_BYTES
            || self.max_body_bytes > self.max_frame_bytes - FRAME_PREFIX_BYTES
            || self.max_replay_entries == 0
            || self.max_progress_total == 0
        {
            Err(ProtocolError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Maps a canonical frame failure to the package failure registry.
const fn map_frame_error(code: ProtocolErrorCode) -> ProtocolError {
    match code {
        ProtocolErrorCode::FrameTooLarge => ProtocolError::FrameTooLarge,
        _ => ProtocolError::InvalidEnvelope,
    }
}

/// Canonical `u32` little-endian length plus UTF-8 JSON framing.
///
/// This is a thin limit-enforcing wrapper over
/// `search-contracts::protocol::{encode_json_frame, decode_json_frame}`
/// (`search-contracts/src/protocol.rs:302-336`). Baseline performs no
/// compression and no fragmented message assembly; the 8 MiB ceiling includes
/// the 4-byte prefix.
pub struct FrameCodec;

impl FrameCodec {
    /// Emits `u32` little-endian length plus canonical UTF-8 JSON.
    pub fn encode(
        payload: &JsonFramePayload,
        limits: ProtocolLimits,
    ) -> Result<BoundedBytes<MAX_FRAME_BYTES>, ProtocolError> {
        encode_frame(payload, limits)
    }

    /// Validates length before body allocation, then UTF-8/JSON shape.
    pub fn decode(bytes: &[u8], limits: ProtocolLimits) -> Result<JsonFramePayload, ProtocolError> {
        decode_frame(bytes, limits)
    }
}

/// Emits `u32` little-endian length plus canonical UTF-8 JSON.
pub fn encode_frame(
    payload: &JsonFramePayload,
    limits: ProtocolLimits,
) -> Result<BoundedBytes<MAX_FRAME_BYTES>, ProtocolError> {
    let limits = limits.validate()?;
    if payload.as_slice().len() > limits.max_body_bytes
        || payload.as_slice().len().saturating_add(FRAME_PREFIX_BYTES) > limits.max_frame_bytes
    {
        return Err(ProtocolError::FrameTooLarge);
    }
    encode_json_frame(payload).map_err(map_frame_error)
}

/// Validates length before body allocation, then UTF-8/JSON shape.
///
/// Oversize input is rejected without unbounded buffering: the configured
/// ceiling is enforced from the declared `u32` prefix before the canonical
/// decoder copies the body.
pub fn decode_frame(
    bytes: &[u8],
    limits: ProtocolLimits,
) -> Result<JsonFramePayload, ProtocolError> {
    let limits = limits.validate()?;
    if bytes.len() > limits.max_frame_bytes {
        return Err(ProtocolError::FrameTooLarge);
    }
    if bytes.len() < FRAME_PREFIX_BYTES {
        return Err(ProtocolError::InvalidEnvelope);
    }
    let declared = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let declared = usize::try_from(declared).map_err(|_| ProtocolError::FrameTooLarge)?;
    if declared > limits.max_body_bytes
        || declared.saturating_add(FRAME_PREFIX_BYTES) > limits.max_frame_bytes
    {
        return Err(ProtocolError::FrameTooLarge);
    }
    decode_json_frame(bytes).map_err(map_frame_error)
}

/// Selects the highest mutually supported `(major, minor)` version.
///
/// Major mismatch fails. Minor negotiation is explicit and cannot reinterpret
/// load-bearing fields. Delegates to the canonical
/// `search-contracts::protocol::ProtocolRange::negotiate`.
pub fn negotiate_hello(
    local: ProtocolRange,
    remote: ProtocolRange,
) -> Result<ProtocolVersion, ProtocolError> {
    local
        .negotiate(remote)
        .map_err(|_| ProtocolError::NoCompatibleVersion)
}

/// Alias for [`negotiate_hello`] preserved for intra-package callers.
pub fn negotiate_version(
    client: ProtocolRange,
    provider: ProtocolRange,
) -> Result<ProtocolVersion, ProtocolError> {
    negotiate_hello(client, provider)
}

/// Validates the envelope tag and requires its version to be in the
/// negotiated canonical range.
pub fn validate_envelope_version(
    envelope: &ProviderEnvelope,
    supported: ProtocolRange,
) -> Result<(), ProtocolError> {
    envelope
        .validate_version_and_limits(supported)
        .map_err(|error| {
            if error.kind() == ContractErrorKind::UnsupportedVersion {
                ProtocolError::NoCompatibleVersion
            } else {
                ProtocolError::InvalidEnvelope
            }
        })
}

/// Direction-local sequence observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceObservation {
    /// Exact next value accepted.
    Accepted,
    /// Immediately preceding value repeated.
    Duplicate,
    /// Value moved behind accepted history.
    Regression,
    /// Value skipped one or more expected values.
    Gap,
    /// Sequence space cannot advance beyond `u64::MAX`.
    Exhausted,
}

/// Strict contiguous sequence tracker for one direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceTracker {
    next_expected: Option<u64>,
    last_accepted: Option<u64>,
}

impl SequenceTracker {
    /// Creates a tracker beginning at `first_expected`.
    pub const fn new(first_expected: u64) -> Self {
        Self {
            next_expected: Some(first_expected),
            last_accepted: None,
        }
    }

    /// Exact next sequence, or `None` after exhaustion.
    pub const fn next_expected(self) -> Option<u64> {
        self.next_expected
    }

    /// Observes and conditionally advances one sequence.
    pub fn observe(&mut self, sequence: u64) -> SequenceObservation {
        if self.last_accepted == Some(sequence) {
            return SequenceObservation::Duplicate;
        }
        match self.next_expected {
            Some(expected) if sequence == expected => {
                self.last_accepted = Some(sequence);
                self.next_expected = expected.checked_add(1);
                SequenceObservation::Accepted
            }
            Some(expected) if sequence < expected => SequenceObservation::Regression,
            Some(_) => SequenceObservation::Gap,
            None => SequenceObservation::Exhausted,
        }
    }

    /// Converts an observation to a typed protocol result.
    pub const fn require_accepted(observation: SequenceObservation) -> Result<(), ProtocolError> {
        match observation {
            SequenceObservation::Accepted => Ok(()),
            SequenceObservation::Duplicate => Err(ProtocolError::DuplicateSequence),
            SequenceObservation::Regression => Err(ProtocolError::SequenceRegression),
            SequenceObservation::Gap => Err(ProtocolError::SequenceGap),
            SequenceObservation::Exhausted => Err(ProtocolError::SequenceExhausted),
        }
    }
}

/// Independent client-to-provider and provider-to-client sequence spaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BidirectionalSequence {
    client: SequenceTracker,
    provider: SequenceTracker,
}

impl BidirectionalSequence {
    /// Creates independent sequence spaces.
    pub const fn new(client_first: u64, provider_first: u64) -> Self {
        Self {
            client: SequenceTracker::new(client_first),
            provider: SequenceTracker::new(provider_first),
        }
    }

    /// Mutable client-to-provider tracker.
    pub const fn client_mut(&mut self) -> &mut SequenceTracker {
        &mut self.client
    }

    /// Mutable provider-to-client tracker.
    pub const fn provider_mut(&mut self) -> &mut SequenceTracker {
        &mut self.provider
    }

    /// Client-to-provider state.
    pub const fn client(&self) -> SequenceTracker {
        self.client
    }

    /// Provider-to-client state.
    pub const fn provider(&self) -> SequenceTracker {
        self.provider
    }
}

/// Fixed-size cryptographic binding-proof digest.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProofDigest([u8; 32]);

impl ProofDigest {
    /// Creates a proof digest produced by the secret-owning adapter.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Exact bytes for framing or platform cryptographic verification.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for ProofDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProofDigest(<redacted>)")
    }
}

/// Performs constant-work equality over fixed-size proof digests.
#[must_use]
pub fn verify_proof(expected: &ProofDigest, observed: &ProofDigest) -> bool {
    let mut difference = 0_u8;
    for (left, right) in expected.0.iter().zip(observed.0.iter()) {
        difference |= left ^ right;
    }
    difference == 0
}

/// Closed request command supported by the bootable W1 shell.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ControlCommand {
    /// Return bounded daemon health and version information.
    Health,
    /// Return protocol and build version information.
    Version,
    /// Begin authenticated graceful shutdown.
    Shutdown,
}

/// Closed terminal response class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TerminalKind {
    /// Operation completed its declared work.
    Success,
    /// Operation completed with explicit partial coverage.
    Partial,
    /// Operation was cancelled before success.
    Cancelled,
    /// Operation failed before a verified success postcondition.
    Failed,
    /// A possible mutation requires authoritative readback.
    OutcomeUnknown,
}

/// Monotone progress and exactly-one terminal response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressState {
    total: u64,
    completed: u64,
    terminal: Option<TerminalKind>,
}

impl ProgressState {
    /// Creates a finite progress counter within limits.
    pub fn new(total: u64, limits: ProtocolLimits) -> Result<Self, ProtocolError> {
        let limits = limits.validate()?;
        if total > limits.max_progress_total {
            return Err(ProtocolError::ProgressExceededTotal);
        }
        Ok(Self {
            total,
            completed: 0,
            terminal: None,
        })
    }

    /// Declared total work units.
    pub const fn total(self) -> u64 {
        self.total
    }

    /// Monotone completed work units.
    pub const fn completed(self) -> u64 {
        self.completed
    }

    /// Terminal response when already emitted.
    pub const fn terminal(self) -> Option<TerminalKind> {
        self.terminal
    }

    /// Advances progress monotonically and within its denominator.
    pub fn advance(&mut self, completed: u64) -> Result<(), ProtocolError> {
        if self.terminal.is_some() {
            return Err(ProtocolError::DuplicateTerminal);
        }
        if completed < self.completed {
            return Err(ProtocolError::ProgressRegression);
        }
        if completed > self.total {
            return Err(ProtocolError::ProgressExceededTotal);
        }
        self.completed = completed;
        Ok(())
    }

    /// Records exactly one terminal response.
    pub fn finish(&mut self, terminal: TerminalKind) -> Result<(), ProtocolError> {
        if self.terminal.is_some() {
            return Err(ProtocolError::DuplicateTerminal);
        }
        if terminal == TerminalKind::Success && self.completed != self.total {
            return Err(ProtocolError::IncompleteTerminalSuccess);
        }
        self.terminal = Some(terminal);
        Ok(())
    }
}

/// Provider-session lifecycle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SessionState {
    /// Transport exists but no version is selected.
    Offered,
    /// Version was selected; binding proof is still required.
    Negotiated,
    /// Authenticated session admits bounded requests.
    Active,
    /// New work is denied while accepted work drains.
    Draining,
    /// Session ended and cannot reopen.
    Closed,
    /// Contradictory state blocks all requests.
    Quarantined,
}

/// Deterministic authenticated session machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionMachine {
    state: SessionState,
    version: Option<ProtocolVersion>,
    binding_verified: bool,
    sequences: BidirectionalSequence,
    replay: BTreeSet<RequestId>,
    limits: ProtocolLimits,
}

impl SessionMachine {
    /// Creates an offered unauthenticated session.
    pub fn new(
        limits: ProtocolLimits,
        client_first_sequence: u64,
        provider_first_sequence: u64,
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            state: SessionState::Offered,
            version: None,
            binding_verified: false,
            sequences: BidirectionalSequence::new(client_first_sequence, provider_first_sequence),
            replay: BTreeSet::new(),
            limits: limits.validate()?,
        })
    }

    /// Current session state.
    pub const fn state(&self) -> SessionState {
        self.state
    }

    /// Negotiated version when present.
    pub const fn version(&self) -> Option<ProtocolVersion> {
        self.version
    }

    /// Direction-local sequence state.
    pub const fn sequences(&self) -> &BidirectionalSequence {
        &self.sequences
    }

    /// Records exact version negotiation.
    pub fn negotiate(&mut self, version: ProtocolVersion) -> Result<(), ProtocolError> {
        if self.state != SessionState::Offered {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        self.version = Some(version);
        self.state = SessionState::Negotiated;
        Ok(())
    }

    /// Activates after a constant-work binding-proof comparison.
    pub fn activate(
        &mut self,
        expected: &ProofDigest,
        observed: &ProofDigest,
    ) -> Result<(), ProtocolError> {
        if self.state != SessionState::Negotiated {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        if !verify_proof(expected, observed) {
            return Err(ProtocolError::AuthenticationRequired);
        }
        self.binding_verified = true;
        self.state = SessionState::Active;
        Ok(())
    }

    /// Admits one exact client request after sequence and replay checks.
    pub fn admit_request(
        &mut self,
        request_id: RequestId,
        sequence: u64,
    ) -> Result<(), ProtocolError> {
        match self.state {
            SessionState::Active if self.binding_verified => {}
            SessionState::Offered | SessionState::Negotiated | SessionState::Active => {
                return Err(ProtocolError::AuthenticationRequired);
            }
            SessionState::Draining => return Err(ProtocolError::SessionDraining),
            SessionState::Closed => return Err(ProtocolError::SessionClosed),
            SessionState::Quarantined => return Err(ProtocolError::Quarantined),
        }
        SequenceTracker::require_accepted(self.sequences.client_mut().observe(sequence))?;
        if self.replay.contains(&request_id) {
            return Err(ProtocolError::ReplayDetected);
        }
        if self.replay.len() >= self.limits.max_replay_entries {
            return Err(ProtocolError::ReplayCapacityExceeded);
        }
        self.replay.insert(request_id);
        Ok(())
    }

    /// Accepts one provider-to-client sequence.
    pub fn accept_provider_sequence(&mut self, sequence: u64) -> Result<(), ProtocolError> {
        if !matches!(self.state, SessionState::Active | SessionState::Draining) {
            return Err(match self.state {
                SessionState::Closed => ProtocolError::SessionClosed,
                SessionState::Quarantined => ProtocolError::Quarantined,
                _ => ProtocolError::AuthenticationRequired,
            });
        }
        SequenceTracker::require_accepted(self.sequences.provider_mut().observe(sequence))
    }

    /// Begins graceful drain.
    pub fn begin_drain(&mut self) -> Result<(), ProtocolError> {
        if self.state != SessionState::Active || !self.binding_verified {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        self.state = SessionState::Draining;
        Ok(())
    }

    /// Closes an active or draining session.
    pub fn close(&mut self) -> Result<(), ProtocolError> {
        if !matches!(self.state, SessionState::Active | SessionState::Draining) {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        self.state = SessionState::Closed;
        self.binding_verified = false;
        Ok(())
    }

    /// Quarantines any non-closed session.
    pub fn quarantine(&mut self) -> Result<(), ProtocolError> {
        if self.state == SessionState::Closed {
            return Err(ProtocolError::SessionClosed);
        }
        self.state = SessionState::Quarantined;
        self.binding_verified = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u16, minor: u16) -> ProtocolVersion {
        ProtocolVersion { major, minor }
    }

    fn range(min_major: u16, min_minor: u16, max_major: u16, max_minor: u16) -> ProtocolRange {
        ProtocolRange::new(version(min_major, min_minor), version(max_major, max_minor))
            .expect("range")
    }

    #[test]
    fn negotiation_selects_highest_minor_and_rejects_major_mismatch() {
        let selected = negotiate_hello(range(1, 0, 1, 2), range(1, 1, 1, 5)).expect("overlap");
        assert_eq!(selected, version(1, 2));
        assert_eq!(
            negotiate_hello(range(1, 0, 1, 1), range(2, 0, 2, 0)),
            Err(ProtocolError::NoCompatibleVersion)
        );
        assert_eq!(
            negotiate_hello(range(1, 0, 1, 1), range(1, 2, 1, 3)),
            Err(ProtocolError::NoCompatibleVersion)
        );
    }

    #[test]
    fn canonical_types_are_not_duplicated() {
        assert_eq!(
            std::any::type_name::<ProtocolVersion>(),
            std::any::type_name::<search_contracts::protocol::ProtocolVersion>()
        );
        assert_eq!(
            std::any::type_name::<ProtocolRange>(),
            std::any::type_name::<search_contracts::protocol::ProtocolRange>()
        );
        assert_eq!(
            std::any::type_name::<RequestId>(),
            std::any::type_name::<search_contracts::RequestId>()
        );
    }

    #[test]
    fn canonical_framing_round_trips_with_le_prefix() {
        let payload = JsonFramePayload::new(br#"{"ok":true}"#.to_vec()).expect("payload");
        let encoded = FrameCodec::encode(&payload, DEFAULT_PROTOCOL_LIMITS).expect("encode");
        assert_eq!(&encoded.as_slice()[..4], &11_u32.to_le_bytes());
        assert_eq!(
            FrameCodec::decode(encoded.as_slice(), DEFAULT_PROTOCOL_LIMITS).expect("decode"),
            payload
        );
    }

    #[test]
    fn framing_rejects_truncated_and_length_mismatch() {
        assert_eq!(
            FrameCodec::decode(&[1, 0, 0], DEFAULT_PROTOCOL_LIMITS),
            Err(ProtocolError::InvalidEnvelope)
        );
        assert_eq!(
            FrameCodec::decode(&[2, 0, 0, 0, b'{'], DEFAULT_PROTOCOL_LIMITS),
            Err(ProtocolError::InvalidEnvelope)
        );
        assert_eq!(
            FrameCodec::decode(&[11, 0, 0, 0, 0xff], DEFAULT_PROTOCOL_LIMITS),
            Err(ProtocolError::InvalidEnvelope)
        );
    }

    #[test]
    fn oversize_is_rejected_before_body_copy() {
        let bytes = vec![0; DEFAULT_PROTOCOL_LIMITS.max_frame_bytes + 1];
        assert_eq!(
            FrameCodec::decode(&bytes, DEFAULT_PROTOCOL_LIMITS),
            Err(ProtocolError::FrameTooLarge)
        );
    }

    #[test]
    fn sequence_errors_are_distinct() {
        let mut tracker = SequenceTracker::new(10);
        assert_eq!(tracker.observe(10), SequenceObservation::Accepted);
        assert_eq!(tracker.observe(10), SequenceObservation::Duplicate);
        assert_eq!(tracker.observe(9), SequenceObservation::Regression);
        assert_eq!(tracker.observe(12), SequenceObservation::Gap);
    }

    #[test]
    fn direction_sequences_are_independent() {
        let mut sequences = BidirectionalSequence::new(1, 100);
        assert_eq!(
            sequences.client_mut().observe(1),
            SequenceObservation::Accepted
        );
        assert_eq!(sequences.provider().next_expected(), Some(100));
    }

    #[test]
    fn proof_comparison_processes_fixed_width_values() {
        let expected = ProofDigest::from_bytes([1; 32]);
        assert!(verify_proof(&expected, &ProofDigest::from_bytes([1; 32])));
        assert!(!verify_proof(&expected, &ProofDigest::from_bytes([2; 32])));
        assert!(!format!("{expected:?}").contains('1'));
    }

    #[test]
    fn unauthenticated_configuration_does_not_admit_requests() {
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 100).expect("session");
        session.negotiate(version(1, 0)).expect("negotiate");
        assert_eq!(
            session.admit_request(RequestId::from_bytes([1; 16]), 1),
            Err(ProtocolError::AuthenticationRequired)
        );
    }

    #[test]
    fn replay_is_rejected_after_authentication() {
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 100).expect("session");
        session.negotiate(version(1, 0)).expect("negotiate");
        let proof = ProofDigest::from_bytes([7; 32]);
        session.activate(&proof, &proof).expect("activate");
        let id = RequestId::from_bytes([1; 16]);
        session.admit_request(id, 1).expect("first");
        assert_eq!(
            session.admit_request(id, 2),
            Err(ProtocolError::ReplayDetected)
        );
    }

    #[test]
    fn drain_denies_new_work_and_close_is_terminal() {
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 100).expect("session");
        session.negotiate(version(1, 0)).expect("negotiate");
        let proof = ProofDigest::from_bytes([7; 32]);
        session.activate(&proof, &proof).expect("activate");
        session.begin_drain().expect("drain");
        assert_eq!(
            session.admit_request(RequestId::from_bytes([1; 16]), 1),
            Err(ProtocolError::SessionDraining)
        );
        session.close().expect("close");
        assert_eq!(session.quarantine(), Err(ProtocolError::SessionClosed));
    }

    #[test]
    fn success_requires_complete_progress_and_terminal_is_unique() {
        let mut progress = ProgressState::new(2, DEFAULT_PROTOCOL_LIMITS).expect("progress");
        progress.advance(1).expect("advance");
        assert_eq!(
            progress.finish(TerminalKind::Success),
            Err(ProtocolError::IncompleteTerminalSuccess)
        );
        progress.advance(2).expect("complete");
        progress.finish(TerminalKind::Success).expect("finish");
        assert_eq!(
            progress.finish(TerminalKind::Failed),
            Err(ProtocolError::DuplicateTerminal)
        );
    }
}
