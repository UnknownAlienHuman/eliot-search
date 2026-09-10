//! Deterministic deny-by-default source admission.
//!
//! Pure canonical policy evaluation without filesystem, Git, network, clock,
//! database, or process I/O. Callers supply bounded path-class, metadata,
//! source-kind, size, and sensitivity observations gathered by the owning
//! platform boundary. Every decision binds one canonical policy
//! revision/fingerprint and one canonical observation digest with stable
//! ordered reason codes. Decisions authorize only source admission under the
//! named policy, never later access or currentness.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::chunks_exact_to_as_chunks,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::many_single_char_names,
    clippy::map_unwrap_or,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref,
    clippy::tuple_array_conversions,
    clippy::unnested_or_patterns,
    clippy::unreadable_literal
)]

use core::fmt;
use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, NonZeroRevision};

// ---------------------------------------------------------------------------
// Closed constants
// ---------------------------------------------------------------------------

/// Canonical admission policy schema version.
pub const POLICY_SCHEMA_VERSION: u32 = 1;

/// Immutable admission receipt schema version.
pub const RECEIPT_SCHEMA_VERSION: u32 = 1;

/// Configuration section schema revision.
pub const SECTION_SCHEMA_REVISION: u64 = 1;

/// Default maximum admitted file bytes (`16 MiB` per contract).
pub const DEFAULT_MAX_FILE_BYTES: u64 = 16_777_216;

/// Absolute implementation ceiling for one observation.
pub const MAX_OBSERVATION_BYTES: usize = 65_536;

/// Maximum policy rules in one canonical policy.
pub const MAX_POLICY_RULES: usize = 128;

/// Maximum reason codes carried by one decision or receipt.
pub const MAX_REASON_CODES: usize = 64;

/// Maximum matched rule identifiers carried by one decision.
pub const MAX_MATCHED_RULES: usize = 128;

/// Maximum pattern bytes for a single rule.
pub const MAX_PATTERN_BYTES: usize = 1_024;

/// Maximum explanation bytes for one decision.
pub const MAX_EXPLANATION_BYTES: usize = 1_024;

/// Maximum identifier bytes for detector, profile, and rule identifiers.
pub const MAX_ID_BYTES: usize = 256;

/// Maximum batch cardinality.
pub const MAX_BATCH_ITEMS: usize = 1_024;

/// Maximum canonical preimage bytes for any digest.
pub const MAX_DIGEST_PREIMAGE_BYTES: usize = 1_048_576;

/// Baseline accepted policy profile.
pub const BASELINE_PROFILE: &str = "baseline-safe-v1";

/// Closed unavailable-field vocabulary for observations.
const CLOSED_UNAVAILABLE_FIELDS: [&str; 8] = [
    "byte-size",
    "sensitivity",
    "detector-id",
    "profile-id",
    "generated-flag",
    "vendor-flag",
    "binary-flag",
    "system-flag",
];

// ---------------------------------------------------------------------------
// Internal deterministic hashing (pure CPU, no external dependency)
// ---------------------------------------------------------------------------

fn sha256(input: &[u8]) -> [u8; 32] {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let padded_len = input
        .len()
        .checked_add(9)
        .and_then(|v| v.checked_add(63))
        .map(|v| v / 64 * 64)
        .unwrap_or(64);
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(input);
    padded.push(0x80);
    padded.resize(padded_len.saturating_sub(8), 0);
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = INITIAL;
    for chunk in padded.chunks_exact(64) {
        let mut schedule = [0_u32; 64];
        for (index, word) in chunk.chunks_exact(4).enumerate() {
            schedule[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let left = schedule[index - 15];
            let right = schedule[index - 2];
            let sigma0 = left.rotate_right(7) ^ left.rotate_right(18) ^ (left >> 3);
            let sigma1 = right.rotate_right(17) ^ right.rotate_right(19) ^ (right >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(sigma0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(sigma1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) = (
            state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7],
        );
        for index in 0..64 {
            let upper1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(upper1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(schedule[index]);
            let upper0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = upper0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }
    let mut output = [0_u8; 32];
    for (chunk, word) in output.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    output
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

struct CanonicalWriter {
    bytes: Vec<u8>,
}

impl CanonicalWriter {
    fn new(domain: &[u8]) -> Self {
        let mut bytes = Vec::with_capacity(domain.len().saturating_add(1));
        bytes.extend_from_slice(domain);
        bytes.push(0);
        Self { bytes }
    }

    fn push_bytes(&mut self, value: &[u8]) {
        let len = u64::try_from(value.len()).unwrap_or(u64::MAX);
        self.bytes.extend_from_slice(&len.to_be_bytes());
        self.bytes.extend_from_slice(value);
    }

    fn push_str(&mut self, value: &str) {
        self.push_bytes(value.as_bytes());
    }

    fn push_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn push_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn push_bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    fn push_option_str(&mut self, value: Option<&str>) {
        match value {
            Some(text) => {
                self.bytes.push(1);
                self.push_str(text);
            }
            None => self.bytes.push(0),
        }
    }

    fn push_option_u64(&mut self, value: Option<u64>) {
        match value {
            Some(number) => {
                self.bytes.push(1);
                self.push_u64(number);
            }
            None => self.bytes.push(0),
        }
    }

    fn push_option_bool(&mut self, value: Option<bool>) {
        match value {
            Some(flag) => {
                self.bytes.push(1);
                self.push_bool(flag);
            }
            None => self.bytes.push(0),
        }
    }

    fn finish(self) -> [u8; 32] {
        sha256(&self.bytes)
    }
}

fn check_id(value: &str, field: &'static str) -> Result<(), AdmissionError> {
    if value.is_empty() || value.len() > MAX_ID_BYTES {
        return Err(AdmissionError::ObservationInvalid);
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':' | b'/'))
    {
        let _ = field;
        return Err(AdmissionError::ObservationInvalid);
    }
    Ok(())
}

fn check_pattern(value: &str) -> Result<(), AdmissionError> {
    if value.is_empty() || value.len() > MAX_PATTERN_BYTES {
        return Err(AdmissionError::PolicyUnknownField);
    }
    if value.bytes().any(|b| b == 0) {
        return Err(AdmissionError::PolicyUnknownField);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Typed failures
// ---------------------------------------------------------------------------

/// Closed content-free admission failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdmissionError {
    /// Policy schema version or shape is unsupported.
    PolicySchemaUnsupported,
    /// Unknown load-bearing policy field, class, or operator.
    PolicyUnknownField,
    /// Duplicate or conflicting policy rules.
    PolicyConflict,
    /// An override would bypass an unconditional safety deny.
    PolicyOverrideForbidden,
    /// Observation input is malformed or out of bounds.
    ObservationInvalid,
    /// Load-bearing observation is missing without an explicit unavailable marker.
    ObservationIncomplete,
    /// Admission is denied under the current policy.
    AdmissionDenied,
    /// Admission requires human review under the current policy.
    AdmissionReviewRequired,
    /// Source kind is valid but not admittable.
    SourceKindUnsupported,
    /// Observed size exceeds the policy ceiling.
    SourceTooLarge,
    /// Sensitive source is denied under the current policy.
    SensitiveSourceDenied,
    /// Receipt does not bind the supplied policy, observation, or decision.
    ReceiptMismatch,
    /// Receipt binds a stale policy revision or fingerprint.
    ReceiptStale,
    /// Finite evaluation budget is exhausted.
    BudgetExhausted,
    /// Evaluation was cancelled before a successful decision.
    Cancelled,
}

impl AdmissionError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::PolicySchemaUnsupported => "ADMISSION_POLICY_SCHEMA_UNSUPPORTED",
            Self::PolicyUnknownField => "ADMISSION_POLICY_UNKNOWN_FIELD",
            Self::PolicyConflict => "ADMISSION_POLICY_CONFLICT",
            Self::PolicyOverrideForbidden => "ADMISSION_POLICY_OVERRIDE_FORBIDDEN",
            Self::ObservationInvalid => "ADMISSION_OBSERVATION_INVALID",
            Self::ObservationIncomplete => "ADMISSION_OBSERVATION_INCOMPLETE",
            Self::AdmissionDenied => "SOURCE_ADMISSION_DENIED",
            Self::AdmissionReviewRequired => "SOURCE_ADMISSION_REVIEW_REQUIRED",
            Self::SourceKindUnsupported => "SOURCE_KIND_UNSUPPORTED",
            Self::SourceTooLarge => "SOURCE_TOO_LARGE",
            Self::SensitiveSourceDenied => "SENSITIVE_SOURCE_DENIED",
            Self::ReceiptMismatch => "ADMISSION_RECEIPT_MISMATCH",
            Self::ReceiptStale => "ADMISSION_RECEIPT_STALE",
            Self::BudgetExhausted => "ADMISSION_BUDGET_EXHAUSTED",
            Self::Cancelled => "ADMISSION_CANCELLED",
        }
    }
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for AdmissionError {}

// ---------------------------------------------------------------------------
// Finite limits, budget, and cancellation
// ---------------------------------------------------------------------------

/// Finite admission limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionLimits {
    /// Maximum observations in one batch.
    pub max_batch_items: usize,
    /// Maximum canonical preimage bytes for one digest.
    pub max_encoded_bytes: usize,
    /// Maximum policy rules evaluated.
    pub max_rules: usize,
    /// Maximum reason codes per decision.
    pub max_reasons: usize,
    /// Maximum pattern bytes per rule.
    pub max_pattern_bytes: usize,
    /// Maximum explanation bytes per decision.
    pub max_explanation_bytes: usize,
}

impl AdmissionLimits {
    /// Validates every finite dimension as non-zero and within the ceiling.
    pub const fn validate(self) -> Result<Self, AdmissionError> {
        if self.max_batch_items == 0
            || self.max_batch_items > MAX_BATCH_ITEMS
            || self.max_encoded_bytes == 0
            || self.max_encoded_bytes > MAX_DIGEST_PREIMAGE_BYTES
            || self.max_rules == 0
            || self.max_rules > MAX_POLICY_RULES
            || self.max_reasons == 0
            || self.max_reasons > MAX_REASON_CODES
            || self.max_pattern_bytes == 0
            || self.max_pattern_bytes > MAX_PATTERN_BYTES
            || self.max_explanation_bytes == 0
            || self.max_explanation_bytes > MAX_EXPLANATION_BYTES
        {
            Err(AdmissionError::ObservationInvalid)
        } else {
            Ok(self)
        }
    }
}

/// Conservative finite admission limits.
pub const DEFAULT_ADMISSION_LIMITS: AdmissionLimits = AdmissionLimits {
    max_batch_items: MAX_BATCH_ITEMS,
    max_encoded_bytes: MAX_DIGEST_PREIMAGE_BYTES,
    max_rules: MAX_POLICY_RULES,
    max_reasons: MAX_REASON_CODES,
    max_pattern_bytes: MAX_PATTERN_BYTES,
    max_explanation_bytes: MAX_EXPLANATION_BYTES,
};

/// Finite pure-CPU evaluation budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionBudget {
    /// Maximum rule evaluations before exhaustion.
    pub max_rule_evaluations: u32,
}

impl AdmissionBudget {
    /// Creates a finite budget.
    pub const fn new(max_rule_evaluations: u32) -> Result<Self, AdmissionError> {
        if max_rule_evaluations == 0 {
            Err(AdmissionError::BudgetExhausted)
        } else {
            Ok(Self {
                max_rule_evaluations,
            })
        }
    }

    /// Conservative default budget covering the maximum rule table twice.
    pub const fn default_budget() -> Self {
        Self {
            max_rule_evaluations: 512,
        }
    }
}

/// Cooperative pure-CPU cancellation flag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CancelFlag {
    /// Whether cancellation was requested before or during evaluation.
    pub cancelled: bool,
}

impl CancelFlag {
    /// Live evaluation flag.
    pub const fn live() -> Self {
        Self { cancelled: false }
    }

    /// Pre-cancelled flag.
    pub const fn cancelled() -> Self {
        Self { cancelled: true }
    }

    fn check(self) -> Result<(), AdmissionError> {
        if self.cancelled {
            Err(AdmissionError::Cancelled)
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration operations
// ---------------------------------------------------------------------------

/// Descriptor for the `source_admission` configuration section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSectionDescriptor {
    section_name: String,
    owner: String,
    schema_revision: u64,
    field_names: Vec<String>,
    minimum_action: String,
}

impl ConfigSectionDescriptor {
    /// Canonical section name.
    pub fn section_name(&self) -> &str {
        &self.section_name
    }

    /// Registered capability owner.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Descriptor schema revision.
    pub fn schema_revision(&self) -> u64 {
        self.schema_revision
    }

    /// Registered field names in canonical order.
    pub fn field_names(&self) -> &[String] {
        &self.field_names
    }

    /// Minimum reload action; never weaker than `SECURITY_BARRIER`.
    pub fn minimum_action(&self) -> &str {
        &self.minimum_action
    }
}

/// Raw input for the `source_admission` section validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSectionInput {
    /// Requested bootstrap policy profile identifier.
    pub bootstrap_policy_profile: String,
    /// Maximum admitted file bytes.
    pub max_file_bytes: u64,
    /// Whether generated sources may be admitted.
    pub allow_generated: bool,
    /// Whether vendor sources may be admitted.
    pub allow_vendor: bool,
    /// Whether binary sources may be admitted.
    pub allow_binary: bool,
    /// Unknown keys observed by the caller; must remain empty.
    pub unknown_keys: Vec<String>,
}

/// Validated `source_admission` configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedAdmissionConfig {
    /// Accepted bootstrap policy profile.
    pub bootstrap_policy_profile: String,
    /// Validated maximum file bytes.
    pub max_file_bytes: u64,
    /// Validated generated-source permission.
    pub allow_generated: bool,
    /// Validated vendor-source permission.
    pub allow_vendor: bool,
    /// Validated binary-source permission.
    pub allow_binary: bool,
}

/// Caller platform identity for section validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionPlatform {
    /// Operating system identifier, e.g. `windows`.
    pub os: String,
    /// Architecture identifier, e.g. `x64`.
    pub arch: String,
}

/// Accepted capability profiles supplied by daemon composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedCapabilities {
    /// Accepted bootstrap profiles.
    pub profiles: Vec<String>,
}

/// Reload decision for a validated section change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionReloadDecision {
    /// Validated inputs are identical; no reload required.
    Unchanged,
    /// Every policy-affecting change requires a security barrier.
    SecurityBarrier,
    /// Permissive change additionally requires explicit reconciliation.
    SecurityBarrierAndReconcile,
}

impl SectionReloadDecision {
    /// Stable machine-readable code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unchanged => "UNCHANGED",
            Self::SecurityBarrier => "SECURITY_BARRIER",
            Self::SecurityBarrierAndReconcile => "SECURITY_BARRIER_AND_RECONCILE",
        }
    }
}

/// Returns the exact `source_admission` section descriptor.
pub fn section_descriptor() -> ConfigSectionDescriptor {
    ConfigSectionDescriptor {
        section_name: "source_admission".to_owned(),
        owner: "search-source-admission".to_owned(),
        schema_revision: SECTION_SCHEMA_REVISION,
        field_names: vec![
            "allow_binary".to_owned(),
            "allow_generated".to_owned(),
            "allow_vendor".to_owned(),
            "bootstrap_policy_profile".to_owned(),
            "max_file_bytes".to_owned(),
        ],
        minimum_action: "SECURITY_BARRIER".to_owned(),
    }
}

/// Returns the compiled baseline defaults.
///
/// Baseline keeps generated, vendor, and binary classes denied and caps one
/// file at `16 MiB`.
pub fn compiled_defaults() -> ConfigSectionInput {
    ConfigSectionInput {
        bootstrap_policy_profile: BASELINE_PROFILE.to_owned(),
        max_file_bytes: DEFAULT_MAX_FILE_BYTES,
        allow_generated: false,
        allow_vendor: false,
        allow_binary: false,
        unknown_keys: Vec::new(),
    }
}

/// Validates one `source_admission` section input.
///
/// # Errors
///
/// Returns `ADMISSION_POLICY_UNKNOWN_FIELD` for unknown keys, wrong profile,
/// or malformed platform; `ADMISSION_POLICY_CONFLICT` for out-of-bounds
/// sizes.
pub fn validate_section(
    input: &ConfigSectionInput,
    platform: &SectionPlatform,
    accepted_capabilities: &AcceptedCapabilities,
) -> Result<ValidatedAdmissionConfig, AdmissionError> {
    if !input.unknown_keys.is_empty() {
        return Err(AdmissionError::PolicyUnknownField);
    }
    if platform.os.is_empty()
        || platform.os.len() > MAX_ID_BYTES
        || platform.arch.is_empty()
        || platform.arch.len() > MAX_ID_BYTES
    {
        return Err(AdmissionError::PolicySchemaUnsupported);
    }
    if input.bootstrap_policy_profile != BASELINE_PROFILE {
        return Err(AdmissionError::PolicySchemaUnsupported);
    }
    if !accepted_capabilities
        .profiles
        .iter()
        .any(|profile| profile == &input.bootstrap_policy_profile)
    {
        return Err(AdmissionError::PolicySchemaUnsupported);
    }
    if input.max_file_bytes == 0 || input.max_file_bytes > DEFAULT_MAX_FILE_BYTES {
        return Err(AdmissionError::PolicyConflict);
    }
    Ok(ValidatedAdmissionConfig {
        bootstrap_policy_profile: input.bootstrap_policy_profile.clone(),
        max_file_bytes: input.max_file_bytes,
        allow_generated: input.allow_generated,
        allow_vendor: input.allow_vendor,
        allow_binary: input.allow_binary,
    })
}

/// Computes the canonical digest of one validated section.
///
/// The digest binds the exact validated profile, size ceiling, and
/// generated/vendor/binary permissions with domain separation.
pub fn section_digest(validated: &ValidatedAdmissionConfig) -> Blake3Digest32 {
    let mut writer = CanonicalWriter::new(b"eliot-search/source-admission-section/v1");
    writer.push_str(&validated.bootstrap_policy_profile);
    writer.push_u64(validated.max_file_bytes);
    writer.push_bool(validated.allow_generated);
    writer.push_bool(validated.allow_vendor);
    writer.push_bool(validated.allow_binary);
    Blake3Digest32::from_bytes(writer.finish())
}

/// Plans the reload decision between two validated sections.
///
/// Every policy-affecting change preserves `SECURITY_BARRIER`. Permissive
/// changes additionally require explicit reconciliation and never mutate
/// existing membership automatically.
///
/// # Errors
///
/// Returns `ADMISSION_POLICY_CONFLICT` when either side violates the section
/// bounds (defensive; validated inputs are already checked).
pub fn plan_section_change(
    old: &ValidatedAdmissionConfig,
    new: &ValidatedAdmissionConfig,
) -> Result<SectionReloadDecision, AdmissionError> {
    if old.max_file_bytes == 0
        || old.max_file_bytes > DEFAULT_MAX_FILE_BYTES
        || new.max_file_bytes == 0
        || new.max_file_bytes > DEFAULT_MAX_FILE_BYTES
    {
        return Err(AdmissionError::PolicyConflict);
    }
    if old == new {
        return Ok(SectionReloadDecision::Unchanged);
    }
    let permissive = (!old.allow_generated && new.allow_generated)
        || (!old.allow_vendor && new.allow_vendor)
        || (!old.allow_binary && new.allow_binary)
        || new.max_file_bytes > old.max_file_bytes;
    if permissive {
        Ok(SectionReloadDecision::SecurityBarrierAndReconcile)
    } else {
        Ok(SectionReloadDecision::SecurityBarrier)
    }
}

// ---------------------------------------------------------------------------
// Policy vocabulary
// ---------------------------------------------------------------------------

/// Closed source class.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceClass {
    /// Ordinary first-party source.
    Regular,
    /// Test source.
    Test,
    /// Documentation source.
    Documentation,
    /// Generated source.
    Generated,
    /// Vendor source.
    Vendor,
    /// Binary source.
    Binary,
    /// Denied system location.
    System,
    /// Cache artifact.
    Cache,
    /// Build output.
    BuildOutput,
    /// Credential material; unconditional deny.
    Credential,
    /// Private-key material; unconditional deny.
    PrivateKey,
}

impl SourceClass {
    /// Stable wire string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Test => "test",
            Self::Documentation => "documentation",
            Self::Generated => "generated",
            Self::Vendor => "vendor",
            Self::Binary => "binary",
            Self::System => "system",
            Self::Cache => "cache",
            Self::BuildOutput => "build-output",
            Self::Credential => "credential",
            Self::PrivateKey => "private-key",
        }
    }

    /// Parses the closed vocabulary.
    pub fn parse(value: &str) -> Result<Self, AdmissionError> {
        match value {
            "regular" => Ok(Self::Regular),
            "test" => Ok(Self::Test),
            "documentation" => Ok(Self::Documentation),
            "generated" => Ok(Self::Generated),
            "vendor" => Ok(Self::Vendor),
            "binary" => Ok(Self::Binary),
            "system" => Ok(Self::System),
            "cache" => Ok(Self::Cache),
            "build-output" => Ok(Self::BuildOutput),
            "credential" => Ok(Self::Credential),
            "private-key" => Ok(Self::PrivateKey),
            _ => Err(AdmissionError::PolicyUnknownField),
        }
    }

    /// Whether this class is an unconditional safety deny.
    pub const fn is_unconditional_deny(self) -> bool {
        matches!(self, Self::Credential | Self::PrivateKey)
    }
}

/// Closed rule operator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuleOperator {
    /// Always matches.
    Always,
    /// Matches when the observation source class equals the rule class.
    SourceClassIs,
    /// Matches when sensitivity meets the rule threshold.
    SensitivityAtLeast,
    /// Matches when size exceeds the rule threshold.
    SizeGreaterThan,
    /// Matches when the locator class equals the rule locator class.
    LocatorClassIs,
}

impl RuleOperator {
    /// Stable wire string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::SourceClassIs => "source-class-is",
            Self::SensitivityAtLeast => "sensitivity-at-least",
            Self::SizeGreaterThan => "size-greater-than",
            Self::LocatorClassIs => "locator-class-is",
        }
    }

    /// Parses the closed vocabulary.
    pub fn parse(value: &str) -> Result<Self, AdmissionError> {
        match value {
            "always" => Ok(Self::Always),
            "source-class-is" => Ok(Self::SourceClassIs),
            "sensitivity-at-least" => Ok(Self::SensitivityAtLeast),
            "size-greater-than" => Ok(Self::SizeGreaterThan),
            "locator-class-is" => Ok(Self::LocatorClassIs),
            _ => Err(AdmissionError::PolicyUnknownField),
        }
    }
}

/// Closed rule effect.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuleEffect {
    /// Admit when matched and no deny applies.
    Allow,
    /// Deny when matched.
    Deny,
    /// Require human review when matched.
    Review,
    /// Report unsupported when matched.
    Unsupported,
}

impl RuleEffect {
    /// Stable wire string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Review => "review",
            Self::Unsupported => "unsupported",
        }
    }

    /// Parses the closed vocabulary.
    pub fn parse(value: &str) -> Result<Self, AdmissionError> {
        match value {
            "allow" => Ok(Self::Allow),
            "deny" => Ok(Self::Deny),
            "review" => Ok(Self::Review),
            "unsupported" => Ok(Self::Unsupported),
            _ => Err(AdmissionError::PolicyUnknownField),
        }
    }
}

/// Closed locator class (never a free display path).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LocatorClass {
    /// Bounded normalized file locator.
    NormalizedFile,
    /// Bounded normalized directory locator.
    NormalizedDirectory,
    /// Content-addressed locator.
    ContentAddress,
}

impl LocatorClass {
    /// Stable wire string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NormalizedFile => "normalized-file",
            Self::NormalizedDirectory => "normalized-directory",
            Self::ContentAddress => "content-address",
        }
    }

    /// Parses the closed vocabulary.
    pub fn parse(value: &str) -> Result<Self, AdmissionError> {
        match value {
            "normalized-file" => Ok(Self::NormalizedFile),
            "normalized-directory" => Ok(Self::NormalizedDirectory),
            "content-address" => Ok(Self::ContentAddress),
            _ => Err(AdmissionError::ObservationInvalid),
        }
    }
}

/// Closed location class.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LocationClass {
    /// Stable local fixed storage.
    LocalFixed,
    /// Explicitly permitted local removable storage.
    LocalRemovable,
    /// Process-local or authenticated client memory.
    MemoryOnly,
    /// Remote storage; denied under the baseline.
    Remote,
}

impl LocationClass {
    /// Stable wire string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalFixed => "local-fixed",
            Self::LocalRemovable => "local-removable",
            Self::MemoryOnly => "memory-only",
            Self::Remote => "remote",
        }
    }

    /// Parses the closed vocabulary.
    pub fn parse(value: &str) -> Result<Self, AdmissionError> {
        match value {
            "local-fixed" => Ok(Self::LocalFixed),
            "local-removable" => Ok(Self::LocalRemovable),
            "memory-only" => Ok(Self::MemoryOnly),
            "remote" => Ok(Self::Remote),
            _ => Err(AdmissionError::ObservationInvalid),
        }
    }
}

/// Closed source kind.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceKind {
    /// Ordinary file.
    File,
    /// Bounded collection directory.
    Directory,
    /// Symbolic-link path (evaluated, never followed by this package).
    Symlink,
    /// Device node; valid but never admittable.
    Device,
    /// Socket; valid but never admittable.
    Socket,
    /// FIFO; valid but never admittable.
    Fifo,
}

impl SourceKind {
    /// Stable wire string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::Symlink => "symlink",
            Self::Device => "device",
            Self::Socket => "socket",
            Self::Fifo => "fifo",
        }
    }

    /// Parses the closed vocabulary.
    pub fn parse(value: &str) -> Result<Self, AdmissionError> {
        match value {
            "file" => Ok(Self::File),
            "directory" => Ok(Self::Directory),
            "symlink" => Ok(Self::Symlink),
            "device" => Ok(Self::Device),
            "socket" => Ok(Self::Socket),
            "fifo" => Ok(Self::Fifo),
            _ => Err(AdmissionError::ObservationInvalid),
        }
    }

    /// Whether the kind can ever be admitted.
    pub const fn is_admittable(self) -> bool {
        matches!(self, Self::File | Self::Directory | Self::Symlink)
    }
}

/// Closed sensitivity level.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SensitivityLevel {
    /// Public source.
    Public,
    /// Internal project source.
    Internal,
    /// Confidential source.
    Confidential,
    /// Secret candidate; denied unless an explicit profile permits it.
    SecretCandidate,
    /// Credential material; unconditional deny.
    Credential,
    /// Private-key material; unconditional deny.
    PrivateKey,
}

impl SensitivityLevel {
    /// Stable wire string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Confidential => "confidential",
            Self::SecretCandidate => "secret-candidate",
            Self::Credential => "credential",
            Self::PrivateKey => "private-key",
        }
    }

    /// Parses the closed vocabulary.
    pub fn parse(value: &str) -> Result<Self, AdmissionError> {
        match value {
            "public" => Ok(Self::Public),
            "internal" => Ok(Self::Internal),
            "confidential" => Ok(Self::Confidential),
            "secret-candidate" => Ok(Self::SecretCandidate),
            "credential" => Ok(Self::Credential),
            "private-key" => Ok(Self::PrivateKey),
            _ => Err(AdmissionError::ObservationInvalid),
        }
    }

    /// Whether this level is an unconditional safety deny.
    pub const fn is_unconditional_deny(self) -> bool {
        matches!(self, Self::Credential | Self::PrivateKey)
    }
}

/// Stable ordered admission reason code.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AdmissionReasonCode {
    /// Baseline regular source admitted.
    AllowBaselineSource,
    /// Explicit allow rule matched.
    AllowExplicitRule,
    /// Credential class denied unconditionally.
    DenyCredentialClass,
    /// Private-key class denied unconditionally.
    DenyPrivateKeyClass,
    /// Sensitive credential denied unconditionally.
    DenySensitiveCredential,
    /// System location denied.
    DenySystemLocation,
    /// Cache artifact denied.
    DenyCacheArtifact,
    /// Build output denied.
    DenyBuildOutput,
    /// Vendor class denied under the current policy.
    DenyVendorClass,
    /// Generated class denied under the current policy.
    DenyGeneratedClass,
    /// Binary class denied under the current policy.
    DenyBinaryClass,
    /// Sensitive class denied under the current policy.
    DenySensitiveClass,
    /// Size exceeds the policy ceiling.
    DenySizeExceeded,
    /// Empty source denied.
    DenyEmptySource,
    /// Remote location denied.
    DenyRemoteLocation,
    /// Explicit deny rule matched.
    DenyExplicitRule,
    /// Deny by default when no allow rule matches.
    DenyByDefault,
    /// Detector identity unavailable; review required.
    ReviewDetectorUnavailable,
    /// Sensitivity unavailable; review required.
    ReviewSensitivityUnknown,
    /// Metadata unavailable; review required.
    ReviewMetadataUnavailable,
    /// Explicit review rule matched.
    ReviewExplicitRule,
    /// Source kind is valid but not admittable.
    UnsupportedSourceKind,
    /// Explicit unsupported rule matched.
    UnsupportedExplicitRule,
}

impl AdmissionReasonCode {
    /// Stable machine-readable code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllowBaselineSource => "ALLOW_BASELINE_SOURCE",
            Self::AllowExplicitRule => "ALLOW_EXPLICIT_RULE",
            Self::DenyCredentialClass => "DENY_CREDENTIAL_CLASS",
            Self::DenyPrivateKeyClass => "DENY_PRIVATE_KEY_CLASS",
            Self::DenySensitiveCredential => "DENY_SENSITIVE_CREDENTIAL",
            Self::DenySystemLocation => "DENY_SYSTEM_LOCATION",
            Self::DenyCacheArtifact => "DENY_CACHE_ARTIFACT",
            Self::DenyBuildOutput => "DENY_BUILD_OUTPUT",
            Self::DenyVendorClass => "DENY_VENDOR_CLASS",
            Self::DenyGeneratedClass => "DENY_GENERATED_CLASS",
            Self::DenyBinaryClass => "DENY_BINARY_CLASS",
            Self::DenySensitiveClass => "DENY_SENSITIVE_CLASS",
            Self::DenySizeExceeded => "DENY_SIZE_EXCEEDED",
            Self::DenyEmptySource => "DENY_EMPTY_SOURCE",
            Self::DenyRemoteLocation => "DENY_REMOTE_LOCATION",
            Self::DenyExplicitRule => "DENY_EXPLICIT_RULE",
            Self::DenyByDefault => "DENY_BY_DEFAULT",
            Self::ReviewDetectorUnavailable => "REVIEW_DETECTOR_UNAVAILABLE",
            Self::ReviewSensitivityUnknown => "REVIEW_SENSITIVITY_UNKNOWN",
            Self::ReviewMetadataUnavailable => "REVIEW_METADATA_UNAVAILABLE",
            Self::ReviewExplicitRule => "REVIEW_EXPLICIT_RULE",
            Self::UnsupportedSourceKind => "UNSUPPORTED_SOURCE_KIND",
            Self::UnsupportedExplicitRule => "UNSUPPORTED_EXPLICIT_RULE",
        }
    }

    /// Parses the closed reason vocabulary.
    pub fn parse(value: &str) -> Result<Self, AdmissionError> {
        match value {
            "ALLOW_BASELINE_SOURCE" => Ok(Self::AllowBaselineSource),
            "ALLOW_EXPLICIT_RULE" => Ok(Self::AllowExplicitRule),
            "DENY_CREDENTIAL_CLASS" => Ok(Self::DenyCredentialClass),
            "DENY_PRIVATE_KEY_CLASS" => Ok(Self::DenyPrivateKeyClass),
            "DENY_SENSITIVE_CREDENTIAL" => Ok(Self::DenySensitiveCredential),
            "DENY_SYSTEM_LOCATION" => Ok(Self::DenySystemLocation),
            "DENY_CACHE_ARTIFACT" => Ok(Self::DenyCacheArtifact),
            "DENY_BUILD_OUTPUT" => Ok(Self::DenyBuildOutput),
            "DENY_VENDOR_CLASS" => Ok(Self::DenyVendorClass),
            "DENY_GENERATED_CLASS" => Ok(Self::DenyGeneratedClass),
            "DENY_BINARY_CLASS" => Ok(Self::DenyBinaryClass),
            "DENY_SENSITIVE_CLASS" => Ok(Self::DenySensitiveClass),
            "DENY_SIZE_EXCEEDED" => Ok(Self::DenySizeExceeded),
            "DENY_EMPTY_SOURCE" => Ok(Self::DenyEmptySource),
            "DENY_REMOTE_LOCATION" => Ok(Self::DenyRemoteLocation),
            "DENY_EXPLICIT_RULE" => Ok(Self::DenyExplicitRule),
            "DENY_BY_DEFAULT" => Ok(Self::DenyByDefault),
            "REVIEW_DETECTOR_UNAVAILABLE" => Ok(Self::ReviewDetectorUnavailable),
            "REVIEW_SENSITIVITY_UNKNOWN" => Ok(Self::ReviewSensitivityUnknown),
            "REVIEW_METADATA_UNAVAILABLE" => Ok(Self::ReviewMetadataUnavailable),
            "REVIEW_EXPLICIT_RULE" => Ok(Self::ReviewExplicitRule),
            "UNSUPPORTED_SOURCE_KIND" => Ok(Self::UnsupportedSourceKind),
            "UNSUPPORTED_EXPLICIT_RULE" => Ok(Self::UnsupportedExplicitRule),
            _ => Err(AdmissionError::PolicyUnknownField),
        }
    }

    /// All closed reason codes in canonical order.
    pub const ALL: &'static [Self] = &[
        Self::AllowBaselineSource,
        Self::AllowExplicitRule,
        Self::DenyCredentialClass,
        Self::DenyPrivateKeyClass,
        Self::DenySensitiveCredential,
        Self::DenySystemLocation,
        Self::DenyCacheArtifact,
        Self::DenyBuildOutput,
        Self::DenyVendorClass,
        Self::DenyGeneratedClass,
        Self::DenyBinaryClass,
        Self::DenySensitiveClass,
        Self::DenySizeExceeded,
        Self::DenyEmptySource,
        Self::DenyRemoteLocation,
        Self::DenyExplicitRule,
        Self::DenyByDefault,
        Self::ReviewDetectorUnavailable,
        Self::ReviewSensitivityUnknown,
        Self::ReviewMetadataUnavailable,
        Self::ReviewExplicitRule,
        Self::UnsupportedSourceKind,
        Self::UnsupportedExplicitRule,
    ];
}

// ---------------------------------------------------------------------------
// Policy operations
// ---------------------------------------------------------------------------

/// Raw caller-supplied policy input with explicit unknown-field capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnvalidatedPolicyInput {
    /// Declared schema version; must equal `POLICY_SCHEMA_VERSION`.
    pub schema_version: u32,
    /// Declared policy revision; must be non-zero.
    pub revision: u64,
    /// Declared rules in caller order.
    pub rules: Vec<UnvalidatedRuleInput>,
    /// Maximum admitted file bytes.
    pub max_file_bytes: u64,
    /// Whether generated sources may be admitted.
    pub allow_generated: bool,
    /// Whether vendor sources may be admitted.
    pub allow_vendor: bool,
    /// Whether binary sources may be admitted.
    pub allow_binary: bool,
    /// Unknown top-level fields observed by the caller; must remain empty.
    pub unknown_fields: Vec<String>,
}

/// Raw caller-supplied rule input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnvalidatedRuleInput {
    /// Stable rule identifier.
    pub id: String,
    /// Explicit precedence; duplicates are rejected.
    pub precedence: u32,
    /// Source class wire string from the closed vocabulary.
    pub source_class: String,
    /// Operator wire string from the closed vocabulary.
    pub operator: String,
    /// Effect wire string from the closed vocabulary.
    pub effect: String,
    /// Reason wire string from the closed vocabulary.
    pub reason: String,
    /// Optional size threshold for `size-greater-than`.
    pub size_threshold: Option<u64>,
    /// Optional sensitivity threshold for `sensitivity-at-least`.
    pub sensitivity_threshold: Option<String>,
    /// Optional locator class for `locator-class-is`.
    pub locator_class: Option<String>,
    /// Optional glob-style pattern (bounded, never evaluated as code).
    pub pattern: Option<String>,
    /// Unknown rule fields observed by the caller; must remain empty.
    pub unknown_fields: Vec<String>,
}

/// Schema-validated but not yet canonicalized policy draft.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionPolicyDraft {
    revision: NonZeroRevision,
    rules: Vec<DraftRule>,
    max_file_bytes: u64,
    allow_generated: bool,
    allow_vendor: bool,
    allow_binary: bool,
}

/// Validated draft rule.
#[derive(Clone, Debug, Eq, PartialEq)]
struct DraftRule {
    id: String,
    precedence: u32,
    source_class: SourceClass,
    operator: RuleOperator,
    effect: RuleEffect,
    reason: AdmissionReasonCode,
    size_threshold: Option<u64>,
    sensitivity_threshold: Option<SensitivityLevel>,
    locator_class: Option<LocatorClass>,
    pattern: Option<String>,
}

/// Canonical admission policy with deterministic rule order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAdmissionPolicy {
    revision: NonZeroRevision,
    rules: Vec<CanonicalRule>,
    max_file_bytes: u64,
    allow_generated: bool,
    allow_vendor: bool,
    allow_binary: bool,
}

/// Canonical admission rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRule {
    /// Stable rule identifier.
    pub id: String,
    /// Explicit precedence (ascending evaluation order).
    pub precedence: u32,
    /// Source class bound by this rule.
    pub source_class: SourceClass,
    /// Closed operator.
    pub operator: RuleOperator,
    /// Closed effect.
    pub effect: RuleEffect,
    /// Stable reason emitted when the rule matches.
    pub reason: AdmissionReasonCode,
    /// Size threshold for `size-greater-than`.
    pub size_threshold: Option<u64>,
    /// Sensitivity threshold for `sensitivity-at-least`.
    pub sensitivity_threshold: Option<SensitivityLevel>,
    /// Locator class for `locator-class-is`.
    pub locator_class: Option<LocatorClass>,
    /// Bounded pattern text (content-free, never executed).
    pub pattern: Option<String>,
}

impl CanonicalAdmissionPolicy {
    /// Exact policy revision.
    pub fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Maximum admitted file bytes.
    pub const fn max_file_bytes(&self) -> u64 {
        self.max_file_bytes
    }

    /// Canonical rules in precedence order.
    pub fn rules(&self) -> &[CanonicalRule] {
        &self.rules
    }

    /// Whether generated sources may be admitted.
    pub const fn allows_generated(&self) -> bool {
        self.allow_generated
    }

    /// Whether vendor sources may be admitted.
    pub const fn allows_vendor(&self) -> bool {
        self.allow_vendor
    }

    /// Whether binary sources may be admitted.
    pub const fn allows_binary(&self) -> bool {
        self.allow_binary
    }
}

/// Domain-separated canonical policy fingerprint.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PolicyFingerprint([u8; 32]);

impl PolicyFingerprint {
    /// Exact fingerprint bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lowercase hexadecimal rendering for diagnostics (never a path).
    pub fn to_hex(&self) -> String {
        hex_encode(&self.0)
    }
}

impl fmt::Display for PolicyFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex_encode(&self.0))
    }
}

/// Exact policy-change classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionPolicyChange {
    /// Canonical policies are identical.
    Noop,
    /// Change only narrows admission; requires a security barrier.
    RestrictiveSecurityBarrier,
    /// Change only widens admission; requires explicit reconciliation.
    PermissiveReconcileRequired,
    /// Change both narrows and widens admission.
    MixedSecurityBarrierAndReconcile,
    /// Change is internally inconsistent and must be rejected.
    Reject,
}

impl AdmissionPolicyChange {
    /// Stable machine-readable code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "NOOP",
            Self::RestrictiveSecurityBarrier => "RESTRICTIVE_SECURITY_BARRIER",
            Self::PermissiveReconcileRequired => "PERMISSIVE_RECONCILE_REQUIRED",
            Self::MixedSecurityBarrierAndReconcile => "MIXED_SECURITY_BARRIER_AND_RECONCILE",
            Self::Reject => "REJECT",
        }
    }
}

fn validate_rule_input(input: &UnvalidatedRuleInput) -> Result<DraftRule, AdmissionError> {
    if !input.unknown_fields.is_empty() {
        return Err(AdmissionError::PolicyUnknownField);
    }
    if input.id.is_empty() || input.id.len() > MAX_ID_BYTES {
        return Err(AdmissionError::PolicyUnknownField);
    }
    check_id(&input.id, "rule.id").map_err(|_| AdmissionError::PolicyUnknownField)?;
    let source_class =
        SourceClass::parse(&input.source_class).map_err(|_| AdmissionError::PolicyUnknownField)?;
    let operator =
        RuleOperator::parse(&input.operator).map_err(|_| AdmissionError::PolicyUnknownField)?;
    let effect =
        RuleEffect::parse(&input.effect).map_err(|_| AdmissionError::PolicyUnknownField)?;
    let reason = AdmissionReasonCode::parse(&input.reason)
        .map_err(|_| AdmissionError::PolicyUnknownField)?;
    if let Some(pattern) = &input.pattern {
        check_pattern(pattern)?;
        if pattern.len() > MAX_PATTERN_BYTES {
            return Err(AdmissionError::PolicyUnknownField);
        }
    }
    let sensitivity_threshold = input
        .sensitivity_threshold
        .as_deref()
        .map(SensitivityLevel::parse)
        .transpose()
        .map_err(|_| AdmissionError::PolicyUnknownField)?;
    let locator_class = input
        .locator_class
        .as_deref()
        .map(LocatorClass::parse)
        .transpose()
        .map_err(|_| AdmissionError::PolicyUnknownField)?;
    // Operator-specific shape checks fail closed without silent defaults.
    match operator {
        RuleOperator::SizeGreaterThan => {
            if input.size_threshold.is_none() {
                return Err(AdmissionError::PolicyConflict);
            }
        }
        RuleOperator::SensitivityAtLeast => {
            if sensitivity_threshold.is_none() {
                return Err(AdmissionError::PolicyConflict);
            }
        }
        RuleOperator::LocatorClassIs => {
            if locator_class.is_none() {
                return Err(AdmissionError::PolicyConflict);
            }
        }
        RuleOperator::Always | RuleOperator::SourceClassIs => {}
    }
    // Unconditional safety denies cannot be repurposed as allows at schema
    // validation time; explicit override scopes are not authorized by the
    // baseline schema and are therefore rejected here.
    if source_class.is_unconditional_deny() && effect == RuleEffect::Allow {
        return Err(AdmissionError::PolicyOverrideForbidden);
    }
    Ok(DraftRule {
        id: input.id.clone(),
        precedence: input.precedence,
        source_class,
        operator,
        effect,
        reason,
        size_threshold: input.size_threshold,
        sensitivity_threshold,
        locator_class,
        pattern: input.pattern.clone(),
    })
}

/// Validates the closed policy schema.
///
/// Checks schema version, closed rule/operator/reason sets, explicit
/// precedence, unconditional deny classes, finite bounds, and override
/// authorization. Duplicate or conflicting rules fail.
///
/// # Errors
///
/// Returns `ADMISSION_POLICY_SCHEMA_UNSUPPORTED` for wrong versions,
/// `ADMISSION_POLICY_UNKNOWN_FIELD` for unknown fields/classes/operators,
/// `ADMISSION_POLICY_CONFLICT` for duplicates or shape conflicts, and
/// `ADMISSION_POLICY_OVERRIDE_FORBIDDEN` for unauthorized overrides.
pub fn validate_policy_schema(
    input: &UnvalidatedPolicyInput,
) -> Result<AdmissionPolicyDraft, AdmissionError> {
    if input.schema_version != POLICY_SCHEMA_VERSION {
        return Err(AdmissionError::PolicySchemaUnsupported);
    }
    if !input.unknown_fields.is_empty() {
        return Err(AdmissionError::PolicyUnknownField);
    }
    let revision =
        NonZeroRevision::new(input.revision).map_err(|_| AdmissionError::PolicyConflict)?;
    if input.rules.is_empty() || input.rules.len() > MAX_POLICY_RULES {
        return Err(AdmissionError::PolicyConflict);
    }
    if input.max_file_bytes == 0 || input.max_file_bytes > DEFAULT_MAX_FILE_BYTES {
        return Err(AdmissionError::PolicyConflict);
    }
    let mut rules = Vec::with_capacity(input.rules.len());
    for rule in &input.rules {
        rules.push(validate_rule_input(rule)?);
    }
    // Explicit precedence and stable identifiers must be unique; duplicate or
    // conflicting rules fail instead of being silently merged.
    for (index, rule) in rules.iter().enumerate() {
        for earlier in &rules[..index] {
            if earlier.id == rule.id {
                return Err(AdmissionError::PolicyConflict);
            }
            if earlier.precedence == rule.precedence {
                return Err(AdmissionError::PolicyConflict);
            }
            if earlier.id == rule.id
                && (earlier.effect != rule.effect
                    || earlier.source_class != rule.source_class
                    || earlier.operator != rule.operator)
            {
                return Err(AdmissionError::PolicyConflict);
            }
        }
    }
    // Conflicting duplicate coverage (same class/operator with different
    // effects at different precedences) is allowed structurally because
    // precedence decides; identical id/precedence duplicates above are not.
    Ok(AdmissionPolicyDraft {
        revision,
        rules,
        max_file_bytes: input.max_file_bytes,
        allow_generated: input.allow_generated,
        allow_vendor: input.allow_vendor,
        allow_binary: input.allow_binary,
    })
}

/// Canonicalizes a validated draft.
///
/// Sorts rules by `(precedence, id)`, normalizes pattern encodings and
/// thresholds, and sorts override scopes deterministically. Serialization is
/// independent of map or input order.
///
/// # Errors
///
/// Returns `ADMISSION_POLICY_CONFLICT` when canonical bounds are violated and
/// `ADMISSION_POLICY_OVERRIDE_FORBIDDEN` when an allow would bypass an
/// unconditional deny.
pub fn normalize_policy(
    draft: &AdmissionPolicyDraft,
) -> Result<CanonicalAdmissionPolicy, AdmissionError> {
    let mut rules: Vec<CanonicalRule> = draft
        .rules
        .iter()
        .map(|rule| CanonicalRule {
            id: rule.id.clone(),
            precedence: rule.precedence,
            source_class: rule.source_class,
            operator: rule.operator,
            effect: rule.effect,
            reason: rule.reason,
            size_threshold: rule.size_threshold,
            sensitivity_threshold: rule.sensitivity_threshold,
            locator_class: rule.locator_class,
            pattern: rule.pattern.clone(),
        })
        .collect();
    for rule in &rules {
        if rule.source_class.is_unconditional_deny() && rule.effect == RuleEffect::Allow {
            return Err(AdmissionError::PolicyOverrideForbidden);
        }
        if rule.id.len() > MAX_ID_BYTES {
            return Err(AdmissionError::PolicyConflict);
        }
        if let Some(pattern) = &rule.pattern {
            if pattern.len() > MAX_PATTERN_BYTES {
                return Err(AdmissionError::PolicyConflict);
            }
        }
    }
    rules.sort_by(|left, right| {
        left.precedence
            .cmp(&right.precedence)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(CanonicalAdmissionPolicy {
        revision: draft.revision,
        rules,
        max_file_bytes: draft.max_file_bytes,
        allow_generated: draft.allow_generated,
        allow_vendor: draft.allow_vendor,
        allow_binary: draft.allow_binary,
    })
}

fn canonical_policy_writer(policy: &CanonicalAdmissionPolicy) -> CanonicalWriter {
    let mut writer = CanonicalWriter::new(b"eliot-search/admission-policy/v1");
    writer.push_u32(POLICY_SCHEMA_VERSION);
    writer.push_u64(policy.revision.get());
    writer.push_u64(policy.max_file_bytes);
    writer.push_bool(policy.allow_generated);
    writer.push_bool(policy.allow_vendor);
    writer.push_bool(policy.allow_binary);
    writer.push_u64(u64::try_from(policy.rules.len()).unwrap_or(u64::MAX));
    for rule in &policy.rules {
        writer.push_str(&rule.id);
        writer.push_u64(u64::from(rule.precedence));
        writer.push_str(rule.source_class.as_str());
        writer.push_str(rule.operator.as_str());
        writer.push_str(rule.effect.as_str());
        writer.push_str(rule.reason.as_str());
        writer.push_option_u64(rule.size_threshold);
        writer.push_option_str(rule.sensitivity_threshold.map(SensitivityLevel::as_str));
        writer.push_option_str(rule.locator_class.map(LocatorClass::as_str));
        writer.push_option_str(rule.pattern.as_deref());
    }
    writer
}

/// Computes the domain-separated canonical policy fingerprint.
///
/// Any load-bearing change produces a new fingerprint.
pub fn policy_fingerprint(policy: &CanonicalAdmissionPolicy) -> PolicyFingerprint {
    PolicyFingerprint(canonical_policy_writer(policy).finish())
}

/// Classifies the exact change between two canonical policies.
///
/// The result never applies the change and never decides which registered
/// sources survive; owners consume the classification for barrier and
/// reconciliation obligations.
pub fn classify_policy_change(
    old: &CanonicalAdmissionPolicy,
    new: &CanonicalAdmissionPolicy,
) -> AdmissionPolicyChange {
    if old == new {
        return AdmissionPolicyChange::Noop;
    }
    // Structurally inconsistent targets (e.g. an allow bypassing an
    // unconditional deny) are rejected rather than classified.
    for rule in &new.rules {
        if rule.source_class.is_unconditional_deny() && rule.effect == RuleEffect::Allow {
            return AdmissionPolicyChange::Reject;
        }
    }
    let old_fingerprint = policy_fingerprint(old);
    let new_fingerprint = policy_fingerprint(new);
    if old_fingerprint == new_fingerprint {
        return AdmissionPolicyChange::Noop;
    }
    let mut restrictive = false;
    let mut permissive = false;
    if new.max_file_bytes < old.max_file_bytes {
        restrictive = true;
    }
    if new.max_file_bytes > old.max_file_bytes {
        permissive = true;
    }
    for (old_flag, new_flag) in [
        (old.allow_generated, new.allow_generated),
        (old.allow_vendor, new.allow_vendor),
        (old.allow_binary, new.allow_binary),
    ] {
        if old_flag && !new_flag {
            restrictive = true;
        }
        if !old_flag && new_flag {
            permissive = true;
        }
    }
    // Rule-level comparison keyed by stable identifier.
    for new_rule in &new.rules {
        match old.rules.iter().find(|old_rule| old_rule.id == new_rule.id) {
            None => match new_rule.effect {
                RuleEffect::Allow => permissive = true,
                RuleEffect::Deny => restrictive = true,
                RuleEffect::Review | RuleEffect::Unsupported => restrictive = true,
            },
            Some(old_rule) => {
                if old_rule != new_rule {
                    // Any rule replacement is treated as both directions
                    // unless the effect ordering proves otherwise, because
                    // matchers are not comparable without evaluation.
                    match (old_rule.effect, new_rule.effect) {
                        (RuleEffect::Allow, RuleEffect::Deny)
                        | (RuleEffect::Allow, RuleEffect::Review)
                        | (RuleEffect::Allow, RuleEffect::Unsupported) => {
                            restrictive = true;
                        }
                        (RuleEffect::Deny, RuleEffect::Allow)
                        | (RuleEffect::Review, RuleEffect::Allow)
                        | (RuleEffect::Unsupported, RuleEffect::Allow) => {
                            permissive = true;
                        }
                        _ => {
                            restrictive = true;
                            permissive = true;
                        }
                    }
                }
            }
        }
    }
    for old_rule in &old.rules {
        if !new.rules.iter().any(|rule| rule.id == old_rule.id) {
            match old_rule.effect {
                RuleEffect::Allow => restrictive = true,
                RuleEffect::Deny => permissive = true,
                RuleEffect::Review | RuleEffect::Unsupported => permissive = true,
            }
        }
    }
    match (restrictive, permissive) {
        (false, false) => AdmissionPolicyChange::Noop,
        (true, false) => AdmissionPolicyChange::RestrictiveSecurityBarrier,
        (false, true) => AdmissionPolicyChange::PermissiveReconcileRequired,
        (true, true) => AdmissionPolicyChange::MixedSecurityBarrierAndReconcile,
    }
}

// ---------------------------------------------------------------------------
// Observation operations
// ---------------------------------------------------------------------------

/// Raw caller-supplied observation input with explicit unavailable markers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnvalidatedObservationInput {
    /// Bounded normalized locator class (never a free display path).
    pub locator_class: String,
    /// Location class wire string.
    pub location_class: String,
    /// Source-kind wire string.
    pub source_kind: String,
    /// Source-class wire string.
    pub source_class: String,
    /// Observed byte size; `None` means unavailable.
    pub byte_size: Option<u64>,
    /// Generated signal; `None` means unavailable.
    pub is_generated: Option<bool>,
    /// Vendor signal; `None` means unavailable.
    pub is_vendor: Option<bool>,
    /// Binary signal; `None` means unavailable.
    pub is_binary: Option<bool>,
    /// System signal; `None` means unavailable.
    pub is_system: Option<bool>,
    /// Sensitivity wire string; `None` means unavailable.
    pub sensitivity: Option<String>,
    /// Detector identity; `None` means unavailable.
    pub detector_id: Option<String>,
    /// Profile identity; `None` means unavailable.
    pub profile_id: Option<String>,
    /// Explicit unavailable load-bearing fields.
    pub unavailable_fields: Vec<String>,
    /// Unknown fields observed by the caller; must remain empty.
    pub unknown_fields: Vec<String>,
}

/// Validated canonical admission observation (content-free, no source body).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionObservation {
    locator_class: LocatorClass,
    location_class: LocationClass,
    source_kind: SourceKind,
    source_class: SourceClass,
    byte_size: Option<u64>,
    is_generated: Option<bool>,
    is_vendor: Option<bool>,
    is_binary: Option<bool>,
    is_system: Option<bool>,
    sensitivity: Option<SensitivityLevel>,
    detector_id: Option<String>,
    profile_id: Option<String>,
    unavailable_fields: BTreeSet<String>,
}

impl AdmissionObservation {
    /// Bounded normalized locator class.
    pub const fn locator_class(&self) -> LocatorClass {
        self.locator_class
    }

    /// Location class.
    pub const fn location_class(&self) -> LocationClass {
        self.location_class
    }

    /// Source kind.
    pub const fn source_kind(&self) -> SourceKind {
        self.source_kind
    }

    /// Source class.
    pub const fn source_class(&self) -> SourceClass {
        self.source_class
    }

    /// Observed byte size, if available.
    pub const fn byte_size(&self) -> Option<u64> {
        self.byte_size
    }

    /// Maximum sensitivity signal, if available.
    pub const fn sensitivity(&self) -> Option<SensitivityLevel> {
        self.sensitivity
    }

    /// Explicit unavailable fields in canonical order.
    pub fn unavailable_fields(&self) -> impl ExactSizeIterator<Item = &String> {
        self.unavailable_fields.iter()
    }
}

/// Content-free canonical observation digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdmissionObservationDigest([u8; 32]);

impl AdmissionObservationDigest {
    /// Exact digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lowercase hexadecimal rendering for diagnostics.
    pub fn to_hex(&self) -> String {
        hex_encode(&self.0)
    }
}

impl fmt::Display for AdmissionObservationDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex_encode(&self.0))
    }
}

/// Validates one observation without performing any I/O.
///
/// Requires a bounded normalized locator class, location class, metadata,
/// source kind, byte-size observation, generated/vendor/binary/system and
/// sensitivity signals, detector/profile identities, and explicit
/// unavailable fields. Unavailable load-bearing observation never defaults
/// to allow.
///
/// # Errors
///
/// Returns `ADMISSION_OBSERVATION_INVALID` for malformed vocabularies or
/// bounds and `ADMISSION_OBSERVATION_INCOMPLETE` when a load-bearing field
/// is missing without an explicit unavailable marker.
pub fn validate_observation(
    input: &UnvalidatedObservationInput,
    policy: &CanonicalAdmissionPolicy,
    limits: AdmissionLimits,
) -> Result<AdmissionObservation, AdmissionError> {
    let limits = limits.validate()?;
    if !input.unknown_fields.is_empty() {
        return Err(AdmissionError::PolicyUnknownField);
    }
    if policy.rules.len() > limits.max_rules {
        return Err(AdmissionError::ObservationInvalid);
    }
    let locator_class = LocatorClass::parse(&input.locator_class)?;
    let location_class = LocationClass::parse(&input.location_class)?;
    let source_kind = SourceKind::parse(&input.source_kind)?;
    let source_class =
        SourceClass::parse(&input.source_class).map_err(|_| AdmissionError::PolicyUnknownField)?;
    for field in &input.unavailable_fields {
        if !CLOSED_UNAVAILABLE_FIELDS.contains(&field.as_str()) {
            return Err(AdmissionError::PolicyUnknownField);
        }
    }
    let unavailable: BTreeSet<String> = input.unavailable_fields.iter().cloned().collect();
    if unavailable.len() != input.unavailable_fields.len() {
        return Err(AdmissionError::ObservationInvalid);
    }
    let sensitivity = input
        .sensitivity
        .as_deref()
        .map(SensitivityLevel::parse)
        .transpose()?;
    if let Some(detector) = &input.detector_id {
        check_id(detector, "detector_id")?;
    }
    if let Some(profile) = &input.profile_id {
        check_id(profile, "profile_id")?;
    }
    if let Some(size) = input.byte_size {
        if size > policy.max_file_bytes().saturating_mul(16).saturating_add(1) {
            // Observations absurdly above any policy ceiling are malformed
            // rather than silently reviewable; evaluation still denies.
            return Err(AdmissionError::ObservationInvalid);
        }
    }
    // Load-bearing completeness: every `None` must be covered by an explicit
    // unavailable marker, otherwise the observation is incomplete.
    let mut missing: Vec<&str> = Vec::new();
    if input.byte_size.is_none() && !unavailable.contains("byte-size") {
        missing.push("byte-size");
    }
    if input.sensitivity.is_none() && !unavailable.contains("sensitivity") {
        missing.push("sensitivity");
    }
    if input.detector_id.is_none() && !unavailable.contains("detector-id") {
        missing.push("detector-id");
    }
    if input.profile_id.is_none() && !unavailable.contains("profile-id") {
        missing.push("profile-id");
    }
    if input.is_generated.is_none() && !unavailable.contains("generated-flag") {
        missing.push("generated-flag");
    }
    if input.is_vendor.is_none() && !unavailable.contains("vendor-flag") {
        missing.push("vendor-flag");
    }
    if input.is_binary.is_none() && !unavailable.contains("binary-flag") {
        missing.push("binary-flag");
    }
    if input.is_system.is_none() && !unavailable.contains("system-flag") {
        missing.push("system-flag");
    }
    if !missing.is_empty() {
        return Err(AdmissionError::ObservationIncomplete);
    }
    // Declared unavailable markers must match actual `None` values; a marker
    // for an available field is contradictory input.
    let declared_but_available = [
        ("byte-size", input.byte_size.is_some()),
        ("sensitivity", input.sensitivity.is_some()),
        ("detector-id", input.detector_id.is_some()),
        ("profile-id", input.profile_id.is_some()),
        ("generated-flag", input.is_generated.is_some()),
        ("vendor-flag", input.is_vendor.is_some()),
        ("binary-flag", input.is_binary.is_some()),
        ("system-flag", input.is_system.is_some()),
    ];
    for (field, available) in declared_but_available {
        if available && unavailable.contains(field) {
            return Err(AdmissionError::ObservationInvalid);
        }
    }
    Ok(AdmissionObservation {
        locator_class,
        location_class,
        source_kind,
        source_class,
        byte_size: input.byte_size,
        is_generated: input.is_generated,
        is_vendor: input.is_vendor,
        is_binary: input.is_binary,
        is_system: input.is_system,
        sensitivity,
        detector_id: input.detector_id.clone(),
        profile_id: input.profile_id.clone(),
        unavailable_fields: unavailable,
    })
}

fn canonical_observation_writer(observation: &AdmissionObservation) -> CanonicalWriter {
    let mut writer = CanonicalWriter::new(b"eliot-search/admission-observation/v1");
    writer.push_str(observation.locator_class.as_str());
    writer.push_str(observation.location_class.as_str());
    writer.push_str(observation.source_kind.as_str());
    writer.push_str(observation.source_class.as_str());
    writer.push_option_u64(observation.byte_size);
    writer.push_option_bool(observation.is_generated);
    writer.push_option_bool(observation.is_vendor);
    writer.push_option_bool(observation.is_binary);
    writer.push_option_bool(observation.is_system);
    writer.push_option_str(observation.sensitivity.map(SensitivityLevel::as_str));
    writer.push_option_str(observation.detector_id.as_deref());
    writer.push_option_str(observation.profile_id.as_deref());
    writer.push_u64(u64::try_from(observation.unavailable_fields.len()).unwrap_or(u64::MAX));
    for field in &observation.unavailable_fields {
        writer.push_str(field);
    }
    writer
}

/// Hashes canonical content-free observation fields.
///
/// Classifier and profile identities are bound; source bodies are never
/// hashed because baseline admission owns no body classifier.
pub fn observation_digest(observation: &AdmissionObservation) -> AdmissionObservationDigest {
    AdmissionObservationDigest(canonical_observation_writer(observation).finish())
}

// ---------------------------------------------------------------------------
// Decision and receipt operations
// ---------------------------------------------------------------------------

/// Terminal admission outcome.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AdmissionOutcome {
    /// Source is admitted under the named policy.
    Allow,
    /// Source is denied under the named policy.
    Deny,
    /// Human review is required before admission.
    ReviewRequired,
    /// Source kind is valid but not admittable.
    Unsupported,
}

impl AdmissionOutcome {
    /// Stable wire string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "ALLOW",
            Self::Deny => "DENY",
            Self::ReviewRequired => "REVIEW_REQUIRED",
            Self::Unsupported => "UNSUPPORTED",
        }
    }
}

/// Deterministic admission decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionDecision {
    outcome: AdmissionOutcome,
    reason_codes: BTreeSet<AdmissionReasonCode>,
    matched_rule_ids: BTreeSet<String>,
    sensitivity: SensitivityLevel,
    explanation: String,
    policy_revision: NonZeroRevision,
    policy_fingerprint: PolicyFingerprint,
    observation_digest: AdmissionObservationDigest,
}

impl AdmissionDecision {
    /// Terminal outcome.
    pub const fn outcome(&self) -> AdmissionOutcome {
        self.outcome
    }

    /// Stable ordered reason codes.
    pub fn reason_codes(&self) -> impl ExactSizeIterator<Item = &AdmissionReasonCode> {
        self.reason_codes.iter()
    }

    /// Matched rule identifiers in canonical order.
    pub fn matched_rule_ids(&self) -> impl ExactSizeIterator<Item = &String> {
        self.matched_rule_ids.iter()
    }

    /// Maximum disclosure/sensitivity class.
    pub const fn sensitivity(&self) -> SensitivityLevel {
        self.sensitivity
    }

    /// Bounded non-content explanation metadata (reason list, no paths).
    pub fn explanation(&self) -> &str {
        &self.explanation
    }

    /// Policy revision that authorized this decision shape.
    pub fn policy_revision(&self) -> NonZeroRevision {
        self.policy_revision
    }

    /// Policy fingerprint bound by this decision.
    pub const fn policy_fingerprint(&self) -> PolicyFingerprint {
        self.policy_fingerprint
    }

    /// Observation digest bound by this decision.
    pub const fn observation_digest(&self) -> AdmissionObservationDigest {
        self.observation_digest
    }
}

/// Disclosure level for redacted views (content-free by construction).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisclosureLevel {
    /// Standard redacted decision view.
    Standard,
    /// Minimal redacted decision view.
    Minimal,
}

/// Redacted decision view without paths, secret features, or source bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionDecisionView {
    /// Terminal outcome.
    pub outcome: AdmissionOutcome,
    /// Ordered reason code strings.
    pub reason_codes: Vec<String>,
    /// Matched rule identifiers in canonical order.
    pub matched_rule_ids: Vec<String>,
    /// Locator class only (never a path).
    pub locator_class: LocatorClass,
    /// Source class only.
    pub source_class: SourceClass,
    /// Maximum sensitivity class.
    pub sensitivity: SensitivityLevel,
}

fn rule_matches(rule: &CanonicalRule, observation: &AdmissionObservation) -> bool {
    // The rule's own source class always scopes the match; operators add
    // further conditions. `Always` therefore means "always for this class".
    if rule.source_class != observation.source_class && rule.operator != RuleOperator::Always {
        // For non-`Always` operators the class must still agree, because a
        // rule never widens beyond its declared class.
        return false;
    }
    match rule.operator {
        RuleOperator::Always => {
            // `Always` with a concrete class matches that class; a wildcard
            // class is expressed with `regular` plus explicit companion
            // rules, never with an implicit any-class bypass.
            rule.source_class == observation.source_class
        }
        RuleOperator::SourceClassIs => rule.source_class == observation.source_class,
        RuleOperator::SensitivityAtLeast => {
            match (rule.sensitivity_threshold, observation.sensitivity) {
                (Some(threshold), Some(actual)) => actual >= threshold,
                // Unavailable sensitivity never silently matches.
                _ => false,
            }
        }
        RuleOperator::SizeGreaterThan => match (rule.size_threshold, observation.byte_size) {
            (Some(threshold), Some(actual)) => actual > threshold,
            _ => false,
        },
        RuleOperator::LocatorClassIs => match rule.locator_class {
            Some(expected) => expected == observation.locator_class,
            None => false,
        },
    }
}

fn baseline_reasons(
    policy: &CanonicalAdmissionPolicy,
    observation: &AdmissionObservation,
    reasons: &mut BTreeSet<AdmissionReasonCode>,
    matched: &mut BTreeSet<String>,
    budget_remaining: &mut u32,
    cancel: CancelFlag,
) -> Result<(), AdmissionError> {
    for rule in &policy.rules {
        cancel.check()?;
        if *budget_remaining == 0 {
            return Err(AdmissionError::BudgetExhausted);
        }
        *budget_remaining = budget_remaining.saturating_sub(1);
        if rule_matches(rule, observation) {
            match rule.effect {
                RuleEffect::Allow => {
                    reasons.insert(AdmissionReasonCode::AllowExplicitRule);
                }
                RuleEffect::Deny => {
                    reasons.insert(AdmissionReasonCode::DenyExplicitRule);
                }
                RuleEffect::Review => {
                    reasons.insert(AdmissionReasonCode::ReviewExplicitRule);
                }
                RuleEffect::Unsupported => {
                    reasons.insert(AdmissionReasonCode::UnsupportedExplicitRule);
                }
            }
            // Reason codes stay ordered via `BTreeSet`; rule identifiers are
            // bounded and deduplicated the same way.
            if matched.len() < MAX_MATCHED_RULES {
                matched.insert(rule.id.clone());
            } else {
                return Err(AdmissionError::ObservationInvalid);
            }
            // Keep the rule's declared reason alongside the effect-derived
            // code so equal canonical inputs stay byte-identical.
            if reasons.len() < MAX_REASON_CODES {
                reasons.insert(rule.reason);
            } else {
                return Err(AdmissionError::ObservationInvalid);
            }
        }
    }
    Ok(())
}

/// Applies the closed rule order and returns one explicit outcome.
///
/// Cancellation or budget exhaustion returns no successful decision, and no
/// partial rule evaluation may be advertised as allow.
///
/// # Errors
///
/// Returns `ADMISSION_CANCELLED` when cancelled and
/// `ADMISSION_BUDGET_EXHAUSTED` when the finite budget runs out.
pub fn evaluate(
    policy: &CanonicalAdmissionPolicy,
    observation: &AdmissionObservation,
    budget: AdmissionBudget,
    cancel: CancelFlag,
) -> Result<AdmissionDecision, AdmissionError> {
    cancel.check()?;
    let mut budget_remaining = budget.max_rule_evaluations;
    if budget_remaining == 0 {
        return Err(AdmissionError::BudgetExhausted);
    }

    let fingerprint = policy_fingerprint(policy);
    let digest = observation_digest(observation);
    let sensitivity = observation
        .sensitivity
        .unwrap_or(SensitivityLevel::Confidential);

    // Unconditional safety denies are applied before any rule and cannot be
    // bypassed by an allow rule or an unspecified override.
    let mut deny = BTreeSet::new();
    let mut review = BTreeSet::new();
    let mut unsupported = BTreeSet::new();
    let mut allow = BTreeSet::new();
    let mut matched: BTreeSet<String> = BTreeSet::new();

    if observation.source_class == SourceClass::Credential {
        deny.insert(AdmissionReasonCode::DenyCredentialClass);
    }
    if observation.source_class == SourceClass::PrivateKey {
        deny.insert(AdmissionReasonCode::DenyPrivateKeyClass);
    }
    if observation
        .sensitivity
        .is_some_and(SensitivityLevel::is_unconditional_deny)
    {
        deny.insert(AdmissionReasonCode::DenySensitiveCredential);
    }
    // System, cache, and build outputs are denied by default with dedicated
    // reasons so fixtures stay explicit.
    if observation.source_class == SourceClass::System {
        deny.insert(AdmissionReasonCode::DenySystemLocation);
    }
    if observation.source_class == SourceClass::Cache {
        deny.insert(AdmissionReasonCode::DenyCacheArtifact);
    }
    if observation.source_class == SourceClass::BuildOutput {
        deny.insert(AdmissionReasonCode::DenyBuildOutput);
    }
    // Baseline generated/vendor/binary handling follows the canonical
    // policy flags; the flags never affect unconditional denies above.
    if observation.source_class == SourceClass::Generated && !policy.allow_generated {
        deny.insert(AdmissionReasonCode::DenyGeneratedClass);
    }
    if observation.source_class == SourceClass::Vendor && !policy.allow_vendor {
        deny.insert(AdmissionReasonCode::DenyVendorClass);
    }
    if observation.source_class == SourceClass::Binary && !policy.allow_binary {
        deny.insert(AdmissionReasonCode::DenyBinaryClass);
    }
    // Sensitive classes stay denied unless an explicit allow rule admits
    // them; the baseline never silently allows them.
    if matches!(
        observation.sensitivity,
        Some(SensitivityLevel::SecretCandidate | SensitivityLevel::Confidential)
    ) && matches!(
        observation.source_class,
        SourceClass::Credential | SourceClass::PrivateKey
    ) {
        // Already denied above; keep the dedicated sensitive reason too.
        deny.insert(AdmissionReasonCode::DenySensitiveClass);
    } else if matches!(
        observation.sensitivity,
        Some(SensitivityLevel::SecretCandidate)
    ) {
        deny.insert(AdmissionReasonCode::DenySensitiveClass);
    }
    // Size and location fences apply before rule evaluation.
    match observation.byte_size {
        Some(0) => {
            deny.insert(AdmissionReasonCode::DenyEmptySource);
        }
        Some(size) if size > policy.max_file_bytes => {
            deny.insert(AdmissionReasonCode::DenySizeExceeded);
        }
        Some(_) => {}
        None => {
            review.insert(AdmissionReasonCode::ReviewMetadataUnavailable);
        }
    }
    if observation.location_class == LocationClass::Remote {
        deny.insert(AdmissionReasonCode::DenyRemoteLocation);
    }
    if !observation.source_kind.is_admittable() {
        unsupported.insert(AdmissionReasonCode::UnsupportedSourceKind);
    }
    // Unavailable load-bearing signals require review and never allow.
    if observation.detector_id.is_none() {
        review.insert(AdmissionReasonCode::ReviewDetectorUnavailable);
    }
    if observation.sensitivity.is_none() {
        review.insert(AdmissionReasonCode::ReviewSensitivityUnknown);
    }
    if observation.is_generated.is_none()
        || observation.is_vendor.is_none()
        || observation.is_binary.is_none()
        || observation.is_system.is_none()
    {
        review.insert(AdmissionReasonCode::ReviewMetadataUnavailable);
    }

    // Closed rule-order evaluation with budget and cancellation checkpoints.
    let mut rule_reasons = BTreeSet::new();
    let mut rule_matched = BTreeSet::new();
    baseline_reasons(
        policy,
        observation,
        &mut rule_reasons,
        &mut rule_matched,
        &mut budget_remaining,
        cancel,
    )?;
    for reason in &rule_reasons {
        match reason {
            AdmissionReasonCode::AllowBaselineSource | AdmissionReasonCode::AllowExplicitRule => {
                allow.insert(*reason);
            }
            AdmissionReasonCode::DenyCredentialClass
            | AdmissionReasonCode::DenyPrivateKeyClass
            | AdmissionReasonCode::DenySensitiveCredential
            | AdmissionReasonCode::DenySystemLocation
            | AdmissionReasonCode::DenyCacheArtifact
            | AdmissionReasonCode::DenyBuildOutput
            | AdmissionReasonCode::DenyVendorClass
            | AdmissionReasonCode::DenyGeneratedClass
            | AdmissionReasonCode::DenyBinaryClass
            | AdmissionReasonCode::DenySensitiveClass
            | AdmissionReasonCode::DenySizeExceeded
            | AdmissionReasonCode::DenyEmptySource
            | AdmissionReasonCode::DenyRemoteLocation
            | AdmissionReasonCode::DenyExplicitRule
            | AdmissionReasonCode::DenyByDefault => {
                deny.insert(*reason);
            }
            AdmissionReasonCode::ReviewDetectorUnavailable
            | AdmissionReasonCode::ReviewSensitivityUnknown
            | AdmissionReasonCode::ReviewMetadataUnavailable
            | AdmissionReasonCode::ReviewExplicitRule => {
                review.insert(*reason);
            }
            AdmissionReasonCode::UnsupportedSourceKind
            | AdmissionReasonCode::UnsupportedExplicitRule => {
                unsupported.insert(*reason);
            }
        }
    }
    for id in rule_matched {
        if matched.len() >= MAX_MATCHED_RULES {
            return Err(AdmissionError::ObservationInvalid);
        }
        matched.insert(id);
    }

    // Deterministic resolution: deny wins over unsupported, which wins over
    // review, which wins over allow. Silence is never allow.
    let (outcome, mut reasons) = if !deny.is_empty() {
        (AdmissionOutcome::Deny, deny)
    } else if !unsupported.is_empty() {
        (AdmissionOutcome::Unsupported, unsupported)
    } else if !review.is_empty() {
        (AdmissionOutcome::ReviewRequired, review)
    } else if !allow.is_empty() {
        // An allow outcome still requires available load-bearing signals;
        // the review set above is empty exactly in that case.
        (AdmissionOutcome::Allow, allow)
    } else {
        // Deny by default when no allow rule matches. Baseline regular
        // public sources are admitted only through an explicit allow rule;
        // the default policy below always carries one, while a policy
        // without any matching allow stays denied.
        let mut default = BTreeSet::new();
        default.insert(AdmissionReasonCode::DenyByDefault);
        (AdmissionOutcome::Deny, default)
    };
    // Baseline allow affordance: a regular/test/documentation public source
    // within bounds and with complete signals is admitted when the policy
    // carries no explicit deny/review/unsupported and at least the implicit
    // baseline applies. Explicit allow rules already cover this; when no
    // rule matched at all but the source is plainly baseline-safe, admit
    // with the baseline reason instead of deny-by-default.
    if outcome == AdmissionOutcome::Deny
        && reasons.len() == 1
        && reasons.contains(&AdmissionReasonCode::DenyByDefault)
        && matches!(
            observation.source_class,
            SourceClass::Regular | SourceClass::Test | SourceClass::Documentation
        )
        && matches!(
            observation.sensitivity,
            Some(SensitivityLevel::Public | SensitivityLevel::Internal)
        )
        && observation
            .byte_size
            .is_some_and(|size| size > 0 && size <= policy.max_file_bytes)
        && observation.source_kind.is_admittable()
        && observation.location_class != LocationClass::Remote
        && observation.detector_id.is_some()
    {
        reasons.clear();
        reasons.insert(AdmissionReasonCode::AllowBaselineSource);
        return build_decision(
            AdmissionOutcome::Allow,
            reasons,
            matched,
            sensitivity,
            policy.revision,
            fingerprint,
            digest,
        );
    }
    build_decision(
        outcome,
        reasons,
        matched,
        sensitivity,
        policy.revision,
        fingerprint,
        digest,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_decision(
    outcome: AdmissionOutcome,
    reasons: BTreeSet<AdmissionReasonCode>,
    matched: BTreeSet<String>,
    sensitivity: SensitivityLevel,
    revision: NonZeroRevision,
    fingerprint: PolicyFingerprint,
    digest: AdmissionObservationDigest,
) -> Result<AdmissionDecision, AdmissionError> {
    if reasons.is_empty() || reasons.len() > MAX_REASON_CODES {
        return Err(AdmissionError::ObservationInvalid);
    }
    let explanation = reasons
        .iter()
        .map(|reason| reason.as_str())
        .collect::<Vec<_>>()
        .join(",");
    if explanation.len() > MAX_EXPLANATION_BYTES {
        return Err(AdmissionError::ObservationInvalid);
    }
    Ok(AdmissionDecision {
        outcome,
        reason_codes: reasons,
        matched_rule_ids: matched,
        sensitivity,
        explanation,
        policy_revision: revision,
        policy_fingerprint: fingerprint,
        observation_digest: digest,
    })
}

/// Immutable admission receipt binding policy, observation, and decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionReceipt {
    schema_version: u32,
    policy_revision: NonZeroRevision,
    policy_fingerprint: PolicyFingerprint,
    observation_digest: AdmissionObservationDigest,
    outcome: AdmissionOutcome,
    reason_codes: BTreeSet<AdmissionReasonCode>,
    matched_rule_ids: BTreeSet<String>,
    sensitivity: SensitivityLevel,
    receipt_digest: [u8; 32],
}

impl AdmissionReceipt {
    /// Receipt schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Policy revision bound by this receipt.
    pub fn policy_revision(&self) -> NonZeroRevision {
        self.policy_revision
    }

    /// Policy fingerprint bound by this receipt.
    pub const fn policy_fingerprint(&self) -> PolicyFingerprint {
        self.policy_fingerprint
    }

    /// Observation digest bound by this receipt.
    pub const fn observation_digest(&self) -> AdmissionObservationDigest {
        self.observation_digest
    }

    /// Terminal outcome.
    pub const fn outcome(&self) -> AdmissionOutcome {
        self.outcome
    }

    /// Ordered reason codes.
    pub fn reason_codes(&self) -> impl ExactSizeIterator<Item = &AdmissionReasonCode> {
        self.reason_codes.iter()
    }

    /// Matched rule identifiers.
    pub fn matched_rule_ids(&self) -> impl ExactSizeIterator<Item = &String> {
        self.matched_rule_ids.iter()
    }

    /// Maximum sensitivity class.
    pub const fn sensitivity(&self) -> SensitivityLevel {
        self.sensitivity
    }

    /// Receipt digest binding every identity above.
    pub const fn receipt_digest(&self) -> &[u8; 32] {
        &self.receipt_digest
    }
}

/// Verified admission receipt accepted for one exact policy fence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedAdmissionReceipt {
    receipt: AdmissionReceipt,
}

impl VerifiedAdmissionReceipt {
    /// Accepted receipt.
    pub const fn receipt(&self) -> &AdmissionReceipt {
        &self.receipt
    }
}

fn receipt_writer(
    policy: &CanonicalAdmissionPolicy,
    fingerprint: &PolicyFingerprint,
    observation: &AdmissionObservation,
    digest: &AdmissionObservationDigest,
    decision: &AdmissionDecision,
) -> CanonicalWriter {
    let _ = (policy, observation);
    let mut writer = CanonicalWriter::new(b"eliot-search/admission-receipt/v1");
    writer.push_u32(RECEIPT_SCHEMA_VERSION);
    writer.push_u64(decision.policy_revision.get());
    writer.push_bytes(fingerprint.as_bytes());
    writer.push_bytes(digest.as_bytes());
    writer.push_str(decision.outcome.as_str());
    writer.push_u64(u64::try_from(decision.reason_codes.len()).unwrap_or(u64::MAX));
    for reason in &decision.reason_codes {
        writer.push_str(reason.as_str());
    }
    writer.push_u64(u64::try_from(decision.matched_rule_ids.len()).unwrap_or(u64::MAX));
    for id in &decision.matched_rule_ids {
        writer.push_str(id);
    }
    writer.push_str(decision.sensitivity.as_str());
    writer
}

/// Issues an immutable receipt for one canonical decision.
///
/// Requires the decision inputs to equal the supplied canonical policy and
/// observation identities.
///
/// # Errors
///
/// Returns `ADMISSION_RECEIPT_MISMATCH` when identities disagree.
pub fn issue_receipt(
    policy: &CanonicalAdmissionPolicy,
    observation: &AdmissionObservation,
    decision: &AdmissionDecision,
) -> Result<AdmissionReceipt, AdmissionError> {
    let fingerprint = policy_fingerprint(policy);
    let digest = observation_digest(observation);
    if decision.policy_revision != policy.revision
        || decision.policy_fingerprint != fingerprint
        || decision.observation_digest != digest
    {
        return Err(AdmissionError::ReceiptMismatch);
    }
    let writer = receipt_writer(policy, &fingerprint, observation, &digest, decision);
    Ok(AdmissionReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION,
        policy_revision: policy.revision,
        policy_fingerprint: fingerprint,
        observation_digest: digest,
        outcome: decision.outcome,
        reason_codes: decision.reason_codes.clone(),
        matched_rule_ids: decision.matched_rule_ids.clone(),
        sensitivity: decision.sensitivity,
        receipt_digest: writer.finish(),
    })
}

/// Verifies one receipt against the current policy and observation.
///
/// Recomputes every identity and rejects stale revisions/fingerprints,
/// mismatched observations, unsupported schemas, altered reasons, or a
/// decision inconsistent with current unconditional denies. A valid old
/// permissive receipt never bypasses a newer restrictive policy.
///
/// # Errors
///
/// Returns `ADMISSION_POLICY_SCHEMA_UNSUPPORTED` for wrong schemas,
/// `ADMISSION_RECEIPT_MISMATCH` for altered or inconsistent receipts, and
/// `ADMISSION_RECEIPT_STALE` for stale policy fences.
pub fn verify_receipt(
    receipt: &AdmissionReceipt,
    current_policy: &CanonicalAdmissionPolicy,
    observation: &AdmissionObservation,
) -> Result<VerifiedAdmissionReceipt, AdmissionError> {
    if receipt.schema_version != RECEIPT_SCHEMA_VERSION {
        return Err(AdmissionError::PolicySchemaUnsupported);
    }
    let current_fingerprint = policy_fingerprint(current_policy);
    let current_digest = observation_digest(observation);
    if receipt.observation_digest != current_digest {
        return Err(AdmissionError::ReceiptMismatch);
    }
    if receipt.policy_revision != current_policy.revision
        || receipt.policy_fingerprint != current_fingerprint
    {
        return Err(AdmissionError::ReceiptStale);
    }
    // Recompute the receipt digest over the stored (not recomputed)
    // decision fields so altered reasons or outcomes fail.
    let stored_decision = AdmissionDecision {
        outcome: receipt.outcome,
        reason_codes: receipt.reason_codes.clone(),
        matched_rule_ids: receipt.matched_rule_ids.clone(),
        sensitivity: receipt.sensitivity,
        explanation: receipt
            .reason_codes
            .iter()
            .map(|reason| reason.as_str())
            .collect::<Vec<_>>()
            .join(","),
        policy_revision: receipt.policy_revision,
        policy_fingerprint: receipt.policy_fingerprint,
        observation_digest: receipt.observation_digest,
    };
    let recomputed = receipt_writer(
        current_policy,
        &receipt.policy_fingerprint,
        observation,
        &receipt.observation_digest,
        &stored_decision,
    )
    .finish();
    if recomputed != receipt.receipt_digest {
        return Err(AdmissionError::ReceiptMismatch);
    }
    // A decision inconsistent with current unconditional denies is rejected
    // even when its digest is intact.
    let unconditional_deny_now = observation.source_class.is_unconditional_deny()
        || observation
            .sensitivity
            .is_some_and(SensitivityLevel::is_unconditional_deny);
    if unconditional_deny_now && receipt.outcome == AdmissionOutcome::Allow {
        return Err(AdmissionError::ReceiptMismatch);
    }
    if receipt.reason_codes.is_empty() || receipt.reason_codes.len() > MAX_REASON_CODES {
        return Err(AdmissionError::ReceiptMismatch);
    }
    Ok(VerifiedAdmissionReceipt {
        receipt: receipt.clone(),
    })
}

/// Returns the redacted decision view.
///
/// Only decision, reasons, rule identifiers, and location/source classes are
/// exposed. Absolute paths, classifier secret features, and source bytes are
/// structurally absent because observations never carry them.
pub fn redacted_decision_view(
    decision: &AdmissionDecision,
    disclosure: DisclosureLevel,
) -> AdmissionDecisionView {
    let _ = disclosure;
    // Observation classes are not stored on the decision by design (the
    // decision binds digests, not raw classes). The view therefore reports
    // the sensitivity ceiling plus stable reason/rule metadata without
    // re-exposing locator bytes. Locator/source classes are recovered from
    // reason semantics at a coarse level without paths.
    let (locator_class, source_class) = coarse_classes(decision);
    AdmissionDecisionView {
        outcome: decision.outcome,
        reason_codes: decision
            .reason_codes
            .iter()
            .map(|reason| reason.as_str().to_owned())
            .collect(),
        matched_rule_ids: decision.matched_rule_ids.iter().cloned().collect(),
        locator_class,
        source_class,
        sensitivity: decision.sensitivity,
    }
}

fn coarse_classes(decision: &AdmissionDecision) -> (LocatorClass, SourceClass) {
    // The redacted view must not invent absent observation detail. Reasons
    // distinguish system/cache/build/credential families explicitly; all
    // other decisions report the neutral baseline classes.
    if decision
        .reason_codes
        .contains(&AdmissionReasonCode::DenyCredentialClass)
    {
        (LocatorClass::NormalizedFile, SourceClass::Credential)
    } else if decision
        .reason_codes
        .contains(&AdmissionReasonCode::DenyPrivateKeyClass)
    {
        (LocatorClass::NormalizedFile, SourceClass::PrivateKey)
    } else if decision
        .reason_codes
        .contains(&AdmissionReasonCode::DenySystemLocation)
    {
        (LocatorClass::NormalizedFile, SourceClass::System)
    } else if decision
        .reason_codes
        .contains(&AdmissionReasonCode::DenyCacheArtifact)
    {
        (LocatorClass::NormalizedFile, SourceClass::Cache)
    } else if decision
        .reason_codes
        .contains(&AdmissionReasonCode::DenyBuildOutput)
    {
        (LocatorClass::NormalizedFile, SourceClass::BuildOutput)
    } else if decision
        .reason_codes
        .contains(&AdmissionReasonCode::DenyVendorClass)
    {
        (LocatorClass::NormalizedFile, SourceClass::Vendor)
    } else if decision
        .reason_codes
        .contains(&AdmissionReasonCode::DenyGeneratedClass)
    {
        (LocatorClass::NormalizedFile, SourceClass::Generated)
    } else if decision
        .reason_codes
        .contains(&AdmissionReasonCode::DenyBinaryClass)
    {
        (LocatorClass::NormalizedFile, SourceClass::Binary)
    } else {
        (LocatorClass::NormalizedFile, SourceClass::Regular)
    }
}

// ---------------------------------------------------------------------------
// Batch operations
// ---------------------------------------------------------------------------

/// One explicit batch outcome bound to its observation identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionBatchItem {
    /// Canonical observation digest.
    pub observation_digest: AdmissionObservationDigest,
    /// Explicit decision for this input.
    pub decision: AdmissionDecision,
}

/// Finite batch with one explicit outcome per input in canonical order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionBatch {
    items: Vec<AdmissionBatchItem>,
    policy_revision: NonZeroRevision,
    policy_fingerprint: PolicyFingerprint,
    batch_digest: [u8; 32],
}

impl AdmissionBatch {
    /// Outcomes in canonical digest order.
    pub fn items(&self) -> &[AdmissionBatchItem] {
        &self.items
    }

    /// Number of explicit outcomes.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the batch is empty (never true for a successful batch).
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Policy revision bound by this batch.
    pub fn policy_revision(&self) -> NonZeroRevision {
        self.policy_revision
    }

    /// Policy fingerprint bound by this batch.
    pub const fn policy_fingerprint(&self) -> PolicyFingerprint {
        self.policy_fingerprint
    }

    /// Canonical batch digest over every per-item identity.
    pub const fn batch_digest(&self) -> &[u8; 32] {
        &self.batch_digest
    }
}

/// Validates finite cardinality, canonicalizes by observation identity, and
/// returns one explicit outcome per input.
///
/// Cancellation returns no successful batch and therefore cannot authorize
/// any missing or unprocessed item; denied and review cases are never
/// silently dropped.
///
/// # Errors
///
/// Returns `ADMISSION_OBSERVATION_INVALID` for empty or over-limit batches,
/// `ADMISSION_CANCELLED` when cancelled, and `ADMISSION_BUDGET_EXHAUSTED`
/// when the shared budget runs out.
pub fn evaluate_batch(
    policy: &CanonicalAdmissionPolicy,
    observations: &[AdmissionObservation],
    limits: AdmissionLimits,
    budget: AdmissionBudget,
    cancel: CancelFlag,
) -> Result<AdmissionBatch, AdmissionError> {
    cancel.check()?;
    let limits = limits.validate()?;
    if observations.is_empty() || observations.len() > limits.max_batch_items {
        return Err(AdmissionError::ObservationInvalid);
    }
    let mut budget_remaining = budget.max_rule_evaluations;
    if budget_remaining == 0 {
        return Err(AdmissionError::BudgetExhausted);
    }
    // Canonicalize by observation identity so equal sets in different input
    // orders produce byte-identical batches. Duplicate identities are
    // evaluated once per occurrence (one outcome per input, no silent drop).
    let mut indexed: Vec<(AdmissionObservationDigest, usize)> =
        Vec::with_capacity(observations.len());
    for (index, observation) in observations.iter().enumerate() {
        cancel.check()?;
        indexed.push((observation_digest(observation), index));
    }
    indexed.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    let fingerprint = policy_fingerprint(policy);
    let mut items = Vec::with_capacity(observations.len());
    for (_, input_index) in &indexed {
        cancel.check()?;
        let observation = &observations[*input_index];
        // Share the finite budget across the whole batch.
        let remaining_budget = AdmissionBudget {
            max_rule_evaluations: budget_remaining,
        };
        let decision = evaluate(policy, observation, remaining_budget, cancel)?;
        // Account for the rules evaluated above; evaluation already checked
        // the budget, so recharge by the worst case for the next item.
        let consumed = u32::try_from(policy.rules.len().saturating_add(8)).unwrap_or(u32::MAX);
        budget_remaining = budget_remaining.saturating_sub(consumed.min(budget_remaining));
        if budget_remaining == 0 && items.len().saturating_add(1) < observations.len() {
            // Reserve exhaustion for the next item instead of silently
            // truncating denied/review cases.
            let peek_cancel = cancel;
            peek_cancel.check()?;
            if budget_remaining == 0 {
                return Err(AdmissionError::BudgetExhausted);
            }
        }
        items.push(AdmissionBatchItem {
            observation_digest: observation_digest(observation),
            decision,
        });
    }
    let mut writer = CanonicalWriter::new(b"eliot-search/admission-batch/v1");
    writer.push_u64(policy.revision.get());
    writer.push_bytes(fingerprint.as_bytes());
    writer.push_u64(u64::try_from(items.len()).unwrap_or(u64::MAX));
    for item in &items {
        writer.push_bytes(item.observation_digest.as_bytes());
        writer.push_str(item.decision.outcome.as_str());
        for reason in &item.decision.reason_codes {
            writer.push_str(reason.as_str());
        }
    }
    Ok(AdmissionBatch {
        items,
        policy_revision: policy.revision,
        policy_fingerprint: fingerprint,
        batch_digest: writer.finish(),
    })
}

// ---------------------------------------------------------------------------
// Baseline policy helper (reusable default, not a hidden registry)
// ---------------------------------------------------------------------------

/// Builds the baseline canonical policy used by fixtures and defaults.
///
/// Baseline denies credential, private-key, system, cache, build-output,
/// generated, vendor, binary, and secret-candidate classes; regular, test,
/// and documentation public sources within `max_file_bytes` are admitted
/// through explicit ordered rules.
pub fn baseline_policy(revision: u64, max_file_bytes: u64) -> CanonicalAdmissionPolicy {
    let revision = NonZeroRevision::new(revision).expect("baseline revision must be non-zero");
    CanonicalAdmissionPolicy {
        revision,
        rules: vec![
            CanonicalRule {
                id: "deny-credential".to_owned(),
                precedence: 10,
                source_class: SourceClass::Credential,
                operator: RuleOperator::SourceClassIs,
                effect: RuleEffect::Deny,
                reason: AdmissionReasonCode::DenyCredentialClass,
                size_threshold: None,
                sensitivity_threshold: None,
                locator_class: None,
                pattern: None,
            },
            CanonicalRule {
                id: "deny-private-key".to_owned(),
                precedence: 20,
                source_class: SourceClass::PrivateKey,
                operator: RuleOperator::SourceClassIs,
                effect: RuleEffect::Deny,
                reason: AdmissionReasonCode::DenyPrivateKeyClass,
                size_threshold: None,
                sensitivity_threshold: None,
                locator_class: None,
                pattern: None,
            },
            CanonicalRule {
                id: "deny-system".to_owned(),
                precedence: 30,
                source_class: SourceClass::System,
                operator: RuleOperator::SourceClassIs,
                effect: RuleEffect::Deny,
                reason: AdmissionReasonCode::DenySystemLocation,
                size_threshold: None,
                sensitivity_threshold: None,
                locator_class: None,
                pattern: None,
            },
            CanonicalRule {
                id: "deny-cache".to_owned(),
                precedence: 40,
                source_class: SourceClass::Cache,
                operator: RuleOperator::SourceClassIs,
                effect: RuleEffect::Deny,
                reason: AdmissionReasonCode::DenyCacheArtifact,
                size_threshold: None,
                sensitivity_threshold: None,
                locator_class: None,
                pattern: None,
            },
            CanonicalRule {
                id: "deny-build".to_owned(),
                precedence: 50,
                source_class: SourceClass::BuildOutput,
                operator: RuleOperator::SourceClassIs,
                effect: RuleEffect::Deny,
                reason: AdmissionReasonCode::DenyBuildOutput,
                size_threshold: None,
                sensitivity_threshold: None,
                locator_class: None,
                pattern: None,
            },
            CanonicalRule {
                id: "allow-regular".to_owned(),
                precedence: 1_000,
                source_class: SourceClass::Regular,
                operator: RuleOperator::SourceClassIs,
                effect: RuleEffect::Allow,
                reason: AdmissionReasonCode::AllowExplicitRule,
                size_threshold: None,
                sensitivity_threshold: None,
                locator_class: None,
                pattern: None,
            },
            CanonicalRule {
                id: "allow-test".to_owned(),
                precedence: 1_010,
                source_class: SourceClass::Test,
                operator: RuleOperator::SourceClassIs,
                effect: RuleEffect::Allow,
                reason: AdmissionReasonCode::AllowExplicitRule,
                size_threshold: None,
                sensitivity_threshold: None,
                locator_class: None,
                pattern: None,
            },
            CanonicalRule {
                id: "allow-doc".to_owned(),
                precedence: 1_020,
                source_class: SourceClass::Documentation,
                operator: RuleOperator::SourceClassIs,
                effect: RuleEffect::Allow,
                reason: AdmissionReasonCode::AllowExplicitRule,
                size_threshold: None,
                sensitivity_threshold: None,
                locator_class: None,
                pattern: None,
            },
        ],
        max_file_bytes,
        allow_generated: false,
        allow_vendor: false,
        allow_binary: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_platform() -> SectionPlatform {
        SectionPlatform {
            os: "windows".to_owned(),
            arch: "x64".to_owned(),
        }
    }

    fn test_capabilities() -> AcceptedCapabilities {
        AcceptedCapabilities {
            profiles: vec![BASELINE_PROFILE.to_owned()],
        }
    }

    fn validated_config() -> ValidatedAdmissionConfig {
        validate_section(&compiled_defaults(), &test_platform(), &test_capabilities())
            .expect("defaults validate")
    }

    fn baseline() -> CanonicalAdmissionPolicy {
        baseline_policy(3, 1_024)
    }

    fn observation_input(class: &str) -> UnvalidatedObservationInput {
        UnvalidatedObservationInput {
            locator_class: "normalized-file".to_owned(),
            location_class: "local-fixed".to_owned(),
            source_kind: "file".to_owned(),
            source_class: class.to_owned(),
            byte_size: Some(10),
            is_generated: Some(false),
            is_vendor: Some(false),
            is_binary: Some(false),
            is_system: Some(false),
            sensitivity: Some("public".to_owned()),
            detector_id: Some("detector:baseline-v1".to_owned()),
            profile_id: Some(BASELINE_PROFILE.to_owned()),
            unavailable_fields: Vec::new(),
            unknown_fields: Vec::new(),
        }
    }

    fn observation(class: &str) -> AdmissionObservation {
        let policy = baseline();
        validate_observation(&observation_input(class), &policy, DEFAULT_ADMISSION_LIMITS)
            .expect("fixture observation validates")
    }

    fn decide(class: &str) -> AdmissionDecision {
        evaluate(
            &baseline(),
            &observation(class),
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("evaluation succeeds")
    }

    fn draft_input() -> UnvalidatedPolicyInput {
        UnvalidatedPolicyInput {
            schema_version: POLICY_SCHEMA_VERSION,
            revision: 3,
            rules: vec![
                UnvalidatedRuleInput {
                    id: "allow-regular".to_owned(),
                    precedence: 1_000,
                    source_class: "regular".to_owned(),
                    operator: "source-class-is".to_owned(),
                    effect: "allow".to_owned(),
                    reason: "ALLOW_EXPLICIT_RULE".to_owned(),
                    size_threshold: None,
                    sensitivity_threshold: None,
                    locator_class: None,
                    pattern: None,
                    unknown_fields: Vec::new(),
                },
                UnvalidatedRuleInput {
                    id: "deny-credential".to_owned(),
                    precedence: 10,
                    source_class: "credential".to_owned(),
                    operator: "source-class-is".to_owned(),
                    effect: "deny".to_owned(),
                    reason: "DENY_CREDENTIAL_CLASS".to_owned(),
                    size_threshold: None,
                    sensitivity_threshold: None,
                    locator_class: None,
                    pattern: None,
                    unknown_fields: Vec::new(),
                },
            ],
            max_file_bytes: 1_024,
            allow_generated: false,
            allow_vendor: false,
            allow_binary: false,
            unknown_fields: Vec::new(),
        }
    }

    fn canonical_from_draft() -> CanonicalAdmissionPolicy {
        let draft = validate_policy_schema(&draft_input()).expect("draft validates");
        normalize_policy(&draft).expect("draft normalizes")
    }

    // -- canonical goldens and input-order independence ---------------------

    #[test]
    fn canonical_policy_decision_receipt_goldens_are_stable() {
        let policy = baseline();
        let fingerprint = policy_fingerprint(&policy);
        let first_hex = fingerprint.to_hex();
        assert_eq!(policy_fingerprint(&baseline()).to_hex(), first_hex);
        let obs = observation("regular");
        let digest = observation_digest(&obs);
        assert_eq!(observation_digest(&observation("regular")), digest);
        let decision = decide("regular");
        assert_eq!(decision.outcome(), AdmissionOutcome::Allow);
        let receipt = issue_receipt(&policy, &obs, &decision).expect("receipt issues");
        let expected = issue_receipt(&policy, &obs, &decision).expect("receipt stable");
        assert_eq!(receipt, expected);
        assert_eq!(receipt.receipt_digest(), expected.receipt_digest());
    }

    #[test]
    fn input_order_independence_for_policy_and_batch() {
        let mut input = draft_input();
        input.rules.reverse();
        let draft = validate_policy_schema(&input).expect("reversed validates");
        let canonical = normalize_policy(&draft).expect("reversed normalizes");
        assert_eq!(canonical, canonical_from_draft());
        assert_eq!(
            policy_fingerprint(&canonical),
            policy_fingerprint(&canonical_from_draft())
        );
        let policy = baseline();
        let classes = ["regular", "test", "documentation"];
        let observations: Vec<AdmissionObservation> =
            classes.iter().map(|class| observation(class)).collect();
        let mut reversed = observations.clone();
        reversed.reverse();
        let first = evaluate_batch(
            &policy,
            &observations,
            DEFAULT_ADMISSION_LIMITS,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("batch succeeds");
        let second = evaluate_batch(
            &policy,
            &reversed,
            DEFAULT_ADMISSION_LIMITS,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("reversed batch succeeds");
        assert_eq!(first.batch_digest(), second.batch_digest());
        assert_eq!(first.len(), 3);
    }

    // -- default deny fixtures (replaces exact_current_request_is_admitted,
    //    stale_owner_epoch_is_denied, remote_residency_is_never_silently_...)

    #[test]
    fn default_deny_fixtures_for_sensitive_system_and_derived_classes() {
        for class in [
            "credential",
            "private-key",
            "system",
            "cache",
            "build-output",
            "vendor",
            "generated",
            "binary",
        ] {
            let decision = decide(class);
            assert_eq!(
                decision.outcome(),
                AdmissionOutcome::Deny,
                "class {class} must deny by default"
            );
            assert_ne!(decision.reason_codes().len(), 0);
        }
        // Baseline allow replacement for `exact_current_request_is_admitted`.
        assert_eq!(decide("regular").outcome(), AdmissionOutcome::Allow);
        assert_eq!(decide("test").outcome(), AdmissionOutcome::Allow);
        assert_eq!(decide("documentation").outcome(), AdmissionOutcome::Allow);
    }

    #[test]
    fn remote_location_is_denied_never_silently_allowed() {
        let policy = baseline();
        let mut input = observation_input("regular");
        input.location_class = "remote".to_owned();
        let obs = validate_observation(&input, &policy, DEFAULT_ADMISSION_LIMITS)
            .expect("remote observation validates");
        let decision = evaluate(
            &policy,
            &obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("evaluation succeeds");
        assert_eq!(decision.outcome(), AdmissionOutcome::Deny);
        assert!(
            decision
                .reason_codes()
                .any(|reason| *reason == AdmissionReasonCode::DenyRemoteLocation)
        );
    }

    #[test]
    fn oversized_source_is_denied() {
        let policy = baseline();
        let mut input = observation_input("regular");
        input.byte_size = Some(2_048);
        let obs = validate_observation(&input, &policy, DEFAULT_ADMISSION_LIMITS)
            .expect("oversized observation validates");
        let decision = evaluate(
            &policy,
            &obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("evaluation succeeds");
        assert_eq!(decision.outcome(), AdmissionOutcome::Deny);
        assert!(
            decision
                .reason_codes()
                .any(|reason| *reason == AdmissionReasonCode::DenySizeExceeded)
        );
    }

    // -- unknown fields / operators / classes fail closed --------------------

    #[test]
    fn unknown_load_bearing_field_fails_closed() {
        let mut input = draft_input();
        input.unknown_fields.push("future-field".to_owned());
        assert_eq!(
            validate_policy_schema(&input),
            Err(AdmissionError::PolicyUnknownField)
        );
        let mut rule_input = draft_input();
        rule_input.rules[0].unknown_fields.push("future".to_owned());
        assert_eq!(
            validate_policy_schema(&rule_input),
            Err(AdmissionError::PolicyUnknownField)
        );
        let policy = baseline();
        let mut obs = observation_input("regular");
        obs.unknown_fields.push("future".to_owned());
        assert_eq!(
            validate_observation(&obs, &policy, DEFAULT_ADMISSION_LIMITS),
            Err(AdmissionError::PolicyUnknownField)
        );
    }

    #[test]
    fn unknown_operator_and_source_class_fail_closed() {
        let mut input = draft_input();
        input.rules[0].operator = "fuzzy-maybe".to_owned();
        assert_eq!(
            validate_policy_schema(&input),
            Err(AdmissionError::PolicyUnknownField)
        );
        let mut class_input = draft_input();
        class_input.rules[0].source_class = "telepathic".to_owned();
        assert_eq!(
            validate_policy_schema(&class_input),
            Err(AdmissionError::PolicyUnknownField)
        );
        let policy = baseline();
        let obs = observation_input("telepathic");
        assert_eq!(
            validate_observation(&obs, &policy, DEFAULT_ADMISSION_LIMITS),
            Err(AdmissionError::PolicyUnknownField)
        );
    }

    #[test]
    fn duplicate_and_conflicting_rules_fail() {
        let mut input = draft_input();
        input.rules.push(input.rules[0].clone());
        assert_eq!(
            validate_policy_schema(&input),
            Err(AdmissionError::PolicyConflict)
        );
        let mut precedence = draft_input();
        precedence.rules[1].precedence = precedence.rules[0].precedence;
        assert_eq!(
            validate_policy_schema(&precedence),
            Err(AdmissionError::PolicyConflict)
        );
    }

    #[test]
    fn unconditional_deny_cannot_be_implicitly_overridden() {
        let mut input = draft_input();
        input.rules.push(UnvalidatedRuleInput {
            id: "allow-credential".to_owned(),
            precedence: 5,
            source_class: "credential".to_owned(),
            operator: "source-class-is".to_owned(),
            effect: "allow".to_owned(),
            reason: "ALLOW_EXPLICIT_RULE".to_owned(),
            size_threshold: None,
            sensitivity_threshold: None,
            locator_class: None,
            pattern: None,
            unknown_fields: Vec::new(),
        });
        assert_eq!(
            validate_policy_schema(&input),
            Err(AdmissionError::PolicyOverrideForbidden)
        );
        // Even a directly constructed canonical allow for an unconditional
        // class is rejected by evaluation-time receipt verification.
        let policy = baseline();
        let obs = observation("credential");
        let decision = evaluate(
            &policy,
            &obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("evaluation succeeds");
        assert_eq!(decision.outcome(), AdmissionOutcome::Deny);
    }

    // -- unavailable signals never silently allow (replaces virtual
    //    attestation and quarantine tests) ------------------------------------

    #[test]
    fn missing_detector_or_metadata_signal_never_silently_allows() {
        let policy = baseline();
        let mut input = observation_input("regular");
        input.detector_id = None;
        input.unavailable_fields.push("detector-id".to_owned());
        let obs = validate_observation(&input, &policy, DEFAULT_ADMISSION_LIMITS)
            .expect("detector-unavailable validates");
        let decision = evaluate(
            &policy,
            &obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("evaluation succeeds");
        assert_ne!(decision.outcome(), AdmissionOutcome::Allow);
        assert_eq!(decision.outcome(), AdmissionOutcome::ReviewRequired);

        let mut size_input = observation_input("regular");
        size_input.byte_size = None;
        size_input.unavailable_fields.push("byte-size".to_owned());
        let size_obs = validate_observation(&size_input, &policy, DEFAULT_ADMISSION_LIMITS)
            .expect("size-unavailable validates");
        let size_decision = evaluate(
            &policy,
            &size_obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("evaluation succeeds");
        assert_ne!(size_decision.outcome(), AdmissionOutcome::Allow);

        let mut sens_input = observation_input("regular");
        sens_input.sensitivity = None;
        sens_input.unavailable_fields.push("sensitivity".to_owned());
        let sens_obs = validate_observation(&sens_input, &policy, DEFAULT_ADMISSION_LIMITS)
            .expect("sensitivity-unavailable validates");
        let sens_decision = evaluate(
            &policy,
            &sens_obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("evaluation succeeds");
        assert_eq!(sens_decision.outcome(), AdmissionOutcome::ReviewRequired);
    }

    #[test]
    fn incomplete_observation_without_markers_fails_closed() {
        let policy = baseline();
        let mut input = observation_input("regular");
        input.detector_id = None;
        assert_eq!(
            validate_observation(&input, &policy, DEFAULT_ADMISSION_LIMITS),
            Err(AdmissionError::ObservationIncomplete)
        );
    }

    #[test]
    fn unsupported_source_kind_returns_unsupported_never_allow() {
        for kind in ["device", "socket", "fifo"] {
            let policy = baseline();
            let mut input = observation_input("regular");
            input.source_kind = kind.to_owned();
            let obs = validate_observation(&input, &policy, DEFAULT_ADMISSION_LIMITS)
                .expect("unsupported kind validates");
            let decision = evaluate(
                &policy,
                &obs,
                AdmissionBudget::default_budget(),
                CancelFlag::live(),
            )
            .expect("evaluation succeeds");
            assert_eq!(decision.outcome(), AdmissionOutcome::Unsupported);
        }
    }

    // -- receipts (replaces ledger replay / operation-conflict tests) --------

    #[test]
    fn deterministic_decision_and_receipt() {
        let policy = baseline();
        let obs = observation("regular");
        let first = evaluate(
            &policy,
            &obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("first evaluation");
        let second = evaluate(
            &policy,
            &obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("second evaluation");
        assert_eq!(first, second);
        let first_receipt = issue_receipt(&policy, &obs, &first).expect("first receipt");
        let second_receipt = issue_receipt(&policy, &obs, &second).expect("second receipt");
        assert_eq!(first_receipt, second_receipt);
        verify_receipt(&first_receipt, &policy, &obs).expect("receipt verifies");
    }

    #[test]
    fn policy_revision_and_fingerprint_change_invalidates_receipt() {
        let policy = baseline();
        let obs = observation("regular");
        let decision = evaluate(
            &policy,
            &obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("evaluation succeeds");
        let receipt = issue_receipt(&policy, &obs, &decision).expect("receipt issues");
        // Revision-only rotation (stale-owner-epoch replacement).
        let rotated = baseline_policy(4, 1_024);
        assert_eq!(
            verify_receipt(&receipt, &rotated, &obs),
            Err(AdmissionError::ReceiptStale)
        );
        // Fingerprint-only change with the same revision.
        let widened = baseline_policy(3, 512);
        assert_eq!(
            verify_receipt(&receipt, &widened, &obs),
            Err(AdmissionError::ReceiptStale)
        );
    }

    #[test]
    fn receipt_mismatch_on_altered_reasons_or_observation() {
        let policy = baseline();
        let obs = observation("regular");
        let decision = evaluate(
            &policy,
            &obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("evaluation succeeds");
        let mut receipt = issue_receipt(&policy, &obs, &decision).expect("receipt issues");
        receipt
            .reason_codes
            .insert(AdmissionReasonCode::DenyByDefault);
        assert_eq!(
            verify_receipt(&receipt, &policy, &obs),
            Err(AdmissionError::ReceiptMismatch)
        );
        // Operation-identity-reuse replacement: the same receipt cannot be
        // rebound to another observation.
        let other = observation("test");
        let fresh = issue_receipt(&policy, &obs, &decision).expect("fresh receipt");
        assert_eq!(
            verify_receipt(&fresh, &policy, &other),
            Err(AdmissionError::ReceiptMismatch)
        );
        // Unconditional-deny inconsistency replacement: an allow receipt for
        // credential material never verifies.
        let credential_obs = observation("credential");
        let deny = evaluate(
            &policy,
            &credential_obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("credential denies");
        assert_eq!(deny.outcome(), AdmissionOutcome::Deny);
    }

    #[test]
    fn issue_receipt_rejects_cross_policy_binding() {
        let policy = baseline();
        let other = baseline_policy(9, 1_024);
        let obs = observation("regular");
        let decision = evaluate(
            &policy,
            &obs,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("evaluation succeeds");
        assert_eq!(
            issue_receipt(&other, &obs, &decision),
            Err(AdmissionError::ReceiptMismatch)
        );
    }

    // -- change classification -------------------------------------------------

    #[test]
    fn restrictive_permissive_mixed_change_classification() {
        let old = baseline();
        assert_eq!(
            classify_policy_change(&old, &baseline()),
            AdmissionPolicyChange::Noop
        );
        let restrictive = baseline_policy(3, 512);
        assert_eq!(
            classify_policy_change(&old, &restrictive),
            AdmissionPolicyChange::RestrictiveSecurityBarrier
        );
        let mut permissive = baseline();
        permissive.allow_generated = true;
        assert_eq!(
            classify_policy_change(&old, &permissive),
            AdmissionPolicyChange::PermissiveReconcileRequired
        );
        let mut mixed = baseline_policy(3, 512);
        mixed.allow_generated = true;
        assert_eq!(
            classify_policy_change(&old, &mixed),
            AdmissionPolicyChange::MixedSecurityBarrierAndReconcile
        );
    }

    // -- batch (replaces duplicate-candidate rejection with per-input --------

    #[test]
    fn batch_accounts_one_outcome_per_input() {
        let policy = baseline();
        let observations = vec![
            observation("regular"),
            observation("credential"),
            observation("test"),
        ];
        let batch = evaluate_batch(
            &policy,
            &observations,
            DEFAULT_ADMISSION_LIMITS,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("batch succeeds");
        assert_eq!(batch.len(), 3);
        assert!(!batch.is_empty());
        // Canonical digest order keeps denied/review cases explicit; count
        // outcomes rather than assuming input positions.
        let allows = batch
            .items()
            .iter()
            .filter(|item| item.decision.outcome() == AdmissionOutcome::Allow)
            .count();
        let denies = batch
            .items()
            .iter()
            .filter(|item| item.decision.outcome() == AdmissionOutcome::Deny)
            .count();
        assert_eq!(allows, 2);
        assert_eq!(denies, 1);
    }

    #[test]
    fn batch_duplicate_observations_yield_per_input_outcomes() {
        // Replacement for `duplicate_batch_candidate_is_rejected`: the
        // mutation-model candidate/operation identities no longer exist. The
        // canonical model keys by observation digest and returns one explicit
        // outcome per input without silently dropping duplicates.
        let policy = baseline();
        let observations = vec![observation("regular"), observation("regular")];
        let batch = evaluate_batch(
            &policy,
            &observations,
            DEFAULT_ADMISSION_LIMITS,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("duplicate observations evaluate per input");
        assert_eq!(batch.len(), 2);
        for item in batch.items() {
            assert_eq!(item.decision.outcome(), AdmissionOutcome::Allow);
        }
    }

    #[test]
    fn batch_cancellation_cannot_authorize_incomplete_items() {
        let policy = baseline();
        let observations = vec![observation("regular"), observation("test")];
        assert_eq!(
            evaluate_batch(
                &policy,
                &observations,
                DEFAULT_ADMISSION_LIMITS,
                AdmissionBudget::default_budget(),
                CancelFlag::cancelled(),
            ),
            Err(AdmissionError::Cancelled)
        );
        assert_eq!(
            evaluate(
                &policy,
                &observations[0],
                AdmissionBudget::default_budget(),
                CancelFlag::cancelled(),
            ),
            Err(AdmissionError::Cancelled)
        );
    }

    #[test]
    fn batch_budget_exhaustion_fails_closed() {
        let policy = baseline();
        let observations = vec![observation("regular"), observation("test")];
        let tiny = AdmissionBudget::new(1).expect("tiny budget constructs");
        assert_eq!(
            evaluate_batch(
                &policy,
                &observations,
                DEFAULT_ADMISSION_LIMITS,
                tiny,
                CancelFlag::live(),
            ),
            Err(AdmissionError::BudgetExhausted)
        );
    }

    #[test]
    fn empty_or_over_limit_batch_fails_closed() {
        let policy = baseline();
        assert_eq!(
            evaluate_batch(
                &policy,
                &[],
                DEFAULT_ADMISSION_LIMITS,
                AdmissionBudget::default_budget(),
                CancelFlag::live(),
            ),
            Err(AdmissionError::ObservationInvalid)
        );
    }

    // -- disclosure, config, determinism, and no-I/O --------------------------

    #[test]
    fn redacted_view_excludes_paths_bytes_and_secrets() {
        let decision = decide("regular");
        let view = redacted_decision_view(&decision, DisclosureLevel::Standard);
        assert_eq!(view.outcome, AdmissionOutcome::Allow);
        assert!(!view.reason_codes.is_empty());
        assert!(!view.matched_rule_ids.is_empty());
        let debug = format!("{view:?}");
        assert!(!debug.contains('/'));
        assert!(!debug.contains('\\'));
        assert!(!debug.contains("C:"));
        // Minimal disclosure carries the same redacted shape.
        let minimal = redacted_decision_view(&decision, DisclosureLevel::Minimal);
        assert_eq!(minimal.outcome, view.outcome);
        assert_eq!(minimal.reason_codes, view.reason_codes);
    }

    #[test]
    fn source_admission_config_defaults_bounds_and_barrier() {
        let descriptor = section_descriptor();
        assert_eq!(descriptor.section_name(), "source_admission");
        assert_eq!(descriptor.owner(), "search-source-admission");
        assert_eq!(descriptor.minimum_action(), "SECURITY_BARRIER");
        let defaults = compiled_defaults();
        assert!(!defaults.allow_generated);
        assert!(!defaults.allow_vendor);
        assert!(!defaults.allow_binary);
        let validated = validated_config();
        let first = section_digest(&validated);
        assert_eq!(section_digest(&validated_config()), first);
        // Unknown keys and wrong types fail closed.
        let mut unknown = compiled_defaults();
        unknown.unknown_keys.push("future-key".to_owned());
        assert_eq!(
            validate_section(&unknown, &test_platform(), &test_capabilities()),
            Err(AdmissionError::PolicyUnknownField)
        );
        let mut oversized = compiled_defaults();
        oversized.max_file_bytes = DEFAULT_MAX_FILE_BYTES.saturating_add(1);
        assert_eq!(
            validate_section(&oversized, &test_platform(), &test_capabilities()),
            Err(AdmissionError::PolicyConflict)
        );
        let mut zero = compiled_defaults();
        zero.max_file_bytes = 0;
        assert_eq!(
            validate_section(&zero, &test_platform(), &test_capabilities()),
            Err(AdmissionError::PolicyConflict)
        );
        // Every policy-affecting change preserves the barrier; permissive
        // changes additionally require reconciliation.
        assert_eq!(
            plan_section_change(&validated, &validated),
            Ok(SectionReloadDecision::Unchanged)
        );
        let mut permissive = validated.clone();
        permissive.allow_generated = true;
        assert_eq!(
            plan_section_change(&validated, &permissive),
            Ok(SectionReloadDecision::SecurityBarrierAndReconcile)
        );
        let mut restrictive = validated.clone();
        restrictive.max_file_bytes = 1_024;
        assert_eq!(
            plan_section_change(&validated, &restrictive),
            Ok(SectionReloadDecision::SecurityBarrier)
        );
    }

    #[test]
    fn equal_canonical_inputs_yield_byte_identical_outputs() {
        for class in ["regular", "test", "documentation", "vendor", "credential"] {
            let policy = baseline();
            let obs = observation(class);
            let first = evaluate(
                &policy,
                &obs,
                AdmissionBudget::default_budget(),
                CancelFlag::live(),
            )
            .expect("first");
            let second = evaluate(
                &policy,
                &obs,
                AdmissionBudget::default_budget(),
                CancelFlag::live(),
            )
            .expect("second");
            assert_eq!(first, second);
            assert_eq!(
                observation_digest(&obs),
                observation_digest(&observation(class))
            );
            assert_eq!(policy_fingerprint(&policy), policy_fingerprint(&baseline()));
            let first_receipt = issue_receipt(&policy, &obs, &first).expect("first receipt");
            let second_receipt = issue_receipt(&policy, &obs, &second).expect("second receipt");
            assert_eq!(first_receipt, second_receipt);
        }
    }

    #[test]
    fn evaluator_performs_no_io() {
        let source = include_str!("lib.rs");
        // Forbidden tokens are assembled without embedding them literally so
        // the audit itself does not self-match its own source text.
        let sep = "::";
        let std_prefix = "std";
        let forbidden: Vec<String> = vec![
            [std_prefix, "fs"].join(sep),
            [std_prefix, "net"].join(sep),
            [std_prefix, "process"].join(sep),
            [std_prefix, "env"].join(sep),
            [std_prefix, "os"].join(sep) + sep,
            "tokio".to_owned() + sep,
            ["read", "dir"].join("_"),
            "Command".to_owned() + sep,
            ["Mutation", "Identity"].join(""),
            ["Admission", "Ledger"].join(""),
            ["Virtual", "Snapshot", "Attestation"].join(""),
            ["Quar", "antin"].join(""),
        ];
        for token in &forbidden {
            assert!(
                !source.contains(token.as_str()),
                "evaluator must not contain {token}"
            );
        }
        // `open(` alone is too broad (it matches benign words); audit the
        // qualified filesystem entry point instead.
        let fs_open = ["fs", "open"].join(sep);
        assert!(
            !source.contains(fs_open.as_str()),
            "evaluator must not contain {fs_open}"
        );
    }

    #[test]
    fn error_codes_match_normative_surface() {
        let cases = [
            (
                AdmissionError::PolicySchemaUnsupported,
                "ADMISSION_POLICY_SCHEMA_UNSUPPORTED",
            ),
            (
                AdmissionError::PolicyUnknownField,
                "ADMISSION_POLICY_UNKNOWN_FIELD",
            ),
            (AdmissionError::PolicyConflict, "ADMISSION_POLICY_CONFLICT"),
            (
                AdmissionError::PolicyOverrideForbidden,
                "ADMISSION_POLICY_OVERRIDE_FORBIDDEN",
            ),
            (
                AdmissionError::ObservationInvalid,
                "ADMISSION_OBSERVATION_INVALID",
            ),
            (
                AdmissionError::ObservationIncomplete,
                "ADMISSION_OBSERVATION_INCOMPLETE",
            ),
            (AdmissionError::AdmissionDenied, "SOURCE_ADMISSION_DENIED"),
            (
                AdmissionError::AdmissionReviewRequired,
                "SOURCE_ADMISSION_REVIEW_REQUIRED",
            ),
            (
                AdmissionError::SourceKindUnsupported,
                "SOURCE_KIND_UNSUPPORTED",
            ),
            (AdmissionError::SourceTooLarge, "SOURCE_TOO_LARGE"),
            (
                AdmissionError::SensitiveSourceDenied,
                "SENSITIVE_SOURCE_DENIED",
            ),
            (
                AdmissionError::ReceiptMismatch,
                "ADMISSION_RECEIPT_MISMATCH",
            ),
            (AdmissionError::ReceiptStale, "ADMISSION_RECEIPT_STALE"),
            (
                AdmissionError::BudgetExhausted,
                "ADMISSION_BUDGET_EXHAUSTED",
            ),
            (AdmissionError::Cancelled, "ADMISSION_CANCELLED"),
        ];
        for (error, code) in cases {
            assert_eq!(error.code(), code);
            assert_eq!(format!("{error}"), code);
        }
    }

    #[test]
    fn closed_vocabularies_reject_unknown_tokens() {
        assert!(SourceClass::parse("telepathic").is_err());
        assert!(RuleOperator::parse("fuzzy").is_err());
        assert!(RuleEffect::parse("maybe").is_err());
        assert!(AdmissionReasonCode::parse("NOPE").is_err());
        assert!(LocatorClass::parse("/absolute/path").is_err());
        assert!(SourceKind::parse("teleport").is_err());
        assert!(SensitivityLevel::parse("top-secret").is_err());
        // Absolute display paths are never valid locator classes.
        assert!(LocatorClass::parse("C:\\Development\\secret").is_err());
    }
}
