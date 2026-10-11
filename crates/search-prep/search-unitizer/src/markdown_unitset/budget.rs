//! One original operation allowance; nested owner reservations are never refunded.
use super::{MarkdownUnitizationError as Error, MarkdownUnitizerProfile};
use search_contracts::{
    Blake3Digest32, CanonicalDigestDomain, CanonicalValue, DigestInputLimit, MAX_CANONICAL_BYTES,
};
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Positive finite profile and operation limits for complete structural preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkdownUnitizerLimits {
    /// Exact original byte ceiling.
    pub input_bytes: usize,
    /// Complete structural record ceiling, including root.
    pub nodes: usize,
    /// Complete yielded inline record ceiling.
    pub inline_records: usize,
    /// Selected mapped value byte ceiling per value.
    pub metadata_bytes_each: usize,
    /// Cumulative selected mapped value byte ceiling.
    pub metadata_bytes_total: usize,
    /// Maximum raw v3 units admitted by the conservative preflight.
    pub raw_units: usize,
    /// Whole v4 envelope byte ceiling.
    pub encoded_bytes: usize,
    /// Additional measured identity bytes plus nested canonical reservations.
    pub canonical_bytes: usize,
    /// Cumulative logical records, strings and temporaries; excludes peer raw/maps.
    pub owned_bytes: usize,
    /// Cumulative logical work plus nested owner reservations.
    pub work_steps: usize,
}
/// Closed hard ceilings; zero never means unlimited.
pub const MAX_MARKDOWN_UNITIZER_LIMITS: MarkdownUnitizerLimits = MarkdownUnitizerLimits {
    input_bytes: MAX_CANONICAL_BYTES - 128,
    nodes: 4096,
    inline_records: 4096,
    metadata_bytes_each: 4096,
    metadata_bytes_total: 1024 * 1024,
    raw_units: 4096,
    encoded_bytes: MAX_CANONICAL_BYTES - 128,
    canonical_bytes: 128 * 1024 * 1024,
    owned_bytes: 128 * 1024 * 1024,
    work_steps: 256 * 1024 * 1024,
};
impl MarkdownUnitizerLimits {
    pub(super) const fn dimensions(self) -> [usize; 10] {
        [
            self.input_bytes,
            self.nodes,
            self.inline_records,
            self.metadata_bytes_each,
            self.metadata_bytes_total,
            self.raw_units,
            self.encoded_bytes,
            self.canonical_bytes,
            self.owned_bytes,
            self.work_steps,
        ]
    }
    pub(super) fn validate(self, ceiling: Self) -> Result<(), Error> {
        if self
            .dimensions()
            .into_iter()
            .zip(ceiling.dimensions())
            .any(|(value, max)| value == 0 || value > max)
            || self.metadata_bytes_each > self.metadata_bytes_total
        {
            return Err(Error::InvalidLimits);
        }
        Ok(())
    }
}

/// Original live flag/deadline and lower per-operation allowances.
#[derive(Clone, Copy, Debug)]
pub struct MarkdownUnitizationBudget<'a> {
    /// Positive lower dimensions, at most the immutable profile's limits.
    pub limits: MarkdownUnitizerLimits,
    /// Original externally owned cancellation flag used by every nested phase.
    pub cancelled: &'a AtomicBool,
    /// Original absolute monotonic deadline, never reset or persisted.
    pub deadline: Instant,
}
pub(super) struct Meter<'a> {
    pub(super) budget: MarkdownUnitizationBudget<'a>,
    pub(super) work: usize,
    pub(super) canonical: usize,
    pub(super) owned: usize,
}
impl<'a> Meter<'a> {
    pub(super) fn new(
        profile: &MarkdownUnitizerProfile,
        budget: &MarkdownUnitizationBudget<'a>,
    ) -> Result<Self, Error> {
        budget.limits.validate(profile.limits())?;
        let meter = Self {
            budget: *budget,
            work: 0,
            canonical: 0,
            owned: 0,
        };
        meter.poll()?;
        Ok(meter)
    }
    pub(super) fn poll(&self) -> Result<(), Error> {
        if self.budget.cancelled.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        if Instant::now() >= self.budget.deadline {
            return Err(Error::DeadlineExceeded);
        }
        Ok(())
    }
    pub(super) fn work(&mut self, amount: usize) -> Result<(), Error> {
        self.poll()?;
        self.work = add(self.work, amount)?;
        if self.work > self.budget.limits.work_steps {
            return Err(Error::WorkLimit);
        }
        Ok(())
    }
    pub(super) fn owned(&mut self, amount: usize) -> Result<(), Error> {
        self.poll()?;
        self.owned = add(self.owned, amount)?;
        if self.owned > self.budget.limits.owned_bytes {
            return Err(Error::StorageLimit);
        }
        Ok(())
    }
    pub(super) fn canonical(&mut self, amount: usize) -> Result<(), Error> {
        self.poll()?;
        self.canonical = add(self.canonical, amount)?;
        if self.canonical > self.budget.limits.canonical_bytes {
            return Err(Error::CanonicalLimit);
        }
        Ok(())
    }
    pub(super) fn rows(&mut self, count: usize, copied_bytes: usize) -> Result<(), Error> {
        let cost = add(mul(count, size_of::<CanonicalValue>())?, copied_bytes)?;
        self.work(cost)?;
        self.owned(cost)
    }
    pub(super) fn hash(
        &mut self,
        domain: &str,
        value: CanonicalValue,
    ) -> Result<Blake3Digest32, Error> {
        self.poll()?;
        let CanonicalValue::Array(rows) = value else {
            return Err(Error::Codec);
        };
        let remaining = self
            .budget
            .limits
            .canonical_bytes
            .checked_sub(self.canonical)
            .ok_or(Error::CanonicalLimit)?;
        let work_remaining = self
            .budget
            .limits
            .work_steps
            .checked_sub(self.work)
            .ok_or(Error::WorkLimit)?;
        let cap = remaining.min(work_remaining).min(MAX_CANONICAL_BYTES);
        if work_remaining == 0 {
            return Err(Error::WorkLimit);
        }
        if cap == 0 {
            return Err(Error::CanonicalLimit);
        }
        // Both remaining allowances constrain the shared streaming sink before
        // it runs; construction was charged first. Debit exact successful bytes.
        let output = search_contracts::blake3_canonical_array_with_usage(
            &CanonicalDigestDomain::parse(domain).map_err(|_| Error::Codec)?,
            rows.len(),
            rows.into_vec().into_iter().map(Ok),
            DigestInputLimit::new(cap).map_err(|_| Error::CanonicalLimit)?,
        )
        .map_err(|_| {
            if work_remaining <= remaining && work_remaining <= MAX_CANONICAL_BYTES {
                Error::WorkLimit
            } else {
                Error::CanonicalLimit
            }
        })?;
        self.work(output.complete_preimage_bytes())?;
        self.canonical(output.complete_preimage_bytes())?;
        self.poll()?;
        Ok(output.digest())
    }
    pub(super) fn hash_raw(&mut self, domain: &str, bytes: &[u8]) -> Result<Blake3Digest32, Error> {
        let cost = add(add(domain.len(), 1)?, bytes.len())?;
        self.work(cost)?;
        self.canonical(cost)?;
        let digest = search_contracts::blake3_raw(
            &CanonicalDigestDomain::parse(domain).map_err(|_| Error::Codec)?,
            bytes,
            DigestInputLimit::new(cost).map_err(|_| Error::CanonicalLimit)?,
        )
        .map_err(|_| Error::CanonicalLimit)?;
        self.poll()?;
        Ok(digest)
    }
}
pub(super) fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or(Error::ArithmeticOverflow)
}
pub(super) fn mul(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_mul(b).ok_or(Error::ArithmeticOverflow)
}
pub(super) fn number(n: usize) -> Result<u64, Error> {
    u64::try_from(n).map_err(|_| Error::ArithmeticOverflow)
}
pub(super) fn index(n: u64) -> Result<usize, Error> {
    usize::try_from(n).map_err(|_| Error::ArithmeticOverflow)
}
