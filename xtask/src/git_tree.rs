//! Immutable Git-tree reader shared by Rust-only advisory tooling.
//!
//! Every repository input is addressed through one exact commit object. The
//! working tree is never consulted for source bytes, TOML documents or file
//! inventories.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use toml::Value;

use crate::ticket_planner::{safe_path, PLANNER_FILE_BYTE_CEILING};

/// One exact Git tree entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitTreeEntry {
    /// Repository-relative path.
    pub path: String,
    /// Git file mode.
    pub mode: String,
    /// Git object type.
    pub object_type: String,
    /// Raw object identifier without algorithm prefix.
    pub object_id: String,
}

impl GitTreeEntry {
    /// True only for ordinary executable or non-executable blobs.
    #[must_use]
    pub fn regular_blob(&self) -> bool {
        self.object_type == "blob" && matches!(self.mode.as_str(), "100644" | "100755")
    }
}

/// Closed immutable-tree read failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitTreeError {
    reason: &'static str,
    message: String,
}

impl GitTreeError {
    fn new(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    /// Stable machine reason code.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }

    /// Content-free diagnostic detail.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for GitTreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason, self.message)
    }
}

impl std::error::Error for GitTreeError {}

/// Read-only view of one immutable Git commit.
#[derive(Clone, Debug)]
pub struct GitTree {
    root: PathBuf,
    object_format: String,
    oid: String,
    tagged_commit: String,
}

impl GitTree {
    /// Opens `root` and resolves exactly one full algorithm-tagged commit.
    ///
    /// # Errors
    ///
    /// Returns `GIT_REPOSITORY_INVALID` for a non-repository or unsupported
    /// object format, and `BASE_COMMIT_INVALID` for malformed, foreign-format,
    /// missing or non-commit base identities.
    pub fn open(root: &Path, tagged_commit: &str) -> Result<Self, GitTreeError> {
        let root = std::fs::canonicalize(root).map_err(|error| {
            GitTreeError::new(
                "GIT_REPOSITORY_INVALID",
                format!("unable to canonicalize repository root: {error}"),
            )
        })?;
        let probe = run_git(&root, ["rev-parse", "--git-dir"])?;
        if !probe.status.success() {
            return Err(GitTreeError::new(
                "GIT_REPOSITORY_INVALID",
                "repository root is not a Git repository",
            ));
        }
        let object_format = run_git_text(&root, ["rev-parse", "--show-object-format"])?
            .trim()
            .to_owned();
        if !matches!(object_format.as_str(), "sha1" | "sha256") {
            return Err(GitTreeError::new(
                "GIT_REPOSITORY_INVALID",
                "unsupported Git object format",
            ));
        }
        let (algorithm, oid) = parse_tagged_commit(tagged_commit).ok_or_else(|| {
            GitTreeError::new(
                "BASE_COMMIT_INVALID",
                "base commit must be a full algorithm-tagged Git object ID",
            )
        })?;
        if algorithm != object_format {
            return Err(GitTreeError::new(
                "BASE_COMMIT_INVALID",
                "base commit algorithm differs from repository object format",
            ));
        }
        let expected = if object_format == "sha1" { 40 } else { 64 };
        if oid.len() != expected || !is_lower_hex(oid) {
            return Err(GitTreeError::new(
                "BASE_COMMIT_INVALID",
                "base commit object ID is not canonical lowercase hex",
            ));
        }
        let commit_ref = format!("{oid}^{{commit}}");
        if !run_git(&root, ["cat-file", "-e", commit_ref.as_str()])?
            .status
            .success()
        {
            return Err(GitTreeError::new(
                "BASE_COMMIT_INVALID",
                "base commit does not exist as a commit object",
            ));
        }
        Ok(Self {
            root,
            object_format,
            oid: oid.to_owned(),
            tagged_commit: format!("{algorithm}:{oid}"),
        })
    }

    /// Canonical repository root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Git object format (`sha1` or `sha256`).
    #[must_use]
    pub fn object_format(&self) -> &str {
        &self.object_format
    }

    /// Raw immutable commit object identifier.
    #[must_use]
    pub fn oid(&self) -> &str {
        &self.oid
    }

    /// Full algorithm-tagged immutable commit.
    #[must_use]
    pub fn tagged_commit(&self) -> &str {
        &self.tagged_commit
    }

    /// Resolves exactly one tree entry for `path`.
    ///
    /// # Errors
    ///
    /// Repository or UTF-8 decoding failures return
    /// `GIT_REPOSITORY_INVALID`. Unsafe paths return `Ok(None)`.
    pub fn entry(&self, path: &str) -> Result<Option<GitTreeEntry>, GitTreeError> {
        if !safe_path(path) {
            return Ok(None);
        }
        let output = run_git(
            &self.root,
            ["ls-tree", "-z", self.oid.as_str(), "--", path],
        )?;
        if !output.status.success() {
            return Err(GitTreeError::new(
                "GIT_REPOSITORY_INVALID",
                "git ls-tree failed",
            ));
        }
        let mut matches = Vec::new();
        for record in output.stdout.split(|byte| *byte == 0).filter(|row| !row.is_empty()) {
            let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
                return Err(GitTreeError::new(
                    "GIT_REPOSITORY_INVALID",
                    "malformed git ls-tree record",
                ));
            };
            let meta = std::str::from_utf8(&record[..tab]).map_err(|_| {
                GitTreeError::new("GIT_REPOSITORY_INVALID", "non-ASCII git tree metadata")
            })?;
            let decoded = std::str::from_utf8(&record[tab + 1..]).map_err(|_| {
                GitTreeError::new("GIT_REPOSITORY_INVALID", "non-UTF-8 git tree path")
            })?;
            let mut fields = meta.split(' ');
            let (Some(mode), Some(object_type), Some(object_id), None) = (
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
            ) else {
                return Err(GitTreeError::new(
                    "GIT_REPOSITORY_INVALID",
                    "malformed git tree metadata",
                ));
            };
            if decoded == path {
                matches.push(GitTreeEntry {
                    path: decoded.to_owned(),
                    mode: mode.to_owned(),
                    object_type: object_type.to_owned(),
                    object_id: object_id.to_owned(),
                });
            }
        }
        Ok((matches.len() == 1).then(|| matches.remove(0)))
    }

    /// Reads one exact regular committed blob with the planner byte ceiling.
    ///
    /// # Errors
    ///
    /// Missing, non-regular, over-budget or unreadable committed entries fail
    /// with the same closed codes used by the retired Python reader.
    pub fn read_bytes(
        &self,
        path: &str,
    ) -> Result<(Vec<u8>, GitTreeEntry), GitTreeError> {
        self.read_bytes_bounded(path, PLANNER_FILE_BYTE_CEILING)
    }

    /// Reads one exact regular committed blob with an explicit byte ceiling.
    ///
    /// # Errors
    ///
    /// See [`Self::read_bytes`].
    pub fn read_bytes_bounded(
        &self,
        path: &str,
        max_bytes: u64,
    ) -> Result<(Vec<u8>, GitTreeEntry), GitTreeError> {
        let entry = self.entry(path)?.ok_or_else(|| {
            GitTreeError::new(
                "CONTEXT_SOURCE_MISSING",
                format!("missing committed path: {path}"),
            )
        })?;
        if !entry.regular_blob() {
            return Err(GitTreeError::new(
                "CONTEXT_SOURCE_NOT_REGULAR",
                format!("path is not a regular committed blob: {path}"),
            ));
        }
        let size_text = run_git_text(
            &self.root,
            ["cat-file", "-s", entry.object_id.as_str()],
        )?;
        let size = size_text.trim().parse::<u64>().map_err(|_| {
            GitTreeError::new(
                "GIT_REPOSITORY_INVALID",
                format!("invalid Git blob size: {path}"),
            )
        })?;
        if size > max_bytes {
            return Err(GitTreeError::new(
                "CONTEXT_BUDGET_EXCEEDED",
                format!("committed file exceeds planner byte ceiling: {path}"),
            ));
        }
        let output = run_git(
            &self.root,
            ["cat-file", "blob", entry.object_id.as_str()],
        )?;
        if !output.status.success() {
            return Err(GitTreeError::new(
                "GIT_REPOSITORY_INVALID",
                format!("unable to read Git blob: {path}"),
            ));
        }
        if u64::try_from(output.stdout.len()).ok() != Some(size) {
            return Err(GitTreeError::new(
                "GIT_REPOSITORY_INVALID",
                format!("Git blob readback length mismatch: {path}"),
            ));
        }
        Ok((output.stdout, entry))
    }

    /// Reads one exact committed UTF-8 blob.
    ///
    /// # Errors
    ///
    /// Non-UTF-8 bytes return `CONTEXT_SOURCE_NOT_UTF8`.
    pub fn read_text(&self, path: &str) -> Result<(String, GitTreeEntry), GitTreeError> {
        let (raw, entry) = self.read_bytes(path)?;
        let text = String::from_utf8(raw).map_err(|_| {
            GitTreeError::new(
                "CONTEXT_SOURCE_NOT_UTF8",
                format!("committed file is not UTF-8: {path}"),
            )
        })?;
        Ok((text, entry))
    }

    /// Loads one committed TOML document.
    ///
    /// # Errors
    ///
    /// Invalid UTF-8/TOML or a non-table root returns
    /// `CONTROL_SCHEMA_MISMATCH`.
    pub fn load_toml(&self, path: &str) -> Result<(Value, GitTreeEntry), GitTreeError> {
        let (raw, entry) = self.read_bytes(path)?;
        let text = std::str::from_utf8(&raw).map_err(|_| {
            GitTreeError::new(
                "CONTROL_SCHEMA_MISMATCH",
                format!("invalid committed TOML UTF-8: {path}"),
            )
        })?;
        let value: Value = toml::from_str(text).map_err(|error| {
            GitTreeError::new(
                "CONTROL_SCHEMA_MISMATCH",
                format!("invalid committed TOML: {path}: {error}"),
            )
        })?;
        if !value.is_table() {
            return Err(GitTreeError::new(
                "CONTROL_SCHEMA_MISMATCH",
                format!("invalid TOML root: {path}"),
            ));
        }
        Ok((value, entry))
    }

    /// Lists committed paths recursively below one safe prefix.
    ///
    /// # Errors
    ///
    /// Unsafe prefixes or malformed output fail closed.
    pub fn list_files(&self, prefix: &str) -> Result<Vec<String>, GitTreeError> {
        if !safe_path(prefix) {
            return Err(GitTreeError::new(
                "CONTROL_SCHEMA_MISMATCH",
                format!("unsafe tree prefix: {prefix}"),
            ));
        }
        let output = run_git(
            &self.root,
            [
                "ls-tree",
                "-r",
                "-z",
                "--name-only",
                self.oid.as_str(),
                "--",
                prefix,
            ],
        )?;
        if !output.status.success() {
            return Err(GitTreeError::new(
                "GIT_REPOSITORY_INVALID",
                format!("unable to list tree: {prefix}"),
            ));
        }
        let mut paths = Vec::new();
        for raw in output.stdout.split(|byte| *byte == 0).filter(|row| !row.is_empty()) {
            paths.push(
                std::str::from_utf8(raw)
                    .map_err(|_| {
                        GitTreeError::new(
                            "GIT_REPOSITORY_INVALID",
                            "non-UTF-8 path in Git tree",
                        )
                    })?
                    .to_owned(),
            );
        }
        paths.sort();
        Ok(paths)
    }

    /// Algorithm-tagged blob identity.
    #[must_use]
    pub fn blob_identity(&self, entry: &GitTreeEntry) -> String {
        format!("{}:{}", self.object_format, entry.object_id)
    }

    /// Returns whether a full tagged object resolves to a commit in this repository.
    #[must_use]
    pub fn commit_exists(&self, tagged: &str) -> bool {
        let Some((algorithm, oid)) = parse_tagged_commit(tagged) else {
            return false;
        };
        if algorithm != self.object_format {
            return false;
        }
        let expected = if algorithm == "sha1" { 40 } else { 64 };
        if oid.len() != expected || !is_lower_hex(oid) {
            return false;
        }
        let commit_ref = format!("{oid}^{{commit}}");
        run_git(&self.root, ["cat-file", "-e", commit_ref.as_str()])
            .is_ok_and(|output| output.status.success())
    }
}

fn parse_tagged_commit(value: &str) -> Option<(&str, &str)> {
    let (algorithm, oid) = value.split_once(':')?;
    matches!(algorithm, "sha1" | "sha256").then_some((algorithm, oid))
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn run_git<I, S>(root: &Path, args: I) -> Result<Output, GitTreeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| {
            GitTreeError::new(
                "GIT_REPOSITORY_INVALID",
                format!("unable to execute Git: {error}"),
            )
        })
}

fn run_git_text<I, S>(root: &Path, args: I) -> Result<String, GitTreeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = run_git(root, args)?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(GitTreeError::new(
            "GIT_REPOSITORY_INVALID",
            if detail.trim().is_empty() {
                "Git command failed".to_owned()
            } else {
                detail.trim().to_owned()
            },
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| {
        GitTreeError::new(
            "GIT_REPOSITORY_INVALID",
            "Git command returned non-UTF-8 output",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tagged_commit_grammar_is_closed() {
        assert_eq!(
            parse_tagged_commit(&format!("sha1:{}", "a".repeat(40))),
            Some(("sha1", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"))
        );
        assert!(parse_tagged_commit("HEAD").is_none());
        assert!(parse_tagged_commit("sha512:abcd").is_none());
        assert!(!is_lower_hex("ABCDEF"));
        assert!(is_lower_hex("0123456789abcdef"));
    }
}
