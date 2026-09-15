//! Closed byte-layout and bounded-work constants for preparation storage.

use std::time::Duration;

use crate::direct_preparation::MAX_LAYOUT_BYTES;

pub(crate) const MAGIC: &[u8; 8] = b"ELSPRP02";
pub(crate) const REF_MAGIC: &[u8; 8] = b"ELSPRF01";
pub(crate) const BINDING_BYTES: usize = 208;
pub(crate) const OLD_BINDING_BYTES: usize = 176;
pub(crate) const REPRESENTATION_BYTES: usize = 32;
pub(crate) const BINDING_SUFFIX_BYTES: usize = 19;
pub(crate) const HEADER_BYTES: usize =
    BINDING_BYTES + REPRESENTATION_BYTES + BINDING_SUFFIX_BYTES;
pub(crate) const REF_BYTES: usize = 81;
pub(crate) const MAX_MANIFEST_BYTES: usize = HEADER_BYTES + MAX_LAYOUT_BYTES + 1;
pub(crate) const MAX_OBJECT_BYTES: usize = 65 * 1024 * 1024;

pub(crate) const MAX_BATCH_REVISIONS: usize = 64;
pub(crate) const MAX_BATCH_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
pub(crate) const BATCH_SLICE: Duration = Duration::from_secs(10);
