//! One actual Native wall-clock observation for Source retention.
//!
//! This is a prerequisite, not an authority: the value below is a real
//! `SystemTime::now` reading taken under the original retained Native custody,
//! the original absolute deadline/cancellation and the original cumulative
//! meter. It is not a public clock, an issuer, a lease, a ready flag, a
//! standalone endpoint, a probe or an authorization, and it mints no identity,
//! context or allowance. The upcoming normal consumer joins it to an existing
//! completed/pending lookup; nothing here performs or observes that lookup.

use std::time::SystemTime;

use search_contracts::UtcTimestamp;

use eliot_searchd::native_time::timestamp_from_system_time;

use super::{FreshRootContext, FreshRootError, Work};

/// Exact width of the shared canonical UTC contract: 27 ASCII bytes.
const RETENTION_TIMESTAMP_BYTES: usize = 27;

/// Closed refusal when the actual clock cannot satisfy the shared UTC contract.
///
/// The clock syscall itself is cooperative: it is admitted, charged and polled
/// around rather than preempted, so the original retained refusal survives the
/// ordinary owner checkpoint instead of being replaced by this code.
const NATIVE_RETENTION_CLOCK_REFUSED: &str = "NATIVE_RETENTION_CLOCK_REFUSED";

impl FreshRootContext<'_, '_> {
    /// Reads the actual system clock once under the original Native custody.
    ///
    /// Callers supply no instant, and no caller-selected time is ever accepted.
    /// The original checkpoint runs before the observation; the actual
    /// observation, the bounded canonical text and its exact parse are each
    /// admitted through the existing work meter before they are performed.
    ///
    /// # Errors
    ///
    /// Returns the original retained refusal from any owner checkpoint, or the
    /// closed `NATIVE_RETENTION_CLOCK_REFUSED` code when the observed instant is
    /// outside the shared canonical UTC contract.
    pub(crate) fn capture_retention_observed_at(&self) -> Result<UtcTimestamp, FreshRootError> {
        // Original Native checkpoint before any observation or accounting.
        self.checkpoint()?;
        // Admit one cooperative wall-clock observation before reading it.
        self.charge(Work::Observe)?;
        let observed = self.observe_system_time()?;
        // Admit the bounded canonical text and its exact parse before building
        // either. The one shared date algorithm stays in crate::native_time.
        self.charge(Work::Allocate(RETENTION_TIMESTAMP_BYTES))?;
        self.charge(Work::Compare(RETENTION_TIMESTAMP_BYTES))?;
        let timestamp = timestamp_from_system_time(observed)
            .map_err(|_| FreshRootError::Source(NATIVE_RETENTION_CLOCK_REFUSED))?;
        // Same original context after the observation; no renewed deadline.
        self.checkpoint()?;
        Ok(timestamp)
    }

    /// One admitted `SystemTime::now` under the retained request and meter.
    fn observe_system_time(&self) -> Result<SystemTime, FreshRootError> {
        self.operation
            .borrow_mut()
            .with_binding(|_, request, work, _| work.call(request, || Ok(SystemTime::now())))
            .map_err(FreshRootError::Native)
    }
}

