//! No-execute Git source composition over canonical source-owner kernels.
//!
//! This compatibility module exists for the Git qualification process and the
//! daemon's staged Git integration. It does not own admission policy, receipt
//! semantics, source/revision identity formulas or registry state. Those enter
//! through the same canonical DIRECT composition used by live file ingestion.

#[cfg(test)]
#[path = "direct_store/composition.rs"]
mod canonical;

#[cfg(test)]
pub(crate) use canonical::{AdmissionPolicy, PriorSourceView, RegistryView};
#[cfg(test)]
use canonical::{derive_git_stable_digest, plan_snapshot};

#[cfg(not(test))]
pub(crate) use crate::source_composition::{AdmissionPolicy, PriorSourceView, RegistryView};
#[cfg(not(test))]
use crate::source_composition::{derive_git_stable_digest, plan_snapshot};

use std::path::Path;

use search_safe_reader::git::{
    GitObjectId, GitObjectKind, GitReadError, GitReadLimits, parse_loose_object,
};

use crate::sha256;

/// Statically pinned no-execute invariant for Git composition.
pub const GIT_SOURCE_NO_EXECUTE: bool = true;
const _: () = assert!(GIT_SOURCE_NO_EXECUTE);

/// Maximum decompressed Git object bytes.
pub const MAX_GIT_OBJECT_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum bytes in one derived loose-object path token.
pub const MAX_GIT_PATH_TOKEN_BYTES: usize = 32_768;

/// Repository digest text is not exactly one non-zero lowercase/uppercase
/// 64-hex value.
pub const GIT_SOURCE_REPOSITORY_INVALID: &str =
    "GIT_SOURCE_REPOSITORY_INVALID";
/// Lineage evidence digest is not exactly one non-zero 64-hex value.
pub const GIT_SOURCE_LINEAGE_INVALID: &str = "GIT_SOURCE_LINEAGE_INVALID";

/// Closed Git lineage relationship.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GitLineageKind {
    /// Canonical repository object.
    Repository,
    /// Linked worktree view of the same repository.
    Worktree,
    /// Submodule repository bound by superproject evidence.
    Submodule,
    /// Forked repository with distinct lineage evidence.
    Fork,
    /// Mirror repository with distinct lineage evidence.
    Mirror,
}

impl GitLineageKind {
    /// Stable wire string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::Worktree => "worktree",
            Self::Submodule => "submodule",
            Self::Fork => "fork",
            Self::Mirror => "mirror",
        }
    }
}

/// Exact lineage binding carried as non-identity evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitLineage {
    /// Closed lineage relationship.
    pub kind: GitLineageKind,
    /// Exact evidence digest binding that relationship.
    pub evidence_digest_hex: String,
}

/// Full canonical plan for one retained Git object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitPlannedSource {
    /// Durable source identifier.
    pub source_id: String,
    /// Revision identifier binding source, content and exact length.
    pub revision_id: String,
    /// Canonical lowercase object identifier.
    pub object_id_hex: String,
    /// Canonical lowercase admitted repository digest.
    pub repository_digest_hex: String,
    /// Validated Git object kind.
    pub kind: GitObjectKind,
    /// Exact payload length, excluding the loose-object header.
    pub payload_len: u64,
    /// Bound lineage evidence; it never substitutes for stable identity.
    pub lineage: GitLineage,
    /// Git repository plus object identity is always native stable evidence.
    pub identity_native: bool,
}

/// Validates one admitted repository identity digest.
pub fn validate_git_repository_hex(value: &str) -> Result<[u8; 32], String> {
    let raw = sha256::decode_digest(value)
        .ok_or_else(|| GIT_SOURCE_REPOSITORY_INVALID.to_owned())?;
    if raw == [0_u8; 32] {
        return Err(GIT_SOURCE_REPOSITORY_INVALID.to_owned());
    }
    Ok(raw)
}

/// Validates one exact Git object ID.
pub fn validate_git_object_hex(value: &str) -> Result<GitObjectId, String> {
    GitObjectId::parse_hex(value).map_err(|error| error.code().to_owned())
}

/// Derives canonical `objects/aa/bb…` addressing from object bytes only.
pub fn derive_git_loose_path(object_id_hex: &str) -> Result<String, String> {
    let object_id = validate_git_object_hex(object_id_hex)?;
    let token = object_id
        .loose_relative_path(git_read_limits())
        .map_err(|error| error.code().to_owned())?;
    Ok(token.as_str().to_owned())
}

/// Derives stable Git identity through the canonical source-identity owner.
pub fn git_stable_identity_hex(
    repository_identity_digest_hex: &str,
    object_id_hex: &str,
) -> Result<String, String> {
    let repository = validate_git_repository_hex(repository_identity_digest_hex)?;
    let object_id = validate_git_object_hex(object_id_hex)?;
    Ok(sha256::hex(&derive_git_stable_digest(
        &repository,
        object_id.as_bytes(),
    )))
}

/// Validates one bounded lineage-evidence binding.
pub fn validate_git_lineage(lineage: &GitLineage) -> Result<(), String> {
    let raw = sha256::decode_digest(&lineage.evidence_digest_hex)
        .ok_or_else(|| GIT_SOURCE_LINEAGE_INVALID.to_owned())?;
    if raw == [0_u8; 32] {
        return Err(GIT_SOURCE_LINEAGE_INVALID.to_owned());
    }
    Ok(())
}

/// Maps one no-execute Git kernel failure to its closed reason code.
pub const fn git_error_code(error: GitReadError) -> &'static str {
    error.code()
}

const fn git_read_limits() -> GitReadLimits {
    GitReadLimits {
        max_decompressed_bytes: MAX_GIT_OBJECT_BYTES,
        max_path_token_bytes: MAX_GIT_PATH_TOKEN_BYTES,
    }
}

/// Plans one already-read loose Git object through canonical admission,
/// identity and registry owners.
///
/// The function performs no filesystem, Git command, process, hook, filter,
/// credential-helper or network operation. Paths classify admission only;
/// stable identity is the admitted repository digest plus the exact object ID.
#[allow(clippy::too_many_arguments)]
pub fn plan_git_snapshot(
    repository_identity_digest_hex: &str,
    object_id_hex: &str,
    expected_kind: Option<GitObjectKind>,
    decompressed_object: &[u8],
    logical_path_for_admission: &Path,
    lineage: &GitLineage,
    namespace_hex: &str,
    policy: &AdmissionPolicy,
    view: &RegistryView,
) -> Result<GitPlannedSource, String> {
    let object_len = u64::try_from(decompressed_object.len())
        .map_err(|_| "GIT_OBJECT_TOO_LARGE".to_owned())?;
    if object_len > MAX_GIT_OBJECT_BYTES {
        return Err("GIT_OBJECT_TOO_LARGE".to_owned());
    }

    let repository = validate_git_repository_hex(repository_identity_digest_hex)?;
    let object_id = validate_git_object_hex(object_id_hex)?;
    let _ = derive_git_loose_path(object_id_hex)?;
    validate_git_lineage(lineage)?;

    let parsed = parse_loose_object(
        decompressed_object,
        git_read_limits(),
        expected_kind,
    )
    .map_err(|error| error.code().to_owned())?;
    let payload = parsed.payload;
    let payload_len = u64::try_from(payload.len())
        .map_err(|_| "GIT_OBJECT_TOO_LARGE".to_owned())?;
    let stable_identity = sha256::hex(&derive_git_stable_digest(
        &repository,
        object_id.as_bytes(),
    ));
    let content_digest = sha256::hex(&sha256::digest(payload));
    let plan = plan_snapshot(
        logical_path_for_admission,
        &stable_identity,
        "native",
        &content_digest,
        payload,
        namespace_hex,
        policy,
        view,
    )?;

    Ok(GitPlannedSource {
        source_id: plan.source_id,
        revision_id: plan.revision_id,
        object_id_hex: object_id.hex(),
        repository_digest_hex: sha256::hex(&repository),
        kind: parsed.kind,
        payload_len,
        lineage: lineage.clone(),
        identity_native: true,
    })
}
