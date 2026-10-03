//! Filesystem boundary for ordinary context-artifact candidate outputs.
//!
//! Candidate files are local advisory artifacts. This module never writes
//! control records, context manifests, tickets, leases or immutable artifact
//! references.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use crate::context_artifact::advisory_output_target;

/// Closed candidate-output publication failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateOutputError {
    reason: &'static str,
    message: String,
}

impl CandidateOutputError {
    fn new(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    /// Stable machine-readable reason code.
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

impl fmt::Display for CandidateOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason, self.message)
    }
}

impl std::error::Error for CandidateOutputError {}

/// Validated candidate output directory below the repository root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateOutputRoot {
    root: PathBuf,
    relative: String,
    path: PathBuf,
}

impl CandidateOutputRoot {
    /// Repository root used for output publication.
    #[must_use]
    pub fn repository_root(&self) -> &Path {
        &self.root
    }

    /// Slash-normalized repository-relative output directory.
    #[must_use]
    pub fn relative(&self) -> &str {
        &self.relative
    }

    /// Absolute output directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Resolves a package-specific file below the validated root.
    ///
    /// # Errors
    ///
    /// Unsafe package/file components fail with
    /// `OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT`.
    pub fn file(
        &self,
        package: &str,
        file_name: &str,
    ) -> Result<PathBuf, CandidateOutputError> {
        if !simple_component(package) || !simple_component(file_name) {
            return Err(CandidateOutputError::new(
                "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
                "candidate output contains an unsafe path component",
            ));
        }
        Ok(self.path.join(package).join(file_name))
    }
}

/// Validates one output directory without creating it.
///
/// Existing symlink/reparse components fail closed. Non-existing descendants
/// are accepted and created later one component at a time.
///
/// # Errors
///
/// Returns `OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT` for grammar/containment failures
/// and `OUTPUT_PATH_SYMLINK` for symlink or non-directory ancestors.
pub fn validate_output_root(
    root: &Path,
    relative: &str,
) -> Result<CandidateOutputRoot, CandidateOutputError> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        CandidateOutputError::new(
            "CANDIDATE_OUTPUT_WRITE_FAILED",
            format!("unable to canonicalize repository root: {error}"),
        )
    })?;
    let relative = advisory_output_target(relative)
        .map_err(|error| CandidateOutputError::new(error.reason(), error.to_string()))?;
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    reject_existing_non_directories(&root, &path)?;
    Ok(CandidateOutputRoot {
        root,
        relative,
        path,
    })
}

/// Writes exact bytes once and accepts byte-identical replay.
///
/// # Errors
///
/// Symlink/non-regular targets fail with `OUTPUT_PATH_SYMLINK`; a different
/// existing file fails with `CANDIDATE_OUTPUT_CONFLICT`; all other I/O or
/// readback failures return `CANDIDATE_OUTPUT_WRITE_FAILED`.
pub fn write_exact_idempotent(
    root: &Path,
    target: &Path,
    data: &[u8],
) -> Result<(), CandidateOutputError> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        CandidateOutputError::new(
            "CANDIDATE_OUTPUT_WRITE_FAILED",
            format!("unable to canonicalize repository root: {error}"),
        )
    })?;
    let target = if target.is_absolute() {
        target.to_owned()
    } else {
        root.join(target)
    };
    if !target.starts_with(&root) {
        return Err(CandidateOutputError::new(
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
            "candidate target escapes repository root",
        ));
    }
    let parent = target.parent().ok_or_else(|| {
        CandidateOutputError::new(
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
            "candidate target has no parent",
        )
    })?;
    ensure_parent_without_symlink(&root, parent)?;

    match fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(CandidateOutputError::new(
                    "OUTPUT_PATH_SYMLINK",
                    format!(
                        "output target is not a regular file: {}",
                        target.display()
                    ),
                ));
            }
            let existing = fs::read(&target).map_err(|error| {
                CandidateOutputError::new(
                    "CANDIDATE_OUTPUT_WRITE_FAILED",
                    format!(
                        "unable to read candidate output: {}: {error}",
                        target.display()
                    ),
                )
            })?;
            if existing == data {
                return Ok(());
            }
            return Err(CandidateOutputError::new(
                "CANDIDATE_OUTPUT_CONFLICT",
                format!("existing candidate output differs: {}", target.display()),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(CandidateOutputError::new(
                "CANDIDATE_OUTPUT_WRITE_FAILED",
                format!(
                    "unable to inspect candidate output: {}: {error}",
                    target.display()
                ),
            ));
        }
    }

    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            CandidateOutputError::new(
                "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
                "candidate target file name is not UTF-8",
            )
        })?;
    let temporary = parent.join(format!(".{name}.tmp-{}", std::process::id()));
    match fs::symlink_metadata(&temporary) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(CandidateOutputError::new(
                    "OUTPUT_PATH_SYMLINK",
                    format!(
                        "temporary output is not a regular file: {}",
                        temporary.display()
                    ),
                ));
            }
            fs::remove_file(&temporary).map_err(|error| {
                CandidateOutputError::new(
                    "CANDIDATE_OUTPUT_WRITE_FAILED",
                    format!("unable to remove stale temporary output: {error}"),
                )
            })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(CandidateOutputError::new(
                "CANDIDATE_OUTPUT_WRITE_FAILED",
                format!("unable to inspect temporary output: {error}"),
            ));
        }
    }

    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &target)?;
        sync_directory(parent);
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(CandidateOutputError::new(
            "CANDIDATE_OUTPUT_WRITE_FAILED",
            format!(
                "unable to write candidate output: {}: {error}",
                target.display()
            ),
        ));
    }

    let readback = fs::read(&target).map_err(|error| {
        CandidateOutputError::new(
            "CANDIDATE_OUTPUT_WRITE_FAILED",
            format!(
                "unable to read candidate output: {}: {error}",
                target.display()
            ),
        )
    })?;
    if readback != data {
        return Err(CandidateOutputError::new(
            "CANDIDATE_OUTPUT_WRITE_FAILED",
            format!("candidate output readback differs: {}", target.display()),
        ));
    }
    Ok(())
}

fn reject_existing_non_directories(
    root: &Path,
    target: &Path,
) -> Result<(), CandidateOutputError> {
    let relative = target.strip_prefix(root).map_err(|_| {
        CandidateOutputError::new(
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
            "candidate output root escapes repository root",
        )
    })?;
    let mut cursor = root.to_owned();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(CandidateOutputError::new(
                "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
                "candidate output contains a non-normal component",
            ));
        };
        cursor.push(part);
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(CandidateOutputError::new(
                        "OUTPUT_PATH_SYMLINK",
                        format!(
                            "output path component is not a regular directory: {}",
                            cursor.display()
                        ),
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(CandidateOutputError::new(
                    "CANDIDATE_OUTPUT_WRITE_FAILED",
                    format!(
                        "unable to inspect output path: {}: {error}",
                        cursor.display()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn ensure_parent_without_symlink(
    root: &Path,
    parent: &Path,
) -> Result<(), CandidateOutputError> {
    let relative = parent.strip_prefix(root).map_err(|_| {
        CandidateOutputError::new(
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
            "candidate output parent escapes repository root",
        )
    })?;
    let mut cursor = root.to_owned();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(CandidateOutputError::new(
                "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
                "candidate output parent contains a non-normal component",
            ));
        };
        cursor.push(part);
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(CandidateOutputError::new(
                        "OUTPUT_PATH_SYMLINK",
                        format!(
                            "output parent is not a regular directory: {}",
                            cursor.display()
                        ),
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&cursor).map_err(|error| {
                    CandidateOutputError::new(
                        "CANDIDATE_OUTPUT_WRITE_FAILED",
                        format!(
                            "unable to create output directory: {}: {error}",
                            cursor.display()
                        ),
                    )
                })?;
            }
            Err(error) => {
                return Err(CandidateOutputError::new(
                    "CANDIDATE_OUTPUT_WRITE_FAILED",
                    format!(
                        "unable to inspect output parent: {}: {error}",
                        cursor.display()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn simple_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
        })
}

#[cfg(unix)]
fn sync_directory(path: &Path) {
    let _ = fs::File::open(path).and_then(|file| file.sync_all());
}

#[cfg(not(unix))]
const fn sync_directory(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "eliot-context-output-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("scratch root");
        root
    }

    #[test]
    fn identical_replay_is_idempotent_and_difference_conflicts() {
        let root = scratch();
        let target = root.join("artifacts/context-artifact-candidates/p/demo.json");
        write_exact_idempotent(&root, &target, b"one\n").expect("first write");
        write_exact_idempotent(&root, &target, b"one\n").expect("identical replay");
        let error = write_exact_idempotent(&root, &target, b"two\n")
            .expect_err("conflict");
        assert_eq!(error.reason(), "CANDIDATE_OUTPUT_CONFLICT");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn output_root_is_closed_to_candidate_tree() {
        let root = scratch();
        assert!(validate_output_root(&root, "artifacts/context-artifact-candidates").is_ok());
        let error = validate_output_root(&root, "artifacts/elsewhere").expect_err("outside");
        assert_eq!(error.reason(), "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT");
        let _ = fs::remove_dir_all(root);
    }
}
