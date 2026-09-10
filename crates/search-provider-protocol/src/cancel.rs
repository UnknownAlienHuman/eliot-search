//! Idempotent request cancellation with deterministic resource release.
//!
//! Cancelling an in-flight request releases its in-flight slot exactly once
//! and marks its guard; cancelling an unknown or already-terminal identity
//! returns a bounded non-sensitive outcome instead of an error.

use std::collections::BTreeMap;

use search_contracts::RequestId;

use crate::request::{InFlightRegistry, RequestGuard};

/// Bounded outcome of one cancellation request.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CancelOutcome {
    /// The request was in flight and its slot is now released.
    ///
    /// `terminal` reports whether the guard already emitted its terminal
    /// response before the cancel arrived.
    Cancelled {
        /// Whether a terminal response already exists for this request.
        terminal: bool,
    },
    /// The identity is unknown or already terminal; nothing was released.
    UnknownOrTerminal,
}

/// Idempotently cancels one request and releases its in-flight slot.
///
/// The first cancel of an in-flight identity releases exactly one slot; every
/// later cancel of the same identity reports [`CancelOutcome::UnknownOrTerminal`].
pub fn cancel_request(
    registry: &mut InFlightRegistry,
    guards: &mut BTreeMap<RequestId, RequestGuard>,
    target: &RequestId,
) -> CancelOutcome {
    if !registry.remove(target) {
        return CancelOutcome::UnknownOrTerminal;
    }
    let terminal = guards.get_mut(target).is_some_and(|guard| {
        guard.mark_cancelled();
        guard
            .progress()
            .is_some_and(|progress| progress.terminal().is_some())
    });
    CancelOutcome::Cancelled { terminal }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DEFAULT_PROTOCOL_LIMITS, ProtocolLimits};
    use crate::request::{InFlightEntry, MonotonicMillis};

    fn guard(id: RequestId) -> RequestGuard {
        RequestGuard::new(
            id,
            1,
            MonotonicMillis::new(10),
            None,
            ProtocolLimits {
                max_replay_entries: 8,
                max_in_flight_requests: 8,
                max_progress_total: 8,
                ..DEFAULT_PROTOCOL_LIMITS
            },
        )
        .expect("guard")
    }

    #[test]
    fn cancel_is_idempotent_and_releases_once() {
        let mut registry = InFlightRegistry::new(4).expect("registry");
        let mut guards = BTreeMap::new();
        let id = RequestId::from_bytes([7; 16]);
        registry
            .insert(id, InFlightEntry::new(1, MonotonicMillis::new(10), None))
            .expect("insert");
        guards.insert(id, guard(id));
        assert_eq!(
            cancel_request(&mut registry, &mut guards, &id),
            CancelOutcome::Cancelled { terminal: false }
        );
        assert!(registry.is_empty());
        assert!(guards[&id].is_cancelled());
        assert_eq!(
            cancel_request(&mut registry, &mut guards, &id),
            CancelOutcome::UnknownOrTerminal
        );
        assert_eq!(
            cancel_request(&mut registry, &mut guards, &RequestId::from_bytes([8; 16])),
            CancelOutcome::UnknownOrTerminal
        );
    }
}
