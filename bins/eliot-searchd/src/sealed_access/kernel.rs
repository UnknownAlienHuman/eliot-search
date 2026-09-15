//! Sealed access composition behind the stable module facade.

mod append;
mod chain;
mod model;
mod platform;
mod read;
mod spec;

pub use append::append_fence;
pub use model::{
    AccessAppendDisposition, AccessFenceMutation, AccessFenceReceipt,
    AccessFenceSnapshot, ActiveAccessFence,
};
pub use read::{current_fence, require_active_fence};
pub use spec::{MAX_ACCESS_FENCE_GENERATIONS, SealedAccessError};
