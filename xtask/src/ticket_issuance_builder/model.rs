//! Ticket-issuance planner state and closed result types.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toml::Value as TomlValue;

use crate::git_tree::GitTree;
use crate::ticket_planner::CLOSED_REASON_CODES;

/// Immutable planner selection and output options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketIssuanceBuildOptions {
    /// Exact P00 package name.
    pub package: String,
    /// Optional exact algorithm-tagged immutable base commit.
    pub base_commit: Option<String>,
    /// Optional selected writer identity.
    pub writer: Option<String>,
    /// Optional selected independent reviewer identity.
    pub reviewer: Option<String>,
    /// Explicit committed accepted-handoff paths.
    pub accepted_handoffs: Vec<String>,
    /// `-` for stdout or repository-relative JSON below the plan artifact root.
    pub output: String,
}

impl TicketIssuanceBuildOptions {
    /// Creates a zero-selection advisory build for one package.
    #[must_use]
    pub fn new(package: impl Into<String>) -> Self {
        Self {
            package: package.into(),
            base_commit: None,
            writer: None,
            reviewer: None,
            accepted_handoffs: Vec::new(),
            output: "-".to_owned(),
        }
    }
}

/// One deterministic planner check embedded in the advisory plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannerCheck {
    /// Stable check identity.
    pub id: String,
    /// `PASS` or `FAIL`.
    pub status: &'static str,
    /// Closed reason code on failure.
    pub reason_code: Option<String>,
    /// Content-free diagnostic detail.
    pub detail: String,
}

impl PlannerCheck {
    /// Converts the check to the canonical plan JSON shape.
    #[must_use]
    pub fn as_json(&self) -> Value {
        let mut value = json!({
            "id": self.id,
            "status": self.status,
            "detail": self.detail,
        });
        if let Some(reason) = &self.reason_code {
            value
                .as_object_mut()
                .expect("planner check JSON is an object")
                .insert("reason_code".to_owned(), Value::String(reason.clone()));
        }
        value
    }
}

/// Fatal planner failure that prevents even a truthful advisory plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketIssuanceBuildError {
    reason: String,
    message: String,
}

impl TicketIssuanceBuildError {
    pub(super) fn new(
        reason: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            reason: reason.into(),
            message: message.into(),
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
}

impl std::fmt::Display for TicketIssuanceBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.reason, self.message)
    }
}

impl std::error::Error for TicketIssuanceBuildError {}

/// Fully assembled non-authoritative plan and optional local output target.
#[derive(Clone, Debug, PartialEq)]
pub struct TicketIssuanceBuild {
    root: PathBuf,
    plan: Value,
    plan_bytes: Vec<u8>,
    output_target: Option<PathBuf>,
}

impl TicketIssuanceBuild {
    pub(super) fn new(
        root: PathBuf,
        plan: Value,
        plan_bytes: Vec<u8>,
        output_target: Option<PathBuf>,
    ) -> Self {
        Self {
            root,
            plan,
            plan_bytes,
            output_target,
        }
    }

    /// Canonical repository root used only for output fencing.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Advisory plan JSON value.
    #[must_use]
    pub fn plan(&self) -> &Value {
        &self.plan
    }

    /// Exact canonical plan bytes with one trailing LF.
    #[must_use]
    pub fn plan_bytes(&self) -> &[u8] {
        &self.plan_bytes
    }

    /// Validated ordinary local output target, or `None` for stdout/invalid output.
    #[must_use]
    pub fn output_target(&self) -> Option<&Path> {
        self.output_target.as_deref()
    }
}

pub(super) struct Checks {
    items: Vec<PlannerCheck>,
    reasons: Vec<String>,
}

impl Checks {
    pub(super) fn new() -> Self {
        Self {
            items: Vec::new(),
            reasons: Vec::new(),
        }
    }

    pub(super) fn pass(&mut self, id: impl Into<String>, detail: impl Into<String>) {
        self.items.push(PlannerCheck {
            id: id.into(),
            status: "PASS",
            reason_code: None,
            detail: detail.into(),
        });
    }

    pub(super) fn fail(
        &mut self,
        id: impl Into<String>,
        reason: &str,
        detail: impl Into<String>,
    ) {
        assert!(
            CLOSED_REASON_CODES.contains(&reason),
            "unregistered ticket-planner reason: {reason}"
        );
        self.items.push(PlannerCheck {
            id: id.into(),
            status: "FAIL",
            reason_code: Some(reason.to_owned()),
            detail: detail.into(),
        });
        if !self.reasons.iter().any(|existing| existing == reason) {
            self.reasons.push(reason.to_owned());
        }
    }

    pub(super) fn reasons(&self) -> &[String] {
        &self.reasons
    }

    pub(super) fn checks_json(&self) -> Vec<Value> {
        self.items.iter().map(PlannerCheck::as_json).collect()
    }
}

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
    pub(super) ticket_blob: String,
    pub(super) ticket_sha256: String,
    pub(super) context_blob: String,
    pub(super) context_sha256: String,
}

#[derive(Clone, Debug)]
pub(super) struct RegistrySnapshot {
    pub(super) launch: TomlValue,
    pub(super) package_row: Option<TomlValue>,
    pub(super) function_row: Option<TomlValue>,
    pub(super) stage_row: Option<TomlValue>,
}

#[derive(Clone, Debug)]
pub(super) struct PlannerView {
    pub(super) tree: GitTree,
    pub(super) selection_state: &'static str,
}
