//! Shared handle-lifecycle support types.

/// Closed handle invalidation scope.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HandleInvalidationScope {
    /// Invalidate one exact handle identity.
    Handle,
    /// Invalidate all handles bound to one request.
    Request,
    /// Invalidate all handles bound to one client binding.
    Binding,
    /// Invalidate all handles bound to one source revision.
    Revision,
    /// Invalidate all handles affected by a security or purge fence.
    SecurityFence,
}

/// Finite server-owned handle limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandleLimits {
    /// Maximum lifetime in milliseconds.
    pub ttl_ms: u64,
    /// Maximum successful expansions.
    pub max_expansions: u32,
    /// Maximum bytes returned across all expansions.
    pub max_total_bytes: u64,
}
