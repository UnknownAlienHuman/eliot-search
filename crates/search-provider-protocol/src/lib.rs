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
use core::num::NonZeroU16;
use std::collections::BTreeSet;

/// Fixed wire magic for protocol version one framing.
pub const FRAME_MAGIC: [u8; 4] = *b"ELS1";
/// Fixed header length in bytes.
pub const FRAME_HEADER_BYTES: usize = 36;
/// Conservative default protocol limits.
pub const DEFAULT_PROTOCOL_LIMITS: ProtocolLimits = ProtocolLimits {
    max_frame_bytes: 8 * 1024 * 1024,
    max_body_bytes: 8 * 1024 * 1024 - FRAME_HEADER_BYTES,
    max_replay_entries: 4_096,
    max_progress_total: 1_000_000_000,
};

/// Closed protocol failure registry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProtocolError {
    /// Complete frame exceeds the configured byte ceiling.
    FrameTooLarge,
    /// Complete frame is shorter than the fixed header.
    TruncatedHeader,
    /// Frame magic does not identify this protocol.
    InvalidMagic,
    /// Reserved header flags are non-zero.
    ReservedFlags,
    /// Closed frame-kind tag is unknown.
    UnknownFrameKind,
    /// Declared body length and received bytes disagree.
    LengthMismatch,
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
            Self::TruncatedHeader => "PROTOCOL_TRUNCATED_HEADER",
            Self::InvalidMagic => "PROTOCOL_INVALID_MAGIC",
            Self::ReservedFlags => "PROTOCOL_RESERVED_FLAGS",
            Self::UnknownFrameKind => "PROTOCOL_UNKNOWN_FRAME_KIND",
            Self::LengthMismatch => "PROTOCOL_LENGTH_MISMATCH",
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
        if self.max_frame_bytes < FRAME_HEADER_BYTES
            || self.max_body_bytes > self.max_frame_bytes - FRAME_HEADER_BYTES
            || self.max_replay_entries == 0
            || self.max_progress_total == 0
        {
            Err(ProtocolError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Non-zero protocol version.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProtocolVersion(NonZeroU16);

impl ProtocolVersion {
    /// Creates a non-zero version.
    pub const fn new(value: u16) -> Result<Self, ProtocolError> {
        match NonZeroU16::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(ProtocolError::InvalidVersion),
        }
    }

    /// Numeric version value.
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

/// Inclusive supported version range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolRange {
    min: ProtocolVersion,
    max: ProtocolVersion,
}

impl ProtocolRange {
    /// Creates an ordered inclusive range.
    pub const fn new(
        min: ProtocolVersion,
        max: ProtocolVersion,
    ) -> Result<Self, ProtocolError> {
        if min.get() > max.get() {
            Err(ProtocolError::InvalidVersion)
        } else {
            Ok(Self { min, max })
        }
    }

    /// Lowest supported version.
    pub const fn min(self) -> ProtocolVersion {
        self.min
    }

    /// Highest supported version.
    pub const fn max(self) -> ProtocolVersion {
        self.max
    }

    /// Returns whether the range contains a version.
    pub const fn contains(self, version: ProtocolVersion) -> bool {
        version.get() >= self.min.get() && version.get() <= self.max.get()
    }
}

/// Selects the highest mutually supported protocol version.
pub const fn negotiate_version(
    client: ProtocolRange,
    provider: ProtocolRange,
) -> Result<ProtocolVersion, ProtocolError> {
    let lower = if client.min().get() > provider.min().get() {
        client.min()
    } else {
        provider.min()
    };
    let upper = if client.max().get() < provider.max().get() {
        client.max()
    } else {
        provider.max()
    };
    if lower.get() > upper.get() {
        Err(ProtocolError::NoCompatibleVersion)
    } else {
        Ok(upper)
    }
}

/// Opaque fixed-size request correlation identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RequestId([u8; 16]);

impl RequestId {
    /// Creates a fixed-size request identity.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Exact wire bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Closed frame-kind registry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FrameKind {
    /// Client hello and supported version range.
    ClientHello = 1,
    /// Provider hello and negotiated version.
    ProviderHello = 2,
    /// Pairing challenge.
    PairingChallenge = 3,
    /// Pairing proof.
    PairingProof = 4,
    /// Bounded request body.
    Request = 5,
    /// Monotone progress update.
    Progress = 6,
    /// Cancellation request.
    Cancel = 7,
    /// Exactly one terminal response.
    Terminal = 8,
    /// Graceful drain notification.
    Drain = 9,
}

impl TryFrom<u8> for FrameKind {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::ClientHello),
            2 => Ok(Self::ProviderHello),
            3 => Ok(Self::PairingChallenge),
            4 => Ok(Self::PairingProof),
            5 => Ok(Self::Request),
            6 => Ok(Self::Progress),
            7 => Ok(Self::Cancel),
            8 => Ok(Self::Terminal),
            9 => Ok(Self::Drain),
            _ => Err(ProtocolError::UnknownFrameKind),
        }
    }
}

/// One complete bounded protocol frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireFrame {
    version: ProtocolVersion,
    kind: FrameKind,
    sequence: u64,
    request_id: RequestId,
    body: Vec<u8>,
}

impl WireFrame {
    /// Creates a frame after validating its body against limits.
    pub fn new(
        version: ProtocolVersion,
        kind: FrameKind,
        sequence: u64,
        request_id: RequestId,
        body: Vec<u8>,
        limits: ProtocolLimits,
    ) -> Result<Self, ProtocolError> {
        let limits = limits.validate()?;
        if body.len() > limits.max_body_bytes {
            return Err(ProtocolError::FrameTooLarge);
        }
        Ok(Self {
            version,
            kind,
            sequence,
            request_id,
            body,
        })
    }

    /// Negotiated version.
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Closed frame kind.
    pub const fn kind(&self) -> FrameKind {
        self.kind
    }

    /// Direction-local sequence.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Request correlation identity.
    pub const fn request_id(&self) -> RequestId {
        self.request_id
    }

    /// Finite body bytes.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Encodes the complete bounded frame.
    pub fn encode(&self, limits: ProtocolLimits) -> Result<Vec<u8>, ProtocolError> {
        let limits = limits.validate()?;
        if self.body.len() > limits.max_body_bytes {
            return Err(ProtocolError::FrameTooLarge);
        }
        let total = FRAME_HEADER_BYTES
            .checked_add(self.body.len())
            .ok_or(ProtocolError::FrameTooLarge)?;
        if total > limits.max_frame_bytes {
            return Err(ProtocolError::FrameTooLarge);
        }
        let body_len = u32::try_from(self.body.len())
            .map_err(|_| ProtocolError::FrameTooLarge)?;
        let mut encoded = Vec::with_capacity(total);
        encoded.extend_from_slice(&FRAME_MAGIC);
        encoded.extend_from_slice(&self.version.get().to_be_bytes());
        encoded.push(self.kind as u8);
        encoded.push(0);
        encoded.extend_from_slice(&self.sequence.to_be_bytes());
        encoded.extend_from_slice(self.request_id.as_bytes());
        encoded.extend_from_slice(&body_len.to_be_bytes());
        encoded.extend_from_slice(&self.body);
        Ok(encoded)
    }

    /// Decodes one complete frame with length checks before body allocation.
    pub fn decode(bytes: &[u8], limits: ProtocolLimits) -> Result<Self, ProtocolError> {
        let limits = limits.validate()?;
        if bytes.len() > limits.max_frame_bytes {
            return Err(ProtocolError::FrameTooLarge);
        }
        if bytes.len() < FRAME_HEADER_BYTES {
            return Err(ProtocolError::TruncatedHeader);
        }
        if bytes[..4] != FRAME_MAGIC {
            return Err(ProtocolError::InvalidMagic);
        }
        if bytes[7] != 0 {
            return Err(ProtocolError::ReservedFlags);
        }
        let version = ProtocolVersion::new(u16::from_be_bytes([bytes[4], bytes[5]]))?;
        let kind = FrameKind::try_from(bytes[6])?;
        let sequence = u64::from_be_bytes(
            bytes[8..16]
                .try_into()
                .map_err(|_| ProtocolError::TruncatedHeader)?,
        );
        let request_id = RequestId::from_bytes(
            bytes[16..32]
                .try_into()
                .map_err(|_| ProtocolError::TruncatedHeader)?,
        );
        let body_len = usize::try_from(u32::from_be_bytes(
            bytes[32..36]
                .try_into()
                .map_err(|_| ProtocolError::TruncatedHeader)?,
        ))
        .map_err(|_| ProtocolError::FrameTooLarge)?;
        if body_len > limits.max_body_bytes {
            return Err(ProtocolError::FrameTooLarge);
        }
        let expected = FRAME_HEADER_BYTES
            .checked_add(body_len)
            .ok_or(ProtocolError::FrameTooLarge)?;
        if bytes.len() != expected {
            return Err(ProtocolError::LengthMismatch);
        }
        Self::new(
            version,
            kind,
            sequence,
            request_id,
            bytes[FRAME_HEADER_BYTES..].to_vec(),
            limits,
        )
    }
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
    pub const fn require_accepted(
        observation: SequenceObservation,
    ) -> Result<(), ProtocolError> {
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
            sequences: BidirectionalSequence::new(
                client_first_sequence,
                provider_first_sequence,
            ),
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

    fn version(value: u16) -> ProtocolVersion {
        ProtocolVersion::new(value).expect("version")
    }

    #[test]
    fn highest_version_overlap_is_selected() {
        let selected = negotiate_version(
            ProtocolRange::new(version(1), version(4)).expect("range"),
            ProtocolRange::new(version(3), version(6)).expect("range"),
        )
        .expect("overlap");
        assert_eq!(selected.get(), 4);
    }

    #[test]
    fn frame_round_trips_exactly() {
        let frame = WireFrame::new(
            version(1),
            FrameKind::Request,
            7,
            RequestId::from_bytes([3; 16]),
            vec![1, 2, 3],
            DEFAULT_PROTOCOL_LIMITS,
        )
        .expect("frame");
        let encoded = frame
            .encode(DEFAULT_PROTOCOL_LIMITS)
            .expect("encode");
        assert_eq!(
            WireFrame::decode(&encoded, DEFAULT_PROTOCOL_LIMITS).expect("decode"),
            frame
        );
    }

    #[test]
    fn oversize_is_rejected_before_body_copy() {
        let bytes = vec![0; DEFAULT_PROTOCOL_LIMITS.max_frame_bytes + 1];
        assert_eq!(
            WireFrame::decode(&bytes, DEFAULT_PROTOCOL_LIMITS),
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
        assert_eq!(sequences.client_mut().observe(1), SequenceObservation::Accepted);
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
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 100)
            .expect("session");
        session.negotiate(version(1)).expect("negotiate");
        assert_eq!(
            session.admit_request(RequestId::from_bytes([1; 16]), 1),
            Err(ProtocolError::AuthenticationRequired)
        );
    }

    #[test]
    fn replay_is_rejected_after_authentication() {
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 100)
            .expect("session");
        session.negotiate(version(1)).expect("negotiate");
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
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 100)
            .expect("session");
        session.negotiate(version(1)).expect("negotiate");
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
        let mut progress = ProgressState::new(2, DEFAULT_PROTOCOL_LIMITS)
            .expect("progress");
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
