#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum Reply {
    Complete,
    Rejected,
    Shutdown,
    /// The child reported a terminal service failure, not a reusable rejection.
    Fatal,
}
