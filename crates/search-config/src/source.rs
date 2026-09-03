//! Captured configuration source identity.

use search_contracts::Blake3Digest32;

use crate::ConfigSourceRef;

/// Fixed deterministic precedence order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConfigSourceKind {
    /// Capability-owned compiled defaults.
    Defaults,
    /// Captured configuration file.
    File,
    /// Captured process environment.
    Environment,
    /// Captured typed command line.
    Cli,
}

impl ConfigSourceKind {
    /// Canonical precedence rank.
    #[must_use]
    pub const fn precedence(self) -> u8 {
        match self {
            Self::Defaults => 0,
            Self::File => 1,
            Self::Environment => 2,
            Self::Cli => 3,
        }
    }

    /// Stable wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Defaults => "defaults",
            Self::File => "file",
            Self::Environment => "environment",
            Self::Cli => "cli",
        }
    }
}

/// Exact captured source identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSource {
    /// Fixed source kind.
    pub kind: ConfigSourceKind,
    /// Opaque source reference safe for ordinary diagnostics.
    pub source_ref: ConfigSourceRef,
    /// Digest supplied by the acquisition owner over exact captured bytes/value set.
    pub source_digest: Blake3Digest32,
}
