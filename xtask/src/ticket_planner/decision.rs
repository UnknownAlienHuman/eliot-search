//! Ticket selection and terminal decision classification.

use super::grammar::actor_identity_valid;
use super::spec::{
    CONFLICT_REASONS, DECISION_CONFLICT, DECISION_INVALID, DECISION_MISSING,
    DECISION_PREREQUISITE, DECISION_READY, INVALID_REASONS,
    PREREQUISITE_REASONS,
};

/// Invalid beats conflict beats prerequisite; otherwise selection state
/// decides between missing, conflict and ready.
#[must_use]
pub fn choose_decision(state: &str, reasons: &[&str]) -> &'static str {
    if reasons.iter().any(|reason| INVALID_REASONS.contains(reason)) {
        return DECISION_INVALID;
    }
    if reasons.iter().any(|reason| CONFLICT_REASONS.contains(reason)) {
        return DECISION_CONFLICT;
    }
    if reasons
        .iter()
        .any(|reason| PREREQUISITE_REASONS.contains(reason))
    {
        return DECISION_PREREQUISITE;
    }
    if state == "NONE" {
        return DECISION_MISSING;
    }
    if state != "COMPLETE" {
        return DECISION_CONFLICT;
    }
    DECISION_READY
}

/// Pure selection classification for base/writer/reviewer input.
#[must_use]
pub fn selection_state(
    base: Option<&str>,
    writer: Option<&str>,
    reviewer: Option<&str>,
) -> (&'static str, Vec<&'static str>) {
    let count = [base, writer, reviewer]
        .iter()
        .filter(|value| value.is_some())
        .count();
    if count == 0 {
        return ("NONE", Vec::new());
    }
    if count != 3 {
        return ("PARTIAL", vec!["PARTIAL_ISSUANCE_SELECTION"]);
    }
    let (writer, reviewer) = (writer.unwrap_or(""), reviewer.unwrap_or(""));
    let mut reasons = Vec::new();
    if !actor_identity_valid(writer) || !actor_identity_valid(reviewer) {
        reasons.push("ACTOR_IDENTITY_INVALID");
    }
    if writer == reviewer {
        reasons.push("WRITER_REVIEWER_CONFLICT");
    }
    ("COMPLETE", reasons)
}
