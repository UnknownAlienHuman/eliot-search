//! Finite page and process-local stream descriptors.

use core::marker::PhantomData;
use core::num::{NonZeroU32, NonZeroU64};

use search_contracts::{BoundedList, ContractError, ContractErrorKind, OpaqueRef};

use crate::PackageOpaque;

/// Maximum generic page cardinality exposed by this support type.
pub const DEFAULT_PAGE_LIMIT: usize = 4096;

/// Finite deterministic page with an optional continuation reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedPage<T, const LIMIT: usize = DEFAULT_PAGE_LIMIT> {
    items: BoundedList<T, LIMIT>,
    continuation_ref: Option<OpaqueRef>,
    complete: bool,
}

impl<T, const LIMIT: usize> BoundedPage<T, LIMIT> {
    /// Creates a page with coherent completion and continuation state.
    ///
    /// # Errors
    ///
    /// A complete page cannot carry a continuation and an incomplete page must
    /// carry one.
    pub fn new(
        items: BoundedList<T, LIMIT>,
        continuation_ref: Option<OpaqueRef>,
        complete: bool,
    ) -> Result<Self, ContractError> {
        if complete == continuation_ref.is_some() {
            return Err(ContractError::new(
                ContractErrorKind::ContradictoryState,
                "bounded_page.continuation_ref",
            ));
        }
        Ok(Self {
            items,
            continuation_ref,
            complete,
        })
    }

    /// Page items in deterministic order.
    #[must_use]
    pub const fn items(&self) -> &BoundedList<T, LIMIT> {
        &self.items
    }

    /// Opaque continuation reference when the page is incomplete.
    #[must_use]
    pub const fn continuation_ref(&self) -> Option<&OpaqueRef> {
        self.continuation_ref.as_ref()
    }

    /// Whether this page completes the finite result set.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

/// Terminal state of a bounded process-local stream.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StreamTerminal {
    /// Every admitted stream item was delivered.
    Complete,
    /// Delivery stopped with explicit bounded omissions.
    Partial,
    /// Cancellation terminated delivery.
    Cancelled,
    /// Deadline terminated delivery.
    TimedOut,
    /// Required dependency became unavailable.
    DependencyUnavailable,
}

/// Descriptor for a finite process-local stream capability.
///
/// The item type is carried only at the Rust type level. Stream state remains
/// behind `stream_ref`; no executor, channel, file, socket, or vendor handle
/// crosses this API.
#[derive(Debug)]
pub struct BoundedStream<T, S>
where
    S: PackageOpaque,
{
    stream_ref: S,
    item_limit: NonZeroU32,
    byte_limit: NonZeroU64,
    deadline_ms: NonZeroU64,
    item: PhantomData<fn() -> T>,
}

impl<T, S> BoundedStream<T, S>
where
    S: PackageOpaque,
{
    /// Creates a finite stream descriptor.
    ///
    /// # Errors
    ///
    /// Zero item, byte, or deadline limits are rejected.
    pub fn new(
        stream_ref: S,
        item_limit: u32,
        byte_limit: u64,
        deadline_ms: u64,
    ) -> Result<Self, ContractError> {
        let item_limit = NonZeroU32::new(item_limit).ok_or_else(|| {
            ContractError::new(
                ContractErrorKind::ZeroNotAllowed,
                "bounded_stream.item_limit",
            )
        })?;
        let byte_limit = NonZeroU64::new(byte_limit).ok_or_else(|| {
            ContractError::new(
                ContractErrorKind::ZeroNotAllowed,
                "bounded_stream.byte_limit",
            )
        })?;
        let deadline_ms = NonZeroU64::new(deadline_ms).ok_or_else(|| {
            ContractError::new(
                ContractErrorKind::ZeroNotAllowed,
                "bounded_stream.deadline_ms",
            )
        })?;
        Ok(Self {
            stream_ref,
            item_limit,
            byte_limit,
            deadline_ms,
            item: PhantomData,
        })
    }

    /// Opaque owner-package stream capability.
    #[must_use]
    pub const fn stream_ref(&self) -> &S {
        &self.stream_ref
    }

    /// Maximum number of emitted items.
    #[must_use]
    pub const fn item_limit(&self) -> NonZeroU32 {
        self.item_limit
    }

    /// Maximum sum of emitted item bytes.
    #[must_use]
    pub const fn byte_limit(&self) -> NonZeroU64 {
        self.byte_limit
    }

    /// Finite stream deadline.
    #[must_use]
    pub const fn deadline_ms(&self) -> NonZeroU64 {
        self.deadline_ms
    }
}

#[cfg(test)]
mod tests {
    use core::fmt;

    use search_contracts::{BoundedList, ContractErrorKind, OpaqueRef};

    use super::{BoundedPage, BoundedStream};
    use crate::PackageOpaque;

    struct StreamRef;

    impl fmt::Debug for StreamRef {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("StreamRef(<opaque>)")
        }
    }

    impl PackageOpaque for StreamRef {
        fn owner_package(&self) -> &'static str {
            "search-ports"
        }
    }

    #[test]
    fn page_completion_and_continuation_are_coherent() {
        let items = BoundedList::<u8, 4>::new(vec![1]).expect("items");
        assert!(BoundedPage::new(items.clone(), None, true).is_ok());
        assert!(
            BoundedPage::new(
                items.clone(),
                Some(OpaqueRef::new("continue:1").expect("ref")),
                false,
            )
            .is_ok()
        );
        let error = BoundedPage::new(items, None, false).expect_err("continuation required");
        assert_eq!(error.kind(), ContractErrorKind::ContradictoryState);
    }

    #[test]
    fn stream_zero_limit_is_rejected() {
        let error = BoundedStream::<u8, _>::new(StreamRef, 0, 1, 1).expect_err("zero item limit");
        assert_eq!(error.kind(), ContractErrorKind::ZeroNotAllowed);
    }
}
