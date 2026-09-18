//! Namespace protector assembled from model and operation responsibilities.

mod model;
mod operations;
#[cfg(windows)]
mod windows;

pub(super) use model::{PROTECTED_OBJECT_EXTENSION, RevisionProtector};
