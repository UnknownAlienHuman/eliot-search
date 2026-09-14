//! Ephemeral development-shim key source for the loopback endpoint.

use std::cell::Cell;
use std::rc::Rc;

use crate::endpoint;

/// Development compatibility key source. The product lease-bound path remains
/// owned by `secret_composition`.
pub(super) struct ShimKeySource {
    key: [u8; 32],
    cache: Rc<Cell<Option<[u8; 32]>>>,
}

impl ShimKeySource {
    pub(super) fn new(
        key: [u8; 32],
        cache: Rc<Cell<Option<[u8; 32]>>>,
    ) -> Self {
        Self { key, cache }
    }
}

impl core::fmt::Debug for ShimKeySource {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ShimKeySource")
            .field("key", &"<redacted>")
            .finish()
    }
}

impl Drop for ShimKeySource {
    fn drop(&mut self) {
        self.key.fill(0);
    }
}

impl endpoint::EndpointKeySource for ShimKeySource {
    fn with_endpoint_key<T>(
        &mut self,
        use_key: impl FnOnce(&[u8; 32]) -> T,
    ) -> Result<T, String> {
        self.cache.set(Some(self.key));
        Ok(use_key(&self.key))
    }
}

pub(super) fn cached_key(
    cache: &Rc<Cell<Option<[u8; 32]>>>,
) -> Result<[u8; 32], String> {
    cache
        .get()
        .ok_or_else(|| crate::provider_composition::PROVIDER_HELLO_REQUIRED.to_owned())
}
