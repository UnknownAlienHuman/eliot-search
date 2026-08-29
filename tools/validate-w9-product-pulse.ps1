[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [switch]$Json
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$errors = [System.Collections.Generic.List[string]]::new()
$warnings = [System.Collections.Generic.List[string]]::new()
function Add-Error([string]$Message) { $script:errors.Add($Message) }
function Add-Warning([string]$Message) { $script:warnings.Add($Message) }
function Read-Required([string]$RelativePath) {
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path $path -PathType Leaf)) {
        Add-Error "Missing required file: $RelativePath"
        return ""
    }
    [IO.File]::ReadAllText($path)
}
function Quoted-Values([string]$Text) {
    @([regex]::Matches($Text, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Toml-String([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML string '$Key'." }
        return ""
    }
    $match.Groups[1].Value
}
function Toml-Int([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(-?\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML integer '$Key'." }
        return 0
    }
    [int64]$match.Groups[1].Value
}
function Toml-Bool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML boolean '$Key'." }
        return $false
    }
    $match.Groups[1].Value -eq "true"
}
function Toml-Array([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $match.Success) { return @() }
    Quoted-Values $match.Groups[1].Value
}
function Toml-Raw([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(.+?)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML value '$Key'." }
        return ""
    }
    $match.Groups[1].Value.Trim()
}
function Get-Section([string]$Text, [string]$Name) {
    $match = [regex]::Match($Text, ('(?ms)^\[{0}\]\s*(.*?)(?=^\[|\z)' -f [regex]::Escape($Name)))
    if (-not $match.Success) { Add-Error "Missing TOML section [$Name]."; return "" }
    $match.Groups[1].Value
}
function Require-Tokens([string]$RelativePath, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if (-not $Text.Contains($token, [StringComparison]::Ordinal)) {
            Add-Error "${RelativePath} is missing required token: $token"
        }
    }
}
function Same-Set([string[]]$Left, [string[]]$Right) {
    $a = @($Left | Sort-Object -Unique)
    $b = @($Right | Sort-Object -Unique)
    if ($a.Count -ne $b.Count) { return $false }
    for ($i = 0; $i -lt $a.Count; $i++) {
        if ($a[$i] -cne $b[$i]) { return $false }
    }
    $true
}
function Parse-Fields([string]$Text) {
    $result = [ordered]@{}
    $matches = [regex]::Matches(
        $Text,
        '(?ms)^\[\[([A-Za-z0-9_]+)\.field\]\]\s*(.*?)(?=^\[\[[A-Za-z0-9_]+\.field\]\]|^\[[A-Za-z0-9_]+\]\s*$|\z)'
    )
    foreach ($match in $matches) {
        $section = $match.Groups[1].Value
        $block = $match.Groups[2].Value
        $name = Toml-String $block "name"
        $key = "$section.$name"
        if ($result.Contains($key)) { Add-Error "Duplicate W9 settings field '$key'."; continue }
        $result[$key] = [pscustomobject]@{
            Section = $section
            Name = $name
            Type = Toml-String $block "type"
            Mode = Toml-String $block "mode"
            DefaultRaw = Toml-Raw $block "default"
            MinRaw = Toml-Raw $block "min" $false
            MaxRaw = Toml-Raw $block "max" $false
            ChangeAction = Toml-String $block "change_action" $false
        }
    }
    $result
}

$paths = [ordered]@{
    manifest = "docs/evaluation/manifest.toml"
    contract = "docs/evaluation/W9_PRODUCT_PULSE_CONTRACTS_1.0.md"
    functions = "crates/search-eval/FUNCTIONS.md"
    agents = "crates/search-eval/AGENTS.md"
    assignment = "swarm/assignments/search-eval.md"
    handoff = "docs/handoff/W9_IMPLEMENTATION_PACKET.md"
    qualification = "qualification/product-pulse/W9_QUALIFICATION.md"
    baseline = "qualification/product-pulse/baseline.toml"
    corpus = "qualification/product-pulse/corpus.toml"
    metrics = "qualification/product-pulse/metrics.toml"
    probes = "qualification/product-pulse/probes.toml"
    gate_map = "qualification/product-pulse/gate-map.toml"
    fixture_owners = "qualification/product-pulse/fixture-owners.toml"
    settings = "config/w9-product-pulse.toml"
    settings_doc = "docs/config/W9_PRODUCT_PULSE_SETTINGS_1.0.md"
    swarm = "swarm/w9-product-pulse.toml"
    gates = "swarm/gates.toml"
    launch = "swarm/launch-state.toml"
    workflow = ".github/workflows/w9-product-pulse.yml"
}
$text = [ordered]@{}
foreach ($entry in $paths.GetEnumerator()) { $text[$entry.Key] = Read-Required $entry.Value }

if ((Toml-String $text.manifest "status") -cne "contract-only") { Add-Error "W9 manifest must remain contract-only." }
foreach ($flag in @("implementation_authorized", "product_accepted", "optional_depth_authorized")) {
    if (Toml-Bool $text.manifest $flag) { Add-Error "W9 manifest cannot set $flag=true." }
}
$ownerBlocks = [regex]::Split($text.manifest, '(?m)^\[\[owner\]\]\s*$')
$roles = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $ownerBlocks.Count; $i++) {
    $role = Toml-String $ownerBlocks[$i] "role"
    $roles.Add($role)
    if ($role -ceq "package") {
        if ((Toml-String $ownerBlocks[$i] "package") -cne "search-eval") { Add-Error "W9 package owner must be search-eval." }
        if ((Toml-String $ownerBlocks[$i] "function_contract") -cne $paths.functions) { Add-Error "W9 function contract path mismatch." }
    }
}
if (-not (Same-Set $roles.ToArray() @("package", "integration", "review"))) { Add-Error "W9 owner roles must be package/integration/review." }

Require-Tokens $paths.contract $text.contract @(
    "## 3. Frozen control corpus",
    "## 5. Windows qualification environment",
    "## 8. Hard safety and correctness blockers",
    "## 10. Material product benefit",
    "## 11. Fault and recovery matrix",
    "## 12. Protocol-flow-control stress",
    "## 16. Verdict state machine",
    "Product Pulse: NOT ACCEPTED"
)
Require-Tokens $paths.functions $text.functions @(
    '### `validate_control_corpus',
    '### `validate_acceptance_policy',
    '### `freeze_run_manifest',
    '### `plan_case_block',
    '### `validate_case_evidence',
    '### `compare_abc',
    '### `audit_content_minimization',
    '### `validate_fault_matrix',
    '### `validate_protocol_stress',
    '### `aggregate_product_pulse',
    '### `detect_hard_blockers',
    '### `decide_acceptance',
    '### `issue_product_pulse_receipt',
    "## Cancellation, retry and crash semantics",
    "## Typed failures",
    "## Required tests / qualification evidence"
)
Require-Tokens $paths.handoff $text.handoff @(
    "Package writer",
    "integration owner",
    "independent reviewer",
    "Hard stop conditions",
    "60 UNAVAILABLE"
)

if ((Toml-String $text.swarm "status") -cne "BLOCKED") { Add-Error "W9 swarm packet must remain BLOCKED." }
if ((Toml-String $text.swarm "requires_accepted_gate") -cne "G4") { Add-Error "W9 must require accepted G4." }
foreach ($flag in @("implementation_authorized", "product_accepted", "optional_depth_authorized")) {
    if (Toml-Bool $text.swarm $flag) { Add-Error "W9 swarm packet cannot set $flag=true." }
}
$packetBlocks = [regex]::Split($text.swarm, '(?m)^\[\[packet\]\]\s*$')
if ($packetBlocks.Count -ne 2) { Add-Error "W9 must contain exactly one package packet." }
else {
    $packet = $packetBlocks[1]
    if ((Toml-String $packet "package") -cne "search-eval") { Add-Error "W9 packet package must be search-eval." }
    if ((Toml-String $packet "functions") -cne $paths.functions) { Add-Error "W9 packet function path mismatch." }
    if ((Toml-String $packet "write_scope") -cne "crates/search-eval/**") { Add-Error "W9 writer scope must remain package-local." }
    foreach ($readPath in (Toml-Array $packet "read_set")) {
        if (-not (Test-Path (Join-Path $Root $readPath) -PathType Leaf)) { Add-Error "W9 read set references missing file: $readPath" }
    }
}

$launchOptional = Get-Section $text.launch "optional_depth"
if ((Toml-Int $text.launch "active_wave") -ne 0 -or (Toml-String $text.launch "active_stage") -cne "P00") {
    Add-Error "Central launch authority must remain P00/W0."
}
if (-not (Same-Set @(Toml-Array $text.launch "authorized_packages") @("search-contracts"))) {
    Add-Error "Only search-contracts may remain authorized."
}
if ((Toml-String $launchOptional "model_and_document_packages") -cne "blocked") { Add-Error "Optional depth must remain blocked." }

$expectedEvidence = @(
    "control_corpus_abc_results",
    "latency_and_resource_report",
    "fault_and_recovery_matrix",
    "source_admission_and_content_leakage_audit",
    "provider_protocol_stress",
    "explicit_product_pulse_verdict"
)
$g5Match = [regex]::Match($text.gates, '(?ms)^\[\[gate\]\]\s*id\s*=\s*"G5"(.*?)(?=^\[\[gate\]\]|\z)')
if (-not $g5Match.Success) { Add-Error "Central G5 gate block is missing." }
elseif (-not (Same-Set @(Toml-Array $g5Match.Groups[1].Value "required_evidence") $expectedEvidence)) {
    Add-Error "Central G5 evidence set differs from W9 contract."
}

if ((Toml-String $text.baseline "status") -cne "DESIGNED_NOT_EXECUTED") { Add-Error "W9 baseline must remain DESIGNED_NOT_EXECUTED." }
foreach ($flag in @("implementation_authorized", "product_accepted", "optional_depth_authorized")) {
    if (Toml-Bool $text.baseline $flag) { Add-Error "W9 baseline cannot set $flag=true." }
}
$baselineA = Get-Section $text.baseline "baseline_a"
$baselineB = Get-Section $text.baseline "baseline_b"
$candidateC = Get-Section $text.baseline "candidate_c"
$windows = Get-Section $text.baseline "windows_environment"
$experiment = Get-Section $text.baseline "experiment"
$hardSafety = Get-Section $text.baseline "hard_safety"
$selection = Get-Section $text.baseline "selection"
if ((Toml-String $baselineA "status") -cne "UNSELECTED" -or (Toml-String $baselineB "status") -cne "UNSELECTED") { Add-Error "A/B baselines must remain UNSELECTED." }
if ((Toml-String $candidateC "status") -cne "UNAVAILABLE") { Add-Error "Candidate C must remain UNAVAILABLE." }
if ((Toml-String $windows "status") -cne "UNSELECTED") { Add-Error "Windows environment must remain UNSELECTED." }
foreach ($flag in @("blocked_paired_execution", "randomized_order", "seed_captured", "warmup_observations_preserved", "cold_and_warm_separate", "preparation_cost_separate", "retry_preserves_original_failure")) {
    if (-not (Toml-Bool $experiment $flag)) { Add-Error "Required experiment flag disabled: $flag" }
}
if (Toml-Bool $experiment "network_allowed") { Add-Error "Product Pulse network access must remain disabled." }
foreach ($key in @("stale_leakage_tolerance", "access_leakage_tolerance", "secret_content_leakage_tolerance", "false_complete_negative_tolerance")) {
    if ((Toml-Int $hardSafety $key) -ne 0) { Add-Error "$key must remain zero." }
}
foreach ($flag in @("oracle_feedback_allowed", "hidden_case_removal_allowed", "failed_observation_omission_allowed", "hard_blocker_weighting_allowed", "self_acceptance_allowed", "prose_only_evidence_allowed")) {
    if (Toml-Bool $hardSafety $flag) { Add-Error "Unsafe baseline flag enabled: $flag" }
}
foreach ($flag in @("latest_allowed", "version_range_allowed", "floating_revision_allowed", "mutable_fixture_allowed", "criteria_after_candidate_allowed", "self_review_allowed", "unit_tests_only_acceptance_allowed", "compilation_only_acceptance_allowed")) {
    if (Toml-Bool $selection $flag) { Add-Error "Unsafe selection flag enabled: $flag" }
}

if ((Toml-String $text.corpus "status") -cne "UNMATERIALIZED") { Add-Error "Control corpus must remain UNMATERIALIZED." }
if ((Toml-Int $text.corpus "minimum_independent_reference_lineages") -ne 8) { Add-Error "Control corpus must require eight independent lineages." }
foreach ($flag in @("oracle_visible_to_production", "oracle_visible_to_baselines")) {
    if (Toml-Bool $text.corpus $flag) { Add-Error "Oracle visibility floor violated: $flag" }
}
if (Toml-Bool $text.corpus "network_allowed") { Add-Error "Control corpus network access must remain disabled." }
$caseDefaults = Get-Section $text.corpus "case_defaults"
if (-not (Toml-Bool $caseDefaults "mandatory")) { Add-Error "All default W9 cases must be mandatory." }
if ((Toml-String $caseDefaults "status") -cne "UNMATERIALIZED" -or (Toml-String $caseDefaults "result") -cne "UNAVAILABLE") { Add-Error "Case defaults must remain UNMATERIALIZED/UNAVAILABLE." }
$caseBlocks = [regex]::Split($text.corpus, '(?m)^\[\[case\]\]\s*$')
$caseIds = [System.Collections.Generic.List[string]]::new()
$allowedRecipes = @("locate@1", "find_text@1", "inspect_entity@1", "compare_implementations@1", "explore_entity@1", "corpus_profile@1", "corpus_delta@1", "provenance@1", "compile_exact_scan@1", "execute_exact_scan@1", "expand_handle@1")
$fixtureBlocks = [regex]::Split($text.fixture_owners, '(?m)^\[\[fixture_family\]\]\s*$')
$fixtureIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
for ($i = 1; $i -lt $fixtureBlocks.Count; $i++) { [void]$fixtureIds.Add((Toml-String $fixtureBlocks[$i] "id")) }
for ($i = 1; $i -lt $caseBlocks.Count; $i++) {
    $block = $caseBlocks[$i]
    $id = Toml-String $block "id"
    if ($caseIds.Contains($id)) { Add-Error "Duplicate W9 case ID: $id" }
    $caseIds.Add($id)
    $recipe = Toml-String $block "recipe"
    if ($allowedRecipes -cnotcontains $recipe) { Add-Error "W9 case $id uses unknown recipe $recipe." }
    $fixtureFamily = Toml-String $block "fixture_family"
    if (-not $fixtureIds.Contains($fixtureFamily)) { Add-Error "W9 case $id references unknown fixture family $fixtureFamily." }
    foreach ($key in @("family", "purpose")) { [void](Toml-String $block $key) }
}
$expectedCases = @(
    "active_local_function", "renamed_true_analogue", "same_name_false_positive", "decisive_test_edge_case",
    "caller_evidence_role", "documentation_evidence_role", "configuration_variant_split", "fork_mirror_lineage_collapse",
    "eight_independent_lineages", "nested_repository_boundary", "submodule_boundary", "ambiguous_subject",
    "provenance_chain", "multilingual_documentation", "non_ascii_path", "complete_literal_negative",
    "exact_negative_unreadable", "exact_negative_scope_drift", "comparison_non_normative", "stale_projection_readback",
    "unindexed_reference_gap", "inaccessible_membership_noninterference", "saved_overlay_precedence", "unsaved_overlay_precedence",
    "watcher_gap_currentness_denial", "resume_reconciliation", "live_head_mismatch", "access_revoke_mid_query",
    "purge_restore_nonresurrection", "point_identity_collision", "continuation_expiry", "handle_revocation",
    "source_admission_secret_canary", "content_leakage_canary", "publication_failpoint_matrix", "qdrant_restart_recovery",
    "daemon_kill_reopen", "disk_full_low_space", "sleep_resume_observation", "protocol_oversize_malformed",
    "protocol_replay_duplicate", "protocol_cancel_storm", "protocol_connection_churn", "resource_saturation",
    "background_throttle", "warm_exact_navigation_slo", "warm_lexical_query_slo", "warm_comparison_slo",
    "first_progressive_card_slo"
)
if (-not (Same-Set $caseIds.ToArray() $expectedCases)) { Add-Error "W9 control-corpus case set differs from the required 49 cases." }

if ((Toml-String $text.metrics "status") -cne "SCHEMA_ONLY") { Add-Error "Metric registry must remain SCHEMA_ONLY." }
if ((Toml-String $text.metrics "quality_acceptance_policy_status") -cne "UNSELECTED") { Add-Error "Quality acceptance policy must remain UNSELECTED." }
if (Toml-Bool $text.metrics "incompatible_run_aggregation_allowed" -or Toml-Bool $text.metrics "hard_blockers_may_be_weighted_away") { Add-Error "Unsafe metric aggregation flag enabled." }
$percentiles = Get-Section $text.metrics "percentiles"
if ((Toml-Int $percentiles "minimum_measured_samples") -ne 30) { Add-Error "Required p95 minimum sample count must be 30." }
if (-not (Same-Set @(Toml-Array $percentiles "required") @("p50", "p95"))) { Add-Error "Required percentile set must be p50/p95." }
$slo = Get-Section $text.metrics "candidate_slo"
$expectedSlo = [ordered]@{
    warm_exact_keyword_navigation_p95_ms = 100
    warm_single_scope_lexical_p95_ms = 200
    warm_cross_repository_comparison_p95_ms = 700
    first_useful_progressive_card_ms = 300
}
foreach ($entry in $expectedSlo.GetEnumerator()) { if ((Toml-Int $slo $entry.Key) -ne $entry.Value) { Add-Error "Candidate SLO $($entry.Key) differs from Architecture." } }
$qualityPolicy = Get-Section $text.metrics "quality_policy"
if ((Toml-String $qualityPolicy "policy_ref") -cne "UNSELECTED") { Add-Error "Quality policy ref must remain UNSELECTED." }
$metricBlocks = [regex]::Split($text.metrics, '(?m)^\[\[metric\]\]\s*$')
$metricIds = [System.Collections.Generic.List[string]]::new()
$hardMetricIds = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $metricBlocks.Count; $i++) {
    $block = $metricBlocks[$i]
    $id = Toml-String $block "id"
    if ($metricIds.Contains($id)) { Add-Error "Duplicate metric ID: $id" }
    $metricIds.Add($id)
    foreach ($key in @("unit", "direction", "denominator", "class", "description")) { [void](Toml-String $block $key) }
    if (Toml-Bool $block "hard_blocker") { $hardMetricIds.Add($id) }
}
$expectedMetrics = @(
    "correct_grounded_action_rate", "oracle_definition_recall", "oracle_test_recall", "oracle_documentation_recall",
    "oracle_caller_recall", "oracle_configuration_recall", "false_analogue_rate", "ambiguity_honesty_rate",
    "coverage_gap_honesty_rate", "complete_negative_false_claim_count", "stale_leakage_count", "access_leakage_count",
    "secret_content_leakage_count", "source_read_count", "source_read_bytes", "model_input_tokens", "model_output_tokens",
    "time_to_first_correct_grounded_action_ms", "request_latency_ms", "first_useful_progressive_card_ms", "process_cpu_ms",
    "system_cpu_percent", "working_set_bytes", "private_bytes", "commit_bytes", "disk_bytes_written", "disk_bytes_read",
    "background_cpu_duty_percent", "queue_depth_peak", "resource_exhausted_count", "recovery_correctness_rate",
    "recovery_latency_ms", "protocol_resource_leak_count"
)
if (-not (Same-Set $metricIds.ToArray() $expectedMetrics)) { Add-Error "W9 metric ID set differs from the required 33 metrics." }
$expectedHardMetrics = @("complete_negative_false_claim_count", "stale_leakage_count", "access_leakage_count", "secret_content_leakage_count", "recovery_correctness_rate", "protocol_resource_leak_count")
if (-not (Same-Set $hardMetricIds.ToArray() $expectedHardMetrics)) { Add-Error "W9 hard-blocker metric set is invalid." }

if ((Toml-String $text.probes "status") -cne "NOT_EXECUTED") { Add-Error "W9 probes must remain NOT_EXECUTED." }
foreach ($flag in @("raw_output_required_for_pass", "independent_review_required_for_pass", "all_mandatory_must_pass")) {
    if (-not (Toml-Bool $text.probes $flag)) { Add-Error "Required probe registry flag disabled: $flag" }
}
$probeDefaults = Get-Section $text.probes "probe_defaults"
if (-not (Toml-Bool $probeDefaults "mandatory") -or (Toml-String $probeDefaults "result") -cne "UNAVAILABLE") { Add-Error "Probe defaults must be mandatory/UNAVAILABLE." }
foreach ($key in @("raw_output_ref", "raw_output_digest", "reviewer_receipt_ref")) { if ((Toml-String $probeDefaults $key) -ne "") { Add-Error "Probe default $key must be empty." } }
$probeBlocks = [regex]::Split($text.probes, '(?m)^\[\[probe\]\]\s*$')
$probeIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$probeEvidence = [ordered]@{}
foreach ($id in $expectedEvidence) { $probeEvidence[$id] = [System.Collections.Generic.List[string]]::new() }
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $block = $probeBlocks[$i]
    $id = Toml-String $block "id"
    if (-not $probeIds.Add($id)) { Add-Error "Duplicate W9 probe ID: $id" }
    $evidenceId = Toml-String $block "evidence_id"
    if (-not $probeEvidence.Contains($evidenceId)) { Add-Error "Probe $id references unknown G5 evidence $evidenceId." }
    else { $probeEvidence[$evidenceId].Add($id) }
    [void](Toml-String $block "producer")
    [void](Toml-String $block "purpose")
    if ($block -match '(?m)^mandatory\s*=\s*false\s*$') { Add-Error "Probe $id overrides mandatory=false." }
    $resultOverride = Toml-String $block "result" $false
    if ($resultOverride -and $resultOverride -cne "UNAVAILABLE") { Add-Error "Probe $id has executed result before evidence." }
    foreach ($key in @("raw_output_ref", "raw_output_digest", "reviewer_receipt_ref")) {
        $value = Toml-String $block $key $false
        if ($value) { Add-Error "Probe $id contains premature $key." }
    }
}
if ($probeIds.Count -ne 60) { Add-Error "Expected 60 W9 probes; parsed $($probeIds.Count)." }
foreach ($evidenceId in $expectedEvidence) { if ($probeEvidence[$evidenceId].Count -ne 10) { Add-Error "G5 evidence $evidenceId must map exactly ten probes." } }

$mapBlocks = [regex]::Split($text.gate_map, '(?m)^\[\[evidence\]\]\s*$')
$mapIds = [System.Collections.Generic.List[string]]::new()
$mappedProbeIds = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $mapBlocks.Count; $i++) {
    $block = $mapBlocks[$i]
    $id = Toml-String $block "id"
    $mapIds.Add($id)
    if (-not (Toml-Bool $block "required")) { Add-Error "G5 evidence $id must be required." }
    if ((Toml-String $block "owner") -cne "search-eval") { Add-Error "G5 evidence $id must remain owned by search-eval." }
    $mapped = @(Toml-Array $block "probe_ids")
    if ($mapped.Count -ne 10) { Add-Error "G5 evidence $id must list ten probes." }
    foreach ($probeId in $mapped) {
        $mappedProbeIds.Add($probeId)
        if (-not $probeIds.Contains($probeId)) { Add-Error "Gate map references unknown W9 probe $probeId." }
    }
}
if (-not (Same-Set $mapIds.ToArray() $expectedEvidence)) { Add-Error "W9 gate-map evidence set differs from central G5." }
if (-not (Same-Set $mappedProbeIds.ToArray() @($probeIds))) { Add-Error "W9 gate map does not cover exactly all probes." }

if ((Toml-String $text.settings "status") -cne "schema-only") { Add-Error "W9 settings must remain schema-only." }
if (Toml-Bool $text.settings "implementation_authorized" -or Toml-Bool $text.settings "product_accepted") { Add-Error "W9 settings cannot authorize implementation or acceptance." }
$fields = Parse-Fields $text.settings
$lockedExpected = [ordered]@{
    "run.randomized_order" = "true"
    "run.seed_captured" = "true"
    "run.network_allowed" = "false"
    "run.cold_warm_separate" = "true"
    "run.preparation_cost_separate" = "true"
    "corpus.minimum_independent_lineages" = "8"
    "corpus.mutable_fixtures_allowed" = "false"
    "corpus.oracle_visible_to_production" = "false"
    "corpus.oracle_visible_to_baselines" = "false"
    "corpus.hidden_case_removal_allowed" = "false"
    "evidence.raw_output_required" = "true"
    "evidence.independent_review_required" = "true"
    "evidence.prose_only_allowed" = "false"
    "evidence.failed_observations_preserved" = "true"
    "evidence.unavailable_observations_preserved" = "true"
    "evidence.append_only_receipts" = "true"
    "evidence.source_content_in_report_allowed" = "false"
    "evidence.query_text_in_report_allowed" = "false"
    "evidence.secret_or_token_in_report_allowed" = "false"
    "evidence.absolute_path_in_report_allowed" = "false"
    "safety.stale_leakage_tolerance" = "0"
    "safety.access_leakage_tolerance" = "0"
    "safety.secret_content_leakage_tolerance" = "0"
    "safety.false_complete_negative_tolerance" = "0"
    "safety.hard_blocker_weighting_allowed" = "false"
    "safety.oracle_feedback_allowed" = "false"
    "safety.optional_profiles_enabled" = "false"
    "verdict.criteria_preregistered" = "true"
    "verdict.criteria_after_candidate_allowed" = "false"
    "verdict.self_review_allowed" = "false"
    "verdict.self_acceptance_allowed" = "false"
    "verdict.missing_required_evidence_can_pass" = "false"
    "verdict.unit_tests_only_can_pass" = "false"
    "verdict.compilation_only_can_pass" = "false"
    "verdict.only_accepted_receipt_unlocks_w10" = "true"
}
foreach ($entry in $lockedExpected.GetEnumerator()) {
    if (-not $fields.Contains($entry.Key)) { Add-Error "Missing locked W9 setting: $($entry.Key)"; continue }
    $field = $fields[$entry.Key]
    if ($field.Mode -cne "LOCKED") { Add-Error "$($entry.Key) must be LOCKED." }
    if ($field.DefaultRaw -cne $entry.Value) { Add-Error "$($entry.Key) default '$($field.DefaultRaw)' != '$($entry.Value)'." }
}
foreach ($key in @("run.environment_profile_ref", "run.baseline_a_ref", "run.baseline_b_ref", "run.acceptance_policy_ref", "run.raw_output_store_ref")) {
    if (-not $fields.Contains($key) -or $fields[$key].Mode -cne "QUALIFIED_REF" -or $fields[$key].DefaultRaw -cne '"UNSELECTED"') { Add-Error "$key must be an UNSELECTED QUALIFIED_REF." }
}
$tunableExpected = [ordered]@{
    "run.warmup_iterations" = @("0", "20")
    "run.measured_iterations" = @("30", "500")
    "run.case_timeout_ms" = @("1000", "600000")
    "run.block_timeout_ms" = @("60000", "7200000")
    "run.resource_sample_interval_ms" = @("50", "1000")
}
foreach ($entry in $tunableExpected.GetEnumerator()) {
    if (-not $fields.Contains($entry.Key)) { Add-Error "Missing W9 tunable: $($entry.Key)"; continue }
    $field = $fields[$entry.Key]
    if ($field.Mode -cne "TUNABLE" -or $field.MinRaw -cne $entry.Value[0] -or $field.MaxRaw -cne $entry.Value[1] -or $field.ChangeAction -cne "APPLY_NEXT_RUN") { Add-Error "Invalid bounds/action for W9 tunable $($entry.Key)." }
}
Require-Tokens $paths.settings $text.settings @("[forbidden]", "product_acceptance_by_configuration = true", "optional_depth_by_configuration = true", "post_hoc_criteria = true", "self_acceptance = true")
Require-Tokens $paths.settings_doc $text.settings_doc @("LOCKED", "TUNABLE", "QUALIFIED_REF", "thirty measured observations", "Product acceptance and W10 authorization are receipts")

if ($text.workflow -notmatch '(?ms)^on:\s*\r?\n\s*workflow_dispatch:\s*$') { Add-Error "W9 workflow must be manual workflow_dispatch only." }
if ($text.workflow -match '(?m)^\s*(pull_request|push|schedule):') { Add-Error "W9 workflow must not run automatically." }
Require-Tokens $paths.workflow $text.workflow @("permissions:", "contents: read", "persist-credentials: false", "validate-w9-product-pulse.ps1")

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    owner_roles = $roles.Count
    corpus_cases = $caseIds.Count
    metrics = $metricIds.Count
    hard_metrics = $hardMetricIds.Count
    probes = $probeIds.Count
    gate_evidence_ids = $mapIds.Count
    status = Toml-String $text.swarm "status"
    product_pulse = "NOT_ACCEPTED"
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host "ELIOT Search W9 Product Pulse contract validation"
    Write-Host "owners=$($result.owner_roles) cases=$($result.corpus_cases) metrics=$($result.metrics) probes=$($result.probes) status=$($result.status)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host "PASS" -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
