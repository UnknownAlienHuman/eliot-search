//! Full port of `tools/p00_ticket_drafts_validator.py` and its thin wrapper
//! `tools/validate-p00-ticket-drafts.py` (T41, ticket slice).
//!
//! Read-only validator over the ticket/context draft manifests, the P00
//! contract manifest, launch/orchestration state and per-package draft pairs.
//! The JSON report shape (sorted keys, 2-space indent), the minimal
//! `{"errors": ..., "status": "FAIL"}` early-failure object and the exit codes
//! (0 pass, 1 fail) mirror the Python implementation; the optional `--json`
//! flag is accepted and ignored exactly like the Python `argparse` switch.
//!
//! One deliberate non-crashing deviation: where Python raises an uncaught
//! `AttributeError`/`TypeError` on type-corrupted inputs (non-table
//! `invariants`/`content`, non-integer ceilings), this port records the same
//! FAIL outcome with an explanatory error and exit code 1 instead of a
//! traceback.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

/// P00 draft packages in manifest order.
pub const PACKAGES: [&str; 3] = ["search-contracts", "search-domain", "search-ports"];
/// Control-record roots that must contain no premature records.
pub const CONTROL_ROOTS: [&str; 9] = [
    "swarm/context-manifests",
    "swarm/tickets",
    "swarm/leases",
    "swarm/submissions",
    "swarm/reviews",
    "swarm/handoffs",
    "swarm/supersessions",
    "swarm/gate-receipts",
    "swarm/wave-receipts",
];
/// Ordinary static source-file ceiling per context.
pub const ORDINARY_SOURCE_CEILING: i64 = 16;
/// `search-contracts` exact-pack source-file ceiling.
pub const EXACT_PACK_SOURCE_CEILING: i64 = 24;
/// Registry-fragment ceiling per context.
pub const MAX_REGISTRY_FRAGMENTS: i64 = 6;
/// Accepted-handoff-slot ceiling per context.
pub const MAX_HANDOFF_SLOTS: i64 = 1;
/// Expected P00 `required_files` entry count.
pub const P00_REQUIRED_FILE_COUNT: usize = 13;

// --- TOML access ------------------------------------------------------------

fn load_doc(root: &Path, relative: &str) -> Result<toml::Value, String> {
    let bytes = std::fs::read(root.join(relative)).map_err(|err| format!("{relative}: {err}"))?;
    let text = String::from_utf8(bytes).map_err(|err| format!("{relative}: {err}"))?;
    toml::from_str::<toml::Value>(&text).map_err(|err| format!("{relative}: {err}"))
}

fn child<'a>(value: &'a toml::Value, table_key: &str, key: &str) -> Option<&'a toml::Value> {
    value.get(table_key)?.as_table()?.get(key)
}

fn child_str<'a>(value: &'a toml::Value, table_key: &str, key: &str) -> Option<&'a str> {
    child(value, table_key, key)?.as_str()
}

fn as_str<'a>(value: &'a toml::Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

fn as_int(value: &toml::Value, key: &str) -> Option<i64> {
    value.get(key)?.as_integer()
}

fn as_bool(value: &toml::Value, key: &str) -> Option<bool> {
    value.get(key)?.as_bool()
}

fn is_false(value: &toml::Value, table_key: &str, key: &str) -> bool {
    child(value, table_key, key).and_then(toml::Value::as_bool) == Some(false)
}

fn is_true(value: &toml::Value, table_key: &str, key: &str) -> bool {
    child(value, table_key, key).and_then(toml::Value::as_bool) == Some(true)
}

fn str_list(value: &toml::Value, key: &str) -> Option<Vec<String>> {
    value
        .get(key)?
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

/// `rows`: array-of-tables keyed by `name_key`, duplicate-rejecting.
fn rows<'a>(
    document: &'a toml::Value,
    key: &str,
    name_key: &str,
) -> Result<Vec<(String, &'a toml::Value)>, String> {
    let Some(items) = document.get(key).and_then(toml::Value::as_array) else {
        return Err(format!("{key} must be an array of tables"));
    };
    let mut result = Vec::with_capacity(items.len());
    let mut seen = BTreeSet::new();
    for row in items {
        let Some(name) = row.get(name_key).and_then(toml::Value::as_str) else {
            return Err(format!("invalid {key} row"));
        };
        if !seen.insert(name.to_owned()) {
            return Err(format!("duplicate {key} row {name}"));
        }
        result.push((name.to_owned(), row));
    }
    Ok(result)
}

/// `machine_files`: sorted repo-relative POSIX paths below `relative`,
/// ignoring `README.md`/`.gitkeep`/`.gitignore` basenames.
fn machine_files(root: &Path, relative: &str) -> Vec<String> {
    if !root.join(relative).exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut stack = vec![root.join(relative)];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // `metadata` follows symlinks, like Python `is_file()`.
            if std::fs::metadata(&path).is_ok_and(|meta| meta.is_file()) {
                push_machine_file(root, &path, &mut out);
            } else if path.is_dir() {
                stack.push(path);
            }
        }
    }
    out.sort();
    out
}

fn push_machine_file(root: &Path, path: &Path, out: &mut Vec<String>) {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, "README.md" | ".gitkeep" | ".gitignore"))
    {
        return;
    }
    if let Ok(rel) = path.strip_prefix(root) {
        out.push(rel.to_string_lossy().replace('\\', "/"));
    }
}

fn count_is(list_len: usize, value: &toml::Value, key: &str) -> bool {
    as_int(value, key) == i64::try_from(list_len).ok()
}

fn within_ceiling(list_len: usize, ceiling: Option<i64>) -> bool {
    ceiling
        .and_then(|limit| usize::try_from(limit).ok())
        .is_some_and(|limit| list_len <= limit)
}

// --- report -----------------------------------------------------------------

/// Validator outcome: `complete == false` renders the minimal early-failure
/// object (unreadable manifests), exactly like the Python `except` path.
pub struct DraftReport {
    /// Whether all manifests loaded (minimal `{"errors","status"}` when false).
    pub complete: bool,
    /// PASS (no errors) vs FAIL.
    pub passed: bool,
    /// Ticket draft row count.
    pub ticket_drafts: usize,
    /// Context draft row count.
    pub context_drafts: usize,
    /// P00 `required_files` entry count.
    pub p00_required_files: usize,
    /// `search-contracts` expected source count.
    pub search_contracts_sources: usize,
    /// `search-domain` expected source count.
    pub search_domain_sources: usize,
    /// `search-ports` expected source count.
    pub search_ports_sources: usize,
    /// Launch `active_stage` (`None` renders `null`).
    pub active_stage: Option<String>,
    /// Launch `active_wave` (`None` renders `null`).
    pub active_wave: Option<i64>,
    /// Accumulated error messages in check order.
    pub errors: Vec<String>,
}

/// The five manifests backing the validation.
struct Manifests {
    ticket: toml::Value,
    context: toml::Value,
    contract: toml::Value,
    launch: toml::Value,
    orchestration: toml::Value,
}

fn load_manifests(root: &Path) -> Result<Manifests, String> {
    Ok(Manifests {
        ticket: load_doc(root, "swarm/ticket-drafts/manifest.toml")?,
        context: load_doc(root, "swarm/context-drafts/manifest.toml")?,
        contract: load_doc(root, "docs/contracts/p00/manifest.toml")?,
        launch: load_doc(root, "swarm/launch-state.toml")?,
        orchestration: load_doc(root, "swarm/orchestration.toml")?,
    })
}

/// Report counters carried into the final report.
struct Tallies {
    ticket_drafts: usize,
    context_drafts: usize,
    p00_required_files: usize,
    contracts: usize,
    domains: usize,
    ports: usize,
}

fn finish_report(tallies: &Tallies, launch: &toml::Value, errors: Vec<String>) -> DraftReport {
    DraftReport {
        complete: true,
        passed: errors.is_empty(),
        ticket_drafts: tallies.ticket_drafts,
        context_drafts: tallies.context_drafts,
        p00_required_files: tallies.p00_required_files,
        search_contracts_sources: tallies.contracts,
        search_domain_sources: tallies.domains,
        search_ports_sources: tallies.ports,
        active_stage: as_str(launch, "active_stage").map(str::to_owned),
        active_wave: as_int(launch, "active_wave"),
        errors,
    }
}

fn incomplete(message: String) -> DraftReport {
    DraftReport {
        complete: false,
        passed: false,
        ticket_drafts: 0,
        context_drafts: 0,
        p00_required_files: 0,
        search_contracts_sources: 0,
        search_domain_sources: 0,
        search_ports_sources: 0,
        active_stage: None,
        active_wave: None,
        errors: vec![message],
    }
}

/// Process exit code: 0 on PASS, 1 on FAIL.
#[must_use]
pub const fn exit_code(report: &DraftReport) -> i32 {
    if report.complete && report.passed {
        0
    } else {
        1
    }
}

fn append_json_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{09}' => out.push_str("\\t"),
            '\u{0A}' => out.push_str("\\n"),
            '\u{0C}' => out.push_str("\\f"),
            '\u{0D}' => out.push_str("\\r"),
            // `json.dumps` default (`ensure_ascii=True`): short escapes above,
            // lowercase `\uXXXX` otherwise outside printable ASCII.
            c if (c as u32) < 0x20 || (c as u32) > 0x7E => {
                let mut units = [0_u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn append_errors(out: &mut String, errors: &[String]) {
    if errors.is_empty() {
        out.push_str("[]");
        return;
    }
    out.push_str("[\n");
    for (index, error) in errors.iter().enumerate() {
        out.push_str("    ");
        append_json_string(out, error);
        if index + 1 < errors.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]");
}

/// Render the report exactly like `json.dumps(result, indent=2, sort_keys=True)`.
#[must_use]
pub fn render_report_json(report: &DraftReport) -> String {
    let status = if report.complete && report.passed {
        "PASS"
    } else {
        "FAIL"
    };
    let mut out = String::from("{\n");
    if !report.complete {
        out.push_str("  \"errors\": ");
        append_errors(&mut out, &report.errors);
        out.push_str(",\n  \"status\": \"FAIL\"\n}");
        return out;
    }
    let mut stage = String::from("null");
    if let Some(active) = &report.active_stage {
        stage.clear();
        append_json_string(&mut stage, active);
    }
    let wave = report
        .active_wave
        .map_or_else(|| "null".to_owned(), |value| value.to_string());
    let mut errors = String::new();
    append_errors(&mut errors, &report.errors);
    let mut status_json = String::new();
    append_json_string(&mut status_json, status);
    let fields: [(&str, String); 10] = [
        ("active_stage", stage),
        ("active_wave", wave),
        ("context_drafts", report.context_drafts.to_string()),
        ("errors", errors),
        ("p00_required_files", report.p00_required_files.to_string()),
        (
            "search_contracts_sources",
            report.search_contracts_sources.to_string(),
        ),
        (
            "search_domain_sources",
            report.search_domain_sources.to_string(),
        ),
        (
            "search_ports_sources",
            report.search_ports_sources.to_string(),
        ),
        ("status", status_json),
        ("ticket_drafts", report.ticket_drafts.to_string()),
    ];
    for (index, (key, value)) in fields.iter().enumerate() {
        out.push_str("  \"");
        out.push_str(key);
        out.push_str("\": ");
        out.push_str(value);
        if index + 1 < fields.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push('}');
    out
}

// --- validation -------------------------------------------------------------

fn require(errors: &mut Vec<String>, condition: bool, message: String) {
    if !condition {
        errors.push(message);
    }
}

fn python_list(files: &[String]) -> String {
    let mut out = String::from("[");
    for (index, file) in files.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push('\'');
        out.push_str(file);
        out.push('\'');
    }
    out.push(']');
    out
}

fn package_set(names: &[(String, &toml::Value)]) -> BTreeSet<String> {
    names.iter().map(|(name, _)| name.clone()).collect()
}

fn expected_packages() -> BTreeSet<String> {
    PACKAGES.iter().map(|name| (*name).to_owned()).collect()
}

/// Expected per-package draft pair shape.
struct Expected {
    launch: &'static str,
    precondition: &'static str,
    scope: &'static str,
    handoffs: &'static [&'static str],
    sources: Vec<String>,
    fragments: &'static [&'static str],
    ceiling: Option<i64>,
}

/// Manifest-owned ceilings resolved from the context draft manifest.
struct Ceilings {
    ordinary: Option<i64>,
    exact_pack: Option<i64>,
    fragments: Option<i64>,
    handoffs: Option<i64>,
}

/// Read-only P00 draft validation against `root` (repository root).
#[must_use]
pub fn validate_p00_ticket_drafts(root: &Path) -> DraftReport {
    let mut errors: Vec<String> = Vec::new();

    let Manifests {
        ticket: ticket_manifest,
        context: context_manifest,
        contract: contract_manifest,
        launch,
        orchestration,
    } = match load_manifests(root) {
        Ok(manifests) => manifests,
        Err(message) => return incomplete(message),
    };
    let ticket_rows = match rows(&ticket_manifest, "draft", "package") {
        Ok(found) => found,
        Err(message) => return incomplete(message),
    };
    let context_rows = match rows(&context_manifest, "draft", "package") {
        Ok(found) => found,
        Err(message) => return incomplete(message),
    };

    check_ticket_manifest(&ticket_manifest, &ticket_rows, &mut errors);
    let ceilings = check_context_manifest(&context_manifest, &context_rows, &mut errors);
    let required_names = check_required_files(&contract_manifest, root, &mut errors);
    let (contract_sources, domain_sources, port_sources) = expected_sources(&required_names);

    let expected = expected_all(
        contract_sources.clone(),
        domain_sources,
        port_sources,
        &ceilings,
    );

    for (package, spec) in &expected {
        let ticket_path = format!("swarm/ticket-drafts/p00/{package}.toml");
        let context_path = format!("swarm/context-drafts/p00/{package}.toml");
        let ticket = match load_doc(root, &ticket_path) {
            Ok(document) => document,
            Err(first) => {
                errors.push(format!("{package}: {first}"));
                continue;
            }
        };
        let context = match load_doc(root, &context_path) {
            Ok(document) => document,
            Err(message) => {
                errors.push(format!("{package}: {message}"));
                continue;
            }
        };

        check_ticket_basics(&ticket, package, spec, &mut errors);
        check_ticket_identity(&ticket, package, &context_path, &mut errors);
        check_context_pair(&context, root, package, spec, &ceilings, &mut errors);
    }

    check_launch(&launch, &orchestration, &mut errors);
    check_control_state(root, &mut errors);

    finish_report(
        &Tallies {
            ticket_drafts: ticket_rows.len(),
            context_drafts: context_rows.len(),
            p00_required_files: required_names.len(),
            contracts: contract_sources.len(),
            domains: expected[1].1.sources.len(),
            ports: expected[2].1.sources.len(),
        },
        &launch,
        errors,
    )
}

fn owned_sources(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

fn expected_sources(required_names: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut contract_sources = vec![
        "AGENTS.md".to_owned(),
        "crates/search-contracts/AGENTS.md".to_owned(),
        "docs/handoff/AUTHORITY_MAP.md".to_owned(),
        "swarm/ASSIGNMENT_PROTOCOL.md".to_owned(),
        "swarm/assignments/search-contracts.md".to_owned(),
        "docs/handoff/P00_BOOTSTRAP.md".to_owned(),
        "docs/contracts/p00/README.md".to_owned(),
        "docs/contracts/p00/manifest.toml".to_owned(),
    ];
    contract_sources.extend(
        required_names
            .iter()
            .skip(1)
            .map(|name| format!("docs/contracts/p00/{name}")),
    );
    let domain_sources = owned_sources(&[
        "AGENTS.md",
        "crates/search-domain/AGENTS.md",
        "docs/handoff/AUTHORITY_MAP.md",
        "swarm/ASSIGNMENT_PROTOCOL.md",
        "swarm/assignments/search-domain.md",
        "docs/handoff/P00_BOOTSTRAP.md",
        "docs/contracts/p00/manifest.toml",
        "docs/contracts/p00/CANONICAL_TYPES.md",
        "docs/contracts/p00/TYPE_COMPLETIONS.md",
        "docs/contracts/p00/SUPPORT_SCHEMAS.md",
        "docs/contracts/p00/CONTRACT_CHALLENGES.md",
        "docs/contracts/p00/SOURCE_GRAPH.md",
        "docs/contracts/p00/QUERY_AND_RESULTS.md",
        "docs/contracts/p00/RECIPE_RESULTS.md",
        "docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md",
        "docs/contracts/p00/REASON_CODES.md",
    ]);
    let port_sources = owned_sources(&[
        "AGENTS.md",
        "crates/search-ports/AGENTS.md",
        "docs/handoff/AUTHORITY_MAP.md",
        "swarm/ASSIGNMENT_PROTOCOL.md",
        "swarm/assignments/search-ports.md",
        "docs/handoff/P00_BOOTSTRAP.md",
        "docs/contracts/p00/manifest.toml",
        "docs/contracts/p00/CANONICAL_TYPES.md",
        "docs/contracts/p00/TYPE_REGISTRY.md",
        "docs/contracts/p00/TYPE_COMPLETIONS.md",
        "docs/contracts/p00/SUPPORT_SCHEMAS.md",
        "docs/contracts/p00/CONTRACT_CHALLENGES.md",
        "docs/contracts/p00/PORT_OPERATIONS.md",
        "docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md",
        "docs/contracts/p00/REASON_CODES.md",
    ]);
    (contract_sources, domain_sources, port_sources)
}

fn expected_all(
    contract_sources: Vec<String>,
    domain_sources: Vec<String>,
    port_sources: Vec<String>,
    ceilings: &Ceilings,
) -> [(&'static str, Expected); 3] {
    [
        (
            "search-contracts",
            expected_spec("search-contracts", contract_sources, ceilings.exact_pack),
        ),
        (
            "search-domain",
            expected_spec("search-domain", domain_sources, ceilings.ordinary),
        ),
        (
            "search-ports",
            expected_spec("search-ports", port_sources, ceilings.ordinary),
        ),
    ]
}

fn expected_spec(package: &str, sources: Vec<String>, ceiling: Option<i64>) -> Expected {
    match package {
        "search-domain" | "search-ports" => Expected {
            launch: "CONDITIONAL",
            precondition: "ACCEPTED_SEARCH_CONTRACTS_HANDOFF_REQUIRED",
            scope: if package == "search-domain" {
                "crates/search-domain/**"
            } else {
                "crates/search-ports/**"
            },
            handoffs: &["search-contracts::accepted_package_and_api_handoff"],
            sources,
            fragments: if package == "search-domain" {
                &[
                    "swarm/crates.toml::package[name=search-domain]",
                    "swarm/function-packets.toml::foundation[package=search-domain]",
                    "swarm/modules/w0.toml::package[name=search-domain]",
                    "swarm/stages.toml::stage[id=W0]",
                    "swarm/launch-state.toml::conditional_packages[search-domain]",
                    "swarm/launch-state.toml::conditional_activation.search-domain",
                ]
            } else {
                &[
                    "swarm/crates.toml::package[name=search-ports]",
                    "swarm/function-packets.toml::foundation[package=search-ports]",
                    "swarm/modules/w0.toml::package[name=search-ports]",
                    "swarm/stages.toml::stage[id=W0]",
                    "swarm/launch-state.toml::conditional_packages[search-ports]",
                    "swarm/launch-state.toml::conditional_activation.search-ports",
                ]
            },
            ceiling,
        },
        _ => Expected {
            launch: "AUTHORIZED",
            precondition: "CURRENTLY_PRESENT",
            scope: "crates/search-contracts/**",
            handoffs: &[],
            sources,
            fragments: &[
                "swarm/crates.toml::package[name=search-contracts]",
                "swarm/function-packets.toml::foundation[package=search-contracts]",
                "swarm/modules/w0.toml::package[name=search-contracts]",
                "swarm/stages.toml::stage[id=W0]",
                "swarm/launch-state.toml::authorized_packages[search-contracts]",
            ],
            ceiling,
        },
    }
}

fn check_ticket_manifest(
    manifest: &toml::Value,
    rows: &[(String, &toml::Value)],
    errors: &mut Vec<String>,
) {
    require(
        errors,
        as_int(manifest, "schema_version") == Some(2),
        "ticket manifest schema must be v2".to_owned(),
    );
    require(
        errors,
        as_int(manifest, "ticket_draft_schema_version") == Some(2),
        "ticket draft schema must be v2".to_owned(),
    );
    require(
        errors,
        as_int(manifest, "context_draft_manifest_schema_version") == Some(2),
        "context draft manifest schema must be v2".to_owned(),
    );
    require(
        errors,
        as_str(manifest, "status") == Some("DRAFT_ONLY_NOT_ISSUED"),
        "ticket manifest status changed".to_owned(),
    );
    require(
        errors,
        as_int(manifest, "draft_count") == Some(3),
        "ticket manifest draft_count must be 3".to_owned(),
    );
    require(
        errors,
        package_set(rows) == expected_packages(),
        "ticket package set mismatch".to_owned(),
    );
    for key in [
        "issued_ticket_count",
        "active_lease_count",
        "submission_count",
        "accepted_review_count",
        "package_handoff_count",
        "wave_receipt_count",
    ] {
        require(
            errors,
            as_int(manifest, key) == Some(0),
            format!("{key} must remain zero"),
        );
    }
    check_invariants(manifest, errors, "ticket");
}

fn check_invariants(manifest: &toml::Value, errors: &mut Vec<String>, kind: &str) {
    if kind == "ticket" {
        for key in [
            "draft_is_orchestration_state",
            "draft_may_authorize",
            "draft_may_create_lease",
            "draft_may_contain_lease_identity",
            "draft_may_be_writer_acknowledged",
        ] {
            require(
                errors,
                is_false(manifest, "invariants", key),
                format!("unsafe ticket invariant {key}"),
            );
        }
        for key in [
            "draft_uses_distinct_signed_payload_and_exact_file_digest_slots",
            "issued_ticket_requires_new_record",
            "issued_ticket_requires_exact_base_commit",
            "issued_ticket_requires_materialized_context",
            "issued_ticket_requires_writer_and_reviewer",
            "conditional_ticket_requires_accepted_dependency_handoffs",
        ] {
            require(
                errors,
                is_true(manifest, "invariants", key),
                format!("required ticket invariant disabled: {key}"),
            );
        }
        return;
    }
    for key in [
        "architecture_master_allowed",
        "dependency_implementation_source_allowed",
        "materialized_context_may_be_amended",
        "p00_exception_may_add_ad_hoc_sources",
    ] {
        require(
            errors,
            is_false(manifest, "invariants", key),
            format!("unsafe context invariant {key}"),
        );
    }
    for key in [
        "base_commit_required_at_materialization",
        "per_source_sha256_required",
        "registry_selector_must_match_exactly_one_record",
        "accepted_handoff_digests_required_when_declared",
        "canonical_order_required",
        "manifest_and_artifact_identities_are_distinct",
        "p00_exception_requires_manifest_closed_exact_pack",
    ] {
        require(
            errors,
            is_true(manifest, "invariants", key),
            format!("required context invariant disabled: {key}"),
        );
    }
}

fn check_context_manifest(
    manifest: &toml::Value,
    rows: &[(String, &toml::Value)],
    errors: &mut Vec<String>,
) -> Ceilings {
    require(
        errors,
        as_int(manifest, "schema_version") == Some(2),
        "context manifest schema must be v2".to_owned(),
    );
    // NOTE: Python repeats "context manifest schema must be v2" for the
    // context draft schema line; mirror the (odd but load-bearing) message.
    require(
        errors,
        as_int(manifest, "context_draft_schema_version") == Some(2),
        "context manifest schema must be v2".to_owned(),
    );
    require(
        errors,
        as_str(manifest, "status") == Some("NON_CLAIMABLE_CONTEXT_DRAFTS"),
        "context manifest status changed".to_owned(),
    );
    require(
        errors,
        as_int(manifest, "draft_count") == Some(3),
        "context manifest draft_count must be 3".to_owned(),
    );
    require(
        errors,
        as_int(manifest, "materialized_context_count") == Some(0),
        "materialized context count must remain zero".to_owned(),
    );
    require(
        errors,
        as_int(manifest, "writer_visible_artifact_count_per_context") == Some(1),
        "each context must be one writer-visible artifact".to_owned(),
    );
    require(
        errors,
        package_set(rows) == expected_packages(),
        "context package set mismatch".to_owned(),
    );
    let ceilings = Ceilings {
        ordinary: as_int(manifest, "ordinary_static_source_file_ceiling"),
        exact_pack: as_int(manifest, "p00_exact_contract_pack_source_file_ceiling"),
        fragments: as_int(manifest, "max_registry_fragments_per_context"),
        handoffs: as_int(manifest, "max_accepted_handoff_slots_per_context"),
    };
    require(
        errors,
        ceilings.ordinary == Some(ORDINARY_SOURCE_CEILING),
        "ordinary context ceiling must remain 16".to_owned(),
    );
    require(
        errors,
        ceilings.exact_pack == Some(EXACT_PACK_SOURCE_CEILING),
        "P00 exact-pack ceiling must remain 24".to_owned(),
    );
    require(
        errors,
        str_list(manifest, "p00_exact_contract_pack_exception_packages")
            == Some(vec!["search-contracts".to_owned()]),
        "P00 exception package mismatch".to_owned(),
    );
    require(
        errors,
        ceilings.fragments == Some(MAX_REGISTRY_FRAGMENTS),
        "registry fragment ceiling must remain 6".to_owned(),
    );
    require(
        errors,
        ceilings.handoffs == Some(MAX_HANDOFF_SLOTS),
        "accepted handoff ceiling must remain 1".to_owned(),
    );
    check_invariants(manifest, errors, "context");
    ceilings
}

fn check_required_files(
    manifest: &toml::Value,
    root: &Path,
    errors: &mut Vec<String>,
) -> Vec<String> {
    let mut required_names: Vec<String> = Vec::new();
    match str_list(manifest, "required_files") {
        Some(names) => required_names = names,
        None => errors.push("P00 required_files must be a string array".to_owned()),
    }
    require(
        errors,
        required_names.len() == P00_REQUIRED_FILE_COUNT,
        "P00 required_files must contain 13 files".to_owned(),
    );
    require(
        errors,
        required_names.iter().collect::<BTreeSet<_>>().len() == required_names.len(),
        "P00 required_files contains duplicates".to_owned(),
    );
    require(
        errors,
        required_names
            .first()
            .is_some_and(|first| first == "README.md"),
        "README.md must remain first in P00 required_files".to_owned(),
    );
    require(
        errors,
        required_names
            .iter()
            .any(|name| name == "TYPE_COMPLETIONS.md"),
        "TYPE_COMPLETIONS.md missing from P00 manifest".to_owned(),
    );
    for name in &required_names {
        require(
            errors,
            root.join("docs/contracts/p00").join(name).is_file(),
            format!("missing P00 contract file {name}"),
        );
    }
    required_names
}

fn check_ticket_basics(
    ticket: &toml::Value,
    package: &str,
    spec: &Expected,
    errors: &mut Vec<String>,
) {
    require(
        errors,
        as_int(ticket, "schema_version") == Some(2),
        format!("{package}: ticket schema must be v2"),
    );
    require(
        errors,
        as_str(ticket, "record_kind") == Some("assignment_ticket_draft"),
        format!("{package}: ticket kind mismatch"),
    );
    require(
        errors,
        as_str(ticket, "status") == Some("DRAFT_ONLY_NOT_ISSUED"),
        format!("{package}: ticket status changed"),
    );
    for key in [
        "claimable",
        "authorizes_implementation",
        "creates_lease",
        "may_be_writer_acknowledged",
    ] {
        require(
            errors,
            as_bool(ticket, key) == Some(false),
            format!("{package}: ticket authority flag {key}"),
        );
    }
    require(
        errors,
        as_str(ticket, "stage") == Some("W0")
            && as_str(ticket, "phase") == Some("P00")
            && as_int(ticket, "wave") == Some(0),
        format!("{package}: ticket stage mismatch"),
    );
    require(
        errors,
        as_str(ticket, "launch_class") == Some(spec.launch),
        format!("{package}: launch class mismatch"),
    );
    require(
        errors,
        as_str(ticket, "launch_precondition") == Some(spec.precondition),
        format!("{package}: launch precondition mismatch"),
    );
    require(
        errors,
        child_str(ticket, "repository_fence", "write_scope") == Some(spec.scope),
        format!("{package}: ticket write scope mismatch"),
    );
}

fn check_ticket_identity(
    ticket: &toml::Value,
    package: &str,
    context_path: &str,
    errors: &mut Vec<String>,
) {
    require(
        errors,
        child_str(ticket, "unresolved_identity", "ticket_id") == Some("UNASSIGNED"),
        format!("{package}: ticket ID prematurely assigned"),
    );
    require(
        errors,
        child_str(ticket, "unresolved_identity", "writer") == Some("UNASSIGNED")
            && child_str(ticket, "unresolved_identity", "reviewer") == Some("UNASSIGNED"),
        format!("{package}: actors prematurely assigned"),
    );
    require(
        errors,
        child_str(ticket, "unresolved_identity", "base_commit") == Some("UNSELECTED"),
        format!("{package}: base commit prematurely selected"),
    );
    require(
        errors,
        child_str(
            ticket,
            "unresolved_identity",
            "ticket_signed_payload_sha256",
        ) == Some("UNAVAILABLE"),
        format!("{package}: signed payload digest prematurely selected"),
    );
    require(
        errors,
        child_str(
            ticket,
            "unresolved_identity",
            "ticket_exact_record_file_sha256",
        ) == Some("UNAVAILABLE"),
        format!("{package}: exact file digest prematurely selected"),
    );
    require(
        errors,
        child_str(ticket, "context", "context_draft") == Some(context_path),
        format!("{package}: context draft link mismatch"),
    );
    require(
        errors,
        child(ticket, "dependencies", "accepted_handoff_refs")
            .and_then(toml::Value::as_array)
            .is_some_and(Vec::is_empty),
        format!("{package}: accepted handoff refs must remain empty"),
    );
}

fn project_strings(items: &[toml::Value]) -> Option<Vec<String>> {
    items
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

fn check_context_pair(
    context: &toml::Value,
    root: &Path,
    package: &str,
    spec: &Expected,
    ceilings: &Ceilings,
    errors: &mut Vec<String>,
) {
    require(
        errors,
        as_int(context, "schema_version") == Some(2),
        format!("{package}: context schema must be v2"),
    );
    require(
        errors,
        as_str(context, "record_kind") == Some("writer_context_draft"),
        format!("{package}: context kind mismatch"),
    );
    require(
        errors,
        as_str(context, "status") == Some("UNMATERIALIZED_DRAFT"),
        format!("{package}: context status changed"),
    );
    require(
        errors,
        as_bool(context, "claimable") == Some(false)
            && as_bool(context, "authorizes_implementation") == Some(false),
        format!("{package}: context creates authority"),
    );
    require(
        errors,
        as_str(context, "base_commit") == Some("UNSELECTED"),
        format!("{package}: context base prematurely selected"),
    );
    require(
        errors,
        as_int(context, "writer_visible_artifact_count") == Some(1),
        format!("{package}: context must be one artifact"),
    );
    // Faithful to Python: the exact-list comparisons see the raw array
    // (a non-string item fails them); lengths use the raw array length.
    let content_raw = |key: &str| {
        child(context, "content", key)
            .and_then(toml::Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let sources_raw = content_raw("source_files");
    let fragments_raw = content_raw("registry_fragments");
    let handoffs_raw = content_raw("accepted_handoff_slots");
    let owned = |items: &[&str]| {
        items
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>()
    };
    require(
        errors,
        project_strings(&sources_raw) == Some(spec.sources.clone()),
        format!("{package}: source list differs from exact bounded context"),
    );
    require(
        errors,
        project_strings(&fragments_raw) == Some(owned(spec.fragments)),
        format!("{package}: registry fragments differ"),
    );
    require(
        errors,
        project_strings(&handoffs_raw) == Some(owned(spec.handoffs)),
        format!("{package}: handoff slots differ"),
    );
    require(
        errors,
        count_is(sources_raw.len(), context, "source_file_count"),
        format!("{package}: source_file_count mismatch"),
    );
    require(
        errors,
        within_ceiling(sources_raw.len(), spec.ceiling),
        format!("{package}: source ceiling exceeded"),
    );
    require(
        errors,
        count_is(fragments_raw.len(), context, "registry_fragment_count")
            && within_ceiling(fragments_raw.len(), ceilings.fragments),
        format!("{package}: fragment count/ceiling mismatch"),
    );
    require(
        errors,
        count_is(handoffs_raw.len(), context, "accepted_handoff_slot_count")
            && within_ceiling(handoffs_raw.len(), ceilings.handoffs),
        format!("{package}: handoff count/ceiling mismatch"),
    );
    check_context_sources(
        root,
        package,
        &project_strings(&sources_raw).unwrap_or_default(),
        errors,
    );
}

fn check_context_sources(root: &Path, package: &str, sources: &[String], errors: &mut Vec<String>) {
    require(
        errors,
        sources.iter().collect::<BTreeSet<_>>().len() == sources.len(),
        format!("{package}: duplicate context source"),
    );
    for source in sources {
        require(
            errors,
            root.join(source).is_file(),
            format!("{package}: missing context source {source}"),
        );
        require(
            errors,
            !source.starts_with("docs/architecture/"),
            format!("{package}: architecture master in context"),
        );
        if source.contains("/src/") {
            require(
                errors,
                source.starts_with(&format!("crates/{package}/src/")),
                format!("{package}: dependency implementation source in context"),
            );
        }
    }
}

fn check_launch(launch: &toml::Value, orchestration: &toml::Value, errors: &mut Vec<String>) {
    require(
        errors,
        as_str(launch, "active_stage") == Some("P00") && as_int(launch, "active_wave") == Some(0),
        "launch state must remain P00/W0".to_owned(),
    );
    require(
        errors,
        str_list(launch, "authorized_packages") == Some(vec!["search-contracts".to_owned()]),
        "only search-contracts may be authorized".to_owned(),
    );
    require(
        errors,
        str_list(launch, "conditional_packages").is_some_and(|mut names| {
            names.sort();
            names == ["search-domain".to_owned(), "search-ports".to_owned()]
        }),
        "conditional package set mismatch".to_owned(),
    );
    require(
        errors,
        as_str(orchestration, "workflow_policy") == Some("manual_only"),
        "orchestration workflow policy must remain manual_only".to_owned(),
    );
}

fn check_control_state(root: &Path, errors: &mut Vec<String>) {
    for path in CONTROL_ROOTS {
        let files = machine_files(root, path);
        if !files.is_empty() {
            errors.push(format!(
                "premature control records under {path}: {}",
                python_list(&files)
            ));
        }
    }
    let completion_text =
        std::fs::read_to_string(root.join("docs/contracts/p00/TYPE_COMPLETIONS.md"))
            .unwrap_or_default();
    for token in [
        "RecipeIdV1",
        "RecipeBodyV1",
        "ComparisonAxis",
        "ProtocolRange",
        "PackageOpaque",
    ] {
        require(
            errors,
            completion_text.contains(token),
            format!("named type completion missing {token}"),
        );
    }
}
