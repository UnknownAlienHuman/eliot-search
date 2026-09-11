//! Bounded port of pure `tools/context_materialization_planner_v1/core.py` helpers
//! (T41, context-materialization slice).
//!
//! Covers only deterministic, IO-free helpers: closed constants, authority
//! ceiling, domain-separated digests (`plan_digest` / `operation_id`),
//! scalar grammars (`require_sha` / `require_opaque` / `require_actor` /
//! `require_rfc3339` / `require_u64`), the advisory output-root grammar
//! (pure prefix of `validate_output_root`) and the bundle-independent
//! reference validators (`validate_artifact_ref` /
//! `validate_optional_signature`).
//!
//! Filesystem reads/writes (`load_json_file`, symlink checks,
//! `write_artifact`), candidate assembly (`validate_candidate`, shared
//! `context_artifact_builder_v1` lib), TOML rendering
//! (`manifest.py`: `render_signed_payload`, handoff projection) and plan
//! assembly (`plan.py`: `build_plan` / `write_plan`, both entrypoints)
//! remain Python-owned (see report).

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// `SCHEMA_VERSION`: materialization plan schema version.
pub const SCHEMA_VERSION: i64 = 1;
/// `RECORD_KIND`: plan record kind.
pub const RECORD_KIND: &str = "context_materialization_plan_v1";
/// `STATUS`: plans are advisory and never authoritative.
pub const STATUS: &str = "ADVISORY_NON_AUTHORITATIVE";
/// `PLAN_ROOT`: sole writable plan output directory.
pub const PLAN_ROOT: &str = "artifacts/context-materialization-plans";
/// Domain separator for `plan_digest` (includes trailing NUL).
pub const PLAN_DOMAIN: &[u8] = b"eliot-search/context-materialization-plan/v1\0";
/// Domain separator for `operation_id` (includes trailing NUL).
pub const OPERATION_DOMAIN: &[u8] = b"eliot-search/materialize-context/v1\0";
/// `INSTANCE_STATUS`: prospective manifest instance status.
pub const INSTANCE_STATUS: &str = "MATERIALIZED";
/// `REPOSITORY`: pinned source repository.
pub const REPOSITORY: &str = "UnknownAlienHuman/eliot-search";

/// `DECISION_MISSING`: no external selection input.
pub const DECISION_MISSING: &str = "BLOCKED_MISSING_EXTERNAL_INPUT";
/// `DECISION_SIGNATURES`: payload ready, signatures absent.
pub const DECISION_SIGNATURES: &str = "READY_FOR_DUAL_SIGNATURE_COLLECTION";
/// `DECISION_COMMIT`: both signatures present, ready for owner readback.
pub const DECISION_COMMIT: &str = "READY_FOR_INTEGRATION_OWNER_READBACK_AND_COMMIT";
/// `DECISION_PARTIAL_SIGNATURE`: exactly one signature present.
pub const DECISION_PARTIAL_SIGNATURE: &str = "BLOCKED_PARTIAL_SIGNATURE_SET";

/// `REASON_MISSING_SELECTION`: selection-input reason code.
pub const REASON_MISSING_SELECTION: &str = "MATERIALIZATION_SELECTION_MISSING";
/// `REASON_PARTIAL_SIGNATURE`: partial-signature reason code.
pub const REASON_PARTIAL_SIGNATURE: &str = "MATERIALIZATION_SIGNATURE_SET_PARTIAL";

/// Authority ceiling fields (order-pinned, always `false`).
pub const AUTHORITY_FIELDS: [&str; 9] = [
    "materializes_authoritative_context",
    "creates_context_manifest_record",
    "creates_immutable_artifact_ref",
    "creates_assignment_ticket",
    "creates_writer_lease",
    "authorizes_implementation",
    "publishes_package_handoff",
    "accepts_gate_or_wave",
    "advances_launch_state",
];

/// Typed failure carrying the machine `reason_code`
/// (`MaterializationPlanError` in Python).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationPlanError {
    reason: &'static str,
    message: String,
}

impl MaterializationPlanError {
    fn new(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    /// Machine reason code (closed registry).
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl Display for MaterializationPlanError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}: {}", self.reason, self.message)
    }
}

impl Error for MaterializationPlanError {}

/// `authority_map`: all-false authority ceiling object.
#[must_use]
pub fn authority_map() -> Value {
    let mut map = serde_json::Map::new();
    for field in AUTHORITY_FIELDS {
        map.insert(field.to_owned(), Value::Bool(false));
    }
    Value::Object(map)
}

/// `plan_digest`: SHA-256 over the plan domain plus canonical bytes.
#[must_use]
pub fn plan_digest(value_without_digest: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PLAN_DOMAIN);
    hasher.update(crate::ticket_planner::canonical_json_bytes(
        value_without_digest,
    ));
    format!("{:x}", hasher.finalize())
}

/// `operation_id`: SHA-256 over the operation domain plus canonical bytes.
#[must_use]
pub fn operation_id(input_manifest: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(OPERATION_DOMAIN);
    hasher.update(crate::ticket_planner::canonical_json_bytes(input_manifest));
    format!("{:x}", hasher.finalize())
}

/// `SHA256_RE`: lowercase hex digest grammar.
#[must_use]
pub fn sha256_hex_valid(value: &str) -> bool {
    crate::ticket_planner::sha256_hex_valid(value)
}

/// `OPAQUE_RE`: `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`.
#[must_use]
pub fn opaque_id_valid(value: &str) -> bool {
    crate::ticket_planner::opaque_id_valid(value)
}

/// `ACTOR_RE`: `actor:(user|service|reviewer|integration):<opaque>`.
#[must_use]
pub fn actor_identity_valid(value: &str) -> bool {
    crate::ticket_planner::actor_identity_valid(value)
}

const RFC3339_DIGIT_POSITIONS: [usize; 14] = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];

const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

const fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// `RFC3339_RE` shape plus calendar check (whole-second UTC, `...Z`).
///
/// Mirrors `require_rfc3339` without the error wrapper: fixed `20`-byte
/// `YYYY-MM-DDTHH:MM:SSZ` shape, `datetime.strptime` calendar range
/// (`year >= 1`, month/day/hour/minute/second) and the `strftime`
/// roundtrip (implied by zero-padded re-render equality).
#[must_use]
pub fn rfc3339_valid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 20 {
        return false;
    }
    for pos in RFC3339_DIGIT_POSITIONS {
        if !bytes[pos].is_ascii_digit() {
            return false;
        }
    }
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return false;
    }
    let num = |from: usize, len: usize| -> u32 {
        let mut acc: u32 = 0;
        for b in &bytes[from..from + len] {
            acc = acc * 10 + u32::from(b - b'0');
        }
        acc
    };
    let year = num(0, 4);
    let month = num(5, 2);
    let day = num(8, 2);
    let hour = num(11, 2);
    let minute = num(14, 2);
    let second = num(17, 2);
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return false;
    }
    let year_i = i32::try_from(year).unwrap_or(0);
    if day < 1 || day > days_in_month(year_i, month) {
        return false;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return false;
    }
    // `strftime("%Y-%m-%dT%H:%M:%SZ")` roundtrip: zero-padded re-render.
    let mut rendered = [0_u8; 20];
    let push2 = |buf: &mut [u8; 20], at: usize, v: u32| {
        buf[at] = b'0' + u8::try_from(v / 10).unwrap_or(9);
        buf[at + 1] = b'0' + u8::try_from(v % 10).unwrap_or(9);
    };
    let year_bytes = format!("{year:04}");
    rendered[0..4].copy_from_slice(year_bytes.as_bytes());
    rendered[4] = b'-';
    push2(&mut rendered, 5, month);
    rendered[7] = b'-';
    push2(&mut rendered, 8, day);
    rendered[10] = b'T';
    push2(&mut rendered, 11, hour);
    rendered[13] = b':';
    push2(&mut rendered, 14, minute);
    rendered[16] = b':';
    push2(&mut rendered, 17, second);
    rendered[19] = b'Z';
    rendered == *bytes
}

/// `require_sha`: `SHA256_RE` fence.
///
/// # Errors
///
/// Returns `MATERIALIZATION_INPUT_INVALID` when `value` is not a lowercase
/// SHA-256 hex string.
pub fn require_sha(value: &Value, label: &str) -> Result<String, MaterializationPlanError> {
    match value {
        Value::String(text) if sha256_hex_valid(text) => Ok(text.clone()),
        _ => Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not SHA-256"),
        )),
    }
}

/// `require_opaque`: `OPAQUE_RE` fence.
///
/// # Errors
///
/// Returns `MATERIALIZATION_INPUT_INVALID` when `value` is not an `OpaqueId`.
pub fn require_opaque(value: &Value, label: &str) -> Result<String, MaterializationPlanError> {
    match value {
        Value::String(text) if opaque_id_valid(text) => Ok(text.clone()),
        _ => Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not OpaqueId"),
        )),
    }
}

/// `require_actor`: `ACTOR_RE` fence.
///
/// # Errors
///
/// Returns `MATERIALIZATION_INPUT_INVALID` when `value` is not an
/// `ActorIdentity`.
pub fn require_actor(value: &Value, label: &str) -> Result<String, MaterializationPlanError> {
    match value {
        Value::String(text) if actor_identity_valid(text) => Ok(text.clone()),
        _ => Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not ActorIdentity"),
        )),
    }
}

/// `require_rfc3339`: whole-second UTC `RFC3339` fence with calendar check.
///
/// # Errors
///
/// Returns `MATERIALIZATION_INPUT_INVALID` when the shape, calendar date or
/// canonical rendering fails (messages mirror the three Python branches).
pub fn require_rfc3339(value: &Value, label: &str) -> Result<String, MaterializationPlanError> {
    let Some(text) = value.as_str() else {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not whole-second UTC RFC3339"),
        ));
    };
    let bytes = text.as_bytes();
    let shape = bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .iter()
            .all(|pos| bytes[*pos].is_ascii_digit());
    if !shape {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not whole-second UTC RFC3339"),
        ));
    }
    if !rfc3339_valid(text) {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not a valid calendar timestamp"),
        ));
    }
    Ok(text.to_owned())
}

/// `require_u64`: JSON integer in `0..=2^64-1` (booleans rejected).
///
/// # Errors
///
/// Returns `MATERIALIZATION_INPUT_INVALID` when `value` is not a `u64`.
pub fn require_u64(value: &Value, label: &str) -> Result<u64, MaterializationPlanError> {
    let invalid = || {
        MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not u64"),
        )
    };
    match value {
        Value::Number(number) => match (number.as_u64(), number.as_i64()) {
            (Some(u), _) => Ok(u),
            (None, Some(i)) => u64::try_from(i).map_err(|_| invalid()),
            (None, None) => Err(invalid()),
        },
        _ => Err(invalid()),
    }
}

/// Advisory output-root grammar (pure prefix of `validate_output_root`).
///
/// Mirrors the backslash normalization plus `safe_path` + `under(PLAN_ROOT)`
/// fence; parent-directory creation and symlink-component checks stay
/// Python-owned (filesystem IO).
///
/// Returns the slash-normalized target on success.
///
/// # Errors
///
/// Returns `MATERIALIZATION_OUTPUT_PATH_INVALID` when the normalized path is
/// not `PLAN_ROOT` or a descendant.
pub fn advisory_output_target(relative: &str) -> Result<String, MaterializationPlanError> {
    let normalized = relative.replace('\\', "/");
    let normalized = normalized.trim_end_matches('/').to_owned();
    if crate::ticket_planner::safe_path(&normalized)
        && crate::ticket_planner::under(&normalized, PLAN_ROOT)
    {
        Ok(normalized)
    } else {
        Err(MaterializationPlanError::new(
            "MATERIALIZATION_OUTPUT_PATH_INVALID",
            format!("output root must be {PLAN_ROOT} or a descendant"),
        ))
    }
}

/// Normalized immutable artifact reference (`validate_artifact_ref`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    /// Store profile reference (`OpaqueId`).
    pub store_profile_ref: String,
    /// Artifact identifier (`OpaqueId`).
    pub artifact_id: String,
    /// Byte length (must equal `bundle_bytes.len()`).
    pub bytes: u64,
    /// SHA-256 hex (must equal `exact_sha256(bundle_bytes)`).
    pub sha256: String,
}

/// `validate_artifact_ref`: exact field set, grammar checks and bundle
/// readback identity.
///
/// # Errors
///
/// Returns `MATERIALIZATION_ARTIFACT_REF_INVALID` on a wrong field set,
/// `MATERIALIZATION_INPUT_INVALID` on grammar failures, or
/// `MATERIALIZATION_ARTIFACT_READBACK_MISMATCH` when `bytes`/`sha256` do
/// not identify `bundle_bytes`.
pub fn validate_artifact_ref(
    value: &Value,
    bundle_bytes: &[u8],
) -> Result<ArtifactRef, MaterializationPlanError> {
    let Value::Object(map) = value else {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_ARTIFACT_REF_INVALID",
            "artifact_ref field set is invalid",
        ));
    };
    if map.len() != 4
        || !map.contains_key("store_profile_ref")
        || !map.contains_key("artifact_id")
        || !map.contains_key("bytes")
        || !map.contains_key("sha256")
    {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_ARTIFACT_REF_INVALID",
            "artifact_ref field set is invalid",
        ));
    }
    let result = ArtifactRef {
        store_profile_ref: require_opaque(
            &map["store_profile_ref"],
            "artifact_ref.store_profile_ref",
        )?,
        artifact_id: require_opaque(&map["artifact_id"], "artifact_ref.artifact_id")?,
        bytes: require_u64(&map["bytes"], "artifact_ref.bytes")?,
        sha256: require_sha(&map["sha256"], "artifact_ref.sha256")?,
    };
    if result.bytes != u64::try_from(bundle_bytes.len()).unwrap_or(u64::MAX)
        || result.sha256 != crate::ticket_planner::exact_sha256_hex(bundle_bytes)
    {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_ARTIFACT_READBACK_MISMATCH",
            "artifact_ref does not identify bundle bytes",
        ));
    }
    Ok(result)
}

/// Normalized `PRESENT` signature value (`validate_optional_signature`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureValue {
    /// Approval profile reference (`OpaqueId`).
    pub approval_profile_ref: String,
    /// Approval artifact reference (same shape as [`ArtifactRef`]).
    pub approval_artifact_ref: ArtifactRef,
    /// Digest of the signed payload this signature binds.
    pub signed_payload_sha256: String,
    /// Signing actor (must equal the selected actor).
    pub actor_identity: String,
}

/// Normalized `OptionalV1` signature reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalSignature {
    /// `ABSENT` or `PRESENT`.
    pub state: String,
    /// `None` for `ABSENT`, normalized value for `PRESENT`.
    pub value: Option<SignatureValue>,
}

/// `validate_optional_signature`: `OptionalV1` shape, grammar and actor
/// binding.
///
/// # Errors
///
/// Returns `MATERIALIZATION_SIGNATURE_REF_INVALID` on shape/grammar
/// failures, or `MATERIALIZATION_SIGNATURE_ACTOR_MISMATCH` when the
/// embedded actor differs from `expected_actor`.
pub fn validate_optional_signature(
    value: &Value,
    expected_actor: &str,
    label: &str,
) -> Result<OptionalSignature, MaterializationPlanError> {
    let invalid = |message: String| {
        MaterializationPlanError::new("MATERIALIZATION_SIGNATURE_REF_INVALID", message)
    };
    let Value::Object(map) = value else {
        return Err(invalid(format!("{label} OptionalV1 is invalid")));
    };
    if map.len() != 2 || !map.contains_key("state") || !map.contains_key("value") {
        return Err(invalid(format!("{label} OptionalV1 is invalid")));
    }
    let Some(Value::String(state)) = map.get("state") else {
        return Err(invalid(format!("{label} state/value is invalid")));
    };
    if state == "ABSENT" {
        if map["value"] != Value::String(String::new()) {
            return Err(invalid(format!("{label} ABSENT requires empty string")));
        }
        return Ok(OptionalSignature {
            state: state.clone(),
            value: None,
        });
    }
    let Value::Object(wrapped) = &map["value"] else {
        return Err(invalid(format!("{label} state/value is invalid")));
    };
    if state != "PRESENT" {
        return Err(invalid(format!("{label} state/value is invalid")));
    }
    if wrapped.len() != 4
        || !wrapped.contains_key("approval_profile_ref")
        || !wrapped.contains_key("approval_artifact_ref")
        || !wrapped.contains_key("signed_payload_sha256")
        || !wrapped.contains_key("actor_identity")
    {
        return Err(invalid(format!("{label} field set is invalid")));
    }
    let actor = require_actor(
        &wrapped["actor_identity"],
        &format!("{label}.actor_identity"),
    )?;
    if actor != expected_actor {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_SIGNATURE_ACTOR_MISMATCH",
            format!("{label} actor differs from selected actor"),
        ));
    }
    let Value::Object(approval) = &wrapped["approval_artifact_ref"] else {
        return Err(invalid(format!("{label}.approval_artifact_ref is invalid")));
    };
    if approval.len() != 4
        || !approval.contains_key("store_profile_ref")
        || !approval.contains_key("artifact_id")
        || !approval.contains_key("bytes")
        || !approval.contains_key("sha256")
    {
        return Err(invalid(format!("{label}.approval_artifact_ref is invalid")));
    }
    Ok(OptionalSignature {
        state: state.clone(),
        value: Some(SignatureValue {
            approval_profile_ref: require_opaque(
                &wrapped["approval_profile_ref"],
                &format!("{label}.approval_profile_ref"),
            )?,
            approval_artifact_ref: ArtifactRef {
                store_profile_ref: require_opaque(
                    &approval["store_profile_ref"],
                    &format!("{label}.approval_artifact_ref.store_profile_ref"),
                )?,
                artifact_id: require_opaque(
                    &approval["artifact_id"],
                    &format!("{label}.approval_artifact_ref.artifact_id"),
                )?,
                bytes: require_u64(
                    &approval["bytes"],
                    &format!("{label}.approval_artifact_ref.bytes"),
                )?,
                sha256: require_sha(
                    &approval["sha256"],
                    &format!("{label}.approval_artifact_ref.sha256"),
                )?,
            },
            signed_payload_sha256: require_sha(
                &wrapped["signed_payload_sha256"],
                &format!("{label}.signed_payload_sha256"),
            )?,
            actor_identity: actor,
        }),
    })
}
