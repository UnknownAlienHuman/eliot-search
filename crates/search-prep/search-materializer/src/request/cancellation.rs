//! Cooperative cancellation for bounded materialization work.

use core::sync::atomic::{AtomicBool, Ordering};

/// Cooperative cancellation token over an externally owned atomic flag.
///
/// [`CancellationToken::never`] never cancels and carries no flag; long
/// transforms poll [`CancellationToken::is_cancelled`] at every line and map
/// segment, so cancellation never returns a successful complete product.
#[derive(Clone, Copy, Debug)]
pub struct CancellationToken<'a> {
    flag: Option<&'a AtomicBool>,
}

impl<'a> CancellationToken<'a> {
    /// Token that never cancels.
    #[must_use]
    pub const fn never() -> Self {
        Self { flag: None }
    }

    /// Token observing an externally owned atomic flag.
    #[must_use]
    pub const fn new(flag: &'a AtomicBool) -> Self {
        Self { flag: Some(flag) }
    }

    /// Reports whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.is_some_and(|flag| flag.load(Ordering::SeqCst))
    }
}
