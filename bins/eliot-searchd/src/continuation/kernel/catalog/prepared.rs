//! One exclusively borrowed, uncommitted page. No second window catalog.

use super::{ContinuationCatalog, ContinuationError, ContinuationRecord, Instant, SearchPage};

enum WindowChange {
    None,
    Insert(Box<ContinuationRecord>),
    Advance(String),
}

/// A page whose catalog mutation is deferred until complete output.
/// Dropping it discards a new window or leaves an existing cursor unchanged.
#[must_use]
pub(crate) struct PreparedPage<'a> {
    catalog: &'a mut ContinuationCatalog,
    page: SearchPage,
    change: WindowChange,
    expires_at: Option<Instant>,
}

#[derive(Debug)]
pub(super) enum DeliveryError {
    Continuation(ContinuationError),
    Output(String),
}

impl<'a> PreparedPage<'a> {
    pub(super) fn new(
        catalog: &'a mut ContinuationCatalog,
        page: SearchPage,
        record: Option<ContinuationRecord>,
    ) -> Self {
        let expires_at = record.as_ref().map(|value| value.expires_at);
        Self {
            catalog,
            page,
            change: record.map_or(WindowChange::None, |value| WindowChange::Insert(Box::new(value))),
            expires_at,
        }
    }

    pub(super) fn existing(
        catalog: &'a mut ContinuationCatalog,
        token: &str,
        page: SearchPage,
        expires_at: Instant,
    ) -> Self {
        Self {
            catalog,
            page,
            change: WindowChange::Advance(token.to_owned()),
            expires_at: Some(expires_at),
        }
    }

    pub(crate) const fn page(&self) -> &SearchPage {
        &self.page
    }

    /// Original absolute deadline, including a would-be final page.
    /// Never reconstruct this from the rounded, reported remaining TTL.
    pub(crate) const fn expires_at(&self) -> Option<Instant> {
        self.expires_at
    }

    /// Checks the original deadline immediately before the output callback.
    /// Only callback success commits the cursor/new window; output errors must
    /// still trigger the session's fail-stop rule, never retry a partial frame.
    pub(crate) fn deliver(
        self,
        emit: impl FnOnce(&SearchPage) -> Result<(), String>,
    ) -> Result<(), String> {
        self.finish_with_clock(Instant::now, emit)
            .map(|_| ())
            .map_err(|error| match error {
                DeliveryError::Continuation(error) => error.code().to_owned(),
                DeliveryError::Output(error) => error,
            })
    }

    pub(super) fn finish_with_clock(
        mut self,
        mut now: impl FnMut() -> Instant,
        emit: impl FnOnce(&SearchPage) -> Result<(), String>,
    ) -> Result<SearchPage, DeliveryError> {
        self.catalog.expire();
        if let Some(expires_at) = self.expires_at {
            let observed = now();
            let removed = matches!(&self.change, WindowChange::Advance(token)
                if !self.catalog.records.contains_key(token));
            if observed >= expires_at || removed {
                if let WindowChange::Advance(token) = &self.change {
                    self.catalog.drop_window(token);
                }
                return Err(DeliveryError::Continuation(ContinuationError::Expired));
            }
            if !self.page.exhausted {
                self.page.expires_in_ms = Some(
                    u64::try_from(expires_at.duration_since(observed).as_millis())
                        .unwrap_or(u64::MAX),
                );
            }
        }
        // The caller prepares handles/diagnostics before entry and emits the
        // complete response here. No cursor or new token is committed on Err.
        emit(&self.page).map_err(DeliveryError::Output)?;
        match self.change {
            WindowChange::None => {}
            WindowChange::Insert(record) => {
                let token = self.page.continuation_token.as_ref()
                    .expect("prepared new window token");
                self.catalog.retained_matches += record.matches.len();
                self.catalog.records.insert(token.clone(), *record);
            }
            WindowChange::Advance(token) => {
                if self.page.exhausted {
                    self.catalog.drop_window(&token);
                } else {
                    self.catalog.records.get_mut(&token)
                        .expect("exclusively borrowed prepared window")
                        .next_index = self.page.page_end;
                }
            }
        }
        Ok(self.page)
    }

    #[cfg(test)]
    pub(super) fn finish_for_tests(
        self,
        now: impl FnMut() -> Instant,
    ) -> Result<SearchPage, ContinuationError> {
        match self.finish_with_clock(now, |_| Ok(())) {
            Ok(page) => Ok(page),
            Err(DeliveryError::Continuation(error)) => Err(error),
            Err(DeliveryError::Output(_)) => unreachable!("infallible test output"),
        }
    }
}
