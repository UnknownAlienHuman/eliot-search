//! Finite protocol limits checked before allocation or admission.
//!
//! Security floors — pairing requirement, frame/in-flight ceilings, no
//! compression and no fragmented assembly — cannot be weakened through these
//! limits: every ceiling is capped by its canonical `search-contracts` bound.

use search_contracts::{MAX_FRAME_BYTES, MAX_PROTOCOL_IN_FLIGHT};

use crate::error::ProtocolError;

/// Length-prefix size in bytes for canonical framing (`u32` little-endian).
pub const FRAME_PREFIX_BYTES: usize = 4;
/// Conservative default protocol limits.
pub const DEFAULT_PROTOCOL_LIMITS: ProtocolLimits = ProtocolLimits {
    max_frame_bytes: MAX_FRAME_BYTES,
    max_body_bytes: MAX_FRAME_BYTES - FRAME_PREFIX_BYTES,
    max_replay_entries: 4_096,
    max_in_flight_requests: MAX_PROTOCOL_IN_FLIGHT,
    max_progress_total: 1_000_000_000,
};

/// Finite protocol limits checked before body allocation or request admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolLimits {
    /// Maximum complete frame size.
    pub max_frame_bytes: usize,
    /// Maximum frame body size.
    pub max_body_bytes: usize,
    /// Maximum retained request identities in one session.
    pub max_replay_entries: usize,
    /// Maximum concurrent in-flight requests in one session.
    pub max_in_flight_requests: usize,
    /// Maximum declared progress denominator.
    pub max_progress_total: u64,
}

impl ProtocolLimits {
    /// Validates finite internally consistent limits.
    ///
    /// The in-flight ceiling can only be lowered below the canonical
    /// `MAX_PROTOCOL_IN_FLIGHT`, never raised above it.
    pub const fn validate(self) -> Result<Self, ProtocolError> {
        if self.max_frame_bytes < FRAME_PREFIX_BYTES
            || self.max_frame_bytes > MAX_FRAME_BYTES
            || self.max_body_bytes > self.max_frame_bytes - FRAME_PREFIX_BYTES
            || self.max_replay_entries == 0
            || self.max_in_flight_requests == 0
            || self.max_in_flight_requests > MAX_PROTOCOL_IN_FLIGHT
            || self.max_progress_total == 0
        {
            Err(ProtocolError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floors_cannot_be_weakened() {
        assert!(DEFAULT_PROTOCOL_LIMITS.validate().is_ok());
        assert_eq!(DEFAULT_PROTOCOL_LIMITS.max_in_flight_requests, 32);
        let raised = ProtocolLimits {
            max_in_flight_requests: MAX_PROTOCOL_IN_FLIGHT + 1,
            ..DEFAULT_PROTOCOL_LIMITS
        };
        assert_eq!(raised.validate(), Err(ProtocolError::InvalidLimits));
        let zero = ProtocolLimits {
            max_in_flight_requests: 0,
            ..DEFAULT_PROTOCOL_LIMITS
        };
        assert_eq!(zero.validate(), Err(ProtocolError::InvalidLimits));
        // Lowering the ceiling stays permitted.
        let lowered = ProtocolLimits {
            max_in_flight_requests: 4,
            ..DEFAULT_PROTOCOL_LIMITS
        };
        assert!(lowered.validate().is_ok());
    }
}
