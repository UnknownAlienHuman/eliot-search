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

function Read-Text([string]$Relative) {
    $path = Join-Path $Root $Relative
    if (-not (Test-Path $path -PathType Leaf)) {
        Add-Error "Missing file: $Relative"
        return ""
    }
    [IO.File]::ReadAllText($path)
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
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML integer '$Key'." }
        return [int64]0
    }
    [int64]$match.Groups[1].Value
}

function Toml-Bool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML bool '$Key'." }
        return $false
    }
    $match.Groups[1].Value -eq "true"
}

function Require-Tokens([string]$Relative, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if (-not $Text.Contains($token, [StringComparison]::Ordinal)) {
            Add-Error "$Relative is missing required token: $token"
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

function Get-FieldBlock([string]$Text, [string]$FieldName) {
    $escaped = [regex]::Escape($FieldName)
    $pattern = '(?ms)^\[\[[^\]]+\.field\]\]\s*\r?\nname\s*=\s*"{0}"\s*\r?\n(.*?)(?=^\[\[[^\]]+\.field\]\]|^\[[^\]]+\]|\z)' -f $escaped
    $matches = [regex]::Matches($Text, $pattern)
    if ($matches.Count -eq 0) {
        Add-Error "Missing settings field block '$FieldName'."
        return ""
    }
    if ($matches.Count -gt 1) {
        Add-Error "Duplicate settings field block '$FieldName'."
        return ""
    }
    $matches[0].Value
}

function Require-LockedBool([string]$Text, [string]$FieldName, [bool]$Default) {
    $block = Get-FieldBlock $Text $FieldName
    if (-not $block) { return }
    if ($block -notmatch '(?m)^mode\s*=\s*"LOCKED"\s*$') { Add-Error "$FieldName must be LOCKED." }
    $expected = if ($Default) { "true" } else { "false" }
    if ($block -notmatch ('(?m)^default\s*=\s*{0}\s*$' -f $expected)) {
        Add-Error "$FieldName must default to $expected."
    }
}

function Validate-TunableBlocks([string]$Text) {
    $blocks = [regex]::Matches($Text, '(?ms)^\[\[[^\]]+\.field\]\]\s*(.*?)(?=^\[\[[^\]]+\.field\]\]|^\[[^\]]+\]|\z)')
    foreach ($match in $blocks) {
        $block = $match.Value
        $name = Toml-String $block "name"
        $mode = Toml-String $block "mode"
        if ($mode -in @("TUNABLE", "TUNABLE_INTERNAL_CEILING")) {
            $min = Toml-Int $block "min"
            $max = Toml-Int $block "max"
            $default = Toml-Int $block "default"
            [void](Toml-String $block "change_action")
            if ($min -gt $max) { Add-Error "$name has min > max." }
            if ($default -lt $min -or $default -gt $max) {
                Add-Error "$name default is outside min/max."
            }
        }
    }
}

$manifestPath = "docs/current/manifest.toml"
$crossPath = "docs/current/W5_CURRENT_WORKSPACE_CONTRACTS_1.0.md"
$settingsPath = "config/w5-current.toml"
$settingsDocPath = "docs/config/W5_CURRENT_SETTINGS_1.0.md"
$packetPath = "swarm/w5-current.toml"
$launchPath = "swarm/launch-state.toml"
$packageRegistryPath = "swarm/crates.toml"
$artifactPath = "qualification/rust-syntax/artifact.toml"
$probesPath = "qualification/rust-syntax/probes.toml"

$manifest = Read-Text $manifestPath
$cross = Read-Text $crossPath
$settings = Read-Text $settingsPath
$settingsDoc = Read-Text $settingsDocPath
$packets = Read-Text $packetPath
$launch = Read-Text $launchPath
$packageRegistry = Read-Text $packageRegistryPath
$artifact = Read-Text $artifactPath
$probes = Read-Text $probesPath

if ($manifest) {
    if ((Toml-String $manifest "status") -cne "contract-only") { Add-Error "W5 manifest status must be contract-only." }
    if (Toml-Bool $manifest "implementation_authorized") { Add-Error "W5 manifest must not authorize implementation." }
    if ((Toml-String $manifest "requires_accepted_gate") -cne "G2") { Add-Error "W5 must require accepted G2." }
    if (-not (Toml-Bool $manifest "requires_accepted_w4_baseline")) { Add-Error "W5 must require the accepted W4 baseline." }
    foreach ($key in @(
        "reconciliation_runtime", "watcher_overflow_recovery", "unsaved_nonpersistence",
        "overlay_shadow_noninterference", "rust_parser_profile", "rust_structure_assurance",
        "workspace_currentness"
    )) {
        $value = Toml-String $manifest $key
        if ($key -eq "reconciliation_runtime") {
            if ($value -cne "NOT_IMPLEMENTED") { Add-Error "$key must remain NOT_IMPLEMENTED." }
        } elseif ($key -eq "rust_parser_profile") {
            if ($value -cne "UNSELECTED") { Add-Error "$key must remain UNSELECTED." }
        } elseif ($value -cne "UNAVAILABLE") {
            Add-Error "$key must remain UNAVAILABLE before executed evidence."
        }
    }
}

$expectedPackages = @("search-source-reconcile", "search-overlay", "search-code-enricher")
$ownerBlocks = [regex]::Split($manifest, '(?m)^\[\[owner\]\]\s*$')
$manifestPackages = [System.Collections.Generic.List[string]]::new()
$functionFiles = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $ownerBlocks.Count; $i++) {
    $block = $ownerBlocks[$i]
    $package = Toml-String $block "package"
    $functions = Toml-String $block "function_contract"
    if ($manifestPackages.Contains($package)) { Add-Error "Duplicate W5 owner package: $package" }
    $manifestPackages.Add($package)
    $functionFiles.Add($functions)
}
if (-not (Same-Set $expectedPackages $manifestPackages.ToArray())) { Add-Error "W5 owner package set differs from expected set." }

foreach ($file in $functionFiles) {
    $text = Read-Text $file
    if ($text) { Require-Tokens $file $text @("## Typed failures", "## Required tests") }
}

$registryBlocks = [regex]::Split($packageRegistry, '(?m)^\[\[package\]\]\s*$')
$registryWaves = @{}
for ($i = 1; $i -lt $registryBlocks.Count; $i++) {
    $block = $registryBlocks[$i]
    $name = Toml-String $block "name"
    if ($name) { $registryWaves[$name] = [int](Toml-Int $block "wave") }
}
foreach ($package in $expectedPackages) {
    if (-not $registryWaves.ContainsKey($package)) {
        Add-Error "W5 package is absent from swarm/crates.toml: $package"
    } elseif ($registryWaves[$package] -ne 5) {
        Add-Error "$package is W$($registryWaves[$package]); expected W5."
    }
}

Require-Tokens $crossPath $cross @(
    "## 2. Watcher events are hints",
    "## 3. Observation gap state",
    "## 4. Authoritative inventory reconciliation",
    "## 6. Currentness model",
    "## 8. Explicit unsaved buffer snapshots",
    "## 9. Shadow fence before retrieval and IDF",
    "## 13. Rust syntax enrichment profile",
    "## 14. No-execute Rust parsing",
    "## 15. Assurance and malformed input",
    "Receiving another watcher event never resolves a gap",
    "Post-candidate duplicate removal cannot repair base IDF/ordering contamination",
    "unsaved bytes remain in bounded process memory only"
)

Require-Tokens "crates/search-source/search-source-reconcile/FUNCTIONS.md" (Read-Text "crates/search-source/search-source-reconcile/FUNCTIONS.md") @(
    "declare_observation_gap", "inventory_slice", "apply_reconciliation",
    "resolve_observation_gap", "compute_workspace_currentness", "partial slice cannot remove"
)
Require-Tokens "crates/search-query/search-overlay/FUNCTIONS.md" (Read-Text "crates/search-query/search-overlay/FUNCTIONS.md") @(
    "admit_unsaved_snapshot", "build_shadow_fence", "query_overlay_leg",
    "mark_published_base_coverage", "verify_unsaved_nonpersistence", "No Qdrant mutation occurs"
)
Require-Tokens "crates/search-prep/search-code-enricher/FUNCTIONS.md" (Read-Text "crates/search-prep/search-code-enricher/FUNCTIONS.md") @(
    "validate_enrichment_profile", "parse_rust_syntax", "extract_cfg_observations",
    "map_fact_anchor", "classify_fact_assurance", "never invokes Cargo"
)

if ($settings) {
    if ((Toml-String $settings "status") -cne "schema-only") { Add-Error "W5 settings status must be schema-only." }
    if (Toml-Bool $settings "implementation_authorized") { Add-Error "W5 settings must not authorize implementation." }

    Require-LockedBool $settings "watcher_is_authority" $false
    Require-LockedBool $settings "overflow_declares_gap_before_ack" $true
    Require-LockedBool $settings "open_gap_blocks_current_confirmed" $true
    Require-LockedBool $settings "complete_inventory_required_to_resolve_gap" $true
    Require-LockedBool $settings "partial_inventory_may_remove_unseen_source" $false
    Require-LockedBool $settings "guarded_apply_required" $true
    Require-LockedBool $settings "watcher_event_may_confirm_currentness" $false
    Require-LockedBool $settings "qdrant_health_may_confirm_filesystem_currentness" $false
    Require-LockedBool $settings "unsaved_buffer_may_upgrade_disk_currentness" $false
    Require-LockedBool $settings "filesystem_saved_buffer_projection_axes_separate" $true

    Require-LockedBool $settings "unsaved_bytes_to_redb_allowed" $false
    Require-LockedBool $settings "unsaved_bytes_to_cas_allowed" $false
    Require-LockedBool $settings "unsaved_bytes_to_qdrant_allowed" $false
    Require-LockedBool $settings "unsaved_bytes_to_ordinary_telemetry_allowed" $false
    Require-LockedBool $settings "cross_binding_unsaved_visibility_allowed" $false
    Require-LockedBool $settings "shadow_before_retrieval_and_idf_required" $true
    Require-LockedBool $settings "post_candidate_dedup_repairs_shadowing" $false
    Require-LockedBool $settings "overlay_may_be_second_durable_search_database" $false
    Require-LockedBool $settings "durable_handle_to_unsaved_allowed" $false

    Require-LockedBool $settings "execute_cargo_allowed" $false
    Require-LockedBool $settings "execute_rustc_allowed" $false
    Require-LockedBool $settings "execute_build_scripts_allowed" $false
    Require-LockedBool $settings "expand_macros_allowed" $false
    Require-LockedBool $settings "network_or_package_resolution_allowed" $false
    Require-LockedBool $settings "syntax_may_claim_compiler_semantics" $false

    $profileBlock = Get-FieldBlock $settings "parser_profile_ref"
    if ($profileBlock) {
        if ($profileBlock -notmatch '(?m)^mode\s*=\s*"QUALIFIED_REF"\s*$') { Add-Error "parser_profile_ref must be QUALIFIED_REF." }
        if ($profileBlock -notmatch '(?m)^default\s*=\s*"UNSELECTED"\s*$') { Add-Error "parser_profile_ref must default UNSELECTED." }
    }

    Validate-TunableBlocks $settings
    Require-Tokens $settingsPath $settings @(
        "[forbidden]", "watcher_as_source_truth = true", "resolve_gap_from_watcher_event = true",
        "partial_inventory_removal = true", "current_across_open_gap = true",
        "persist_unsaved_bytes = true", "index_unsaved_in_qdrant = true",
        "cross_binding_unsaved_visibility = true", "shadow_after_candidate_generation = true",
        "overlay_second_database = true", "durable_unsaved_handle = true",
        "execute_code_enrichment_toolchain = true", "expand_macros = true",
        "syntax_semantic_overclaim = true", "unbounded_inventory_overlay_or_parser_work = true"
    )
}

Require-Tokens $settingsDocPath $settingsDoc @(
    "watcher is a hint, never authority", "unsaved bytes/units/vectors never persist",
    "parser_profile_ref", "prior effective config remains authoritative"
)

if ($artifact) {
    if ((Toml-String $artifact "status") -cne "UNQUALIFIED") { Add-Error "Rust syntax artifact must remain UNQUALIFIED." }
    if ((Toml-String $artifact "provider") -cne "UNSELECTED") { Add-Error "Rust parser provider must remain UNSELECTED." }
    foreach ($key in @(
        "executes_cargo", "executes_rustc", "executes_build_scripts", "expands_macros",
        "uses_network", "resolves_packages", "uses_credential_prompts", "claims_compiler_semantics",
        "latest_allowed", "version_range_allowed", "floating_git_revision_allowed",
        "compile_only_acceptance_allowed", "single_valid_file_acceptance_allowed"
    )) {
        if (Toml-Bool $artifact $key) { Add-Error "Rust syntax artifact unsafe flag is true: $key" }
    }
}

$expectedProbeIds = @(
    "dependency_identity_and_license", "no_process_network_or_toolchain_execution",
    "deterministic_tree_and_fact_manifest", "rust_item_definition_fixtures",
    "reference_relation_assurance", "cfg_and_cfg_attr_preserved_not_evaluated",
    "macro_uncertainty", "malformed_and_incomplete_source", "native_anchor_byte_span_mapping",
    "unicode_identifier_fixture", "crlf_coordinate_fixture", "utf16_coordinate_fixture",
    "bounded_nodes_depth_errors_and_facts", "cancellation_no_fake_complete_manifest",
    "semantic_overclaim_rejection", "vendor_type_public_api_guard",
    "profile_change_requires_reenrichment"
)
$probeBlocks = [regex]::Split($probes, '(?m)^\[\[probe\]\]\s*$')
$probeIds = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $block = $probeBlocks[$i]
    $id = Toml-String $block "id"
    if ($probeIds.Contains($id)) { Add-Error "Duplicate Rust syntax probe: $id" }
    $probeIds.Add($id)
    if (-not (Toml-Bool $block "mandatory")) { Add-Error "Rust syntax probe must be mandatory: $id" }
    if ((Toml-String $block "result") -cne "UNAVAILABLE") { Add-Error "Probe $id must remain UNAVAILABLE before execution." }
}
if (-not (Same-Set $expectedProbeIds $probeIds.ToArray())) { Add-Error "Rust syntax probe set differs from expected set." }
if ((Toml-String $probes "status") -cne "NOT_EXECUTED") { Add-Error "Rust syntax probes must remain NOT_EXECUTED." }

$packetBlocks = [regex]::Split($packets, '(?m)^\[\[packet\]\]\s*$')
$packetPackages = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $packetBlocks.Count; $i++) {
    $block = $packetBlocks[$i]
    $package = Toml-String $block "package"
    $packetPackages.Add($package)
    foreach ($key in @("assignment", "functions")) {
        $relative = Toml-String $block $key
        if ($relative -and -not (Test-Path (Join-Path $Root $relative) -PathType Leaf)) {
            Add-Error "$package references missing $key file: $relative"
        }
    }
    $config = Toml-String $block "config_packet" $false
    if ($config) {
        $configText = Read-Text $config
        $ownerMarker = '**Owner:** `' + $package + '`'
        if ($configText -and -not $configText.Contains($ownerMarker, [StringComparison]::Ordinal)) {
            Add-Error "$config does not declare owner $package."
        }
    }
    $qualification = Toml-String $block "qualification_packet" $false
    if ($qualification -and -not (Test-Path (Join-Path $Root $qualification) -PathType Leaf)) {
        Add-Error "$package references missing qualification packet: $qualification"
    }
}
if (-not (Same-Set $expectedPackages $packetPackages.ToArray())) { Add-Error "W5 packet package set differs from owner set." }
if ((Toml-String $packets "status") -cne "BLOCKED") { Add-Error "W5 packet registry must remain BLOCKED." }
if ((Toml-String $packets "requires_accepted_gate") -cne "G2") { Add-Error "W5 packet registry must require G2." }
if (-not (Toml-Bool $packets "requires_accepted_w4_baseline")) { Add-Error "W5 packet registry must require W4 baseline." }

if ($launch) {
    $activeWave = [int](Toml-Int $launch "active_wave")
    if ($activeWave -ge 5) { Add-Warning "Launch state has reached W5; verify accepted receipts before contract-only merge." }
    $authorizedMatch = [regex]::Match($launch, '(?ms)^authorized_packages\s*=\s*\[(.*?)\]')
    if ($authorizedMatch.Success) {
        foreach ($package in $expectedPackages) {
            if ($authorizedMatch.Groups[1].Value -match ('"' + [regex]::Escape($package) + '"')) {
                Add-Error "Launch state already authorizes W5 package $package."
            }
        }
    }
}

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    packages = $manifestPackages.Count
    function_contracts = $functionFiles.Count
    rust_syntax_probes = $probeIds.Count
    status = Toml-String $packets "status"
    required_gate = Toml-String $packets "requires_accepted_gate"
    parser_profile = Toml-String $manifest "rust_parser_profile"
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 6 }
else {
    Write-Host "ELIOT Search W5 current-workspace contract validation"
    Write-Host "packages=$($result.packages) functions=$($result.function_contracts) probes=$($result.rust_syntax_probes) status=$($result.status)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host "PASS" -ForegroundColor Green }
}

if (-not $result.ok) { exit 1 }
