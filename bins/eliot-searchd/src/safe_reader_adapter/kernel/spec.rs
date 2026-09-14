//! Closed adapter profile, limits and content-free failure vocabulary.

/// Statically pinned no-execute invariant for this adapter.
pub const ADAPTER_NO_EXECUTE: bool = true;

const _: () = assert!(ADAPTER_NO_EXECUTE);

/// Finite retry budget applied by primary ingestion translation.
pub const ADAPTER_MAX_ATTEMPTS: u8 = 3;

/// Single-read chunk ceiling handed to the kernel (8 MiB, kernel default).
pub const ADAPTER_SINGLE_READ_BYTES: usize = 8 * 1024 * 1024;

/// Qualified platform identity profile proven by this adapter.
#[must_use]
pub const fn qualified_profile() -> &'static str {
    #[cfg(windows)]
    {
        "windows-final-handle/v1"
    }
    #[cfg(all(unix, not(windows)))]
    {
        "unix-final-handle/v1"
    }
    #[cfg(not(any(unix, windows)))]
    {
        "portable-final-handle/v1"
    }
}

/// Closed, content-free adapter failure. No variant carries paths, bytes or
/// raw OS error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterError {
    /// Relative token is absolute, escapes, names a device/stream or is unbounded.
    PathDenied,
    /// Final precheck or opened handle is a symlink/reparse object.
    LinkDenied,
    /// An authoritative ancestor traversal crossed a reparse boundary.
    AncestorReparseDenied,
    /// Final object canonicalizes outside the admitted root.
    EscapeDenied,
    /// Admitted root canonical identity moved between construction and open.
    RootRelocated,
    /// Final object is not a regular file (directory at precheck).
    NotRegular,
    /// Opened handle is not a regular file or is a device/pipe object.
    FinalObjectInvalid,
    /// Multi-link object; hardlink-outside-domain fails closed.
    HardlinkDenied,
    /// FIFO, socket, block/char device or another special object.
    DeviceDenied,
    /// OS open/read/metadata failed (ACL, revocation, races); no detail kept.
    AccessDenied,
    /// Source exceeds the caller-supplied finite byte budget.
    TooLarge,
    /// Content-free receipt/identity text could not be constructed.
    ReceiptDenied,
}

impl AdapterError {
    /// Stable machine-readable reason code without content.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PathDenied => "SAFE_ADAPTER_PATH_DENIED",
            Self::LinkDenied => "SAFE_ADAPTER_LINK_DENIED",
            Self::AncestorReparseDenied => "SAFE_ADAPTER_ANCESTOR_REPARSE_DENIED",
            Self::EscapeDenied => "SAFE_ADAPTER_ESCAPE_DENIED",
            Self::RootRelocated => "SAFE_ADAPTER_ROOT_RELOCATED",
            Self::NotRegular => "SAFE_ADAPTER_NOT_REGULAR",
            Self::FinalObjectInvalid => "SAFE_ADAPTER_FINAL_OBJECT_INVALID",
            Self::HardlinkDenied => "SAFE_ADAPTER_HARDLINK_DENIED",
            Self::DeviceDenied => "SAFE_ADAPTER_DEVICE_DENIED",
            Self::AccessDenied => "SAFE_ADAPTER_ACCESS_DENIED",
            Self::TooLarge => "SAFE_ADAPTER_TOO_LARGE",
            Self::ReceiptDenied => "SAFE_ADAPTER_RECEIPT_DENIED",
        }
    }
}

impl core::fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for AdapterError {}
