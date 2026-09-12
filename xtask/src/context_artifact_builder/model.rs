//! Context-artifact builder state and closed result types.

use serde_json::{Value, json};
use toml::Value as TomlValue;

use crate::git_tree::GitTree;

/// One deterministic preflight check embedded in the advisory candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateCheck {
    /// Stable check identity.
    pub id: String,
    /// `PASS` or `FAIL`.
    pub status: &'static str,
    /// Empty on success; one closed machine reason on failure.
    pub reason_code: Option<String>,
    /// Content-free diagnostic detail.
    pub detail: String,
}

impl CandidateCheck {
    /// Converts the check to the candidate JSON shape.
    #[must_use]
    pub fn as_json(&self) -> Value {
        json!({
            "id": self.id,
            "status": self.status,
            "reason_code": self.reason_code,
            "detail": self.detail,
        })
    }
}

/// Closed builder failure with the checks completed before the failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextArtifactBuildError {
    reason: String,
    message: String,
    checks: Vec<CandidateCheck>,
}

impl ContextArtifactBuildError {
    /// Creates a failure without completed checks.
    #[must_use]
    pub fn new(reason: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            message: message.into(),
            checks: Vec::new(),
        }
    }

    pub(super) fn with_checks(
        reason: impl Into<String>,
        message: impl Into<String>,
        mut checks: Vec<CandidateCheck>,
    ) -> Self {
        let reason = reason.into();
        let message = message.into();
        checks.push(CandidateCheck {
            id: format!("failed:{}", checks.len()),
            status: "FAIL",
            reason_code: Some(reason.clone()),
            detail: message.clone(),
        });
        Self {
            reason,
            message,
            checks,
        }
    }

    /// Stable machine-readable failure code.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Content-free diagnostic detail.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Checks completed before failure.
    #[must_use]
    pub fn checks(&self) -> &[CandidateCheck] {
        &self.checks
    }
}

impl std::fmt::Display for ContextArtifactBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.reason, self.message)
    }
}

impl std::error::Error for ContextArtifactBuildError {}

/// Exact ticket/context draft pair loaded from one immutable Git tree.
#[derive(Clone, Debug)]
pub(super) struct DraftPair {
    pub(super) ticket_path: String,
    pub(super) context_path: String,
    pub(super) ticket: TomlValue,
    pub(super) context: TomlValue,
    pub(super) sources: Vec<String>,
    pub(super) selectors: Vec<String>,
    pub(super) handoff_slots: Vec<String>,
    pub(super) unavailable_checks: Vec<String>,
    pub(super) source_ceiling_class: String,
    pub(super) context_blob: String,
    pub(super) context_sha256: String,
}

/// Accepted handoff summary plus exact committed bytes.
#[derive(Clone, Debug)]
pub(super) struct AcceptedHandoff {
    pub(super) summary: Value,
    pub(super) bytes: Vec<u8>,
}

/// Successful immutable-tree preflight.
#[derive(Clone, Debug)]
pub(super) struct Preflight {
    pub(super) tree: GitTree,
    pub(super) pair: DraftPair,
    pub(super) package_path: String,
    pub(super) handoffs: Vec<AcceptedHandoff>,
    pub(super) checks: Vec<CandidateCheck>,
}

/// Fully assembled ordinary candidate files before or after local publication.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateBuild {
    candidate: Value,
    candidate_bytes: Vec<u8>,
    bundle_bytes: Vec<u8>,
    candidate_relative_path: String,
    bundle_relative_path: String,
}

impl CandidateBuild {
    pub(super) fn new(
        candidate: Value,
        candidate_bytes: Vec<u8>,
        bundle_bytes: Vec<u8>,
        candidate_relative_path: String,
        bundle_relative_path: String,
    ) -> Self {
        Self {
            candidate,
            candidate_bytes,
            bundle_bytes,
            candidate_relative_path,
            bundle_relative_path,
        }
    }

    /// Candidate metadata value.
    #[must_use]
    pub fn candidate(&self) -> &Value {
        &self.candidate
    }

    /// Exact canonical metadata bytes.
    #[must_use]
    pub fn candidate_bytes(&self) -> &[u8] {
        &self.candidate_bytes
    }

    /// Exact context bundle bytes.
    #[must_use]
    pub fn bundle_bytes(&self) -> &[u8] {
        &self.bundle_bytes
    }

    /// Repository-relative candidate metadata path.
    #[must_use]
    pub fn candidate_relative_path(&self) -> &str {
        &self.candidate_relative_path
    }

    /// Repository-relative context bundle path.
    #[must_use]
    pub fn bundle_relative_path(&self) -> &str {
        &self.bundle_relative_path
    }
}
