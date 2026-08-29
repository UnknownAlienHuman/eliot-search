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
function Get-QuotedValues([string]$Text) {
    @([regex]::Matches($Text, '"([^"\r\n]*)"') | ForEach-Object { $_.Groups[1].Value })
}
function Get-TomlString([string]$Text, [string]$Key, [bool]$Required = $true) {
    $pattern = '(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f ([regex]::Escape($Key))
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing string key '$Key'." }
        return $null
    }
    $match.Groups[1].Value
}
function Get-TomlInt([string]$Text, [string]$Key, [bool]$Required = $true) {
    $pattern = '(?m)^{0}\s*=\s*(\d+)\s*$' -f ([regex]::Escape($Key))
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing integer key '$Key'." }
        return $null
    }
    [int64]$match.Groups[1].Value
}
function Get-TomlBool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $pattern = '(?m)^{0}\s*=\s*(true|false)\s*$' -f ([regex]::Escape($Key))
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing boolean key '$Key'." }
        return $null
    }
    $match.Groups[1].Value -eq "true"
}
function Get-TomlArray([string]$Text, [string]$Key) {
    $pattern = '(?ms)^{0}\s*=\s*\[(.*?)\]' -f ([regex]::Escape($Key))
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) { return @() }
    Get-QuotedValues $match.Groups[1].Value
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
function Read-Required([string]$RelativePath) {
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path $path -PathType Leaf)) {
        Add-Error "Missing required file: $RelativePath"
        return ""
    }
    [IO.File]::ReadAllText($path)
}

$registryText = Read-Required "swarm/crates.toml"
$launchText = Read-Required "swarm/launch-state.toml"
$gateText = Read-Required "swarm/gates.toml"
$qualificationText = Read-Required "qualification/query/W4_QUALIFICATION.md"
$baselineText = Read-Required "qualification/query/baseline.toml"
$probeText = Read-Required "qualification/query/probes.toml"
[void](Read-Required "qualification/query/README.md")

$packageBlocks = [regex]::Split($registryText, '(?m)^\[\[package\]\]\s*$')
$registryPreamble = $packageBlocks[0]
$packages = [ordered]@{}
for ($i = 1; $i -lt $packageBlocks.Count; $i++) {
    $block = $packageBlocks[$i]
    $name = Get-TomlString $block "name"
    if ([string]::IsNullOrWhiteSpace($name)) { continue }
    if ($packages.Contains($name)) { Add-Error "Duplicate package '$name'."; continue }
    $packages[$name] = [pscustomobject]@{
        Name = $name
        Functions = Get-TomlString $block "functions" $false
        Qualification = Get-TomlString $block "qualification" $false
    }
}

if ((Get-TomlString $registryPreamble "query_qualification_path") -cne "qualification/query/W4_QUALIFICATION.md") {
    Add-Error "Registry query_qualification_path is missing or incorrect."
}
if ((Get-TomlString $launchText "query_qualification") -cne "qualification/query/W4_QUALIFICATION.md") {
    Add-Error "Launch state does not reference the W4 qualification contract."
}

$expectedPackets = [ordered]@{
    "search-access" = "crates/search-query/search-access/FUNCTIONS.md"
    "search-query-planner" = "crates/search-query/search-query-planner/FUNCTIONS.md"
    "search-retrieval-executor" = "crates/search-query/search-retrieval-executor/FUNCTIONS.md"
    "search-candidate-validator" = "crates/search-query/search-candidate-validator/FUNCTIONS.md"
    "search-handles" = "crates/search-query/search-handles/FUNCTIONS.md"
    "search-result-projector" = "crates/search-query/search-result-projector/FUNCTIONS.md"
    "search-continuation" = "crates/search-query/search-continuation/FUNCTIONS.md"
    "search-eval" = "crates/search-eval/FUNCTIONS.md"
    "search-provider-protocol" = "crates/search-provider-protocol/FUNCTIONS.md"
}

foreach ($entry in $expectedPackets.GetEnumerator()) {
    if (-not $packages.Contains($entry.Key)) {
        Add-Error "Missing W4 package '$($entry.Key)' in registry."
        continue
    }
    $package = $packages[$entry.Key]
    if ($package.Functions -cne $entry.Value) {
        Add-Error "$($entry.Key) functions path mismatch: '$($package.Functions)' != '$($entry.Value)'."
    }
    if ($package.Qualification -cne "qualification/query/W4_QUALIFICATION.md") {
        Add-Error "$($entry.Key) is not bound to W4 qualification."
    }
    $functionText = Read-Required $entry.Value
    foreach ($pattern in @(
        '(?m)^# Function contract',
        '(?m)^### `',
        '(?im)^## Required (fixtures|tests|conformance|qualification)',
        '(?i)(cancellation|cancel|deadline)',
        '(?i)(failure|error|reject)',
        '(?i)(bounded|finite|limit|quota|ceiling)'
    )) {
        if ($functionText -and $functionText -notmatch $pattern) {
            Add-Error "Function packet '$($entry.Value)' does not satisfy pattern '$pattern'."
        }
    }
    if ($functionText -match '(?i)\b(TODO|TBD|placeholder success)\b') {
        Add-Error "Function packet '$($entry.Value)' contains unresolved placeholder language."
    }
}

if (-not $packages.Contains("eliot-searchd") -or $packages["eliot-searchd"].Qualification -cne "qualification/query/W4_QUALIFICATION.md") {
    Add-Error "eliot-searchd is not bound to W4 end-to-end qualification."
}

$expectedRecipes = @(
    "locate@1", "find_text@1", "inspect_entity@1", "compare_implementations@1",
    "explore_entity@1", "find_callers@1", "find_tests@1", "find_configuration@1",
    "exact_scan@1", "expand_handle@1", "continue@1"
)
$recipeIds = @(Get-TomlArray $baselineText "ids")
if ((Get-TomlInt $baselineText "count") -ne 11 -or -not (Same-Set $recipeIds $expectedRecipes)) {
    Add-Error "W4 baseline must contain exactly the eleven v1 recipes."
}
if ((Get-TomlString $baselineText "status") -cne "DESIGNED_NOT_EXECUTED") {
    Add-Error "W4 baseline must remain DESIGNED_NOT_EXECUTED before evidence acceptance."
}
if ((Get-TomlInt $baselineText "max_frame_bytes") -ne 8388608) { Add-Error "Protocol frame ceiling is not 8 MiB." }
if ((Get-TomlInt $baselineText "max_in_flight_per_connection") -ne 32) { Add-Error "Protocol in-flight ceiling is not 32." }

$requiredTrue = @(
    "pairing_required", "monotonic_connection_sequence", "all_s14_axes_required",
    "queues_bounded", "pins_process_local", "exact_revision_readback_required",
    "live_security_precheck_required", "live_security_pre_emission_required",
    "complete_scope_requires_exact_proof", "truncation_must_record_omission"
)
$requiredFalse = @(
    "compression", "fragmented_messages", "client_vendor_plan_allowed", "zero_means_unlimited",
    "raw_score_cross_population_allowed", "ordinary_read_control_writes_allowed",
    "qdrant_payload_is_evidence", "validation_gap_contains_evidence", "full_file_default_allowed",
    "public_token_self_describing", "possession_grants_access", "ephemeral_restart_valid",
    "durable_unsaved_allowed", "server_stores_plaintext_token", "public_raw_qdrant_cursor_allowed",
    "silent_refresh_to_newer_corpus_allowed", "durable_ordinary_query_allowed",
    "durable_record_owns_process_pin"
)
foreach ($key in $requiredTrue) {
    if ((Get-TomlBool $baselineText $key) -ne $true) { Add-Error "W4 baseline requires $key=true." }
}
foreach ($key in $requiredFalse) {
    if ((Get-TomlBool $baselineText $key) -ne $false) { Add-Error "W4 baseline requires $key=false." }
}
if ((Get-TomlString $baselineText "contaminated_leg_policy") -cne "discard_or_replan_whole_leg") {
    Add-Error "W4 baseline permits post-filter-only contaminated ordering."
}
if ((Get-TomlInt $baselineText "default_recommended_handles_min") -ne 2 -or
    (Get-TomlInt $baselineText "default_recommended_handles_max") -ne 4) {
    Add-Error "Default recommended handle range must be 2..4."
}

$expectedOwners = [ordered]@{
    "protocol_frame_golden_and_allocation_bound" = "search-provider-protocol"
    "protocol_pairing_version_and_replay" = "search-provider-protocol"
    "protocol_inflight_cancel_disconnect_cleanup" = "search-provider-protocol"
    "grant_binding_expiry_revocation_matrix" = "search-access"
    "scope_intersection_never_widens" = "search-access"
    "retrieval_idf_predicate_equivalence" = "search-access"
    "access_noninterference_order_count_trace" = "search-access"
    "overlap_proof_required_and_drifted" = "search-access"
    "security_barrier_crash_matrix" = "search-access"
    "revocation_at_every_query_checkpoint" = "search-access"
    "exact_s14_snapshot_axes" = "search-query-planner"
    "direct_and_indexed_plan_tag_validity" = "search-query-planner"
    "deterministic_recipe_plan_fingerprint" = "search-query-planner"
    "observation_gap_strict_currentness_denied" = "search-query-planner"
    "client_vendor_plan_filter_rejected" = "search-query-planner"
    "finite_plan_budget_property" = "search-query-planner"
    "lane_priority_and_bounded_saturation" = "search-retrieval-executor"
    "cancel_disconnect_release_all_pins" = "search-retrieval-executor"
    "raw_score_population_isolation" = "search-retrieval-executor"
    "contaminated_leg_whole_discard" = "search-retrieval-executor"
    "ordinary_query_zero_control_writes" = "search-retrieval-executor"
    "exact_revision_source_backing" = "search-candidate-validator"
    "stale_unreadable_gap_has_no_evidence" = "search-candidate-validator"
    "overlay_shadow_and_live_emission_recheck" = "search-candidate-validator"
    "compact_result_and_deterministic_truncation" = "search-result-projector"
    "complete_scope_requires_exact_proof" = "search-result-projector"
    "validated_only_recipe_results" = "search-result-projector"
    "default_two_to_four_exact_handles" = "search-result-projector"
    "source_handle_token_opacity_and_redaction" = "search-handles"
    "source_handle_live_reauthorization" = "search-handles"
    "ephemeral_and_durable_handle_lifecycle" = "search-handles"
    "continuation_token_no_cursor_or_fence" = "search-continuation"
    "ephemeral_continuation_restart_and_pin_bounds" = "search-continuation"
    "durable_replan_checkpoint_constraints" = "search-continuation"
    "snapshot_expired_no_silent_refresh" = "search-continuation"
    "content_minimized_telemetry_and_errors" = "search-eval"
    "direct_indexed_raw_baseline_comparison" = "search-eval"
    "truthful_direct_degradation_when_index_unavailable" = "eliot-searchd"
}

$probeBlocks = [regex]::Split($probeText, '(?m)^\[\[probe\]\]\s*$')
$probes = [ordered]@{}
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $block = $probeBlocks[$i]
    $id = Get-TomlString $block "id"
    if ([string]::IsNullOrWhiteSpace($id)) { continue }
    if ($probes.Contains($id)) { Add-Error "Duplicate W4 probe '$id'."; continue }
    $probes[$id] = [pscustomobject]@{
        Id = $id
        Owner = Get-TomlString $block "owner"
        Mandatory = Get-TomlBool $block "mandatory"
        Result = Get-TomlString $block "result"
        RawOutputRef = Get-TomlString $block "raw_output_ref"
        ReviewerReceiptRef = Get-TomlString $block "reviewer_receipt_ref"
    }
}
if ($probes.Count -ne $expectedOwners.Count -or -not (Same-Set @($probes.Keys) @($expectedOwners.Keys))) {
    Add-Error "W4 probe registry does not contain exactly the expected $($expectedOwners.Count) probes."
}

$probeStatus = Get-TomlString $probeText "status"
if ($probeStatus -notin @("NOT_EXECUTED", "EXECUTED", "QUALIFIED")) {
    Add-Error "Unknown W4 probe status '$probeStatus'."
}
foreach ($entry in $expectedOwners.GetEnumerator()) {
    if (-not $probes.Contains($entry.Key)) { continue }
    $probe = $probes[$entry.Key]
    if ($probe.Owner -cne $entry.Value) { Add-Error "Probe '$($entry.Key)' owner mismatch." }
    if (-not $probe.Mandatory) { Add-Error "Probe '$($entry.Key)' must be mandatory." }
    if ($probe.Result -notin @("PASS", "FAIL", "UNAVAILABLE")) { Add-Error "Probe '$($entry.Key)' has invalid result '$($probe.Result)'." }
    if ($probeStatus -eq "NOT_EXECUTED") {
        if ($probe.Result -cne "UNAVAILABLE" -or $probe.RawOutputRef -ne "" -or $probe.ReviewerReceiptRef -ne "") {
            Add-Error "NOT_EXECUTED probe '$($entry.Key)' must remain UNAVAILABLE with empty evidence refs."
        }
    } elseif ($probe.Result -ne "UNAVAILABLE") {
        if ([string]::IsNullOrWhiteSpace($probe.RawOutputRef) -or [string]::IsNullOrWhiteSpace($probe.ReviewerReceiptRef)) {
            Add-Error "Executed probe '$($entry.Key)' lacks raw output or reviewer receipt."
        }
    }
    if ($probeStatus -eq "QUALIFIED" -and $probe.Result -cne "PASS") {
        Add-Error "QUALIFIED W4 registry contains non-PASS probe '$($entry.Key)'."
    }
}

foreach ($term in @(
    "## Stop conditions",
    "post-filter-only",
    "exact source-revision readback",
    "opaque source handles",
    "silent newer-corpus continuation",
    "PASS | FAIL | UNAVAILABLE"
)) {
    if ($qualificationText -and -not $qualificationText.Contains($term)) {
        Add-Error "W4 qualification contract lacks required term '$term'."
    }
}

$requiredGateEvidence = @(
    "access_idf_noninterference",
    "exact_query_snapshot_and_plan_determinism",
    "bounded_scheduler_cancellation_and_pin_release",
    "source_backed_candidate_validation",
    "bounded_deterministic_lexical_cards",
    "opaque_handle_and_continuation_lifecycle",
    "truthful_query_partial_and_degradation",
    "query_content_minimization_audit"
)
foreach ($evidenceId in $requiredGateEvidence) {
    if ($gateText -notmatch ('(?m)^\s*"' + [regex]::Escape($evidenceId) + '"[,]?\s*$')) {
        Add-Error "G2 gate registry lacks W4 evidence '$evidenceId'."
    }
}

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    function_packets = $expectedPackets.Count
    recipes = $recipeIds.Count
    probes = $probes.Count
    probe_status = $probeStatus
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host "ELIOT Search W4 query-packet validation"
    Write-Host "function_packets=$($result.function_packets) recipes=$($result.recipes) probes=$($result.probes) status=$probeStatus"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host "PASS" -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
