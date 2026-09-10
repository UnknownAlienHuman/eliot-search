//! Deterministic connection teardown: drain, cancel-all and release.
//!
//! Disconnect never fails: it releases every connection-owned request guard,
//! marks each cancelled, and reports the exact counts. It does not delete
//! durable handles and never fabricates completion of mutations that may
//! have committed.

use std::collections::BTreeMap;

use search_contracts::RequestId;

use crate::request::{InFlightRegistry, RequestGuard};

/// Exact receipt for one connection teardown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisconnectReceipt {
    cancelled_requests: usize,
    released_guards: usize,
}

impl DisconnectReceipt {
    /// Number of in-flight requests cancelled by the disconnect.
    #[must_use]
    pub const fn cancelled_requests(self) -> usize {
        self.cancelled_requests
    }

    /// Number of request guards released by the disconnect.
    #[must_use]
    pub const fn released_guards(self) -> usize {
        self.released_guards
    }
}

/// Cancels every in-flight request, marks every guard and releases all
/// connection-owned request state, reporting exact counts.
#[must_use]
pub fn disconnect_all(
    registry: &mut InFlightRegistry,
    guards: &mut BTreeMap<RequestId, RequestGuard>,
) -> DisconnectReceipt {
    let cancelled_requests = registry.len();
    for guard in guards.values_mut() {
        guard.mark_cancelled();
    }
    let released_guards = guards.len();
    registry.clear();
    guards.clear();
    DisconnectReceipt {
        cancelled_requests,
        released_guards,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DEFAULT_PROTOCOL_LIMITS, ProtocolLimits};
    use crate::request::{InFlightEntry, MonotonicMillis};

    #[test]
    fn disconnect_counts_are_exact() {
        let mut registry = InFlightRegistry::new(4).expect("registry");
        let mut guards = BTreeMap::new();
        let limits = ProtocolLimits {
            max_replay_entries: 8,
            max_in_flight_requests: 8,
            max_progress_total: 8,
            ..DEFAULT_PROTOCOL_LIMITS
        };
        for index in 1_u8..=3 {
            let id = RequestId::from_bytes([index; 16]);
            registry
                .insert(
                    id,
                    InFlightEntry::new(u64::from(index), MonotonicMillis::new(1), None),
                )
                .expect("insert");
            guards.insert(
                id,
                RequestGuard::new(id, u64::from(index), MonotonicMillis::new(1), None, limits)
                    .expect("guard"),
            );
        }
        let receipt = disconnect_all(&mut registry, &mut guards);
        assert_eq!(receipt.cancelled_requests(), 3);
        assert_eq!(receipt.released_guards(), 3);
        assert!(registry.is_empty());
        assert!(guards.is_empty());
        let again = disconnect_all(&mut registry, &mut guards);
        assert_eq!(again.cancelled_requests(), 0);
        assert_eq!(again.released_guards(), 0);
    }
}
