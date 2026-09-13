//! Durable profile-bound unit manifests with exact provenance.
//!
//! Public profile, model, codec, verification and diff operations remain
//! available through this stable facade. The private kernel preserves the
//! existing byte format and identity rules while later responsibility splits
//! stay internal to the package.

use crate::unitize_text;

mod kernel;

pub use kernel::*;
