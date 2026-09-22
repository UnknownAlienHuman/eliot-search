//! Unpublished handles for one page under an exclusive catalog borrow.

use super::{Instant, PublicHandledMatch, ResultHandleCatalog, ResultHandleError, ResultHandleRecord};

/// A complete unpublished batch. Dropping it inserts no records.
#[must_use]
pub(crate) struct PreparedHandles<'a> {
    catalog: &'a mut ResultHandleCatalog,
    records: Vec<(String, ResultHandleRecord)>,
    public: Vec<PublicHandledMatch>,
    expires_at: Instant,
}

impl<'a> PreparedHandles<'a> {
    pub(super) fn new(
        catalog: &'a mut ResultHandleCatalog,
        staged: Vec<(String, ResultHandleRecord, PublicHandledMatch)>,
        expires_at: Instant,
    ) -> Self {
        let mut records = Vec::with_capacity(staged.len());
        let mut public = Vec::with_capacity(staged.len());
        for (token, record, item) in staged {
            records.push((token, record));
            public.push(item);
        }
        Self { catalog, records, public, expires_at }
    }

    pub(crate) fn matches(&self) -> &[PublicHandledMatch] {
        &self.public
    }

    /// Refreshes reported remaining TTL without renewing the original deadline.
    pub(crate) fn revalidate(&mut self) -> Result<(), ResultHandleError> {
        self.revalidate_at(Instant::now())
    }

    pub(super) fn revalidate_at(&mut self, now: Instant) -> Result<(), ResultHandleError> {
        if self.public.is_empty() {
            return Ok(());
        }
        if now >= self.expires_at {
            return Err(ResultHandleError::Expired);
        }
        let remaining = u64::try_from(self.expires_at.duration_since(now).as_millis())
            .unwrap_or(u64::MAX);
        for item in &mut self.public {
            item.expires_in_ms = remaining;
        }
        Ok(())
    }

    /// Publishes only after successful page output. Namespace, source identity,
    /// capacity and intra-batch uniqueness were checked before this exclusive
    /// preparation was returned. External delivery is not proved by insertion.
    pub(crate) fn commit(self) -> Vec<PublicHandledMatch> {
        for (token, record) in self.records {
            self.catalog.records.insert(token, record);
        }
        self.public
    }
}
