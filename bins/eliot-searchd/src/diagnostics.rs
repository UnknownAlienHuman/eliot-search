//! Closed, bounded, redacted diagnostics for the DIRECT service.
//!
//! Every diagnostic carries a closed [`DiagnosticCode`] plus a bounded detail
//! class (ASCII upper-case/digit plus `_`/`-`/`.` only, at most
//! [`MAX_CODE_BYTES`] bytes). Raw OS text, paths, source bytes, queries,
//! secrets and tokens never enter a diagnostic: [`sanitize_code`] reduces any
//! `CODE:free-form` string to its closed head. Partial and degraded outcomes
//! stay typed ([`Outcome`], invariant 15) and are never relabelled success.
//! [`TotalWorkBudget`] enforces caller-visible total-work/response ceilings
//! with backpressure instead of per-file-only limits.

/// Maximum bytes of one closed code or detail class.
pub const MAX_CODE_BYTES: usize = 128;
/// Maximum diagnostics carried by one response event (mirrors the
/// `provider_status` blocker ceiling in `service_output`).
pub const MAX_DIAGNOSTICS_PER_EVENT: usize = 8;
/// Maximum bytes of one serialized diagnostic object.
pub const MAX_DIAGNOSTIC_JSON_BYTES: usize = 1024;

/// Reduces any `CODE:free-form` error string to its closed head.
///
/// Only ASCII upper-case, digits, `_`, `-` and `.` survive (anything else
/// becomes `_`), truncated to [`MAX_CODE_BYTES`] bytes. The suffix after the
/// first `:` — where paths, OS text, secrets or query bytes would hide — is
/// always dropped. An empty result maps to `DIAGNOSTIC_ERROR`.
#[must_use]
pub fn sanitize_code(error: &str) -> String {
    let code = error.split(':').next().unwrap_or("DIAGNOSTIC_ERROR");
    let mut output = String::with_capacity(code.len().min(MAX_CODE_BYTES));
    for character in code.chars().take(MAX_CODE_BYTES) {
        if character.is_ascii_uppercase()
            || character.is_ascii_digit()
            || matches!(character, '_' | '-' | '.')
        {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "DIAGNOSTIC_ERROR".to_owned()
    } else {
        output
    }
}

/// Closed diagnostic reason codes for daemon product paths.
///
/// No code carries payload: the detail class names the failure family, never
/// the path, byte range, query, secret or token involved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
    /// Admission or scan input was denied before content processing.
    IndexDenied,
    /// Search completed with explicit gaps (`complete=false`).
    SearchDegraded,
    /// Currentness could not be proven across an observation gap.
    CurrentnessStale,
    /// Index loss requires a manifest replay before cutover.
    RebuildRequired,
    /// Work was cancelled at a bounded boundary with partial state typed.
    Cancelled,
    /// A declared total-work/response/queue ceiling stopped the batch.
    BudgetExceeded,
    /// Backpressure shed load before any ceiling was breached.
    BackpressureApplied,
    /// An honest measurement or probe could not be produced.
    Unavailable,
}

impl DiagnosticCode {
    /// Stable machine-readable code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::IndexDenied => "DIAGNOSTIC_INDEX_DENIED",
            Self::SearchDegraded => "DIAGNOSTIC_SEARCH_DEGRADED",
            Self::CurrentnessStale => "DIAGNOSTIC_CURRENTNESS_STALE",
            Self::RebuildRequired => "DIAGNOSTIC_REBUILD_REQUIRED",
            Self::Cancelled => "DIAGNOSTIC_CANCELLED",
            Self::BudgetExceeded => "DIAGNOSTIC_BUDGET_EXCEEDED",
            Self::BackpressureApplied => "DIAGNOSTIC_BACKPRESSURE_APPLIED",
            Self::Unavailable => "DIAGNOSTIC_UNAVAILABLE",
        }
    }
}

/// One bounded redacted diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    code: DiagnosticCode,
    detail_class: String,
    truncated: bool,
}

impl Diagnostic {
    /// Builds a diagnostic with a sanitized detail class.
    ///
    /// `detail_class` is reduced through [`sanitize_code`]; `truncated` is
    /// set when input was dropped, so redaction is observable, never silent.
    #[must_use]
    pub fn new(code: DiagnosticCode, detail_class: &str) -> Self {
        let clean = sanitize_code(detail_class);
        let truncated =
            detail_class.len() > MAX_CODE_BYTES || clean_contains_replacement(&clean, detail_class);
        Self {
            code,
            detail_class: clean,
            truncated,
        }
    }

    /// Closed reason code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Sanitized detail class (closed alphabet, bounded).
    #[must_use]
    pub fn detail_class(&self) -> &str {
        &self.detail_class
    }

    /// Whether redaction dropped input bytes.
    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// Bounded JSON object; always fits [`MAX_DIAGNOSTIC_JSON_BYTES`].
    #[must_use]
    pub fn json(&self) -> String {
        let mut output = format!(
            "{{\"code\":\"{}\",\"detail_class\":\"{}\",\"truncated\":{}}}",
            self.code.code(),
            self.detail_class,
            self.truncated,
        );
        if output.len() > MAX_DIAGNOSTIC_JSON_BYTES {
            output.truncate(MAX_DIAGNOSTIC_JSON_BYTES);
        }
        output
    }
}

/// Reports whether the sanitized output replaced any input character.
fn clean_contains_replacement(clean: &str, original: &str) -> bool {
    if clean.len() != original.len() {
        return true;
    }
    clean
        .chars()
        .zip(original.chars())
        .any(|(left, right)| left != right)
}

/// Scans captured output for leaked canary material.
///
/// Returns `true` when any non-empty canary appears verbatim in `haystack`.
/// The caller owns canary secrecy: canaries in the T40 process test are
/// synthetic sentinels, never customer bytes.
#[must_use]
pub fn contains_sentinel(haystack: &str, canaries: &[&str]) -> bool {
    canaries
        .iter()
        .any(|canary| !canary.is_empty() && haystack.contains(canary))
}

/// Caller-visible total-work budget with backpressure.
///
/// Per-file limits alone cannot bound a corpus batch: this ledger caps the
/// whole batch in bytes and wall milliseconds. [`Self::consume`] fails
/// closed with `TOTAL_WORK_BUDGET_EXCEEDED` past either ceiling;
/// [`Self::backpressure`] turns true at 80% so callers shed load before the
/// hard stop. Counters saturate instead of wrapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TotalWorkBudget {
    ceiling_bytes: u64,
    ceiling_ms: u128,
    used_bytes: u64,
    used_ms: u128,
}

impl TotalWorkBudget {
    /// Declares the total ceilings for one batch. Zero ceilings are rejected
    /// at consumption time (every `consume` fails) rather than panicking.
    #[must_use]
    pub const fn new(ceiling_bytes: u64, ceiling_ms: u128) -> Self {
        Self {
            ceiling_bytes,
            ceiling_ms,
            used_bytes: 0,
            used_ms: 0,
        }
    }

    /// Charges one completed unit of work.
    ///
    /// # Errors
    ///
    /// Returns `TOTAL_WORK_BUDGET_EXCEEDED` without recording the charge when
    /// either ceiling would be breached.
    pub const fn consume(&mut self, bytes: u64, elapsed_ms: u128) -> Result<(), &'static str> {
        let next_bytes = self.used_bytes.saturating_add(bytes);
        let next_ms = self.used_ms.saturating_add(elapsed_ms);
        if self.ceiling_bytes == 0
            || self.ceiling_ms == 0
            || next_bytes > self.ceiling_bytes
            || next_ms > self.ceiling_ms
        {
            return Err("TOTAL_WORK_BUDGET_EXCEEDED");
        }
        self.used_bytes = next_bytes;
        self.used_ms = next_ms;
        Ok(())
    }

    /// Whether either ledger reached 80% of its ceiling.
    #[must_use]
    pub const fn backpressure(&self) -> bool {
        self.used_bytes >= self.ceiling_bytes.saturating_mul(4) / 5
            || self.used_ms >= self.ceiling_ms.saturating_mul(4) / 5
    }

    /// Charged bytes so far.
    #[must_use]
    pub const fn used_bytes(&self) -> u64 {
        self.used_bytes
    }

    /// Charged wall milliseconds so far.
    #[must_use]
    pub const fn used_ms(&self) -> u128 {
        self.used_ms
    }
}

/// Typed terminal outcome for one unit of daemon work (invariant 15).
///
/// Partial, degraded, cancelled and unavailable outcomes are data, never
/// success: only [`Outcome::Success`] reports [`Outcome::complete`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// All proof obligations held; safe to label success.
    Success,
    /// Finished with explicit gaps; `complete=false` downstream.
    Partial,
    /// Finished with reduced fidelity; never relabelled success.
    Degraded,
    /// Stopped at a bounded boundary; partial state stays typed.
    Cancelled,
    /// No honest observation could be produced.
    Unavailable,
}

impl Outcome {
    /// Whether this outcome may be reported as success.
    #[must_use]
    pub const fn complete(self) -> bool {
        matches!(self, Self::Success)
    }

    /// Stable machine-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::Partial => "PARTIAL",
            Self::Degraded => "DEGRADED",
            Self::Cancelled => "CANCELLED",
            Self::Unavailable => "UNAVAILABLE",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_drops_suffix_paths_secrets_and_raw_os_text() {
        assert_eq!(
            sanitize_code("SERVICE_HEX_INVALID:private detail"),
            "SERVICE_HEX_INVALID"
        );
        assert_eq!(
            sanitize_code(r"DATA_ROOT_OPEN_ERROR:C:\secret\root path (os error 3)"),
            "DATA_ROOT_OPEN_ERROR"
        );
        assert_eq!(sanitize_code("bearer-token-abc123"), "______-_____-___123");
        assert_eq!(sanitize_code(""), "DIAGNOSTIC_ERROR");
        assert_eq!(sanitize_code("::: x"), "DIAGNOSTIC_ERROR");
    }

    #[test]
    fn sanitize_is_bounded_to_128_ascii_upper_tokens() {
        let long = "A".repeat(500);
        assert_eq!(sanitize_code(&long).len(), MAX_CODE_BYTES);
        assert_eq!(sanitize_code("ok-minus.Ok_09"), "__-_____.O__09");
    }

    #[test]
    fn diagnostic_redaction_is_observable_never_silent() {
        let clean = Diagnostic::new(DiagnosticCode::IndexDenied, "SOURCE_ADMISSION_DENIED");
        assert!(!clean.truncated());
        assert_eq!(clean.detail_class(), "SOURCE_ADMISSION_DENIED");
        assert!(clean.json().len() <= MAX_DIAGNOSTIC_JSON_BYTES);

        let dirty = Diagnostic::new(
            DiagnosticCode::SearchDegraded,
            "SOURCE_GAP:C:\\temp\\corpus secret-bytes",
        );
        assert!(dirty.truncated());
        assert!(!dirty.detail_class().contains('\\'));
        assert!(!dirty.detail_class().contains(' '));
        assert!(!dirty.json().contains("secret"));
    }

    #[test]
    fn sentinel_scan_finds_exact_canaries_only() {
        let hay = "{\"event\":\"match\",\"evidence_id\":\"abc\"}";
        assert!(!contains_sentinel(hay, &["T40-SECRET", ""]));
        assert!(contains_sentinel(
            "leaked T40-SECRET-SENTINEL bytes",
            &["T40-SECRET-SENTINEL"]
        ));
    }

    #[test]
    fn total_work_budget_fails_closed_with_backpressure() {
        let mut budget = TotalWorkBudget::new(100, 100);
        assert!(!budget.backpressure());
        budget.consume(79, 10).expect("under ceiling");
        assert!(!budget.backpressure());
        budget.consume(1, 0).expect("at 80%");
        assert!(budget.backpressure());
        assert_eq!(budget.consume(21, 0), Err("TOTAL_WORK_BUDGET_EXCEEDED"));
        assert_eq!(budget.used_bytes(), 80);
        assert_eq!(
            TotalWorkBudget::new(0, 100).consume(0, 0),
            Err("TOTAL_WORK_BUDGET_EXCEEDED")
        );
    }

    #[test]
    fn partial_degraded_cancelled_unavailable_are_never_success() {
        assert!(Outcome::Success.complete());
        for outcome in [
            Outcome::Partial,
            Outcome::Degraded,
            Outcome::Cancelled,
            Outcome::Unavailable,
        ] {
            assert!(!outcome.complete(), "{outcome:?}");
            assert_ne!(outcome.label(), "SUCCESS");
        }
    }

    #[test]
    fn diagnostic_codes_are_closed_and_stable() {
        let codes = [
            DiagnosticCode::IndexDenied,
            DiagnosticCode::SearchDegraded,
            DiagnosticCode::CurrentnessStale,
            DiagnosticCode::RebuildRequired,
            DiagnosticCode::Cancelled,
            DiagnosticCode::BudgetExceeded,
            DiagnosticCode::BackpressureApplied,
            DiagnosticCode::Unavailable,
        ];
        for code in codes {
            let text = code.code();
            assert_eq!(sanitize_code(text), text, "{text}");
        }
    }
}
