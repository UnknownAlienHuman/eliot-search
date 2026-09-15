use std::time::Duration;

pub(super) const MAX_LINE_BYTES: usize = 64 * 1024;
pub(super) const MAX_RESPONSE_LINES: usize = 1_000_000;
pub(super) const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
pub(super) const POLL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy)]
pub(super) struct ChildLimits {
    pub(super) startup: Duration,
    pub(super) request: Duration,
    pub(super) cleanup: Duration,
}

impl ChildLimits {
    pub(super) const DEFAULT: Self = Self {
        startup: Duration::from_secs(30),
        request: Duration::from_secs(120),
        cleanup: Duration::from_secs(5),
    };
}
