//! Typed current source rows shared by the source reader, control store and CLI.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use search_contracts::{Blake3Digest32, SourceId, SourceNamespaceId};

/// Current plain object format used by the first retained source path.
pub const SOURCE_OBJECT_FORMAT_VERSION: u16 = 1;

/// Bounded byte length accepted by the first source object format.
pub const MAX_SOURCE_OBJECT_BYTES: u64 = 8 * 1024 * 1024;

/// Content kind stored in an [`ObjectRef`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectKind {
    /// Exact acquired source-file bytes.
    Source,
}

impl ObjectKind {
    /// Stable byte used in the current source-row encoding.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Source => 1,
        }
    }

    /// Decodes a stable object-kind byte.
    #[must_use]
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Source),
            _ => None,
        }
    }
}

/// Immutable plain-object address, including the integrity data needed to read it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectRef {
    /// On-disk object format version.
    pub format_version: u16,
    /// Stored content kind.
    pub kind: ObjectKind,
    /// BLAKE3 digest of the exact retained bytes.
    pub digest: Blake3Digest32,
    /// Exact byte length of the retained object.
    pub byte_length: u64,
}

impl ObjectRef {
    /// Creates a reference to exact source bytes.
    #[must_use]
    pub const fn source(digest: Blake3Digest32, byte_length: u64) -> Self {
        Self {
            format_version: SOURCE_OBJECT_FORMAT_VERSION,
            kind: ObjectKind::Source,
            digest,
            byte_length,
        }
    }
}

/// Verified native identity of an opened regular file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeFileKey {
    /// Unix device and inode observed through the opened handle.
    Unix {
        /// Filesystem device number.
        device: u64,
        /// Native inode number.
        inode: u64,
    },
    /// Windows volume serial and file index observed through the opened handle.
    Windows {
        /// Native volume serial number.
        volume_serial: u32,
        /// Native file index.
        file_index: u64,
    },
}

impl NativeFileKey {
    /// Encodes the exact platform key into its fixed current-row representation.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 17] {
        let mut bytes = [0_u8; 17];
        match self {
            Self::Unix { device, inode } => {
                bytes[0] = 1;
                let device = device.to_le_bytes();
                let inode = inode.to_le_bytes();
                let mut index = 0;
                while index < 8 {
                    bytes[index + 1] = device[index];
                    bytes[index + 9] = inode[index];
                    index += 1;
                }
            }
            Self::Windows {
                volume_serial,
                file_index,
            } => {
                bytes[0] = 2;
                let volume_serial = volume_serial.to_le_bytes();
                let file_index = file_index.to_le_bytes();
                let mut index = 0;
                while index < 4 {
                    bytes[index + 1] = volume_serial[index];
                    index += 1;
                }
                index = 0;
                while index < 8 {
                    bytes[index + 9] = file_index[index];
                    index += 1;
                }
            }
        }
        bytes
    }

    /// Decodes a fixed native-key representation, rejecting unknown tags or padding.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 17]) -> Option<Self> {
        match bytes[0] {
            1 => {
                let device = u64::from_le_bytes(bytes[1..9].try_into().ok()?);
                let inode = u64::from_le_bytes(bytes[9..17].try_into().ok()?);
                Some(Self::Unix { device, inode })
            }
            2 if bytes[5..9] == [0; 4] => {
                let volume_serial = u32::from_le_bytes(bytes[1..5].try_into().ok()?);
                let file_index = u64::from_le_bytes(bytes[9..17].try_into().ok()?);
                Some(Self::Windows {
                    volume_serial,
                    file_index,
                })
            }
            _ => None,
        }
    }
}

/// Persisted current source state; it owns identity and a reference, never source bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentSourceRecord {
    /// Persisted root-local source namespace.
    pub source_namespace_id: SourceNamespaceId,
    /// Random persisted identity of this logical selected source.
    pub source_id: SourceId,
    /// Non-reset revision of the source row.
    pub row_revision: u64,
    /// Non-reset registration revision guarding removal and re-registration.
    pub registration_revision: u64,
    /// Exact platform-encoded current locator bytes.
    pub locator: Vec<u8>,
    /// Native identity observed before and after source acquisition.
    pub file_key: NativeFileKey,
    /// Immutable bytes retained before this row was committed.
    pub object_ref: ObjectRef,
}
