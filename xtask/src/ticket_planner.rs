//! Bounded port of pure `tools/ticket_issuance_planner_v2` helpers (T41, ticket slice).
//!
//! Covers only deterministic, IO-free helpers from `core.py` / `control.py` /
//! `context.py`: path grammar (`safe_path`, `under`), SHA-256 helpers,
//! canonical JSON plus plan digest, decision/selection classification, closed
//! reason registries, actor/package/commit grammars, signed-payload digest,
//! advisory output-path rules, context-source fences, manifest-owned ceilings,
//! closed field sets, handoff topology, line limits, exact-pack sources and
//! registry selector resolution over pre-loaded documents.
//!
//! `plan_digest` is byte-exact with `CPython` for float-free payloads (the whole
//! planner domain); finite non-integral floats use Rust `Display` and are
//! outside the pinned domain. Git-tree reads (`GitView`), check emission and
//! both `plan`/`validate` entrypoints remain Python-owned (see report).

use std::collections::BTreeSet;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// `SCHEMA_VERSION`: schema-v2 planner record version.
pub const SCHEMA_VERSION: i64 = 2;
/// `RECORD_KIND`: advisory plan record kind.
pub const RECORD_KIND: &str = "ticket_issuance_plan_v2";
/// `STATUS`: plans are advisory and non-authoritative.
pub const STATUS: &str = "ADVISORY_NON_AUTHORITATIVE";
/// Domain separator prefixed before the canonical plan bytes before hashing.
pub const DOMAIN_SEPARATOR: &[u8] = b"eliot-search/ticket-issuance-plan/v2\0";
/// `PLAN_ARTIFACT_ROOT`: sole writable advisory artifact directory.
pub const PLAN_ARTIFACT_ROOT: &str = "artifacts/ticket-issuance-plans";
/// `REPOSITORY_NAME`: expected repository identity in draft fences.
pub const REPOSITORY_NAME: &str = "UnknownAlienHuman/eliot-search";

/// `DECISION_READY`: full selection, no blocking reasons.
pub const DECISION_READY: &str = "READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW";
/// `DECISION_MISSING`: no issuance identity selected.
pub const DECISION_MISSING: &str = "BLOCKED_MISSING_SELECTION";
/// `DECISION_PREREQUISITE`: accepted-handoff prerequisite unsatisfied.
pub const DECISION_PREREQUISITE: &str = "BLOCKED_PREREQUISITE";
/// `DECISION_CONFLICT`: conflicting or partial selection.
pub const DECISION_CONFLICT: &str = "BLOCKED_CONFLICT";
/// `DECISION_INVALID`: repository state is structurally invalid.
pub const DECISION_INVALID: &str = "INVALID_REPOSITORY_STATE";

/// `CONTROL_ROOTS`: control-record roots that must stay empty pre-issuance.
pub const CONTROL_ROOTS: [&str; 8] = [
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
    "swarm/supersessions",
    "swarm/wave-receipts",
];
/// `CURRENT_PACKAGE_RECORD_ROOTS`: roots scanned for current-package records.
pub const CURRENT_PACKAGE_RECORD_ROOTS: [&str; 6] = [
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
];
/// `ROOT_METADATA_NAMES`: exact root metadata filenames (never exemptions elsewhere).
pub const ROOT_METADATA_NAMES: [&str; 2] = ["README.md", ".gitkeep"];

/// `CLOSED_REASON_CODES`: exact closed machine reason registry (order-pinned).
pub const CLOSED_REASON_CODES: [&str; 32] = [
    "GIT_REPOSITORY_INVALID",
    "PACKAGE_UNKNOWN",
    "PACKAGE_STAGE_MISMATCH",
    "PACKAGE_REGISTRY_MISMATCH",
    "DRAFT_MANIFEST_MISMATCH",
    "DRAFT_PAIR_MISSING",
    "DRAFT_PAIR_MISMATCH",
    "DRAFT_UNKNOWN_FIELD",
    "DRAFT_BECAME_CLAIMABLE",
    "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
    "CONTEXT_BUDGET_EXCEEDED",
    "CONTEXT_SOURCE_MISSING",
    "CONTEXT_SOURCE_NOT_REGULAR",
    "CONTEXT_SOURCE_NOT_UTF8",
    "CONTEXT_SOURCE_FORBIDDEN",
    "CONTEXT_SELECTOR_INVALID",
    "CONTEXT_SELECTOR_NOT_UNIQUE",
    "HANDOFF_SLOT_UNSATISFIED",
    "HANDOFF_SET_UNEXPECTED",
    "HANDOFF_RECORD_INVALID",
    "HANDOFF_RECORD_SUPERSEDED",
    "PARTIAL_ISSUANCE_SELECTION",
    "BASE_COMMIT_INVALID",
    "ACTOR_IDENTITY_INVALID",
    "WRITER_REVIEWER_CONFLICT",
    "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
    "W0_ALREADY_ACCEPTED",
    "CONTROL_SCHEMA_MISMATCH",
    "WORKFLOW_POLICY_VIOLATION",
    "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
    "OUTPUT_PATH_SYMLINK",
    "OUTPUT_WRITE_FAILED",
];
/// Reasons forcing `INVALID_REPOSITORY_STATE` (sorted; note
/// `OUTPUT_WRITE_FAILED` is deliberately absent upstream: a failed advisory
/// write is not a decision reason).
pub const INVALID_REASONS: [&str; 25] = [
    "ACTOR_IDENTITY_INVALID",
    "BASE_COMMIT_INVALID",
    "CONTEXT_BUDGET_EXCEEDED",
    "CONTEXT_SELECTOR_INVALID",
    "CONTEXT_SELECTOR_NOT_UNIQUE",
    "CONTEXT_SOURCE_FORBIDDEN",
    "CONTEXT_SOURCE_MISSING",
    "CONTEXT_SOURCE_NOT_REGULAR",
    "CONTEXT_SOURCE_NOT_UTF8",
    "CONTROL_SCHEMA_MISMATCH",
    "DRAFT_BECAME_CLAIMABLE",
    "DRAFT_IDENTITY_PREMATURELY_RESOLVED",
    "DRAFT_MANIFEST_MISMATCH",
    "DRAFT_PAIR_MISMATCH",
    "DRAFT_PAIR_MISSING",
    "DRAFT_UNKNOWN_FIELD",
    "GIT_REPOSITORY_INVALID",
    "HANDOFF_RECORD_INVALID",
    "HANDOFF_RECORD_SUPERSEDED",
    "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
    "OUTPUT_PATH_SYMLINK",
    "PACKAGE_REGISTRY_MISMATCH",
    "PACKAGE_STAGE_MISMATCH",
    "PACKAGE_UNKNOWN",
    "WORKFLOW_POLICY_VIOLATION",
];
/// Reasons forcing `BLOCKED_CONFLICT` (sorted).
pub const CONFLICT_REASONS: [&str; 5] = [
    "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
    "HANDOFF_SET_UNEXPECTED",
    "PARTIAL_ISSUANCE_SELECTION",
    "W0_ALREADY_ACCEPTED",
    "WRITER_REVIEWER_CONFLICT",
];
/// Reasons forcing `BLOCKED_PREREQUISITE` (sorted).
pub const PREREQUISITE_REASONS: [&str; 1] = ["HANDOFF_SLOT_UNSATISFIED"];

/// `TICKET_ALLOWED`: closed top-level ticket draft fields (sorted).
pub const TICKET_ALLOWED: [&str; 20] = [
    "authorizes_implementation",
    "claimable",
    "context",
    "creates_lease",
    "deliverables",
    "dependencies",
    "issuance_status",
    "launch_class",
    "launch_precondition",
    "limits",
    "may_be_writer_acknowledged",
    "package",
    "phase",
    "record_kind",
    "repository_fence",
    "schema_version",
    "stage",
    "status",
    "unresolved_identity",
    "wave",
];
/// `CONTEXT_ALLOWED`: closed top-level context draft fields (sorted).
pub const CONTEXT_ALLOWED: [&str; 21] = [
    "accepted_handoff_slot_count",
    "authorizes_implementation",
    "base_commit",
    "canonicalization",
    "claimable",
    "content",
    "materialization_mode",
    "materialized_context_artifact_ref",
    "materialized_context_artifact_sha256",
    "materialized_context_manifest_ref",
    "materialized_context_record_sha256",
    "package",
    "phase",
    "record_kind",
    "registry_fragment_count",
    "schema_version",
    "source_file_count",
    "stage",
    "status",
    "wave",
    "writer_visible_artifact_count",
];

/// `TICKET_SECTIONS`: closed per-section ticket draft fields.
pub const TICKET_UNRESOLVED_IDENTITY_FIELDS: [&str; 9] = [
    "base_commit",
    "branch_or_worktree",
    "integration_signature_ref",
    "issued_at",
    "reviewer",
    "ticket_exact_record_file_sha256",
    "ticket_id",
    "ticket_signed_payload_sha256",
    "writer",
];
/// Closed `repository_fence` fields.
pub const TICKET_REPOSITORY_FENCE_FIELDS: [&str; 8] = [
    "feature_profile",
    "function_registry_path",
    "launch_state_path",
    "package_registry_path",
    "registry_digests",
    "repository",
    "stage_registry_path",
    "write_scope",
];
/// Closed ticket `context` fields.
pub const TICKET_CONTEXT_FIELDS: [&str; 6] = [
    "architecture_access",
    "context_artifact_ref",
    "context_artifact_sha256",
    "context_draft",
    "context_manifest_ref",
    "writer_visible_artifact_count",
];
/// Closed ticket `dependencies` fields.
pub const TICKET_DEPENDENCIES_FIELDS: [&str; 5] = [
    "accepted_handoff_refs",
    "required_contract_api_schema_digest",
    "required_contract_commit",
    "required_handoff_packages",
    "status",
];
/// Closed ticket `limits` fields.
pub const TICKET_LIMITS_FIELDS: [&str; 4] = [
    "hard_total_lines",
    "one_active_writer",
    "soft_src_lines",
    "split_review_total_lines",
];
/// Closed ticket `deliverables` fields.
pub const TICKET_DELIVERABLES_FIELDS: [&str; 3] = [
    "issuance_requirements",
    "required_evidence",
    "required_outputs",
];
/// `CONTEXT_SECTIONS`: closed per-section context draft fields.
pub const CONTEXT_CANONICALIZATION_FIELDS: [&str; 7] = [
    "encoding",
    "line_endings",
    "path_header_format",
    "preserve_declared_order",
    "record_fragment_sha256",
    "record_source_sha256",
    "registry_header_format",
];
/// Closed context `content` fields.
pub const CONTEXT_CONTENT_FIELDS: [&str; 5] = [
    "accepted_handoff_slots",
    "forbidden_paths",
    "registry_fragments",
    "required_unavailable_checks",
    "source_files",
];
pub const SPLIT_REVIEW_TOTAL_LINES: i64 = 8500;
/// Hard total line budget (both Python and TOML ceilings agree).
pub const HARD_TOTAL_LINES: i64 = 10_000;
/// Per-file planner read ceiling in bytes.
pub const PLANNER_FILE_BYTE_CEILING: u64 = 4 * 1024 * 1024;
/// Declared-context total byte ceiling.
pub const CONTEXT_TOTAL_BYTE_CEILING: u64 = 16 * 1024 * 1024;
/// Canonical plan artifact byte ceiling.
pub const PLAN_BYTE_CEILING: usize = 262_144;

/// `safe_path`: repository-relative safe path check.
///
/// Mirrors `SAFE_PATH_RE` plus `PurePosixPath` semantics: not absolute, and no
/// empty/`..` segments once single-dot segments are dropped — so `"."` and
/// `"a/./b"` are safe.
#[must_use]
pub fn safe_path(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let grammar = value.split('/').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
    });
    if !grammar {
        return false;
    }
    value
        .split('/')
        .filter(|part| *part != ".")
        .all(|part| part != "..")
}

/// `under`: path equality or strict `prefix/` containment.
#[must_use]
pub fn under(path: &str, prefix: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// `exact_sha256`: SHA-256 hex of raw bytes (`hashlib.sha256(...).hexdigest()`).
#[must_use]
pub fn exact_sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn append_json_escaped(out: &mut Vec<u8>, text: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push(b'"');
    for c in text.chars() {
        match c {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{08}' => out.extend_from_slice(b"\\b"),
            '\u{09}' => out.extend_from_slice(b"\\t"),
            '\u{0A}' => out.extend_from_slice(b"\\n"),
            '\u{0C}' => out.extend_from_slice(b"\\f"),
            '\u{0D}' => out.extend_from_slice(b"\\r"),
            c if (c as u32) < 0x20 => {
                out.extend_from_slice(b"\\u00");
                let b = c as u8;
                out.push(HEX[usize::from(b >> 4)]);
                out.push(HEX[usize::from(b & 0x0F)]);
            }
            c => {
                let mut buf = [0_u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    out.push(b'"');
}

fn append_canonical(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(true) => out.extend_from_slice(b"true"),
        Value::Bool(false) => out.extend_from_slice(b"false"),
        Value::Number(number) => {
            if let Some(i) = number.as_i64() {
                out.extend_from_slice(i.to_string().as_bytes());
            } else if let Some(u) = number.as_u64() {
                out.extend_from_slice(u.to_string().as_bytes());
            } else if let Some(f) = number.as_f64() {
                // Outside the pinned domain: planner payloads are float-free.
                out.extend_from_slice(f.to_string().as_bytes());
            }
        }
        Value::String(text) => append_json_escaped(out, text),
        Value::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                append_canonical(out, item);
            }
            out.push(b']');
        }
        Value::Object(map) => {
            // `serde_json::Map` is a `BTreeMap` (sorted keys), matching
            // `json.dumps(..., sort_keys=True)` for string keys.
            out.push(b'{');
            for (index, (key, item)) in map.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                append_json_escaped(out, key);
                out.push(b':');
                append_canonical(out, item);
            }
            out.push(b'}');
        }
    }
}

/// `canonical_json_bytes`: canonical plan bytes plus trailing newline.
///
/// Mirrors `json.dumps(..., ensure_ascii=False, sort_keys=True,
/// separators=(",", ":"))`. Byte-exact with `CPython` for float-free
/// payloads (the whole planner domain).
#[must_use]
pub fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    append_canonical(&mut out, value);
    out.push(b'\n');
    out
}

/// `plan_digest`: SHA-256 over the domain separator plus canonical bytes.
#[must_use]
pub fn plan_digest(payload_without_digest: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_SEPARATOR);
    hasher.update(canonical_json_bytes(payload_without_digest));
    format!("{:x}", hasher.finalize())
}

/// `choose_decision`: invalid beats conflict beats prerequisite; otherwise the
/// selection state decides (`NONE` missing, non-`COMPLETE` conflict, else ready).
#[must_use]
pub fn choose_decision(state: &str, reasons: &[&str]) -> &'static str {
    if reasons.iter().any(|r| INVALID_REASONS.contains(r)) {
        return DECISION_INVALID;
    }
    if reasons.iter().any(|r| CONFLICT_REASONS.contains(r)) {
        return DECISION_CONFLICT;
    }
    if reasons.iter().any(|r| PREREQUISITE_REASONS.contains(r)) {
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

/// `actor:` identity grammar (`ACTOR_RE`).
#[must_use]
pub fn actor_identity_valid(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("actor:") else {
        return false;
    };
    let Some((role, identity)) = rest.split_once(':') else {
        return false;
    };
    matches!(role, "user" | "service" | "reviewer" | "integration") && opaque_id_valid(identity)
}

/// `PACKAGE_RE` package name grammar.
#[must_use]
pub fn package_name_valid(value: &str) -> bool {
    let mut segments = value.split('-');
    let Some(first) = segments.next() else {
        return false;
    };
    let mut chars = first.chars();
    if !matches!(chars.next(), Some(c) if c.is_ascii_lowercase()) {
        return false;
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
        return false;
    }
    for segment in segments {
        if segment.is_empty()
            || !segment
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return false;
        }
    }
    true
}

/// `TAGGED_GIT_RE` algorithm-tagged commit grammar.
#[must_use]
pub fn tagged_git_valid(value: &str) -> bool {
    let Some((algorithm, oid)) = value.split_once(':') else {
        return false;
    };
    let expected = match algorithm {
        "sha1" => 40,
        "sha256" => 64,
        _ => return false,
    };
    oid.len() == expected
        && oid
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// `SHA256_RE` lowercase hex digest grammar.
#[must_use]
pub fn sha256_hex_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// `OPAQUE_ID_RE` opaque identifier grammar.
#[must_use]
pub fn opaque_id_valid(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 {
        return false;
    }
    let mut chars = value.chars();
    if !matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric()) {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// `selection_state` pure classification: `NONE` (no selection), `PARTIAL`
/// (some of base/writer/reviewer) plus `PARTIAL_ISSUANCE_SELECTION`, or
/// `COMPLETE` with actor-grammar and independence reasons.
#[must_use]
pub fn selection_state(
    base: Option<&str>,
    writer: Option<&str>,
    reviewer: Option<&str>,
) -> (&'static str, Vec<&'static str>) {
    let count = [base, writer, reviewer]
        .iter()
        .filter(|v| v.is_some())
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

/// `signed_payload_digest`: SHA-256 over bytes through the single trailing
/// newline of exactly one `\n[signature]\n` marker (`None` otherwise).
#[must_use]
pub fn signed_payload_digest(raw: &[u8]) -> Option<String> {
    const MARKER: &[u8] = b"\n[signature]\n";
    let offset = find_subslice(raw, MARKER)?;
    if offset == 0 || find_subslice(&raw[offset + 1..], MARKER).is_some() {
        return None;
    }
    Some(exact_sha256_hex(&raw[..=offset]))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Stdout half of `validate_output`: `-` is always selectable.
///
/// The backslash-normalized path rules live in
/// [`advisory_output_path_valid`]; symlink checks remain Python-owned
/// (filesystem IO).
#[must_use]
pub fn advisory_output_selectable(output: &str) -> bool {
    output == "-"
}

/// Path-rule half of `validate_output` for non-stdout outputs.
#[must_use]
pub fn advisory_output_path_valid(output: &str) -> bool {
    let relative = output.replace('\\', "/");
    // Byte comparison keeps the exact case-sensitive `.json` rule (an
    // `extension()` check would accept `.JSON`).
    safe_path(&relative)
        && under(&relative, PLAN_ARTIFACT_ROOT)
        && relative.as_bytes().ends_with(b".json")
        && relative != format!("{PLAN_ARTIFACT_ROOT}/.json")
}

/// `validate_context` source fence predicate.
///
/// Forbidden when unsafe, under `docs/architecture/`, `bins/`, any
/// `crates/*/src/`, or any control root.
#[must_use]
pub fn context_source_forbidden(path: &str) -> bool {
    if !safe_path(path)
        || path.starts_with("docs/architecture/")
        || path.starts_with("bins/")
        || is_crate_src(path)
    {
        return true;
    }
    CONTROL_ROOTS.iter().any(|root| under(path, root))
}

fn is_crate_src(path: &str) -> bool {
    // `^crates/.+/src/`
    let Some(rest) = path.strip_prefix("crates/") else {
        return false;
    };
    rest.split('/').count() >= 3 && rest.split('/').nth(1) == Some("src")
}

/// Manifest-owned ceiling selection (`drafts.py`).
///
/// `ORDINARY` uses the ordinary ceiling; `P00_EXACT_CONTRACT_PACK` uses the
/// exact-pack ceiling and additionally requires the package to be the single
/// listed exception and `search-contracts`.
#[must_use]
pub fn select_ceiling(
    ceiling_class: &str,
    package: &str,
    exceptions: &[&str],
    ordinary: i64,
    exact: i64,
) -> (i64, bool) {
    if ceiling_class == "P00_EXACT_CONTRACT_PACK" {
        let class_ok = exceptions.iter().filter(|e| **e == package).count() == 1
            && package == "search-contracts";
        return (exact, class_ok);
    }
    (ordinary, ceiling_class == "ORDINARY")
}

/// `validate_keys` unknown-field detection: sorted deduplicated difference.
#[must_use]
pub fn unknown_fields<'a>(keys: &[&'a str], allowed: &[&str]) -> Vec<&'a str> {
    let allowed: BTreeSet<&&str> = allowed.iter().collect();
    let mut unknown: Vec<&'a str> = keys
        .iter()
        .filter(|key| !allowed.contains(key))
        .copied()
        .collect();
    unknown.sort_unstable();
    unknown.dedup();
    unknown
}

/// Expected accepted-handoff slots per package (P00 dependency topology).
#[must_use]
pub fn expected_handoff_slots(package: &str) -> &'static [&'static str] {
    if package == "search-contracts" {
        &[]
    } else {
        &["search-contracts::accepted_package_and_api_handoff"]
    }
}

/// Expected required-handoff packages per ticket dependencies.
#[must_use]
pub fn expected_required_handoffs(package: &str) -> &'static [&'static str] {
    if package == "search-contracts" {
        &[]
    } else {
        &["search-contracts"]
    }
}

/// Draft repository-fence line-limit coherence: ordered, positive, and pinned
/// to the 8500/10000 split/hard budgets.
#[must_use]
pub const fn line_limits_ok(
    registry_soft: i64,
    ticket_soft: i64,
    split_review: i64,
    hard: i64,
) -> bool {
    0 < registry_soft
        && registry_soft <= ticket_soft
        && ticket_soft <= split_review
        && split_review <= hard
        && split_review == SPLIT_REVIEW_TOTAL_LINES
        && hard == HARD_TOTAL_LINES
}

/// Declared-context total byte ceiling check.
#[must_use]
pub const fn context_total_bytes_ok(total: u64) -> bool {
    total <= CONTEXT_TOTAL_BYTE_CEILING
}

/// `expected_contract_pack_sources`: manifest-closed exact P00 source list.
///
/// # Errors
///
/// Returns `Err("DRAFT_MANIFEST_MISMATCH")` unless `required_files` is
/// duplicate-free, non-empty and `README.md`-first.
pub fn contract_pack_sources(
    package: &str,
    required_files: &[&str],
) -> Result<Vec<String>, &'static str> {
    const MISMATCH: &str = "DRAFT_MANIFEST_MISMATCH";
    if required_files.is_empty() || required_files[0] != "README.md" {
        return Err(MISMATCH);
    }
    let unique: BTreeSet<&&str> = required_files.iter().collect();
    if unique.len() != required_files.len() {
        return Err(MISMATCH);
    }
    let mut sources = vec![
        "AGENTS.md".to_owned(),
        format!("crates/{package}/AGENTS.md"),
        "docs/handoff/AUTHORITY_MAP.md".to_owned(),
        "swarm/ASSIGNMENT_PROTOCOL.md".to_owned(),
        format!("swarm/assignments/{package}.md"),
        "docs/handoff/P00_BOOTSTRAP.md".to_owned(),
        "docs/contracts/p00/README.md".to_owned(),
        "docs/contracts/p00/manifest.toml".to_owned(),
    ];
    sources.extend(
        required_files[1..]
            .iter()
            .map(|name| format!("docs/contracts/p00/{name}")),
    );
    Ok(sources)
}

/// `one_table`: the single row of an array-of-tables matching `key ==
/// expected` (`None` for non-arrays, zero or duplicate matches).
#[must_use]
pub fn one_table<'a>(rows: &'a Value, key: &str, expected: &str) -> Option<&'a Value> {
    let rows = rows.as_array()?;
    let mut hits = rows
        .iter()
        .filter(|row| row.get(key).and_then(Value::as_str) == Some(expected));
    let hit = hits.next()?;
    if hits.next().is_some() {
        return None;
    }
    Some(hit)
}

fn count_occurrences(values: &Value, target: &str) -> usize {
    values.as_array().map_or(0, |items| {
        items
            .iter()
            .filter(|item| item.as_str() == Some(target))
            .count()
    })
}

fn selector_name_valid(value: &str) -> bool {
    let mut chars = value.chars();
    if !matches!(chars.next(), Some(c) if c.is_ascii_lowercase()) {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn stage_id_valid(value: &str) -> bool {
    if value == "W10" {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() == 2 && bytes[0] == b'W' && bytes[1].is_ascii_digit()
}

fn bracketed<'a>(expression: &'a str, prefix: &str) -> Option<&'a str> {
    expression.strip_prefix(prefix)?.strip_suffix(']')
}

/// Pre-loaded registry documents for [`resolve_selector`].
pub struct SelectorDocs<'a> {
    /// Parsed `swarm/crates.toml` (`None` when missing or invalid).
    pub crates: Option<&'a Value>,
    /// Parsed `swarm/function-packets.toml`.
    pub functions: Option<&'a Value>,
    /// Parsed `swarm/stages.toml`.
    pub stages: Option<&'a Value>,
    /// Parsed `swarm/launch-state.toml`.
    pub launch: Option<&'a Value>,
}

/// Selector resolution outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorStatus {
    /// Selector resolved exactly once.
    Ok,
    /// Selector expression or registry path is unsupported.
    Unsupported,
    /// Registry resolved zero or multiple times.
    NotUnique,
}

impl SelectorStatus {
    /// Python status token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Unsupported => "UNSUPPORTED",
            Self::NotUnique => "NOT_UNIQUE",
        }
    }
}

/// `resolve_selector` over pre-loaded documents.
///
/// Mirrors `context.py` exactly, including the name-mismatch-is-`UNSUPPORTED`
/// (not `NOT_UNIQUE`) quirk and the missing/invalid-registry-is-`NOT_UNIQUE`
/// mapping.
#[must_use]
pub fn resolve_selector(
    docs: &SelectorDocs<'_>,
    selector: &str,
    package: &str,
) -> (SelectorStatus, &'static str) {
    use SelectorStatus::{NotUnique, Unsupported};
    let Some((path, expression)) = selector.split_once("::") else {
        return (Unsupported, "missing :: separator");
    };
    if !matches!(
        path,
        "swarm/crates.toml"
            | "swarm/function-packets.toml"
            | "swarm/stages.toml"
            | "swarm/launch-state.toml"
    ) {
        return (
            Unsupported,
            "registry path is not in the closed selector set",
        );
    }
    let document = match path {
        "swarm/crates.toml" => docs.crates,
        "swarm/function-packets.toml" => docs.functions,
        "swarm/stages.toml" => docs.stages,
        _ => docs.launch,
    };
    let Some(document) = document else {
        return (NotUnique, "registry path is missing or invalid");
    };

    if let Some(outcome) = resolve_bracketed(path, document, expression, package) {
        return outcome;
    }
    if let Some(outcome) = resolve_launchish(path, document, expression, package) {
        return outcome;
    }
    (Unsupported, "unsupported selector expression")
}

fn resolve_bracketed(
    path: &str,
    document: &Value,
    expression: &str,
    package: &str,
) -> Option<(SelectorStatus, &'static str)> {
    use SelectorStatus::{NotUnique, Ok, Unsupported};
    if let Some(name) = bracketed(expression, "package[name=") {
        if !selector_name_valid(name) {
            return Some((Unsupported, "unsupported selector expression"));
        }
        if path != "swarm/crates.toml" || name != package {
            return Some((Unsupported, "package selector path or identity mismatch"));
        }
        let hit = one_table(
            document.get("package").unwrap_or(&Value::Null),
            "name",
            package,
        );
        return Some(match hit {
            Some(_) => (Ok, "one package row"),
            None => (NotUnique, "package selector did not resolve exactly once"),
        });
    }
    if let Some(name) = bracketed(expression, "foundation[package=") {
        if !selector_name_valid(name) {
            return Some((Unsupported, "unsupported selector expression"));
        }
        if path != "swarm/function-packets.toml" || name != package {
            return Some((Unsupported, "foundation selector path or identity mismatch"));
        }
        let hit = one_table(
            document.get("foundation").unwrap_or(&Value::Null),
            "package",
            package,
        );
        return Some(match hit {
            Some(_) => (Ok, "one foundation row"),
            None => (
                NotUnique,
                "foundation selector did not resolve exactly once",
            ),
        });
    }
    if let Some(id) = bracketed(expression, "stage[id=") {
        if !stage_id_valid(id) {
            return Some((Unsupported, "unsupported selector expression"));
        }
        if path != "swarm/stages.toml" || id != "W0" {
            return Some((Unsupported, "stage selector path or stage mismatch"));
        }
        let row = one_table(document.get("stage").unwrap_or(&Value::Null), "id", "W0");
        let Some(row) = row else {
            return Some((NotUnique, "stage selector did not resolve exactly once"));
        };
        if count_occurrences(row.get("packages").unwrap_or(&Value::Null), package) != 1 {
            return Some((
                NotUnique,
                "selected stage does not contain package exactly once",
            ));
        }
        return Some((Ok, "one W0 stage row containing package"));
    }
    None
}

fn resolve_launchish(
    path: &str,
    document: &Value,
    expression: &str,
    package: &str,
) -> Option<(SelectorStatus, &'static str)> {
    use SelectorStatus::{NotUnique, Ok, Unsupported};
    if let Some(rest) = expression.strip_suffix(']') {
        for key in ["authorized_packages", "conditional_packages"] {
            if let Some(inner) = rest.strip_prefix(&format!("{key}[")) {
                if !selector_name_valid(inner) {
                    return Some((Unsupported, "unsupported selector expression"));
                }
                return Some(launch_membership_at_path(
                    path, document, key, inner, package,
                ));
            }
        }
    }
    if let Some(name) = expression.strip_prefix("conditional_activation.") {
        if !selector_name_valid(name) {
            return Some((Unsupported, "unsupported selector expression"));
        }
        if path != "swarm/launch-state.toml" || name != package {
            return Some((
                Unsupported,
                "conditional activation path or package mismatch",
            ));
        }
        let table = document.get("conditional_activation");
        if table
            .and_then(|t| t.get(package))
            .is_some_and(Value::is_object)
        {
            return Some((Ok, "one conditional activation table"));
        }
        return Some((
            NotUnique,
            "conditional activation did not resolve exactly once",
        ));
    }
    None
}

/// Launch membership selector (`authorized_packages[x]` /
/// `conditional_packages[x]`): registry path and package must match, then the
/// membership list must contain the package exactly once.
#[must_use]
pub fn launch_membership_at_path(
    path: &str,
    document: &Value,
    key: &str,
    name: &str,
    package: &str,
) -> (SelectorStatus, &'static str) {
    use SelectorStatus::{NotUnique, Ok, Unsupported};
    if path != "swarm/launch-state.toml" || name != package {
        return (Unsupported, "launch selector path or package mismatch");
    }
    if count_occurrences(document.get(key).unwrap_or(&Value::Null), package) == 1 {
        (Ok, "one launch membership")
    } else {
        (NotUnique, "launch membership did not resolve exactly once")
    }
}
