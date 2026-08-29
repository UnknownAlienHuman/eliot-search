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
function Get-QuotedValues([string]$Text) {
    @([regex]::Matches($Text, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Get-TomlString([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML string '$Key'." }
        return ""
    }
    $match.Groups[1].Value
}
function Get-TomlInt([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML integer '$Key'." }
        return 0
    }
    [int64]$match.Groups[1].Value
}
function Get-TomlBool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML boolean '$Key'." }
        return $false
    }
    $match.Groups[1].Value -eq "true"
}
function Get-TomlArray([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $match.Success) { return @() }
    Get-QuotedValues $match.Groups[1].Value
}
function Require-Tokens([string]$RelativePath, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if (-not $Text.Contains($token, [StringComparison]::Ordinal)) {
            Add-Error "$RelativePath is missing required token: $token"
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

$registryText = Read-Required "swarm/crates.toml"
$launchText = Read-Required "swarm/launch-state.toml"
$qualificationText = Read-Required "qualification/current/W5_QUALIFICATION.md"
$baselineText = Read-Required "qualification/current/baseline.toml"
$probeText = Read-Required "qualification/current/probes.toml"
$handoffText = Read-Required "docs/handoff/W5_IMPLEMENTATION_PACKET.md"

$blocks = [regex]::Split($registryText, '(?m)^\[\[package\]\]\s*$')
$preamble = $blocks[0]
$packages = [ordered]@{}
for ($i = 1; $i -lt $blocks.Count; $i++) {
    $block = $blocks[$i]
    $name = Get-TomlString $block "name"
    if ([string]::IsNullOrWhiteSpace($name)) { continue }
    if ($packages.Contains($name)) { Add-Error "Duplicate package '$name'."; continue }
    $packages[$name] = [pscustomobject]@{
        Name = $name
        Wave = [int](Get-TomlInt $block "wave")
        Functions = Get-TomlString $block "functions" $false
        Qualification = Get-TomlString $block "qualification" $false
        ConfigSections = @(Get-TomlArray $block "config_sections")
    }
}

$queryQualification = Get-TomlString $preamble "query_qualification_path" $false
$currentQualification = Get-TomlString $preamble "current_workspace_qualification_path" $false
if ($queryQualification -cne "qualification/query/W4_QUALIFICATION.md") {
    Add-Error "Registry query_qualification_path is missing or incorrect."
}
if ($currentQualification -cne "qualification/current/W5_QUALIFICATION.md") {
    Add-Error "Registry current_workspace_qualification_path is missing or incorrect."
}
if ((Get-TomlString $launchText "query_qualification" $false) -cne $queryQualification) {
    Add-Error "Launch-state query qualification does not match registry."
}
if ((Get-TomlString $launchText "current_workspace_qualification" $false) -cne $currentQualification) {
    Add-Error "Launch-state current-workspace qualification does not match registry."
}
if ((Get-TomlInt $launchText "active_wave") -ne 0) { Add-Error "Current launch wave must remain W0." }
if ((Get-TomlString $launchText "active_stage") -cne "P00") { Add-Error "Current launch stage must remain P00." }
$authorized = @(Get-TomlArray $launchText "authorized_packages")
if (-not (Same-Set $authorized @("search-contracts"))) { Add-Error "Only search-contracts may be authorized." }

$w4 = [ordered]@{
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
foreach ($entry in $w4.GetEnumerator()) {
    if (-not $packages.Contains($entry.Key)) { Add-Error "Missing W4 package $($entry.Key)."; continue }
    $package = $packages[$entry.Key]
    if ($package.Functions -cne $entry.Value) { Add-Error "$($entry.Key) is not linked to $($entry.Value)." }
    if ($package.Qualification -cne $queryQualification) { Add-Error "$($entry.Key) is not linked to W4 qualification." }
    [void](Read-Required $entry.Value)
}
if (-not $packages.Contains("eliot-searchd") -or $packages["eliot-searchd"].Qualification -cne $queryQualification) {
    Add-Error "eliot-searchd must be linked to the W4 qualification packet."
}

$w5 = [ordered]@{
    "search-source-reconcile" = "crates/search-source/search-source-reconcile/FUNCTIONS.md"
    "search-overlay" = "crates/search-query/search-overlay/FUNCTIONS.md"
    "search-code-enricher" = "crates/search-prep/search-code-enricher/FUNCTIONS.md"
}
foreach ($entry in $w5.GetEnumerator()) {
    if (-not $packages.Contains($entry.Key)) { Add-Error "Missing W5 package $($entry.Key)."; continue }
    $package = $packages[$entry.Key]
    if ($package.Wave -ne 5) { Add-Error "$($entry.Key) must remain W5." }
    if ($package.Functions -cne $entry.Value) { Add-Error "$($entry.Key) is not linked to $($entry.Value)." }
    if ($package.Qualification -cne $currentQualification) { Add-Error "$($entry.Key) is not linked to W5 qualification." }
    [void](Read-Required $entry.Value)
}
if ($packages["search-source-reconcile"].ConfigSections -cnotcontains "reconcile") {
    Add-Error "search-source-reconcile must own reconcile configuration."
}
if ($packages["search-overlay"].ConfigSections -cnotcontains "overlay") {
    Add-Error "search-overlay must own overlay configuration."
}

$reconcilePath = $w5["search-source-reconcile"]
$overlayPath = $w5["search-overlay"]
$codePath = $w5["search-code-enricher"]
$reconcile = Read-Required $reconcilePath
$overlay = Read-Required $overlayPath
$code = Read-Required $codePath

Require-Tokens $reconcilePath $reconcile @(
    "## `open_observation_gap`",
    "## `execute_inventory_slice`",
    "## `commit_reconcile`",
    "## `recover_reconcile_commit`",
    "## `classify_freshness`",
    "## `preflight_current_workspace`",
    "partial/cancelled/timed-out inventory",
    "## Typed failures",
    "## Required tests / qualification evidence"
)
Require-Tokens $overlayPath $overlay @(
    "## `attach_unsaved_snapshot`",
    "## `replace_unsaved_snapshot`",
    "## `compute_shadow_set`",
    "## `retrieve_overlay`",
    "## `prepare_save_admission`",
    "process-memory-only",
    "restart destroys unsaved",
    "## Typed failures",
    "## Required tests / qualification evidence"
)
Require-Tokens $codePath $code @(
    "## `validate_parser_profile`",
    "## `parse_rust_no_execute`",
    "## `extract_structural_facts`",
    "## `extract_configuration_predicate`",
    "tolerant_syntax",
    "compiler truth",
    "proc-macro",
    "## Typed failures",
    "## Required tests / qualification evidence"
)
Require-Tokens "qualification/current/W5_QUALIFICATION.md" $qualificationText @(
    "watchers and USN records are hints",
    "unsaved bytes never enter redb, CAS, Qdrant",
    "one exact parser/grammar/query/profile identity",
    "## Stop conditions",
    "## Current disposition"
)
Require-Tokens "docs/handoff/W5_IMPLEMENTATION_PACKET.md" $handoffText @(
    "search-source-reconcile",
    "search-overlay",
    "search-code-enricher",
    "Hard stop conditions",
    "Handoff requirements"
)

if ((Get-TomlString $baselineText "status") -cne "DESIGNED_NOT_EXECUTED") {
    Add-Error "W5 baseline must remain DESIGNED_NOT_EXECUTED."
}
if (Get-TomlBool $baselineText "implementation_authorized") {
    Add-Error "W5 baseline must not authorize implementation."
}
foreach ($lockedFalse in @(
    "quiet_watcher_is_currentness_proof",
    "partial_inventory_is_complete",
    "cancelled_inventory_is_complete",
    "exact_negative_allows_relaxed_observed_mode",
    "live_head_mismatch_emits_current_evidence",
    "unsaved_durable_allowed",
    "unsaved_redb_allowed",
    "unsaved_cas_allowed",
    "unsaved_qdrant_allowed",
    "budget_failure_may_unshadow_base",
    "restart_recovers_unsaved_bytes",
    "durable_handle_to_unsaved_allowed",
    "compiler_truth",
    "execute_build_scripts",
    "expand_procedural_macros",
    "execute_shell",
    "network_allowed",
    "execute_lsp_or_compiler_build_commands",
    "cfg_unknown_treated_unconditional",
    "malformed_input_may_claim_complete",
    "vendor_types_public"
)) {
    if (Get-TomlBool $baselineText $lockedFalse) { Add-Error "Unsafe W5 baseline flag enabled: $lockedFalse" }
}
foreach ($lockedTrue in @(
    "watchers_are_hints",
    "startup_reconcile_required",
    "overflow_reconcile_required",
    "cursor_gap_opens_before_ack",
    "current_requires_continuous_cursor",
    "current_requires_complete_verified_inventory",
    "unsaved_authenticated_binding_required",
    "unsaved_memory_only",
    "shadow_installed_before_snapshot_visible",
    "saved_overlay_requires_immutable_revision",
    "save_transition_requires_revision_receipt",
    "configuration_predicates_required"
)) {
    if (-not (Get-TomlBool $baselineText $lockedTrue)) { Add-Error "Required W5 baseline flag disabled: $lockedTrue" }
}
foreach ($unselected in @("parser_profile_status", "parser_source", "parser_version", "grammar_source_checksum")) {
    if ((Get-TomlString $baselineText $unselected) -cne "UNSELECTED") {
        Add-Error "$unselected must remain UNSELECTED before qualification."
    }
}

$probeBlocks = [regex]::Split($probeText, '(?m)^\[\[probe\]\]\s*$')
$probeIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$probeOwners = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$probeCount = 0
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $block = $probeBlocks[$i]
    $id = Get-TomlString $block "id"
    $owner = Get-TomlString $block "owner"
    $mandatory = Get-TomlBool $block "mandatory"
    $result = Get-TomlString $block "result"
    $probeCount++
    if (-not $probeIds.Add($id)) { Add-Error "Duplicate W5 probe id '$id'." }
    [void]$probeOwners.Add($owner)
    if (-not $mandatory) { Add-Error "W5 probe '$id' must be mandatory." }
    if ($result -cne "UNAVAILABLE") { Add-Error "W5 probe '$id' must remain UNAVAILABLE before execution." }
}
if ($probeCount -ne 42) { Add-Error "Expected 42 W5 probes; parsed $probeCount." }
if (-not (Same-Set @($probeOwners) @("search-source-reconcile", "search-overlay", "search-code-enricher"))) {
    Add-Error "W5 probe owner set is invalid."
}
$mandatoryProbeIds = @(
    "watcher_overflow_gap_before_ack",
    "bounded_multislice_no_early_complete",
    "commit_timeout_readback_idempotency",
    "live_head_mismatch_shadow_drop",
    "attach_shadow_before_snapshot_visibility",
    "replacement_has_no_stale_base_window",
    "unsaved_absent_from_redb_cas_qdrant",
    "unsaved_absent_from_backup_restore_crash_attachments",
    "restart_destroys_unsaved_bytes_and_tokens",
    "durable_handle_and_continuation_unsaved_rejected",
    "exact_rust_parser_grammar_profile_identity",
    "no_execute_build_proc_macro_lsp_shell_network",
    "malformed_parse_degraded_not_compiler_truth",
    "cfg_predicate_variant_separation",
    "parser_cancellation_and_resource_limits",
    "parser_vendor_type_public_api_guard"
)
foreach ($id in $mandatoryProbeIds) {
    if (-not $probeIds.Contains($id)) { Add-Error "Missing mandatory W5 probe '$id'." }
}

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    packages = $packages.Count
    w4_function_links = $w4.Count
    w5_function_links = $w5.Count
    w5_probes = $probeCount
    launch_stage = Get-TomlString $launchText "active_stage"
    launch_wave = Get-TomlInt $launchText "active_wave"
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host "ELIOT Search W4/W5 packet validation"
    Write-Host "packages=$($result.packages) w4_links=$($result.w4_function_links) w5_links=$($result.w5_function_links) w5_probes=$($result.w5_probes)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host "PASS" -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
