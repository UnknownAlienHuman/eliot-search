//! Stable exact-layout format identity.

use crate::UnitizationLimits;

impl UnitizationLimits {
    /// Versioned layout algorithm/codec identity. Changing boundary semantics or
    /// encoding requires a new identity, never reinterpretation of saved layouts.
    pub const LAYOUT_FORMAT: &'static str = "exact-utf8-line-unit-layout/v1";
}
