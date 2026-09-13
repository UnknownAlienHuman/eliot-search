//! Versioned, bounded serialization of the exact UTF-8 line/unit layout.
//! Source and residency identities are bound by the revision-store envelope.

#[path = "layout_manifest/decode.rs"]
mod decode;
#[path = "layout_manifest/encode.rs"]
mod encode;
#[path = "layout_manifest/format.rs"]
mod format;
#[path = "layout_manifest/wire.rs"]
mod wire;
