[CmdletBinding()]
param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$errors = [System.Collections.Generic.List[string]]::new()
$warnings = [System.Collections.Generic.List[string]]::new()
function Add-Error([string]$Message) { $script:errors.Add($Message) }
function Add-Warning([string]$Message) { $script:warnings.Add($Message) }
function Read-Required([string]$RelativePath) {
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path $path -PathType Leaf)) {
        Add-Error "Missing required file: $RelativePath"
        return ''
    }
    [IO.File]::ReadAllText($path)
}
function Toml-String([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML string '$Key'." }
        return ''
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
        if ($Required) { Add-Error "Missing TOML boolean '$Key'." }
        return $false
    }
    $match.Groups[1].Value -eq 'true'
}
function Toml-Raw([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(.+?)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML value '$Key'." }
        return ''
    }
    $match.Groups[1].Value.Trim()
}
function Toml-Array([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $match.Success) { return @() }
    @([regex]::Matches($match.Groups[1].Value, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Toml-Section([string]$Text, [string]$Name) {
    $match = [regex]::Match($Text, ('(?ms)^\[{0}\]\s*(.*?)(?=^\[|\z)' -f [regex]::Escape($Name)))
    if (-not $match.Success) {
        Add-Error "Missing TOML section [$Name]."
        return ''
    }
    $match.Groups[1].Value
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
function Require-Tokens([string]$RelativePath, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if ($Text.IndexOf($token, [StringComparison]::Ordinal) -lt 0) {
            Add-Error "$RelativePath is missing required token: $token"
        }
    }
}
function Parse-Fields([string]$Text) {
    $result = [ordered]@{}
    $pattern = '(?ms)^\[\[([A-Za-z0-9_]+)\.field\]\]\s*(.*?)(?=^\[\[[A-Za-z0-9_]+\.field\]\]|^\[[A-Za-z0-9_]+\]\s*$|\z)'
    foreach ($match in [regex]::Matches($Text, $pattern)) {
        $section = $match.Groups[1].Value
        $block = $match.Groups[2].Value
        $name = Toml-String $block 'name'
        $key = "$section.$name"
        if ($result.Contains($key)) { Add-Error "Duplicate W5 settings field '$key'."; continue }
        $result[$key] = [pscustomobject]@{
            Mode = Toml-String $block 'mode'
            DefaultRaw = Toml-Raw $block 'default'
            MinRaw = Toml-Raw $block 'min' $false
            MaxRaw = Toml-Raw $block 'max' $false
            ChangeAction = Toml-String $block 'change_action' $false
        }
    }
    $result
}
function Validate-ManualWorkflow([string]$RelativePath, [string]$Text, [string[]]$RequiredTokens) {
    if ($Text.IndexOf('workflow_dispatch:', [StringComparison]::Ordinal) -lt 0) {
        Add-Error "$RelativePath lacks workflow_dispatch."
    }
    if ($Text -match '(?m)^\s*(pull_request|push|schedule):') {
        Add-Error "$RelativePath contains an automatic trigger."
    }
    Require-Tokens $RelativePath $Text $RequiredTokens
}

$paths = [ordered]@{
    manifest = 'docs/current/manifest.toml'
    cross = 'docs/current/W5_CURRENT_WORKSPACE_CONTRACTS_1.0.md'
    settings = 'config/w5-current.toml'
    settings_doc = 'docs/config/W5_CURRENT_SETTINGS_1.0.md'
    packet = 'swarm/w5-current.toml'
    rust_artifact = 'qualification/rust-syntax/artifact.toml'
    rust_probes = 'qualification/rust-syntax/probes.toml'
    current_baseline = 'qualification/current/baseline.toml'
    current_probes = 'qualification/current/probes.toml'
    current_qualification = 'qualification/current/W5_QUALIFICATION.md'
    reconcile = 'crates/search-source/search-source-reconcile/FUNCTIONS.md'
    overlay = 'crates/search-query/search-overlay/FUNCTIONS.md'
    code = 'crates/search-prep/search-code-enricher/FUNCTIONS.md'
    registry = 'swarm/crates.toml'
    launch = 'swarm/launch-state.toml'
    gates = 'swarm/gates.toml'
    workflow = '.github/workflows/w5-current-contracts.yml'
    swarm_workflow = '.github/workflows/swarm-structure.yml'
}
$text = [ordered]@{}
foreach ($entry in $paths.GetEnumerator()) { $text[$entry.Key] = Read-Required $entry.Value }

# Contract manifest and owners.
if ((Toml-String $text.manifest 'status') -cne 'contract-only') { Add-Error 'W5 manifest must remain contract-only.' }
if (Toml-Bool $text.manifest 'implementation_authorized') { Add-Error 'W5 manifest cannot authorize implementation.' }
if ((Toml-String $text.manifest 'requires_accepted_gate') -cne 'G2') { Add-Error 'W5 manifest must require G2.' }
if (-not (Toml-Bool $text.manifest 'requires_accepted_w4_baseline')) { Add-Error 'W5 manifest must require W4 baseline.' }
$ownerBlocks = [regex]::Split($text.manifest, '(?m)^\[\[owner\]\]\s*$')
$owners = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $ownerBlocks.Count; $i++) {
    $block = $ownerBlocks[$i]
    $package = Toml-String $block 'package'
    $functions = Toml-String $block 'function_contract'
    if ($owners.Contains($package)) { Add-Error "Duplicate W5 manifest owner '$package'." }
    $owners.Add($package)
    if (-not (Test-Path (Join-Path $Root $functions) -PathType Leaf)) { Add-Error "$package function packet is missing: $functions" }
}
$expectedPackages = @('search-source-reconcile', 'search-overlay', 'search-code-enricher')
if (-not (Same-Set @($owners) $expectedPackages)) { Add-Error 'W5 manifest owner set is invalid.' }

Require-Tokens $paths.cross $text.cross @(
    '## 2. Watcher events are hints',
    '## 3. Observation gap state',
    '## 4. Authoritative inventory reconciliation',
    '## 6. Currentness model',
    '## 8. Explicit unsaved buffer snapshots',
    '## 9. Shadow fence before retrieval and IDF',
    '## 13. Rust syntax enrichment profile',
    '## 14. No-execute Rust parsing',
    '## 15. Assurance and malformed input',
    'Receiving another watcher event never resolves a gap',
    'Post-candidate duplicate removal cannot repair base IDF/ordering contamination',
    'unsaved bytes remain in bounded process memory only'
)
Require-Tokens $paths.reconcile $text.reconcile @(
    '## `open_observation_gap`',
    '## `execute_inventory_slice`',
    '## `commit_reconcile`',
    '## `recover_reconcile_commit`',
    '## `classify_freshness`',
    '## `preflight_current_workspace`',
    'partial/cancelled/timed-out inventory',
    '## Typed failures',
    '## Required tests / qualification evidence'
)
Require-Tokens $paths.overlay $text.overlay @(
    '## `attach_unsaved_snapshot`',
    '## `replace_unsaved_snapshot`',
    '## `compute_shadow_set`',
    '## `retrieve_overlay`',
    '## `prepare_save_admission`',
    'process-memory',
    'daemon crash destroys unsaved bytes',
    '## Typed failures',
    '## Required tests / qualification evidence'
)
Require-Tokens $paths.code $text.code @(
    '## `validate_parser_profile`',
    '## `parse_rust_no_execute`',
    '## `extract_structural_facts`',
    '## `extract_configuration_predicate`',
    'tolerant_syntax',
    'compiler truth',
    'procedural macros',
    '## Typed failures',
    '## Required tests / qualification evidence'
)

# Stage settings.
if ((Toml-String $text.settings 'status') -cne 'schema-only') { Add-Error 'W5 settings must remain schema-only.' }
if (Toml-Bool $text.settings 'implementation_authorized') { Add-Error 'W5 settings cannot authorize implementation.' }
$fields = Parse-Fields $text.settings
$lockedExpected = [ordered]@{
    'reconciliation.watcher_is_authority' = 'false'
    'reconciliation.overflow_declares_gap_before_ack' = 'true'
    'reconciliation.open_gap_blocks_current_confirmed' = 'true'
    'reconciliation.complete_inventory_required_to_resolve_gap' = 'true'
    'reconciliation.partial_inventory_may_remove_unseen_source' = 'false'
    'reconciliation.guarded_apply_required' = 'true'
    'currentness.watcher_event_may_confirm_currentness' = 'false'
    'currentness.qdrant_health_may_confirm_filesystem_currentness' = 'false'
    'currentness.unsaved_buffer_may_upgrade_disk_currentness' = 'false'
    'currentness.filesystem_saved_buffer_projection_axes_separate' = 'true'
    'overlay.unsaved_bytes_to_redb_allowed' = 'false'
    'overlay.unsaved_bytes_to_cas_allowed' = 'false'
    'overlay.unsaved_bytes_to_qdrant_allowed' = 'false'
    'overlay.unsaved_bytes_to_ordinary_telemetry_allowed' = 'false'
    'overlay.cross_binding_unsaved_visibility_allowed' = 'false'
    'overlay.shadow_before_retrieval_and_idf_required' = 'true'
    'overlay.post_candidate_dedup_repairs_shadowing' = 'false'
    'overlay.overlay_may_be_second_durable_search_database' = 'false'
    'overlay.durable_handle_to_unsaved_allowed' = 'false'
    'rust_enrichment.execute_cargo_allowed' = 'false'
    'rust_enrichment.execute_rustc_allowed' = 'false'
    'rust_enrichment.execute_build_scripts_allowed' = 'false'
    'rust_enrichment.expand_macros_allowed' = 'false'
    'rust_enrichment.network_or_package_resolution_allowed' = 'false'
    'rust_enrichment.syntax_may_claim_compiler_semantics' = 'false'
}
foreach ($entry in $lockedExpected.GetEnumerator()) {
    if (-not $fields.Contains($entry.Key)) { Add-Error "Missing locked W5 field '$($entry.Key)'."; continue }
    $field = $fields[$entry.Key]
    if ($field.Mode -cne 'LOCKED') { Add-Error "$($entry.Key) must be LOCKED." }
    if ($field.DefaultRaw -cne $entry.Value) { Add-Error "$($entry.Key) default '$($field.DefaultRaw)' != '$($entry.Value)'." }
}
if (-not $fields.Contains('rust_enrichment.parser_profile_ref')) { Add-Error 'Missing rust_enrichment.parser_profile_ref.' }
else {
    $profile = $fields['rust_enrichment.parser_profile_ref']
    if ($profile.Mode -cne 'QUALIFIED_REF' -or $profile.DefaultRaw -cne '"UNSELECTED"') {
        Add-Error 'parser_profile_ref must be an UNSELECTED QUALIFIED_REF.'
    }
}
foreach ($entry in $fields.GetEnumerator()) {
    $field = $entry.Value
    if ($field.Mode -in @('TUNABLE', 'TUNABLE_INTERNAL_CEILING')) {
        if (-not $field.MinRaw -or -not $field.MaxRaw) { Add-Error "$($entry.Key) lacks finite min/max."; continue }
        $min = [int64]$field.MinRaw
        $max = [int64]$field.MaxRaw
        $default = [int64]$field.DefaultRaw
        if ($min -gt $max -or $default -lt $min -or $default -gt $max) { Add-Error "$($entry.Key) has invalid bounds/default." }
        if (-not $field.ChangeAction) { Add-Error "$($entry.Key) lacks change_action." }
    }
}
Require-Tokens $paths.settings $text.settings @(
    '[forbidden]',
    'watcher_as_source_truth = true',
    'resolve_gap_from_watcher_event = true',
    'partial_inventory_removal = true',
    'current_across_open_gap = true',
    'persist_unsaved_bytes = true',
    'index_unsaved_in_qdrant = true',
    'shadow_after_candidate_generation = true',
    'execute_code_enrichment_toolchain = true',
    'syntax_semantic_overclaim = true'
)
Require-Tokens $paths.settings_doc $text.settings_doc @(
    'watcher is a hint, never authority',
    'unsaved bytes/units/vectors never persist',
    'parser_profile_ref',
    'prior effective config remains authoritative'
)

# Current-workspace qualification remains unexecuted.
if ((Toml-String $text.current_baseline 'status') -cne 'DESIGNED_NOT_EXECUTED') { Add-Error 'W5 baseline must remain DESIGNED_NOT_EXECUTED.' }
if (Toml-Bool $text.current_baseline 'implementation_authorized') { Add-Error 'W5 baseline cannot authorize implementation.' }
$currentProbeBlocks = [regex]::Split($text.current_probes, '(?m)^\[\[probe\]\]\s*$')
$currentProbeIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$currentOwners = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
for ($i = 1; $i -lt $currentProbeBlocks.Count; $i++) {
    $block = $currentProbeBlocks[$i]
    $id = Toml-String $block 'id'
    $owner = Toml-String $block 'owner'
    if (-not $currentProbeIds.Add($id)) { Add-Error "Duplicate current-workspace probe '$id'." }
    [void]$currentOwners.Add($owner)
    if (-not (Toml-Bool $block 'mandatory')) { Add-Error "Current-workspace probe '$id' must be mandatory." }
    if ((Toml-String $block 'result') -cne 'UNAVAILABLE') { Add-Error "Current-workspace probe '$id' must remain UNAVAILABLE." }
    if ((Toml-String $block 'raw_output_ref') -ne '') { Add-Error "Current-workspace probe '$id' contains premature raw output." }
    if ((Toml-String $block 'reviewer_receipt_ref') -ne '') { Add-Error "Current-workspace probe '$id' contains premature reviewer receipt." }
}
if ($currentProbeIds.Count -ne 42) { Add-Error "Expected 42 current-workspace probes; found $($currentProbeIds.Count)." }
if (-not (Same-Set @($currentOwners) $expectedPackages)) { Add-Error 'Current-workspace probe owner set is invalid.' }

# Rust parser artifact and probes remain unselected/unqualified.
if ((Toml-String $text.rust_artifact 'status') -cne 'UNQUALIFIED') { Add-Error 'Rust syntax artifact must remain UNQUALIFIED.' }
if ((Toml-String $text.rust_artifact 'provider') -cne 'UNSELECTED') { Add-Error 'Rust parser provider must remain UNSELECTED.' }
foreach ($flag in @(
    'executes_cargo', 'executes_rustc', 'executes_build_scripts', 'expands_macros',
    'uses_network', 'resolves_packages', 'uses_credential_prompts', 'claims_compiler_semantics',
    'latest_allowed', 'version_range_allowed', 'floating_git_revision_allowed',
    'compile_only_acceptance_allowed', 'single_valid_file_acceptance_allowed'
)) {
    if (Toml-Bool $text.rust_artifact $flag) { Add-Error "Unsafe Rust syntax artifact flag enabled: $flag" }
}
$rustExpected = @(
    'dependency_identity_and_license',
    'no_process_network_or_toolchain_execution',
    'deterministic_tree_and_fact_manifest',
    'rust_item_definition_fixtures',
    'reference_relation_assurance',
    'cfg_and_cfg_attr_preserved_not_evaluated',
    'macro_uncertainty',
    'malformed_and_incomplete_source',
    'native_anchor_byte_span_mapping',
    'unicode_identifier_fixture',
    'crlf_coordinate_fixture',
    'utf16_coordinate_fixture',
    'bounded_nodes_depth_errors_and_facts',
    'cancellation_no_fake_complete_manifest',
    'semantic_overclaim_rejection',
    'vendor_type_public_api_guard',
    'profile_change_requires_reenrichment'
)
$rustBlocks = [regex]::Split($text.rust_probes, '(?m)^\[\[probe\]\]\s*$')
$rustIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
for ($i = 1; $i -lt $rustBlocks.Count; $i++) {
    $block = $rustBlocks[$i]
    $id = Toml-String $block 'id'
    if (-not $rustIds.Add($id)) { Add-Error "Duplicate Rust syntax probe '$id'." }
    if ((Toml-String $block 'owner') -cne 'search-code-enricher') { Add-Error "Rust syntax probe '$id' has wrong owner." }
    if (-not (Toml-Bool $block 'mandatory')) { Add-Error "Rust syntax probe '$id' must be mandatory." }
    if ((Toml-String $block 'result') -cne 'UNAVAILABLE') { Add-Error "Rust syntax probe '$id' must remain UNAVAILABLE." }
    if ((Toml-String $block 'raw_output_ref') -ne '' -or (Toml-String $block 'receipt_ref') -ne '') {
        Add-Error "Rust syntax probe '$id' contains premature evidence."
    }
}
if ((Toml-String $text.rust_probes 'status') -cne 'NOT_EXECUTED') { Add-Error 'Rust syntax probe registry must remain NOT_EXECUTED.' }
if (-not (Same-Set @($rustIds) $rustExpected)) { Add-Error 'Rust syntax probe set is invalid.' }

# Package registry and launch authority.
$registryBlocks = [regex]::Split($text.registry, '(?m)^\[\[package\]\]\s*$')
$registryPreamble = $registryBlocks[0]
if ((Toml-String $registryPreamble 'current_workspace_qualification_path') -cne $paths.current_qualification) {
    Add-Error 'Registry current_workspace_qualification_path is inconsistent.'
}
$packages = [ordered]@{}
for ($i = 1; $i -lt $registryBlocks.Count; $i++) {
    $block = $registryBlocks[$i]
    $name = Toml-String $block 'name'
    $packages[$name] = [pscustomobject]@{
        Wave = [int](Toml-Int $block 'wave')
        Functions = Toml-String $block 'functions' $false
        Qualification = Toml-String $block 'qualification' $false
        ConfigSections = @(Toml-Array $block 'config_sections')
    }
}
$expectedFunctionPaths = [ordered]@{
    'search-source-reconcile' = $paths.reconcile
    'search-overlay' = $paths.overlay
    'search-code-enricher' = $paths.code
}
foreach ($entry in $expectedFunctionPaths.GetEnumerator()) {
    if (-not $packages.Contains($entry.Key)) { Add-Error "Missing registry package '$($entry.Key)'."; continue }
    $package = $packages[$entry.Key]
    if ($package.Wave -ne 5) { Add-Error "$($entry.Key) must remain W5." }
    if ($package.Functions -cne $entry.Value) { Add-Error "$($entry.Key) function path is inconsistent." }
    if ($package.Qualification -cne $paths.current_qualification) { Add-Error "$($entry.Key) qualification path is inconsistent." }
}
if ($packages['search-source-reconcile'].ConfigSections -cnotcontains 'reconcile') { Add-Error 'search-source-reconcile must own reconcile section.' }
if ($packages['search-overlay'].ConfigSections -cnotcontains 'overlay') { Add-Error 'search-overlay must own overlay section.' }
if ((Toml-String $text.launch 'current_workspace_qualification') -cne $paths.current_qualification) { Add-Error 'Launch-state W5 qualification path is inconsistent.' }
if ((Toml-String $text.launch 'active_stage') -cne 'P00' -or (Toml-Int $text.launch 'active_wave') -ne 0) { Add-Error 'Launch authority must remain P00/W0.' }
if (-not (Same-Set @(Toml-Array $text.launch 'authorized_packages') @('search-contracts'))) { Add-Error 'Only search-contracts may be authorized.' }
foreach ($package in $expectedPackages) {
    if ((Toml-Array $text.launch 'authorized_packages') -contains $package) { Add-Error "Launch state prematurely authorizes $package." }
}

# Bounded W5 swarm packet and exact G3 evidence closure.
if ((Toml-String $text.packet 'status') -cne 'BLOCKED') { Add-Error 'W5 packet must remain BLOCKED.' }
if (Toml-Bool $text.packet 'implementation_authorized') { Add-Error 'W5 packet cannot authorize implementation.' }
if ((Toml-String $text.packet 'requires_accepted_gate') -cne 'G2') { Add-Error 'W5 packet must require G2.' }
if (-not (Toml-Bool $text.packet 'requires_accepted_w4_baseline')) { Add-Error 'W5 packet must require W4 baseline.' }
$packetBlocks = [regex]::Split($text.packet, '(?m)^\[\[packet\]\]\s*$')
$packetPackages = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $packetBlocks.Count; $i++) {
    $block = $packetBlocks[$i]
    $package = Toml-String $block 'package'
    $packetPackages.Add($package)
    foreach ($key in @('assignment', 'functions')) {
        $relative = Toml-String $block $key
        if (-not (Test-Path (Join-Path $Root $relative) -PathType Leaf)) { Add-Error "$package references missing $key file: $relative" }
    }
    $config = Toml-String $block 'config_packet' $false
    if ($config -and -not (Test-Path (Join-Path $Root $config) -PathType Leaf)) { Add-Error "$package references missing config packet: $config" }
    foreach ($relative in (Toml-Array $block 'qualification_packets')) {
        if (-not (Test-Path (Join-Path $Root $relative) -PathType Leaf)) { Add-Error "$package references missing qualification packet: $relative" }
    }
    if ((Toml-Array $block 'cross_sections').Count -eq 0) { Add-Error "$package has no bounded cross-contract sections." }
    if ((Toml-Array $block 'accepted_handoffs_required').Count -eq 0) { Add-Error "$package has no accepted handoff requirements." }
    $expectedScope = $packages[$package] | Out-Null
    $writeScope = Toml-String $block 'write_scope'
    if ($package -eq 'search-source-reconcile' -and $writeScope -cne 'crates/search-source/search-source-reconcile/**') { Add-Error 'search-source-reconcile write scope is invalid.' }
    if ($package -eq 'search-overlay' -and $writeScope -cne 'crates/search-query/search-overlay/**') { Add-Error 'search-overlay write scope is invalid.' }
    if ($package -eq 'search-code-enricher' -and $writeScope -cne 'crates/search-prep/search-code-enricher/**') { Add-Error 'search-code-enricher write scope is invalid.' }
}
if (-not (Same-Set $packetPackages.ToArray() $expectedPackages)) { Add-Error 'W5 swarm packet package set is invalid.' }
$gateSection = Toml-Section $text.packet 'gate'
$w5Evidence = @(Toml-Array $gateSection 'w5_required_evidence')
$downstreamEvidence = @(Toml-Array $gateSection 'downstream_w6_evidence_not_owned')
$g3Match = [regex]::Match($text.gates, '(?ms)^\[\[gate\]\]\s*id\s*=\s*"G3"(.*?)(?=^\[\[gate\]\]|\z)')
if (-not $g3Match.Success) { Add-Error 'Central G3 gate is missing.' }
else {
    $centralG3 = @(Toml-Array $g3Match.Groups[1].Value 'required_evidence')
    if (-not (Same-Set @($w5Evidence + $downstreamEvidence) $centralG3)) { Add-Error 'W5/W6 evidence partition does not equal central G3.' }
}
$expectedW5Evidence = @(
    'observation_gap_currentness_denial',
    'reconciliation_commit_and_snapshot_recovery',
    'live_head_shadow_before_emission',
    'unsaved_bytes_nonpersistence',
    'overlay_precedence_and_restart_invalidation',
    'rust_parser_exact_artifact_and_no_execute',
    'rust_structure_assurance_and_cfg_variants'
)
if (-not (Same-Set $w5Evidence $expectedW5Evidence)) { Add-Error 'W5-owned G3 evidence set is invalid.' }

# Manual-only workflow policy and validator wiring.
Validate-ManualWorkflow $paths.workflow $text.workflow @(
    'contents: read',
    'persist-credentials: false',
    'validate-current-packets.ps1',
    'validate-w5-current.ps1'
)
Validate-ManualWorkflow $paths.swarm_workflow $text.swarm_workflow @(
    'contents: read',
    'persist-credentials: false',
    'validate-current-packets.ps1',
    'validate-w5-current.ps1',
    'validate-proof-packets.ps1'
)

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    owners = $owners.Count
    settings_fields = $fields.Count
    current_probes = $currentProbeIds.Count
    rust_probes = $rustIds.Count
    packet_packages = $packetPackages.Count
    w5_gate_evidence = $w5Evidence.Count
    status = Toml-String $text.packet 'status'
    parser_profile = Toml-String $text.manifest 'rust_parser_profile'
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host 'ELIOT Search W5 deep contract validation'
    Write-Host "owners=$($result.owners) settings=$($result.settings_fields) current_probes=$($result.current_probes) rust_probes=$($result.rust_probes) packets=$($result.packet_packages) status=$($result.status)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
