//! Pure compatibility contract for legacy protected revision envelopes.
//!
//! The model owns finite bindings and failures; the codec owns the frozen byte
//! layout. Neither module performs platform, filesystem, catalog or policy I/O.

mod codec;
mod model;

pub use codec::*;
pub use model::*;

#[cfg(test)]
mod tests;
