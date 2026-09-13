//! Versioned, bounded serialization of the exact UTF-8 line/unit layout.
//! Source and residency identities are bound by the revision-store envelope.

mod decode;
mod encode;
mod format;
mod wire;
