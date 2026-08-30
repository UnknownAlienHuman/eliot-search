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
function Read-Text([string]$RelativePath) {
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
function Toml-Bool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML bool '$Key'." }
        return $false
    }
    $match.Groups[1].Value -eq 'true'
}
function Toml-Int([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML integer '$Key'." }
        return 0
    }
    [int64]$match.Groups[1].Value
}
function Toml-Array([string]$Text, [string]$Key) {
    $match = [regex]::Match($Text, ('(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)))
    if (-not $match.Success) { return @() }
    @([regex]::Matches($match.Groups[1].Value, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
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
function Require-Tokens([string]$Path, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if ($Text.IndexOf($token, [StringComparison]::Ordinal) -lt 0) {
            Add-Error "$Path is missing required token: $token"
        }
    }
}
function Validate-OptionalFile([string]$Owner, [string]$RelativePath, [string]$Kind) {
    if ([string]::IsNullOrWhiteSpace($RelativePath)) { return }
    if (-not (Test-Path (Join-Path $Root $RelativePath) -PathType Leaf)) {
        Add-Error "$Owner references missing $Kind file: $RelativePath"
    }
}
function Parse-Field-Blocks([string]$Text) {
    $fields = @{}
    $pattern = '(?ms)^\[\[([A-Za-z0-9_]+)\.field\]\]\s*(.*?)(?=^\[\[|^\[(?!\[)|\z)'
    foreach ($match in [regex]::Matches($Text, $pattern)) {
        $table = $match.Groups[1].Value
        $body = $match.Groups[2].Value
        $name = Toml-String $body 'name'
        $key = "$table.$name"
        if ($fields.ContainsKey($key)) { Add-Error "Duplicate W8 settings field: $key"; continue }
        $defaultMatch = [regex]::Match($body, '(?m)^default\s*=\s*(.+?)\s*$')
        $fields[$key] = [pscustomobject]@{
            Mode = Toml-String $body 'mode'
            DefaultRaw = if ($defaultMatch.Success) { $defaultMatch.Groups[1].Value.Trim() } else { '' }
        }
    }
    $fields
}

$paths = [ordered]@{
    manifest = 'docs/client/manifest.toml'
    cross = 'docs/client/W8_GENERIC_CLIENT_EDGE_CONTRACTS_1.0.md'
    settings = 'config/w8-client-edge.toml'
    settingsDoc = 'docs/config/W8_CLIENT_EDGE_SETTINGS_1.0.md'
    swarm = 'swarm/w8-client-edge.toml'
    qualification = 'qualification/client-edge/W8_QUALIFICATION.md'
    baseline = 'qualification/client-edge/baseline.toml'
    probes = 'qualification/client-edge/probes.toml'
    gateMap = 'qualification/client-edge/gate-map.toml'
    fixtureOwners = 'qualification/client-edge/fixture-owners.toml'
    centralGates = 'swarm/gates.toml'
    centralRegistry = 'swarm/crates.toml'
    recipes = 'docs/contracts/p00/RECIPES.md'
    protocol = 'crates/search-provider-protocol/W8_HARDENING.md'
    daemon = 'bins/eliot-searchd/W8_INTEGRATION.md'
    cliBase = 'bins/eliot-search/FUNCTIONS.md'
    cliClient = 'bins/eliot-search/W8_CLIENT.md'
    eliotAdapter = 'crates/search-eliot-adapter/FUNCTIONS.md'
    researchAdapter = 'crates/search-research-export-adapter/FUNCTIONS.md'
    workflow = '.github/workflows/w8-client-edge.yml'
}
$text = [ordered]@{}
foreach ($name in $paths.Keys) { $text[$name] = Read-Text $paths[$name] }

# Manifest and bounded packet closure.
if ((Toml-String $text.manifest 'status') -cne 'contract-only') { Add-Error 'W8 manifest status must be contract-only.' }
if (Toml-Bool $text.manifest 'implementation_authorized') { Add-Error 'W8 manifest must not authorize implementation.' }
if (Toml-Bool $text.manifest 'optional_profiles_required_for_baseline') { Add-Error 'Optional profiles cannot be required for standalone baseline.' }
if (Toml-Bool $text.manifest 'new_core_recipe_allowed') { Add-Error 'W8 cannot add a core Search recipe.' }
if ((Toml-String $text.swarm 'status') -cne 'BLOCKED') { Add-Error 'W8 swarm packet must remain BLOCKED.' }
if ((Toml-String $text.swarm 'requires_accepted_gate') -cne 'G3') { Add-Error 'W8 must require accepted G3.' }
if ((Toml-Int $text.swarm 'requires_accepted_wave') -ne 7) { Add-Error 'W8 must require accepted wave 7.' }

$expectedPackages = @(
    'search-provider-protocol', 'eliot-searchd', 'eliot-search',
    'search-eliot-adapter', 'search-research-export-adapter'
)
$ownerBlocks = [regex]::Split($text.manifest, '(?m)^\[\[owner\]\]\s*$')
$owners = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $ownerBlocks.Count; $i++) {
    $block = $ownerBlocks[$i]
    $package = Toml-String $block 'package'
    if ($owners.Contains($package)) { Add-Error "Duplicate W8 owner: $package" }
    else { $owners.Add($package) }
    foreach ($key in @('function_contract','hardening_contract','integration_contract','client_contract')) {
        Validate-OptionalFile $package (Toml-String $block $key $false) $key
    }
}
if (-not (Same-Set $expectedPackages $owners.ToArray())) {
    Add-Error 'W8 manifest owner set differs from expected packages.'
}
$eliotOwner = @($ownerBlocks | Where-Object { $_ -match '(?m)^package\s*=\s*"eliot-search"\s*$' })
if ($eliotOwner.Count -ne 1 -or (Toml-String $eliotOwner[0] 'client_contract') -cne $paths.cliClient) {
    Add-Error 'W8 manifest must bind eliot-search to W8_CLIENT.md.'
}

$packetBlocks = [regex]::Split($text.swarm, '(?m)^\[\[packet\]\]\s*$')
$packetPackages = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $packetBlocks.Count; $i++) {
    $block = $packetBlocks[$i]
    $package = Toml-String $block 'package'
    $packetPackages.Add($package)
    foreach ($key in @('assignment','functions','hardening','integration','client')) {
        Validate-OptionalFile $package (Toml-String $block $key $false) $key
    }
    if ($package -eq 'eliot-search' -and (Toml-String $block 'client') -cne $paths.cliClient) {
        Add-Error 'W8 eliot-search packet must name W8_CLIENT.md.'
    }
}
if (-not (Same-Set $expectedPackages $packetPackages.ToArray())) {
    Add-Error 'W8 packet package set differs from manifest owners.'
}
foreach ($package in $expectedPackages) {
    $pattern = '(?m)^name\s*=\s*"{0}"\s*$' -f [regex]::Escape($package)
    if ($text.centralRegistry -notmatch $pattern) { Add-Error "Central registry lacks W8 package: $package" }
}

# Recipe closure and operation packets.
$expectedRecipes = @(
    'locate@1', 'find_text@1', 'inspect_entity@1', 'compare_implementations@1',
    'explore_entity@1', 'corpus_profile@1', 'corpus_delta@1', 'provenance@1',
    'compile_exact_scan@1', 'execute_exact_scan@1', 'expand_handle@1'
)
foreach ($recipe in $expectedRecipes) {
    if ($text.recipes.IndexOf($recipe, [StringComparison]::Ordinal) -lt 0) { Add-Error "P00 recipe registry lacks $recipe" }
    if ($text.cross.IndexOf($recipe, [StringComparison]::Ordinal) -lt 0) { Add-Error "W8 contract lacks $recipe" }
}
Require-Tokens $paths.cross $text.cross @(
    '## 3. Pairing and binding lifecycle',
    '## 5. Binding-filtered capability descriptor',
    '## 8. Handle and continuation expansion',
    '## 9. Client-owned evidence snapshot, pin and import',
    '## 11. Optional ELIOT compatibility profile',
    '## 12. Optional Research normalized-bundle export',
    'descriptor availability never grants access or client authority',
    'Ordinary export produces an immutable import/reference candidate and transfers no source ownership.',
    '3a5f9fd2b254eebe574b2c4a28f9804df0da9df359e59ceee125fa7da90fef22'
)
Require-Tokens $paths.protocol $text.protocol @(
    'issue_pairing_challenge', 'verify_pairing_proof', 'commit_binding', 'revoke_binding',
    'project_capability_descriptor', 'route_expand_handle', 'Required W8 failures',
    'Required W8 fixtures'
)
Require-Tokens $paths.daemon $text.daemon @(
    'compose_generic_client_edge', 'build_authoritative_capability_snapshot',
    'mint_standalone_grant', 'activate_optional_profile',
    'Coherent availability invariant', 'Forbidden composition', 'Required tests'
)
Require-Tokens $paths.cliBase $text.cliBase @(
    'request_standalone_grant', 'expand_handle', 'map_exit_status',
    'never opens redb', 'Required tests'
)
Require-Tokens $paths.cliClient $text.cliClient @(
    'parse_client_invocation', 'resolve_local_endpoint', 'pair_and_bind', 'open_session',
    'fetch_capabilities', 'build_recipe_request', 'execute_request', 'render_terminal',
    'expand_handle', 'classify_exit_status', 'close_session',
    'Typed failures', 'Required tests / qualification evidence'
)
Require-Tokens $paths.eliotAdapter $text.eliotAdapter @(
    'map_work_scope', 'map_source_view_and_fence', 'map_search_result',
    'validate_no_reverse_authority', 'no ELIOT memory disposition', 'Required tests'
)
Require-Tokens $paths.researchAdapter $text.researchAdapter @(
    'reopen_and_verify_native_content', 'compute_wire_digests', 'validate_ownership_mode',
    'validate_bundle_paths', 'recover_export_operation',
    '3a5f9fd2b254eebe574b2c4a28f9804df0da9df359e59ceee125fa7da90fef22',
    'Required tests'
)

# Settings and baseline authority floors.
if ((Toml-String $text.settings 'status') -cne 'schema-only') { Add-Error 'W8 settings must remain schema-only.' }
if (Toml-Bool $text.settings 'implementation_authorized') { Add-Error 'W8 settings cannot authorize implementation.' }
$fields = Parse-Field-Blocks $text.settings
$lockedExpected = @{
    'generic_edge.mutual_authentication_required' = 'true'
    'generic_edge.pairing_proof_required' = 'true'
    'generic_edge.binding_filtered_capabilities' = 'true'
    'generic_edge.reverse_authority_allowed' = 'false'
    'generic_edge.raw_store_access_allowed' = 'false'
    'generic_edge.client_disposition_in_result_allowed' = 'false'
    'standalone_cli.direct_store_access_allowed' = 'false'
    'standalone_cli.allow_partial_exit_zero' = 'false'
    'eliot_adapter.canonical_credentials_allowed' = 'false'
    'eliot_adapter.reverse_write_channel_allowed' = 'false'
    'eliot_adapter.memory_disposition_output_allowed' = 'false'
    'eliot_adapter.fail_open_on_provider_error' = 'false'
    'research_export.manifest_protocol' = '"eliotr.normalized.v1"'
    'research_export.manifest_body_sha256' = '"3a5f9fd2b254eebe574b2c4a28f9804df0da9df359e59ceee125fa7da90fef22"'
    'research_export.unsaved_content_allowed' = 'false'
    'research_export.ordinary_export_transfers_ownership' = 'false'
    'research_export.cutover_receipt_required' = 'true'
    'research_export.unknown_load_bearing_fields' = '"reject"'
    'research_export.path_traversal_allowed' = 'false'
    'research_export.cross_residency_dedup_allowed' = 'false'
}
foreach ($entry in $lockedExpected.GetEnumerator()) {
    if (-not $fields.ContainsKey($entry.Key)) { Add-Error "Missing locked W8 setting: $($entry.Key)"; continue }
    $field = $fields[$entry.Key]
    if ($field.Mode -cne 'LOCKED') { Add-Error "$($entry.Key) must be LOCKED." }
    if ($field.DefaultRaw -cne $entry.Value) {
        Add-Error "$($entry.Key) default '$($field.DefaultRaw)' != '$($entry.Value)'."
    }
}
foreach ($key in @('eliot_adapter.enabled', 'research_export.enabled')) {
    if (-not $fields.ContainsKey($key) -or $fields[$key].DefaultRaw -cne 'false') {
        Add-Error "$key must default false."
    }
}
foreach ($key in @(
    'eliot_adapter.compiled_feature_ref', 'eliot_adapter.mapping_profile_ref',
    'research_export.compiled_feature_ref'
)) {
    if (-not $fields.ContainsKey($key) -or
        $fields[$key].Mode -cne 'QUALIFIED_REF' -or
        $fields[$key].DefaultRaw -cne '"UNSELECTED"') {
        Add-Error "$key must be an UNSELECTED QUALIFIED_REF."
    }
}
if ((Toml-String $text.baseline 'status') -cne 'UNQUALIFIED') {
    Add-Error 'Client-edge baseline must remain UNQUALIFIED.'
}
Require-Tokens $paths.baseline $text.baseline @(
    'mutual_authentication_required = true', 'binding_filtered = true',
    'availability_grants_authority = false', 'exact_core_recipe_count = 11',
    'result_contains_client_disposition = false',
    'search_writes_client_canonical_store = false', 'status = "DISABLED"',
    'ordinary_export_transfers_ownership = false'
)

# Probe and central G4 closure.
$probeBlocks = [regex]::Split($text.probes, '(?m)^\[\[probe\]\]\s*$')
$probeIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$genericCount = 0
$optionalCount = 0
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $block = $probeBlocks[$i]
    $id = Toml-String $block 'id'
    $profile = Toml-String $block 'profile'
    $mandatory = Toml-Bool $block 'mandatory'
    $result = Toml-String $block 'result'
    if (-not $probeIds.Add($id)) { Add-Error "Duplicate client-edge probe ID: $id" }
    if ($profile -ceq 'generic') {
        $genericCount++
        if (-not $mandatory -or $result -cne 'UNAVAILABLE') {
            Add-Error "Generic probe $id must be mandatory UNAVAILABLE."
        }
    } else {
        $optionalCount++
        if ($mandatory -or $result -cne 'DISABLED') {
            Add-Error "Optional probe $id must be non-mandatory DISABLED."
        }
    }
    if ((Toml-String $block 'raw_output_ref') -ne '') { Add-Error "Unexecuted probe $id has raw output." }
    if ((Toml-String $block 'reviewer_receipt_ref') -ne '') { Add-Error "Unexecuted probe $id has reviewer receipt." }
}
if ($probeIds.Count -ne 50 -or $genericCount -ne 33 -or $optionalCount -ne 17) {
    Add-Error "Expected 50 probes (33 generic, 17 optional); found $($probeIds.Count)/$genericCount/$optionalCount."
}
$expectedEvidence = @(
    'provider_frame_replay_cancel_limits', 'authenticated_binding_and_grant',
    'capability_descriptor_filtering', 'handle_expansion_reauthorization',
    'generic_request_plan_candidate_roundtrip', 'eliot_adapter_mapping_when_enabled',
    'research_export_roundtrip_when_enabled'
)
$evidenceBlocks = [regex]::Split($text.gateMap, '(?m)^\[\[evidence\]\]\s*$')
$evidenceIds = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $evidenceBlocks.Count; $i++) {
    $block = $evidenceBlocks[$i]
    $id = Toml-String $block 'id'
    $evidenceIds.Add($id)
    foreach ($probeId in (Toml-Array $block 'probe_ids')) {
        if (-not $probeIds.Contains($probeId)) {
            Add-Error "Gate evidence $id references unknown probe $probeId."
        }
    }
    if ($text.centralGates.IndexOf('"' + $id + '"', [StringComparison]::Ordinal) -lt 0) {
        Add-Error "Central G4 gate lacks evidence ID $id."
    }
}
if (-not (Same-Set $expectedEvidence $evidenceIds.ToArray())) {
    Add-Error 'Gate-map evidence set differs from central G4 IDs.'
}
Require-Tokens $paths.qualification $text.qualification @(
    '## 2. Generic qualification sequence', '## 4. Capability descriptor evidence',
    '## 6. Client-owned evidence fixture', '## 8. Optional ELIOT profile',
    '## 9. Optional Research export profile', '## 10. Evidence record',
    '## 11. Stop conditions'
)

# Workflows remain manual-only/read-only.
$workflowFiles = @(Get-ChildItem (Join-Path $Root '.github/workflows') -Filter '*.yml' -File)
foreach ($file in $workflowFiles) {
    $workflowText = [IO.File]::ReadAllText($file.FullName)
    if ($workflowText -match '(?m)^\s*(pull_request|pull_request_target|push|schedule|workflow_run|repository_dispatch|workflow_call|merge_group):') {
        Add-Error "Automatic workflow trigger found in $($file.Name)."
    }
    if ($workflowText.IndexOf('workflow_dispatch:', [StringComparison]::Ordinal) -lt 0) {
        Add-Error "Workflow $($file.Name) lacks workflow_dispatch."
    }
}
Require-Tokens $paths.workflow $text.workflow @(
    'contents: read', 'persist-credentials: false', 'validate-w8-client-edge.ps1'
)

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    owners = $owners.Count
    packets = $packetPackages.Count
    generic_probes = $genericCount
    optional_probes = $optionalCount
    gate_evidence_ids = $evidenceIds.Count
    workflows = $workflowFiles.Count
    status = Toml-String $text.swarm 'status'
    optional_profiles = 'DISABLED'
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host 'ELIOT Search W8 client-edge validation'
    Write-Host "owners=$($result.owners) packets=$($result.packets) generic_probes=$genericCount optional_probes=$optionalCount status=$($result.status)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
