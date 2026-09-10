//! Exact no-execute Git loose-object acquisition under an admitted repository.
//!
//! This module owns bounded verification of exact local loose objects addressed
//! by SHA-1 object ID. It performs no filesystem I/O, spawns no process,
//! invokes no hook/filter/smudge driver/credential helper and touches no
//! network: a caller-supplied [`GitObjectBackend`] opens the single derived
//! `objects/aa/bb…` file under the already admitted repository and delivers
//! the decompressed bytes. The module then enforces the finite decompression
//! ceiling again, validates the `<type> <size>\0` header, the declared kind,
//! the exact byte length and the per-kind payload structure, and binds the
//! result to the repository/object identities.
//!
//! Identity is never derived from path text: the validated result carries the
//! admitted repository digest and the requested object ID, and the backend
//! observed values must equal them exactly.
//!
//! Named deferred gaps (no placeholder code paths; every function below is
//! fully implemented):
//!
//! - G1 `loose-inflate`: zlib inflation of the on-disk loose file lives in the
//!   concrete adapter outside this package. The workspace dependency closure
//!   of this package (`search-contracts`, `search-domain`, `search-ports`,
//!   `search-config`, none with external dependencies) contains no
//!   `flate2`/`miniz_oxide` provider, and `Cargo.lock` contains neither, so
//!   adding inflation here would require a new integration-qualified
//!   dependency. The adapter must inflate under the same
//!   [`GitReadLimits::max_decompressed_bytes`] streaming ceiling; this module
//!   re-enforces the ceiling on the delivered buffer before any further work.
//! - G2 `sha1-recompute`: recomputing `SHA-1(header || payload)` requires a
//!   `sha1` provider that is absent from both the package closure and
//!   `Cargo.lock` (the closure has no hash implementation at all; the
//!   workspace `sha2` belongs to `search-revision-crypto` and is the wrong
//!   algorithm for Git object IDs). Until an integration-qualified provider
//!   lands, this module verifies ID syntax, the canonical fanout derivation
//!   and exact observed/requested ID equality, plus kind/size/structure, but
//!   does not recompute the content hash.
//!
//! Packed objects are never substituted: a backend that can only serve an
//! object from a packfile maps to [`GitReadError::PackedObjectUnavailable`],
//! and a promised-but-remote object maps to
//! [`GitReadError::ObjectRequiresNetwork`]. Errors and receipts are redacted:
//! they carry reason codes only, never paths or bytes.

use core::fmt;

use search_contracts::Blake3Digest32;

use super::{DEFAULT_SAFE_READ_LIMITS, RelativePathToken};

/// Statically pinned no-execute invariant for Git acquisition.
///
/// This module never spawns a process, loads a hook/filter driver, prompts
/// for credentials or fetches from a network. Reads are inert byte
/// validations of backend-delivered buffers addressed by exact object ID.
pub const SAFE_GIT_NO_EXECUTE: bool = true;

const _: () = assert!(SAFE_GIT_NO_EXECUTE);

/// Exact length of a Git object ID in hexadecimal characters.
pub const GIT_OBJECT_ID_HEX_LEN: usize = 40;

/// Exact length of a Git object ID in raw bytes (SHA-1 digest length).
pub const GIT_OBJECT_ID_BYTE_LEN: usize = 20;

/// Maximum accepted header length (`<type> <size>`) before the NUL separator.
pub const MAX_GIT_OBJECT_HEADER_BYTES: usize = 64;

/// Maximum accepted decimal digits in the declared object size.
pub const MAX_GIT_OBJECT_SIZE_DIGITS: usize = 20;

/// Exact length of a canonical loose-object path (`objects/aa/` + 38 hex).
pub const GIT_LOOSE_OBJECT_PATH_LEN: usize = 49;

/// Conservative finite Git object limits.
pub const DEFAULT_GIT_READ_LIMITS: GitReadLimits = GitReadLimits {
    max_decompressed_bytes: 64 * 1024 * 1024,
    max_path_token_bytes: 32_768,
};

/// Closed content-free Git acquisition failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GitReadError {
    /// Git limits are zero or internally inconsistent.
    InvalidLimits,
    /// Object ID text is not exactly 40 hexadecimal characters.
    InvalidObjectId,
    /// Loose-object header names an unknown object type.
    InvalidKind,
    /// Loose-object header is not canonical `<type> <size>\0`.
    MalformedHeader,
    /// Object payload does not match its kind structure.
    MalformedPayload,
    /// Object kind differs from the explicitly expected kind.
    KindMismatch,
    /// Declared header size differs from the exact payload length.
    SizeMismatch,
    /// Declared or delivered size exceeds the finite decompression ceiling.
    ObjectTooLarge,
    /// Backend-observed object ID differs from the requested object ID.
    ObjectIdentityMismatch,
    /// Backend-observed repository differs from the admitted repository.
    RepositoryMismatch,
    /// No loose object exists at the exact derived location.
    ObjectNotFound,
    /// Object is only available packed; loose substitution is denied.
    PackedObjectUnavailable,
    /// Object requires a remote fetch; network acquisition is denied.
    ObjectRequiresNetwork,
    /// Cancellation was observed before stable verification completed.
    Cancelled,
    /// Backend failed before a verified result existed.
    BackendFailure,
}

impl GitReadError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "GIT_OBJECT_INVALID_LIMITS",
            Self::InvalidObjectId => "GIT_OBJECT_INVALID_ID",
            Self::InvalidKind => "GIT_OBJECT_UNKNOWN_KIND",
            Self::MalformedHeader => "GIT_OBJECT_MALFORMED_HEADER",
            Self::MalformedPayload => "GIT_OBJECT_MALFORMED_PAYLOAD",
            Self::KindMismatch => "GIT_OBJECT_KIND_MISMATCH",
            Self::SizeMismatch => "GIT_OBJECT_SIZE_MISMATCH",
            Self::ObjectTooLarge => "GIT_OBJECT_TOO_LARGE",
            Self::ObjectIdentityMismatch => "GIT_OBJECT_IDENTITY_MISMATCH",
            Self::RepositoryMismatch => "GIT_OBJECT_REPOSITORY_MISMATCH",
            Self::ObjectNotFound => "GIT_OBJECT_NOT_FOUND",
            Self::PackedObjectUnavailable => "GIT_OBJECT_PACKED_UNAVAILABLE",
            Self::ObjectRequiresNetwork => "GIT_OBJECT_REQUIRES_NETWORK",
            Self::Cancelled => "GIT_OBJECT_CANCELLED",
            Self::BackendFailure => "GIT_OBJECT_BACKEND_FAILURE",
        }
    }
}

impl fmt::Display for GitReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for GitReadError {}

/// Finite Git loose-object limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitReadLimits {
    /// Maximum accepted decompressed object bytes, including the header.
    pub max_decompressed_bytes: u64,
    /// Maximum bytes in the derived loose-object path token.
    pub max_path_token_bytes: usize,
}

impl GitReadLimits {
    /// Validates every finite dimension as non-zero.
    pub const fn validate(self) -> Result<Self, GitReadError> {
        if self.max_decompressed_bytes == 0 || self.max_path_token_bytes == 0 {
            Err(GitReadError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Closed Git object kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GitObjectKind {
    /// Opaque file content.
    Blob,
    /// Directory listing of mode/name/ID entries.
    Tree,
    /// Commit metadata binding a tree and history.
    Commit,
    /// Annotated tag binding another object.
    Tag,
}

impl GitObjectKind {
    /// Canonical lowercase Git type name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blob => "blob",
            Self::Tree => "tree",
            Self::Commit => "commit",
            Self::Tag => "tag",
        }
    }
}

/// Exact 20-byte Git object identity (SHA-1 digest bytes).
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GitObjectId([u8; GIT_OBJECT_ID_BYTE_LEN]);

impl GitObjectId {
    /// Wraps already-validated raw digest bytes.
    pub const fn from_bytes(bytes: [u8; GIT_OBJECT_ID_BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// Parses exactly 40 hexadecimal characters (either case).
    pub fn parse_hex(text: &str) -> Result<Self, GitReadError> {
        let digits = text.as_bytes();
        if digits.len() != GIT_OBJECT_ID_HEX_LEN {
            return Err(GitReadError::InvalidObjectId);
        }
        let mut raw: Vec<u8> = Vec::with_capacity(GIT_OBJECT_ID_BYTE_LEN);
        let mut index: usize = 0;
        while index < GIT_OBJECT_ID_BYTE_LEN {
            let pair = index.checked_mul(2).ok_or(GitReadError::InvalidObjectId)?;
            let high = hex_value(
                digits
                    .get(pair)
                    .copied()
                    .ok_or(GitReadError::InvalidObjectId)?,
            )
            .ok_or(GitReadError::InvalidObjectId)?;
            let low = hex_value(
                digits
                    .get(pair.checked_add(1).ok_or(GitReadError::InvalidObjectId)?)
                    .copied()
                    .ok_or(GitReadError::InvalidObjectId)?,
            )
            .ok_or(GitReadError::InvalidObjectId)?;
            raw.push((high << 4) | low);
            index = index.checked_add(1).ok_or(GitReadError::InvalidObjectId)?;
        }
        let bytes = <[u8; GIT_OBJECT_ID_BYTE_LEN]>::try_from(raw)
            .map_err(|_| GitReadError::InvalidObjectId)?;
        Ok(Self(bytes))
    }

    /// Exact raw digest bytes.
    pub const fn as_bytes(&self) -> &[u8; GIT_OBJECT_ID_BYTE_LEN] {
        &self.0
    }

    /// Canonical lowercase hexadecimal rendering (40 characters, bounded).
    pub fn hex(self) -> String {
        let mut text = String::with_capacity(GIT_OBJECT_ID_HEX_LEN);
        for byte in self.0 {
            for nibble in [byte >> 4, byte & 0x0F] {
                if let Some(digit) = char::from_digit(u32::from(nibble), 16) {
                    text.push(digit);
                }
            }
        }
        text
    }

    /// Derives the canonical `objects/aa/bb…` relative path token.
    ///
    /// The path is derived from the validated ID bytes alone; no repository
    /// state, branch name or HEAD value participates in addressing.
    pub fn loose_relative_path(
        &self,
        limits: GitReadLimits,
    ) -> Result<RelativePathToken, GitReadError> {
        let limits = limits.validate()?;
        if GIT_LOOSE_OBJECT_PATH_LEN > limits.max_path_token_bytes {
            return Err(GitReadError::InvalidLimits);
        }
        let hex = self.hex();
        let hex_bytes = hex.as_bytes();
        let fanout = hex_bytes.get(..2).ok_or(GitReadError::InvalidObjectId)?;
        let rest = hex_bytes.get(2..).ok_or(GitReadError::InvalidObjectId)?;
        let fanout = core::str::from_utf8(fanout).map_err(|_| GitReadError::InvalidObjectId)?;
        let rest = core::str::from_utf8(rest).map_err(|_| GitReadError::InvalidObjectId)?;
        let mut path = String::with_capacity(GIT_LOOSE_OBJECT_PATH_LEN);
        path.push_str("objects/");
        path.push_str(fanout);
        path.push('/');
        path.push_str(rest);
        RelativePathToken::new(path, DEFAULT_SAFE_READ_LIMITS)
            .map_err(|_| GitReadError::InvalidLimits)
    }
}

impl fmt::Debug for GitObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("GitObjectId")
            .field(&self.hex())
            .finish()
    }
}

/// Exact Git object read request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitObjectRequest {
    /// Admitted repository identity digest; branch/HEAD alone is insufficient.
    pub repository_identity_digest: Blake3Digest32,
    /// Exact object identity to read.
    pub object_id: GitObjectId,
    /// Expected kind when the caller binds one; `None` accepts any known kind.
    pub expected_kind: Option<GitObjectKind>,
}

/// Validated request with the derived canonical loose-object location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedGitObjectRequest {
    /// Canonical `objects/aa/bb…` token for the backend to open.
    pub relative_path: RelativePathToken,
    /// Admitted repository identity digest.
    pub repository_identity_digest: Blake3Digest32,
    /// Exact object identity to read.
    pub object_id: GitObjectId,
    /// Expected kind when the caller binds one.
    pub expected_kind: Option<GitObjectKind>,
}

/// Validates finite limits and derives the canonical loose-object location.
///
/// Admission, workspace and owner fences stay with the caller; this operation
/// binds the already admitted repository digest and performs no read.
pub fn validate_git_object_request(
    request: &GitObjectRequest,
    limits: GitReadLimits,
) -> Result<ValidatedGitObjectRequest, GitReadError> {
    let limits = limits.validate()?;
    let relative_path = request.object_id.loose_relative_path(limits)?;
    Ok(ValidatedGitObjectRequest {
        relative_path,
        repository_identity_digest: request.repository_identity_digest,
        object_id: request.object_id,
        expected_kind: request.expected_kind,
    })
}

/// Verified view over one decompressed loose object.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ParsedGitObject<'a> {
    /// Validated object kind.
    pub kind: GitObjectKind,
    /// Exact declared payload length, equal to the delivered payload length.
    pub declared_size: u64,
    /// Exact validated payload bytes (header excluded).
    pub payload: &'a [u8],
}

impl fmt::Debug for ParsedGitObject<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParsedGitObject")
            .field("kind", &self.kind)
            .field("declared_size", &self.declared_size)
            .field("payload", &format_args!("<{} bytes>", self.payload.len()))
            .finish()
    }
}

/// Parses and validates one decompressed loose object.
///
/// The input must already be decompressed by the caller adapter under
/// [`GitReadLimits::max_decompressed_bytes`] (deferred gap G1); this function
/// re-enforces the ceiling, then requires a canonical `<type> <size>\0`
/// header, exact declared/actual length equality, the expected kind when one
/// is bound, and the per-kind payload structure.
pub fn parse_loose_object(
    decompressed: &[u8],
    limits: GitReadLimits,
    expected_kind: Option<GitObjectKind>,
) -> Result<ParsedGitObject<'_>, GitReadError> {
    let limits = limits.validate()?;
    let total = u64::try_from(decompressed.len()).map_err(|_| GitReadError::ObjectTooLarge)?;
    if total > limits.max_decompressed_bytes {
        return Err(GitReadError::ObjectTooLarge);
    }
    let nul = decompressed
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(GitReadError::MalformedHeader)?;
    if nul > MAX_GIT_OBJECT_HEADER_BYTES {
        return Err(GitReadError::MalformedHeader);
    }
    let header = decompressed
        .get(..nul)
        .ok_or(GitReadError::MalformedHeader)?;
    let payload_start = nul.checked_add(1).ok_or(GitReadError::MalformedHeader)?;
    let payload = decompressed
        .get(payload_start..)
        .ok_or(GitReadError::MalformedHeader)?;
    let space = header
        .iter()
        .position(|byte| *byte == b' ')
        .ok_or(GitReadError::MalformedHeader)?;
    let kind_bytes = header.get(..space).ok_or(GitReadError::MalformedHeader)?;
    let size_start = space.checked_add(1).ok_or(GitReadError::MalformedHeader)?;
    let size_bytes = header
        .get(size_start..)
        .ok_or(GitReadError::MalformedHeader)?;
    let kind = kind_from_bytes(kind_bytes)?;
    let declared = parse_object_size(size_bytes)?;
    if declared > limits.max_decompressed_bytes {
        return Err(GitReadError::ObjectTooLarge);
    }
    let actual = u64::try_from(payload.len()).map_err(|_| GitReadError::ObjectTooLarge)?;
    if actual != declared {
        return Err(GitReadError::SizeMismatch);
    }
    if expected_kind.is_some_and(|expected| expected != kind) {
        return Err(GitReadError::KindMismatch);
    }
    match kind {
        GitObjectKind::Blob => {}
        GitObjectKind::Tree => validate_tree_payload(payload)?,
        GitObjectKind::Commit => validate_commit_payload(payload)?,
        GitObjectKind::Tag => validate_tag_payload(payload)?,
    }
    Ok(ParsedGitObject {
        kind,
        declared_size: declared,
        payload,
    })
}

/// Content-free backend open request for one loose object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitLooseOpenRequest {
    /// Canonical `objects/aa/bb…` token derived from the object ID.
    pub relative_path: RelativePathToken,
    /// Admitted repository digest the backend must contain the open under.
    pub expected_repository_identity_digest: Blake3Digest32,
}

/// One backend-delivered decompressed loose object plus observed identities.
#[derive(Clone, Eq, PartialEq)]
pub struct GitBackendObject {
    /// Decompressed `<type> <size>\0<payload>` bytes under the finite ceiling.
    pub decompressed: Vec<u8>,
    /// Repository identity observed by the backend at open time.
    pub observed_repository_identity_digest: Blake3Digest32,
    /// Object identity observed by the backend at open time.
    pub observed_id: GitObjectId,
}

impl fmt::Debug for GitBackendObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitBackendObject")
            .field(
                "decompressed",
                &format_args!("<{} bytes>", self.decompressed.len()),
            )
            .field(
                "observed_repository_identity_digest",
                &self.observed_repository_identity_digest,
            )
            .field("observed_id", &self.observed_id)
            .finish()
    }
}

/// Backend contract for exact loose-object acquisition.
///
/// The backend opens only the exact derived loose path under the admitted
/// repository. It never spawns `git` or shell, never runs hooks, filters,
/// smudge/clean drivers or credential helpers, never fetches from a network
/// and never substitutes a packed or live-path object: pack-only objects map
/// through [`GitObjectBackend::map_backend_error`] to
/// [`GitReadError::PackedObjectUnavailable`], and remote-promised objects to
/// [`GitReadError::ObjectRequiresNetwork`].
pub trait GitObjectBackend {
    /// Backend failure.
    type BackendError;

    /// Reads and decompresses the exact loose object under its finite ceiling.
    fn read_loose_object(
        &mut self,
        request: &GitLooseOpenRequest,
    ) -> Result<GitBackendObject, Self::BackendError>;

    /// Maps a backend error to a bounded content-free package failure.
    fn map_backend_error(error: &Self::BackendError) -> GitReadError;
}

/// Exact verified Git object bytes with identity bindings.
#[derive(Clone, Eq, PartialEq)]
pub struct GitReadResult {
    /// Requested object identity, equal to the backend-observed identity.
    pub object_id: GitObjectId,
    /// Admitted repository digest the object was read under.
    pub repository_identity_digest: Blake3Digest32,
    /// Validated object kind.
    pub kind: GitObjectKind,
    /// Exact payload length, equal to the declared header size.
    pub payload_len: u64,
    /// Exact validated payload bytes (header excluded).
    payload: Vec<u8>,
}

impl GitReadResult {
    /// Exact validated payload bytes.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Exact payload length.
    pub fn len(&self) -> usize {
        self.payload.len()
    }

    /// Returns whether the payload is empty.
    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }
}

impl fmt::Debug for GitReadResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitReadResult")
            .field("object_id", &self.object_id)
            .field(
                "repository_identity_digest",
                &self.repository_identity_digest,
            )
            .field("kind", &self.kind)
            .field("payload_len", &self.payload_len)
            .field("payload", &format_args!("<{} bytes>", self.payload.len()))
            .finish()
    }
}

/// Reads one exact local loose object without executing repository content.
///
/// Cancellation is polled before validation, before the backend read and
/// after the backend read; any observed cancellation drops the bytes and
/// returns [`GitReadError::Cancelled`] with no product.
pub fn read_git_object_no_execute<B: GitObjectBackend>(
    backend: &mut B,
    request: &GitObjectRequest,
    limits: GitReadLimits,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<GitReadResult, GitReadError> {
    if should_cancel() {
        return Err(GitReadError::Cancelled);
    }
    let validated = validate_git_object_request(request, limits)?;
    if should_cancel() {
        return Err(GitReadError::Cancelled);
    }
    let open = GitLooseOpenRequest {
        relative_path: validated.relative_path,
        expected_repository_identity_digest: validated.repository_identity_digest,
    };
    let object = backend
        .read_loose_object(&open)
        .map_err(|error| B::map_backend_error(&error))?;
    if should_cancel() {
        return Err(GitReadError::Cancelled);
    }
    if object.observed_repository_identity_digest != validated.repository_identity_digest {
        return Err(GitReadError::RepositoryMismatch);
    }
    if object.observed_id != validated.object_id {
        return Err(GitReadError::ObjectIdentityMismatch);
    }
    let parsed = parse_loose_object(&object.decompressed, limits, validated.expected_kind)?;
    let payload_len =
        u64::try_from(parsed.payload.len()).map_err(|_| GitReadError::ObjectTooLarge)?;
    Ok(GitReadResult {
        object_id: validated.object_id,
        repository_identity_digest: validated.repository_identity_digest,
        kind: parsed.kind,
        payload_len,
        payload: parsed.payload.to_vec(),
    })
}

fn kind_from_bytes(kind: &[u8]) -> Result<GitObjectKind, GitReadError> {
    if kind == b"blob" {
        Ok(GitObjectKind::Blob)
    } else if kind == b"tree" {
        Ok(GitObjectKind::Tree)
    } else if kind == b"commit" {
        Ok(GitObjectKind::Commit)
    } else if kind == b"tag" {
        Ok(GitObjectKind::Tag)
    } else {
        Err(GitReadError::InvalidKind)
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_object_size(digits: &[u8]) -> Result<u64, GitReadError> {
    if digits.is_empty() || digits.len() > MAX_GIT_OBJECT_SIZE_DIGITS {
        return Err(GitReadError::MalformedHeader);
    }
    if digits.len() > 1 && digits.first() == Some(&b'0') {
        return Err(GitReadError::MalformedHeader);
    }
    let mut size: u64 = 0;
    for byte in digits {
        if !byte.is_ascii_digit() {
            return Err(GitReadError::MalformedHeader);
        }
        let value = u64::from(*byte - b'0');
        size = size
            .checked_mul(10)
            .and_then(|scaled| scaled.checked_add(value))
            .ok_or(GitReadError::MalformedHeader)?;
    }
    Ok(size)
}

fn validate_tree_payload(payload: &[u8]) -> Result<(), GitReadError> {
    let mut cursor: usize = 0;
    while cursor < payload.len() {
        let rest = payload
            .get(cursor..)
            .ok_or(GitReadError::MalformedPayload)?;
        let space = rest
            .iter()
            .position(|byte| *byte == b' ')
            .ok_or(GitReadError::MalformedPayload)?;
        let mode = rest.get(..space).ok_or(GitReadError::MalformedPayload)?;
        if mode.is_empty() || mode.len() > 6 {
            return Err(GitReadError::MalformedPayload);
        }
        if !mode.iter().all(|byte| matches!(byte, b'0'..=b'7')) {
            return Err(GitReadError::MalformedPayload);
        }
        let name_start = cursor
            .checked_add(space)
            .and_then(|value| value.checked_add(1))
            .ok_or(GitReadError::MalformedPayload)?;
        let name_rest = payload
            .get(name_start..)
            .ok_or(GitReadError::MalformedPayload)?;
        let nul = name_rest
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(GitReadError::MalformedPayload)?;
        let name = name_rest.get(..nul).ok_or(GitReadError::MalformedPayload)?;
        if name.is_empty() || name.contains(&b'/') {
            return Err(GitReadError::MalformedPayload);
        }
        if name == b"." || name == b".." {
            return Err(GitReadError::MalformedPayload);
        }
        let id_start = name_start
            .checked_add(nul)
            .and_then(|value| value.checked_add(1))
            .ok_or(GitReadError::MalformedPayload)?;
        let id_end = id_start
            .checked_add(GIT_OBJECT_ID_BYTE_LEN)
            .ok_or(GitReadError::MalformedPayload)?;
        payload
            .get(id_start..id_end)
            .ok_or(GitReadError::MalformedPayload)?;
        cursor = id_end;
    }
    Ok(())
}

fn find_blank_separator(payload: &[u8]) -> Option<usize> {
    payload
        .windows(2)
        .position(|window| window.first() == Some(&b'\n') && window.get(1) == Some(&b'\n'))
}

fn header_region(payload: &[u8]) -> Result<&[u8], GitReadError> {
    match find_blank_separator(payload) {
        Some(separator) => {
            let end = separator
                .checked_add(1)
                .ok_or(GitReadError::MalformedPayload)?;
            payload.get(..end).ok_or(GitReadError::MalformedPayload)
        }
        None => Ok(payload),
    }
}

fn validate_header_line(line: &[u8]) -> Result<(), GitReadError> {
    if line.is_empty() {
        return Err(GitReadError::MalformedPayload);
    }
    let space = line
        .iter()
        .position(|byte| *byte == b' ')
        .ok_or(GitReadError::MalformedPayload)?;
    let key = line.get(..space).ok_or(GitReadError::MalformedPayload)?;
    if key.is_empty() {
        return Err(GitReadError::MalformedPayload);
    }
    let key_valid = key
        .iter()
        .all(|byte| matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-'));
    if !key_valid {
        return Err(GitReadError::MalformedPayload);
    }
    Ok(())
}

fn validate_commit_payload(payload: &[u8]) -> Result<(), GitReadError> {
    if payload.is_empty() {
        return Err(GitReadError::MalformedPayload);
    }
    let region = header_region(payload)?;
    let mut cursor: usize = 0;
    let mut first = true;
    while cursor < region.len() {
        let rest = region.get(cursor..).ok_or(GitReadError::MalformedPayload)?;
        let end_of_line = rest
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or(GitReadError::MalformedPayload)?;
        let line_end = cursor
            .checked_add(end_of_line)
            .ok_or(GitReadError::MalformedPayload)?;
        let line = region
            .get(cursor..line_end)
            .ok_or(GitReadError::MalformedPayload)?;
        if first {
            validate_commit_first_line(line)?;
            first = false;
        } else {
            validate_header_line(line)?;
        }
        cursor = line_end
            .checked_add(1)
            .ok_or(GitReadError::MalformedPayload)?;
    }
    if first {
        return Err(GitReadError::MalformedPayload);
    }
    Ok(())
}

fn validate_commit_first_line(line: &[u8]) -> Result<(), GitReadError> {
    if line.len() != 5 + GIT_OBJECT_ID_HEX_LEN {
        return Err(GitReadError::MalformedPayload);
    }
    if line.get(..5) != Some(b"tree ".as_slice()) {
        return Err(GitReadError::MalformedPayload);
    }
    let id = line.get(5..).ok_or(GitReadError::MalformedPayload)?;
    if id.len() != GIT_OBJECT_ID_HEX_LEN || !is_hex_bytes(id) {
        return Err(GitReadError::MalformedPayload);
    }
    Ok(())
}

fn validate_tag_payload(payload: &[u8]) -> Result<(), GitReadError> {
    if payload.is_empty() {
        return Err(GitReadError::MalformedPayload);
    }
    let region = header_region(payload)?;
    let mut cursor: usize = 0;
    let mut index: usize = 0;
    while cursor < region.len() {
        let rest = region.get(cursor..).ok_or(GitReadError::MalformedPayload)?;
        let end_of_line = rest
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or(GitReadError::MalformedPayload)?;
        let line_end = cursor
            .checked_add(end_of_line)
            .ok_or(GitReadError::MalformedPayload)?;
        let line = region
            .get(cursor..line_end)
            .ok_or(GitReadError::MalformedPayload)?;
        if index == 0 {
            validate_tag_object_line(line)?;
        } else if index == 1 {
            validate_tag_type_line(line)?;
        } else if index == 2 {
            validate_tag_name_line(line)?;
        } else {
            validate_header_line(line)?;
        }
        index = index.checked_add(1).ok_or(GitReadError::MalformedPayload)?;
        cursor = line_end
            .checked_add(1)
            .ok_or(GitReadError::MalformedPayload)?;
    }
    if index < 3 {
        return Err(GitReadError::MalformedPayload);
    }
    Ok(())
}

fn validate_tag_object_line(line: &[u8]) -> Result<(), GitReadError> {
    if line.len() != 7 + GIT_OBJECT_ID_HEX_LEN {
        return Err(GitReadError::MalformedPayload);
    }
    if line.get(..7) != Some(b"object ".as_slice()) {
        return Err(GitReadError::MalformedPayload);
    }
    let id = line.get(7..).ok_or(GitReadError::MalformedPayload)?;
    if id.len() != GIT_OBJECT_ID_HEX_LEN || !is_hex_bytes(id) {
        return Err(GitReadError::MalformedPayload);
    }
    Ok(())
}

fn validate_tag_type_line(line: &[u8]) -> Result<(), GitReadError> {
    if line == b"type blob" || line == b"type tree" || line == b"type commit" || line == b"type tag"
    {
        Ok(())
    } else {
        Err(GitReadError::MalformedPayload)
    }
}

fn validate_tag_name_line(line: &[u8]) -> Result<(), GitReadError> {
    if line.len() <= 4 {
        return Err(GitReadError::MalformedPayload);
    }
    if line.get(..4) != Some(b"tag ".as_slice()) {
        return Err(GitReadError::MalformedPayload);
    }
    Ok(())
}

fn is_hex_bytes(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_hexdigit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::Blake3Digest32;

    const TEST_LIMITS: GitReadLimits = GitReadLimits {
        max_decompressed_bytes: 4096,
        max_path_token_bytes: 256,
    };

    fn repository_digest() -> Blake3Digest32 {
        Blake3Digest32::from_bytes([7; 32])
    }

    fn object_id(fill: u8) -> GitObjectId {
        GitObjectId::from_bytes([fill; 20])
    }

    fn encode_object(kind: &str, payload: &[u8]) -> Vec<u8> {
        let mut object = Vec::with_capacity(kind.len() + 24 + payload.len());
        object.extend_from_slice(kind.as_bytes());
        object.push(b' ');
        object.extend_from_slice(format!("{}", payload.len()).as_bytes());
        object.push(0);
        object.extend_from_slice(payload);
        object
    }

    fn tree_payload() -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"100644 ");
        payload.extend_from_slice(b"README.md");
        payload.push(0);
        payload.extend_from_slice(&[0x11; 20]);
        payload
    }

    fn commit_payload() -> Vec<u8> {
        let tree = "11".repeat(20);
        format!(
            "tree {tree}\nauthor Test <test@example.invalid> 0 +0000\ncommitter Test <test@example.invalid> 0 +0000\n\ninit\n"
        )
        .into_bytes()
    }

    fn tag_payload() -> Vec<u8> {
        let target = "22".repeat(20);
        format!(
            "object {target}\ntype commit\ntag v1.0\ntagger Test <test@example.invalid> 0 +0000\n\nrelease\n"
        )
        .into_bytes()
    }

    fn test_request(id_fill: u8, kind: Option<GitObjectKind>) -> GitObjectRequest {
        GitObjectRequest {
            repository_identity_digest: repository_digest(),
            object_id: object_id(id_fill),
            expected_kind: kind,
        }
    }

    fn backend_object_for(decompressed: Vec<u8>, id_fill: u8) -> GitBackendObject {
        GitBackendObject {
            decompressed,
            observed_id: object_id(id_fill),
            observed_repository_identity_digest: repository_digest(),
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FakeGitFailure {
        NotFound,
        Packed,
        Network,
        Backend,
    }

    struct FakeGitBackend {
        object: Option<GitBackendObject>,
        failure: Option<FakeGitFailure>,
        calls: usize,
    }

    impl FakeGitBackend {
        fn succeeds(object: GitBackendObject) -> Self {
            Self {
                object: Some(object),
                failure: None,
                calls: 0,
            }
        }

        fn fails(failure: FakeGitFailure) -> Self {
            Self {
                object: None,
                failure: Some(failure),
                calls: 0,
            }
        }
    }

    impl GitObjectBackend for FakeGitBackend {
        type BackendError = FakeGitFailure;

        fn read_loose_object(
            &mut self,
            _request: &GitLooseOpenRequest,
        ) -> Result<GitBackendObject, Self::BackendError> {
            self.calls += 1;
            if let Some(failure) = self.failure {
                return Err(failure);
            }
            self.object.clone().ok_or(FakeGitFailure::NotFound)
        }

        fn map_backend_error(error: &Self::BackendError) -> GitReadError {
            match error {
                FakeGitFailure::NotFound => GitReadError::ObjectNotFound,
                FakeGitFailure::Packed => GitReadError::PackedObjectUnavailable,
                FakeGitFailure::Network => GitReadError::ObjectRequiresNetwork,
                FakeGitFailure::Backend => GitReadError::BackendFailure,
            }
        }
    }

    #[test]
    fn object_id_round_trips_hex() {
        let id =
            GitObjectId::parse_hex("ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12").expect("valid hex");
        assert_eq!(id.hex(), "ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12");
        assert_eq!(id.as_bytes().len(), GIT_OBJECT_ID_BYTE_LEN);
    }

    #[test]
    fn object_id_rejects_malformed_hex() {
        assert_eq!(
            GitObjectId::parse_hex(""),
            Err(GitReadError::InvalidObjectId)
        );
        assert_eq!(
            GitObjectId::parse_hex("abc"),
            Err(GitReadError::InvalidObjectId)
        );
        assert_eq!(
            GitObjectId::parse_hex(&"ab".repeat(21)),
            Err(GitReadError::InvalidObjectId)
        );
        assert_eq!(
            GitObjectId::parse_hex("zz12cd34ef56ab12cd34ef56ab12cd34ef56ab12"),
            Err(GitReadError::InvalidObjectId)
        );
    }

    #[test]
    fn loose_path_derives_canonical_objects_location() {
        let id = GitObjectId::from_bytes([0xAB; 20]);
        let path = id.loose_relative_path(TEST_LIMITS).expect("canonical path");
        let rest = "ab".repeat(19);
        assert_eq!(path.as_str(), format!("objects/ab/{rest}"));
    }

    #[test]
    fn request_validation_derives_path() {
        let validated = validate_git_object_request(
            &test_request(0x01, Some(GitObjectKind::Blob)),
            TEST_LIMITS,
        )
        .expect("valid request");
        assert_eq!(validated.object_id, object_id(0x01));
        assert_eq!(validated.repository_identity_digest, repository_digest());
        assert!(validated.relative_path.as_str().starts_with("objects/"));
    }

    #[test]
    fn limits_reject_zero_ceiling() {
        let zero = GitReadLimits {
            max_decompressed_bytes: 0,
            max_path_token_bytes: 256,
        };
        assert_eq!(zero.validate(), Err(GitReadError::InvalidLimits));
        assert_eq!(
            parse_loose_object(b"blob 0\0", zero, None).map(|parsed| parsed.declared_size),
            Err(GitReadError::InvalidLimits)
        );
    }

    #[test]
    fn blob_fixture_parses_with_exact_payload() {
        let payload = b"hello world\n";
        let object = encode_object("blob", payload);
        let parsed = parse_loose_object(&object, TEST_LIMITS, Some(GitObjectKind::Blob))
            .expect("blob parses");
        assert_eq!(parsed.kind, GitObjectKind::Blob);
        assert_eq!(parsed.payload, payload);
        assert_eq!(
            parsed.declared_size,
            u64::try_from(payload.len()).expect("len")
        );
    }

    #[test]
    fn tree_fixture_parses_with_exact_payload() {
        let payload = tree_payload();
        let object = encode_object("tree", &payload);
        let parsed = parse_loose_object(&object, TEST_LIMITS, Some(GitObjectKind::Tree))
            .expect("tree parses");
        assert_eq!(parsed.kind, GitObjectKind::Tree);
        assert_eq!(parsed.payload, payload.as_slice());
    }

    #[test]
    fn commit_fixture_parses_with_exact_payload() {
        let payload = commit_payload();
        let object = encode_object("commit", &payload);
        let parsed = parse_loose_object(&object, TEST_LIMITS, Some(GitObjectKind::Commit))
            .expect("commit parses");
        assert_eq!(parsed.kind, GitObjectKind::Commit);
        assert_eq!(parsed.payload, payload.as_slice());
    }

    #[test]
    fn tag_fixture_parses_with_exact_payload() {
        let payload = tag_payload();
        let object = encode_object("tag", &payload);
        let parsed =
            parse_loose_object(&object, TEST_LIMITS, Some(GitObjectKind::Tag)).expect("tag parses");
        assert_eq!(parsed.kind, GitObjectKind::Tag);
        assert_eq!(parsed.payload, payload.as_slice());
    }

    #[test]
    fn header_without_nul_is_malformed() {
        assert_eq!(
            parse_loose_object(b"blob 3 abc", TEST_LIMITS, None).map(|_| ()),
            Err(GitReadError::MalformedHeader)
        );
        assert_eq!(
            parse_loose_object(b"", TEST_LIMITS, None).map(|_| ()),
            Err(GitReadError::MalformedHeader)
        );
    }

    #[test]
    fn unknown_kind_is_rejected() {
        let object = encode_object("blorb", b"abc");
        assert_eq!(
            parse_loose_object(&object, TEST_LIMITS, None).map(|_| ()),
            Err(GitReadError::InvalidKind)
        );
    }

    #[test]
    fn declared_size_mismatch_is_rejected() {
        let mut object = b"blob 5\0hi".to_vec();
        assert_eq!(
            parse_loose_object(&object, TEST_LIMITS, None).map(|_| ()),
            Err(GitReadError::SizeMismatch)
        );
        object = b"blob 1\0hi".to_vec();
        assert_eq!(
            parse_loose_object(&object, TEST_LIMITS, None).map(|_| ()),
            Err(GitReadError::SizeMismatch)
        );
    }

    #[test]
    fn declared_size_over_ceiling_is_rejected() {
        let tiny = GitReadLimits {
            max_decompressed_bytes: 16,
            max_path_token_bytes: 256,
        };
        let object = b"blob 999999\0hi".to_vec();
        assert_eq!(
            parse_loose_object(&object, tiny, None).map(|_| ()),
            Err(GitReadError::ObjectTooLarge)
        );
    }

    #[test]
    fn input_over_ceiling_is_rejected() {
        let tiny = GitReadLimits {
            max_decompressed_bytes: 8,
            max_path_token_bytes: 256,
        };
        let object = encode_object("blob", b"hello world, this is long");
        assert_eq!(
            parse_loose_object(&object, tiny, None).map(|_| ()),
            Err(GitReadError::ObjectTooLarge)
        );
    }

    #[test]
    fn expected_kind_mismatch_is_rejected() {
        let payload = tree_payload();
        let object = encode_object("tree", &payload);
        assert_eq!(
            parse_loose_object(&object, TEST_LIMITS, Some(GitObjectKind::Blob)).map(|_| ()),
            Err(GitReadError::KindMismatch)
        );
    }

    #[test]
    fn expected_kind_none_accepts_any_known_kind() {
        for (kind, payload) in [
            ("blob", b"data".to_vec()),
            ("tree", tree_payload()),
            ("commit", commit_payload()),
            ("tag", tag_payload()),
        ] {
            let object = encode_object(kind, &payload);
            let parsed =
                parse_loose_object(&object, TEST_LIMITS, None).expect("known kind accepted");
            assert_eq!(parsed.kind.as_str(), kind);
        }
    }

    #[test]
    fn tree_with_truncated_entry_is_malformed() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"100644 ");
        payload.extend_from_slice(b"no-sha-here");
        payload.push(0);
        payload.extend_from_slice(&[0x11; 4]);
        let object = encode_object("tree", &payload);
        assert_eq!(
            parse_loose_object(&object, TEST_LIMITS, None).map(|_| ()),
            Err(GitReadError::MalformedPayload)
        );
    }

    #[test]
    fn commit_without_tree_line_is_malformed() {
        let payload = b"author Test <t@e> 0 +0000\n\nmsg\n".to_vec();
        let object = encode_object("commit", &payload);
        assert_eq!(
            parse_loose_object(&object, TEST_LIMITS, None).map(|_| ()),
            Err(GitReadError::MalformedPayload)
        );
    }

    #[test]
    fn tag_without_object_lines_is_malformed() {
        let payload = b"type commit\ntag v1\n\nmsg\n".to_vec();
        let object = encode_object("tag", &payload);
        assert_eq!(
            parse_loose_object(&object, TEST_LIMITS, None).map(|_| ()),
            Err(GitReadError::MalformedPayload)
        );
    }

    #[test]
    fn missing_object_returns_not_found() {
        let mut backend = FakeGitBackend::fails(FakeGitFailure::NotFound);
        let mut never_cancel = || false;
        assert_eq!(
            read_git_object_no_execute(
                &mut backend,
                &test_request(0x01, Some(GitObjectKind::Blob)),
                TEST_LIMITS,
                &mut never_cancel
            ),
            Err(GitReadError::ObjectNotFound)
        );
    }

    #[test]
    fn packed_object_returns_explicit_unavailable() {
        let mut backend = FakeGitBackend::fails(FakeGitFailure::Packed);
        let mut never_cancel = || false;
        assert_eq!(
            read_git_object_no_execute(
                &mut backend,
                &test_request(0x01, Some(GitObjectKind::Blob)),
                TEST_LIMITS,
                &mut never_cancel
            ),
            Err(GitReadError::PackedObjectUnavailable)
        );
    }

    #[test]
    fn network_promised_object_returns_requires_network() {
        let mut backend = FakeGitBackend::fails(FakeGitFailure::Network);
        let mut never_cancel = || false;
        assert_eq!(
            read_git_object_no_execute(
                &mut backend,
                &test_request(0x01, Some(GitObjectKind::Blob)),
                TEST_LIMITS,
                &mut never_cancel
            ),
            Err(GitReadError::ObjectRequiresNetwork)
        );
    }

    #[test]
    fn backend_failure_is_content_free() {
        let mut backend = FakeGitBackend::fails(FakeGitFailure::Backend);
        let mut never_cancel = || false;
        assert_eq!(
            read_git_object_no_execute(
                &mut backend,
                &test_request(0x01, Some(GitObjectKind::Blob)),
                TEST_LIMITS,
                &mut never_cancel
            ),
            Err(GitReadError::BackendFailure)
        );
    }

    #[test]
    fn observed_id_mismatch_is_rejected() {
        let payload = b"exact-bytes".to_vec();
        let backend_object = backend_object_for(encode_object("blob", &payload), 0x02);
        let mut backend = FakeGitBackend::succeeds(backend_object);
        let mut never_cancel = || false;
        assert_eq!(
            read_git_object_no_execute(
                &mut backend,
                &test_request(0x01, Some(GitObjectKind::Blob)),
                TEST_LIMITS,
                &mut never_cancel
            ),
            Err(GitReadError::ObjectIdentityMismatch)
        );
    }

    #[test]
    fn repository_mismatch_is_rejected() {
        let payload = b"exact-bytes".to_vec();
        let mut backend_object = backend_object_for(encode_object("blob", &payload), 0x01);
        backend_object.observed_repository_identity_digest = Blake3Digest32::from_bytes([9; 32]);
        let mut backend = FakeGitBackend::succeeds(backend_object);
        let mut never_cancel = || false;
        assert_eq!(
            read_git_object_no_execute(
                &mut backend,
                &test_request(0x01, Some(GitObjectKind::Blob)),
                TEST_LIMITS,
                &mut never_cancel
            ),
            Err(GitReadError::RepositoryMismatch)
        );
    }

    #[test]
    fn cancel_before_backend_yields_no_product() {
        let payload = b"exact-bytes".to_vec();
        let mut backend =
            FakeGitBackend::succeeds(backend_object_for(encode_object("blob", &payload), 0x01));
        let mut should_cancel = || true;
        assert_eq!(
            read_git_object_no_execute(
                &mut backend,
                &test_request(0x01, Some(GitObjectKind::Blob)),
                TEST_LIMITS,
                &mut should_cancel
            ),
            Err(GitReadError::Cancelled)
        );
        assert_eq!(backend.calls, 0);
    }

    #[test]
    fn full_read_binds_identity_and_bytes() {
        let payload = b"exact-object-bytes".to_vec();
        let mut backend =
            FakeGitBackend::succeeds(backend_object_for(encode_object("blob", &payload), 0x03));
        let mut never_cancel = || false;
        let result = read_git_object_no_execute(
            &mut backend,
            &test_request(0x03, Some(GitObjectKind::Blob)),
            TEST_LIMITS,
            &mut never_cancel,
        )
        .expect("read succeeds");
        assert_eq!(result.object_id, object_id(0x03));
        assert_eq!(result.repository_identity_digest, repository_digest());
        assert_eq!(result.kind, GitObjectKind::Blob);
        assert_eq!(result.payload(), payload.as_slice());
        assert_eq!(
            result.payload_len,
            u64::try_from(payload.len()).expect("len")
        );
        assert_eq!(backend.calls, 1);
    }

    #[test]
    fn result_and_error_views_are_redacted() {
        let payload = b"top-secret-payload".to_vec();
        let mut backend =
            FakeGitBackend::succeeds(backend_object_for(encode_object("blob", &payload), 0x04));
        let mut never_cancel = || false;
        let result = read_git_object_no_execute(
            &mut backend,
            &test_request(0x04, Some(GitObjectKind::Blob)),
            TEST_LIMITS,
            &mut never_cancel,
        )
        .expect("read succeeds");
        let debug = format!("{result:?}");
        assert!(!debug.contains("top-secret-payload"));
        assert!(debug.contains(&payload.len().to_string()));
        let error = GitReadError::SizeMismatch;
        assert_eq!(format!("{error}"), "GIT_OBJECT_SIZE_MISMATCH");
        assert!(!format!("{error:?}").contains("top-secret-payload"));
    }
}
