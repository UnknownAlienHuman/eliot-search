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
$gatesText = Read-Required "swarm/gates.toml"
$qualificationText = Read-Required "qualification/proof/W6_QUALIFICATION.md"
$baselineText = Read-Required "qualification/proof/baseline.toml"
$profilesText = Read-Required "qualification/proof/profiles.toml"
$probeText = Read-Required "qualification/proof/probes.toml"
$handoffText = Read-Required "docs/handoff/W6_IMPLEMENTATION_PACKET.md"

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
    }
}

$proofQualification = Get-TomlString $preamble "proof_qualification_path" $false
if ($proofQualification -cne "qualification/proof/W6_QUALIFICATION.md") {
    Add-Error "Registry proof_qualification_path is missing or incorrect."
}
if ((Get-TomlString $launchText "proof_qualification" $false) -cne $proofQualification) {
    Add-Error "Launch-state proof qualification does not match registry."
}
if ((Get-TomlInt $launchText "package_registry_schema_version") -ne 7) {
    Add-Error "Launch-state package_registry_schema_version must be 7."
}
if ((Get-TomlInt $launchText "active_wave") -ne 0 -or (Get-TomlString $launchText "active_stage") -cne "P00") {
    Add-Error "Current launch authority must remain P00/W0."
}
if (-not (Same-Set @(Get-TomlArray $launchText "authorized_packages") @("search-contracts"))) {
    Add-Error "Only search-contracts may be authorized."
}

$w6 = [ordered]@{
    "search-subject-resolver" = "crates/search-query/search-subject-resolver/FUNCTIONS.md"
    "search-comparator" = "crates/search-query/search-comparator/FUNCTIONS.md"
    "search-exact" = "crates/search-query/search-exact/FUNCTIONS.md"
}
foreach ($entry in $w6.GetEnumerator()) {
    if (-not $packages.Contains($entry.Key)) { Add-Error "Missing W6 package $($entry.Key)."; continue }
    $package = $packages[$entry.Key]
    if ($package.Wave -ne 6) { Add-Error "$($entry.Key) must remain W6." }
    if ($package.Functions -cne $entry.Value) { Add-Error "$($entry.Key) is not linked to $($entry.Value)." }
    if ($package.Qualification -cne $proofQualification) { Add-Error "$($entry.Key) is not linked to W6 qualification." }
    [void](Read-Required $entry.Value)
}

$resolverPath = $w6["search-subject-resolver"]
$comparatorPath = $w6["search-comparator"]
$exactPath = $w6["search-exact"]
$resolverText = Read-Required $resolverPath
$comparatorText = Read-Required $comparatorPath
$exactText = Read-Required $exactPath

Require-Tokens $resolverPath $resolverText @(
    "## `resolve_explicit_reference`",
    "## `resolve_qualified_key`",
    "## `resolve_exact_name`",
    "## `resolve_signature_and_kind`",
    "## `resolve_structural_lexical_candidates`",
    "## `build_ambiguity_set`",
    "## `revalidate_resolution`",
    "higher-priority incomplete",
    "## Typed failures and reasons",
    "## Required tests / qualification evidence"
)
Require-Tokens $comparatorPath $comparatorText @(
    "## `collapse_repository_lineages`",
    "## `align_evidence_roles`",
    "## `partition_configuration_variants`",
    "## `compare_axes`",
    "## `compute_comparison_coverage`",
    "## `assemble_behavior_set`",
    "NORMATIVE_VERDICT_FORBIDDEN",
    "## Required tests / qualification evidence"
)
Require-Tokens $exactPath $exactText @(
    "## `validate_predicate_profile`",
    "## `freeze_denominator`",
    "## `compile_exact_scan`",
    "## `execute_item`",
    "## `execute_exact_scan`",
    "## `classify_completeness`",
    "NO_MATCH_IN_COMPLETE_SCOPE",
    "Qdrant/top-k/lexical/semantic candidates never define or narrow the denominator",
    "## Typed failures and reasons",
    "## Required tests / qualification evidence"
)
Require-Tokens "qualification/proof/W6_QUALIFICATION.md" $qualificationText @(
    "## Mandatory properties",
    "### Subject resolution",
    "### Cross-repository comparison",
    "### Exact proof",
    "## Stop conditions",
    "## Current disposition"
)
Require-Tokens "docs/handoff/W6_IMPLEMENTATION_PACKET.md" $handoffText @(
    "search-subject-resolver",
    "search-comparator",
    "search-exact",
    "Cross-package invariants",
    "Hard stop conditions",
    "Handoff requirements"
)

if ((Get-TomlString $baselineText "status") -cne "DESIGNED_NOT_EXECUTED") {
    Add-Error "W6 baseline must remain DESIGNED_NOT_EXECUTED."
}
if (Get-TomlBool $baselineText "implementation_authorized") {
    Add-Error "W6 baseline must not authorize implementation."
}
foreach ($lockedFalse in @(
    "semantic_ladder_enabled",
    "invalid_explicit_reference_may_fall_through",
    "same_name_proves_identity",
    "top_rank_proves_identity",
    "mixed_source_views_allowed",
    "mixed_security_fences_allowed",
    "higher_priority_incomplete_allows_lower_resolved",
    "normative_verdict_allowed",
    "correct_implementation_claim_allowed",
    "best_implementation_claim_allowed",
    "adoption_decision_allowed",
    "forks_count_independently",
    "ambiguous_lineage_counted_independent",
    "tests_are_automatic_truth",
    "documentation_is_automatic_truth",
    "unknown_cfg_treated_unconditional",
    "mutually_exclusive_cfg_is_conflict",
    "local_absence_without_exact_proof_is_complete",
    "incomplete_candidate_scope_is_complete",
    "qdrant_candidates_as_denominator_allowed",
    "top_k_as_denominator_allowed",
    "client_file_list_as_authoritative_denominator_allowed",
    "current_path_substitution_allowed",
    "complete_negative_allows_timeout",
    "complete_negative_allows_cancellation",
    "complete_negative_allows_unreadable",
    "complete_negative_allows_revision_unavailable",
    "complete_negative_allows_scope_drift",
    "complete_negative_allows_observation_gap",
    "complete_negative_allows_expired_unsaved_snapshot",
    "complete_negative_allows_access_revocation",
    "complete_negative_allows_purge",
    "semantic_absence_claim_allowed",
    "qdrant_payload_as_exact_evidence_allowed",
    "unsaved_bytes_in_checkpoint_allowed",
    "source_bodies_in_checkpoint_allowed",
    "process_local_pin_assumed_after_restart"
)) {
    if (Get-TomlBool $baselineText $lockedFalse) { Add-Error "Unsafe W6 baseline flag enabled: $lockedFalse" }
}
foreach ($lockedTrue in @(
    "material_ambiguity_must_be_returned",
    "ambiguity_must_report_truncation",
    "resolution_receipt_revalidation_required",
    "unknowns_and_coverage_required",
    "lineage_diversity_required",
    "authoritative_inventory_denominator_required",
    "exact_planned_revision_required",
    "every_denominator_item_accounted",
    "raw_bytes_and_decoded_text_distinct",
    "regex_non_backtracking_required",
    "regex_engine_qualification_required",
    "structural_profile_qualification_required",
    "complete_negative_requires_zero_matches",
    "complete_negative_requires_zero_failures",
    "complete_negative_requires_zero_omissions",
    "historical_and_current_proof_are_distinct",
    "restrictive_security_recheck_before_emission",
    "unknown_control_commit_requires_readback"
)) {
    if (-not (Get-TomlBool $baselineText $lockedTrue)) { Add-Error "Required W6 baseline flag disabled: $lockedTrue" }
}

if ((Get-TomlString $profilesText "status") -cne "UNQUALIFIED") {
    Add-Error "W6 profile registry must remain UNQUALIFIED."
}
foreach ($unselected in @(
    "engine_name",
    "engine_version",
    "source_ref",
    "source_checksum",
    "structural_ir_profile_ref",
    "code_enricher_api_digest"
)) {
    if ((Get-TomlString $profilesText $unselected) -cne "UNSELECTED") {
        Add-Error "$unselected must remain UNSELECTED before qualification."
    }
}
if (-not (Get-TomlBool $profilesText "non_backtracking_required")) {
    Add-Error "Exact regex profile must require non-backtracking semantics."
}
if ((Get-TomlBool $profilesText "backreferences_allowed") -or (Get-TomlBool $profilesText "lookaround_allowed")) {
    Add-Error "Unqualified regex profile permits unsupported backtracking features."
}
foreach ($selectionFalse in @(
    "latest_allowed",
    "version_range_allowed",
    "floating_git_revision_allowed",
    "documentation_only_acceptance_allowed",
    "unit_tests_only_acceptance_allowed",
    "self_review_allowed"
)) {
    if (Get-TomlBool $profilesText $selectionFalse) { Add-Error "Unsafe W6 profile selection flag enabled: $selectionFalse" }
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
    if (-not $probeIds.Add($id)) { Add-Error "Duplicate W6 probe id '$id'." }
    [void]$probeOwners.Add($owner)
    if (-not $mandatory) { Add-Error "W6 probe '$id' must be mandatory." }
    if ($result -cne "UNAVAILABLE") { Add-Error "W6 probe '$id' must remain UNAVAILABLE before execution." }
}
if ($probeCount -ne 52) { Add-Error "Expected 52 W6 probes; parsed $probeCount." }
if (-not (Same-Set @($probeOwners) @("search-subject-resolver", "search-comparator", "search-exact"))) {
    Add-Error "W6 probe owner set is invalid."
}
$mandatoryProbeIds = @(
    "invalid_explicit_reference_no_fallthrough",
    "same_name_material_ambiguity",
    "incomplete_higher_ladder_blocks_lower_resolution",
    "resolution_receipt_drift_invalidation",
    "fork_mirror_copy_collapse",
    "ambiguous_lineage_not_counted_independent",
    "mutually_exclusive_cfg_is_variant",
    "comparison_no_normative_verdict_api",
    "regex_engine_exact_artifact_identity",
    "regex_non_backtracking_adversarial_corpus",
    "authoritative_inventory_denominator_only",
    "qdrant_topk_denominator_rejected",
    "every_denominator_item_accounted_once",
    "complete_negative_requires_all_items",
    "scope_drift_blocks_complete_negative",
    "timeout_blocks_complete_negative",
    "cancellation_blocks_complete_negative",
    "access_and_purge_midscan_recheck",
    "unsaved_snapshot_expiry_and_restart",
    "semantic_absence_overclaim_rejected",
    "qdrant_payload_not_exact_evidence"
)
foreach ($id in $mandatoryProbeIds) {
    if (-not $probeIds.Contains($id)) { Add-Error "Missing mandatory W6 probe '$id'." }
}

$g3BlockMatch = [regex]::Match($gatesText, '(?ms)^\[\[gate\]\]\s*id\s*=\s*"G3"(.*?)(?=^\[\[gate\]\]|\z)')
if (-not $g3BlockMatch.Success) { Add-Error "G3 gate block is missing." }
else {
    $g3 = $g3BlockMatch.Groups[1].Value
    foreach ($evidenceId in @(
        "subject_resolution_ambiguity_and_drift",
        "comparison_lineage_independence_and_cfg_variants",
        "comparison_non_normative_output_and_coverage",
        "exact_predicate_engine_qualification",
        "exact_frozen_denominator_complete_negative",
        "exact_drift_unreadable_cancel_failure",
        "exact_security_and_unsaved_revalidation"
    )) {
        if (-not $g3.Contains(('"' + $evidenceId + '"'), [StringComparison]::Ordinal)) {
            Add-Error "G3 lacks required W6 evidence id '$evidenceId'."
        }
    }
}

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    packages = $packages.Count
    w6_function_links = $w6.Count
    w6_probes = $probeCount
    qualification_status = Get-TomlString $profilesText "status"
    launch_stage = Get-TomlString $launchText "active_stage"
    launch_wave = Get-TomlInt $launchText "active_wave"
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host "ELIOT Search W6 proof packet validation"
    Write-Host "packages=$($result.packages) w6_links=$($result.w6_function_links) w6_probes=$($result.w6_probes) status=$($result.qualification_status)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host "PASS" -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
