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
function Read-Text([string]$RelativePath) {
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path $path -PathType Leaf)) {
        Add-Error "Missing required file: $RelativePath"
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
function Toml-Bool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML bool '$Key'." }
        return $false
    }
    $match.Groups[1].Value -eq "true"
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
function Parse-FieldBlocks([string]$Text) {
    $result = [ordered]@{}
    $matches = [regex]::Matches($Text, '(?ms)^\[\[([A-Za-z0-9_]+)\.field\]\]\s*(.*?)(?=^\[\[|^\[(?!\[)|\z)')
    foreach ($match in $matches) {
        $table = $match.Groups[1].Value
        $body = $match.Groups[2].Value
        $name = Toml-String $body "name"
        $key = "$table.$name"
        if ($result.Contains($key)) { Add-Error "Duplicate W8 settings field: $key"; continue }
        $defaultMatch = [regex]::Match($body, '(?m)^default\s*=\s*(.+?)\s*$')
        $result[$key] = [pscustomobject]@{
            Key = $key
            Table = $table
            Name = $name
            Mode = Toml-String $body "mode"
            DefaultRaw = if ($defaultMatch.Success) { $defaultMatch.Groups[1].Value.Trim() } else { "" }
        }
    }
    $result
}

$paths = [ordered]@{
    manifest = "docs/client/manifest.toml"
    cross = "docs/client/W8_GENERIC_CLIENT_EDGE_CONTRACTS_1.0.md"
    settings = "config/w8-client-edge.toml"
    settings_doc = "docs/config/W8_CLIENT_EDGE_SETTINGS_1.0.md"
    swarm = "swarm/w8-client-edge.toml"
    qualification = "qualification/client-edge/W8_QUALIFICATION.md"
    baseline = "qualification/client-edge/baseline.toml"
    probes = "qualification/client-edge/probes.toml"
    gate_map = "qualification/client-edge/gate-map.toml"
    fixture_owners = "qualification/client-edge/fixture-owners.toml"
    central_gates = "swarm/gates.toml"
    central_registry = "swarm/crates.toml"
    recipes = "docs/contracts/p00/RECIPES.md"
}
$text = [ordered]@{}
foreach ($entry in $paths.GetEnumerator()) { $text[$entry.Key] = Read-Text $entry.Value }

if ((Toml-String $text.manifest "status") -cne "contract-only") { Add-Error "W8 manifest status must be contract-only." }
if (Toml-Bool $text.manifest "implementation_authorized") { Add-Error "W8 manifest must not authorize implementation." }
if (Toml-Bool $text.manifest "optional_profiles_required_for_baseline") { Add-Error "Optional profiles cannot be required for standalone baseline." }
if (Toml-Bool $text.manifest "new_core_recipe_allowed") { Add-Error "W8 cannot add a core Search recipe." }
if ((Toml-String $text.swarm "status") -cne "BLOCKED") { Add-Error "W8 swarm packet must remain BLOCKED." }
if ((Toml-String $text.swarm "requires_accepted_gate") -cne "G3") { Add-Error "W8 must require accepted G3." }
if ((Toml-Int $text.swarm "requires_accepted_wave") -ne 7) { Add-Error "W8 must require accepted wave 7 lifecycle hardening." }

$expectedPackages = @(
    "search-provider-protocol",
    "eliot-searchd",
    "eliot-search",
    "search-eliot-adapter",
    "search-research-export-adapter"
)
$ownerBlocks = [regex]::Split($text.manifest, '(?m)^\[\[owner\]\]\s*$')
$owners = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $ownerBlocks.Count; $i++) {
    $package = Toml-String $ownerBlocks[$i] "package"
    if ($owners.Contains($package)) { Add-Error "Duplicate W8 owner: $package" } else { $owners.Add($package) }
}
if (-not (Same-Set $expectedPackages $owners.ToArray())) { Add-Error "W8 manifest owner set differs from expected package set." }

$packetBlocks = [regex]::Split($text.swarm, '(?m)^\[\[packet\]\]\s*$')
$packetPackages = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $packetBlocks.Count; $i++) {
    $block = $packetBlocks[$i]
    $package = Toml-String $block "package"
    $packetPackages.Add($package)
    foreach ($key in @("assignment", "functions", "hardening", "integration")) {
        $relative = Toml-String $block $key $false
        if ($relative -and -not (Test-Path (Join-Path $Root $relative) -PathType Leaf)) {
            Add-Error "$package references missing $key file: $relative"
        }
    }
}
if (-not (Same-Set $expectedPackages $packetPackages.ToArray())) { Add-Error "W8 swarm packet package set differs from manifest owners." }
foreach ($package in $expectedPackages) {
    if ($text.central_registry -notmatch ('(?m)^name\s*=\s*"' + [regex]::Escape($package) + '"\s*$')) {
        Add-Error "Central package registry lacks W8 package: $package"
    }
}

$expectedRecipes = @(
    "locate@1", "find_text@1", "inspect_entity@1", "compare_implementations@1", "explore_entity@1",
    "corpus_profile@1", "corpus_delta@1", "provenance@1", "compile_exact_scan@1",
    "execute_exact_scan@1", "expand_handle@1"
)
foreach ($recipe in $expectedRecipes) {
    if ($text.recipes.IndexOf($recipe, [StringComparison]::Ordinal) -lt 0) { Add-Error "P00 recipe registry lacks $recipe" }
    if ($text.cross.IndexOf($recipe, [StringComparison]::Ordinal) -lt 0) { Add-Error "W8 cross contract lacks $recipe" }
}

Require-Tokens $paths.cross $text.cross @(
    "## 3. Pairing and binding lifecycle",
    "## 5. Binding-filtered capability descriptor",
    "## 8. Handle and continuation expansion",
    "## 9. Client-owned evidence snapshot, pin and import",
    "## 11. Optional ELIOT compatibility profile",
    "## 12. Optional Research normalized-bundle export",
    "capability availability grants no authority",
    "ordinary export produces an immutable import/reference candidate and transfers no source ownership",
    "3a5f9fd2b254eebe574b2c4a28f9804df0da9df359e59ceee125fa7da90fef22"
)
Require-Tokens "crates/search-provider-protocol/W8_HARDENING.md" (Read-Text "crates/search-provider-protocol/W8_HARDENING.md") @(
    "issue_pairing_challenge", "verify_pairing_proof", "commit_binding", "revoke_binding",
    "project_capability_descriptor", "route_expand_handle", "Required W8 failures", "Required W8 fixtures"
)
Require-Tokens "bins/eliot-searchd/W8_INTEGRATION.md" (Read-Text "bins/eliot-searchd/W8_INTEGRATION.md") @(
    "compose_generic_client_edge", "build_authoritative_capability_snapshot", "mint_standalone_grant",
    "activate_optional_profile", "Coherent availability invariant", "Forbidden composition", "Required tests"
)
Require-Tokens "bins/eliot-search/FUNCTIONS.md" (Read-Text "bins/eliot-search/FUNCTIONS.md") @(
    "request_standalone_grant", "expand_handle", "map_exit_status", "never opens redb", "Required tests"
)
Require-Tokens "crates/search-eliot-adapter/FUNCTIONS.md" (Read-Text "crates/search-eliot-adapter/FUNCTIONS.md") @(
    "map_work_scope", "map_source_view_and_fence", "map_search_result", "validate_no_reverse_authority",
    "no ELIOT memory disposition", "Required tests"
)
Require-Tokens "crates/search-research-export-adapter/FUNCTIONS.md" (Read-Text "crates/search-research-export-adapter/FUNCTIONS.md") @(
    "reopen_and_verify_native_content", "compute_wire_digests", "validate_ownership_mode",
    "validate_bundle_paths", "recover_export_operation", "3a5f9fd2b254eebe574b2c4a28f9804df0da9df359e59ceee125fa7da90fef22",
    "Required tests"
)

if ((Toml-String $text.settings "status") -cne "schema-only") { Add-Error "W8 settings must remain schema-only." }
if (Toml-Bool $text.settings "implementation_authorized") { Add-Error "W8 settings cannot authorize implementation." }
$fields = Parse-FieldBlocks $text.settings
$lockedExpected = [ordered]@{
    "generic_edge.mutual_authentication_required" = "true"
    "generic_edge.pairing_proof_required" = "true"
    "generic_edge.binding_filtered_capabilities" = "true"
    "generic_edge.reverse_authority_allowed" = "false"
    "generic_edge.raw_store_access_allowed" = "false"
    "generic_edge.client_disposition_in_result_allowed" = "false"
    "standalone_cli.direct_store_access_allowed" = "false"
    "standalone_cli.allow_partial_exit_zero" = "false"
    "eliot_adapter.canonical_credentials_allowed" = "false"
    "eliot_adapter.reverse_write_channel_allowed" = "false"
    "eliot_adapter.memory_disposition_output_allowed" = "false"
    "eliot_adapter.fail_open_on_provider_error" = "false"
    "research_export.manifest_protocol" = '"eliotr.normalized.v1"'
    "research_export.manifest_body_sha256" = '"3a5f9fd2b254eebe574b2c4a28f9804df0da9df359e59ceee125fa7da90fef22"'
    "research_export.unsaved_content_allowed" = "false"
    "research_export.ordinary_export_transfers_ownership" = "false"
    "research_export.cutover_receipt_required" = "true"
    "research_export.unknown_load_bearing_fields" = '"reject"'
    "research_export.path_traversal_allowed" = "false"
    "research_export.cross_residency_dedup_allowed" = "false"
}
foreach ($entry in $lockedExpected.GetEnumerator()) {
    if (-not $fields.Contains($entry.Key)) { Add-Error "Missing locked W8 setting: $($entry.Key)"; continue }
    $field = $fields[$entry.Key]
    if ($field.Mode -cne "LOCKED") { Add-Error "$($entry.Key) must be LOCKED." }
    if ($field.DefaultRaw -cne $entry.Value) { Add-Error "$($entry.Key) default '$($field.DefaultRaw)' != '$($entry.Value)'." }
}
foreach ($optionalEnabled in @("eliot_adapter.enabled", "research_export.enabled")) {
    if (-not $fields.Contains($optionalEnabled) -or $fields[$optionalEnabled].DefaultRaw -cne "false") {
        Add-Error "$optionalEnabled must exist and default false."
    }
}
foreach ($qualifiedRef in @("eliot_adapter.compiled_feature_ref", "eliot_adapter.mapping_profile_ref", "research_export.compiled_feature_ref")) {
    if (-not $fields.Contains($qualifiedRef) -or $fields[$qualifiedRef].Mode -cne "QUALIFIED_REF" -or $fields[$qualifiedRef].DefaultRaw -cne '"UNSELECTED"') {
        Add-Error "$qualifiedRef must be an UNSELECTED QUALIFIED_REF."
    }
}

if ((Toml-String $text.baseline "status") -cne "UNQUALIFIED") { Add-Error "Client-edge baseline must remain UNQUALIFIED." }
Require-Tokens $paths.baseline $text.baseline @(
    "mutual_authentication_required = true",
    "binding_filtered = true",
    "availability_grants_authority = false",
    "exact_core_recipe_count = 11",
    "result_contains_client_disposition = false",
    "search_writes_client_canonical_store = false",
    "status = \"DISABLED\"",
    "ordinary_export_transfers_ownership = false"
)

$probeBlocks = [regex]::Split($text.probes, '(?m)^\[\[probe\]\]\s*$')
$probeIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$genericCount = 0
$optionalCount = 0
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $block = $probeBlocks[$i]
    $id = Toml-String $block "id"
    $profile = Toml-String $block "profile"
    $mandatory = Toml-Bool $block "mandatory"
    $result = Toml-String $block "result"
    if (-not $probeIds.Add($id)) { Add-Error "Duplicate client-edge probe ID: $id" }
    if ($profile -ceq "generic") {
        $genericCount++
        if (-not $mandatory -or $result -cne "UNAVAILABLE") { Add-Error "Generic probe $id must be mandatory UNAVAILABLE before execution." }
    } else {
        $optionalCount++
        if ($mandatory -or $result -cne "DISABLED") { Add-Error "Optional probe $id must be non-mandatory DISABLED before activation." }
    }
    if ((Toml-String $block "raw_output_ref") -ne "") { Add-Error "Unexecuted probe $id has raw output." }
    if ((Toml-String $block "reviewer_receipt_ref") -ne "") { Add-Error "Unexecuted probe $id has reviewer receipt." }
}
if ($probeIds.Count -ne 50 -or $genericCount -ne 33 -or $optionalCount -ne 17) {
    Add-Error "Expected 50 probes (33 generic, 17 optional); found $($probeIds.Count)/$genericCount/$optionalCount."
}

$expectedEvidence = @(
    "provider_frame_replay_cancel_limits",
    "authenticated_binding_and_grant",
    "capability_descriptor_filtering",
    "handle_expansion_reauthorization",
    "generic_request_plan_candidate_roundtrip",
    "eliot_adapter_mapping_when_enabled",
    "research_export_roundtrip_when_enabled"
)
$evidenceBlocks = [regex]::Split($text.gate_map, '(?m)^\[\[evidence\]\]\s*$')
$evidenceIds = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $evidenceBlocks.Count; $i++) {
    $block = $evidenceBlocks[$i]
    $id = Toml-String $block "id"
    $evidenceIds.Add($id)
    foreach ($probeId in (Toml-Array $block "probe_ids")) {
        if (-not $probeIds.Contains($probeId)) { Add-Error "Gate evidence $id references unknown probe $probeId." }
    }
    if ($text.central_gates.IndexOf('"' + $id + '"', [StringComparison]::Ordinal) -lt 0) {
        Add-Error "Central G4 gate lacks evidence ID $id."
    }
}
if (-not (Same-Set $expectedEvidence $evidenceIds.ToArray())) { Add-Error "Client-edge gate-map evidence set differs from existing G4 IDs." }

Require-Tokens $paths.qualification $text.qualification @(
    "## 2. Generic qualification sequence",
    "## 4. Capability descriptor evidence",
    "## 6. Client-owned evidence fixture",
    "## 8. Optional ELIOT profile",
    "## 9. Optional Research export profile",
    "## 10. Evidence record",
    "## 11. Stop conditions"
)

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    owners = $owners.Count
    packets = $packetPackages.Count
    generic_probes = $genericCount
    optional_probes = $optionalCount
    gate_evidence_ids = $evidenceIds.Count
    status = Toml-String $text.swarm "status"
    optional_profiles = "DISABLED"
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host "ELIOT Search W8 client-edge validation"
    Write-Host "owners=$($result.owners) packets=$($result.packets) generic_probes=$genericCount optional_probes=$optionalCount status=$($result.status)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host "PASS" -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
