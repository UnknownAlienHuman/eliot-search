use std::time::Duration;

pub(super) const MAX_LINE_BYTES: usize = 64 * 1024;
pub(super) const MAX_RESPONSE_LINES: usize = 1_000_000;
pub(super) const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
pub(super) const POLL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy)]
pub(in super::super) struct ChildLimits {
    pub(in super::super) startup: Duration,
    pub(in super::super) request: Duration,
    pub(in super::super) cleanup: Duration,
}

impl ChildLimits {
    pub(in super::super) const DEFAULT: Self = Self {
        startup: Duration::from_secs(30),
        request: Duration::from_secs(120),
        cleanup: Duration::from_secs(5),
    };
}
