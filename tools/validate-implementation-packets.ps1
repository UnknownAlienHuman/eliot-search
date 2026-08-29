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
function Get-QuotedValues([string]$Text) {
    @([regex]::Matches($Text, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
}
function Get-TomlString([string]$Text, [string]$Key, [bool]$Required = $true) {
    $pattern = '(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing string key '$Key'." }
        return ''
    }
    $match.Groups[1].Value
}
function Get-TomlInt([string]$Text, [string]$Key, [bool]$Required = $true) {
    $pattern = '(?m)^{0}\s*=\s*(\d+)\s*$' -f [regex]::Escape($Key)
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing integer key '$Key'." }
        return 0
    }
    [int64]$match.Groups[1].Value
}
function Get-TomlBool([string]$Text, [string]$Key, [bool]$Required = $true) {
    $pattern = '(?m)^{0}\s*=\s*(true|false)\s*$' -f [regex]::Escape($Key)
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing boolean key '$Key'." }
        return $false
    }
    $match.Groups[1].Value -eq 'true'
}
function Get-TomlArray([string]$Text, [string]$Key) {
    $pattern = '(?ms)^{0}\s*=\s*\[(.*?)\]' -f [regex]::Escape($Key)
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
function Require-Tokens([string]$Path, [string]$Text, [string[]]$Tokens) {
    foreach ($token in $Tokens) {
        if ($Text.IndexOf($token, [StringComparison]::Ordinal) -lt 0) {
            Add-Error "$Path is missing required token: $token"
        }
    }
}

$registryText = Read-Required 'swarm/crates.toml'
$sectionsText = Read-Required 'config/sections.toml'
$exampleText = Read-Required 'config/eliot-search.example.toml'
$launchText = Read-Required 'swarm/launch-state.toml'

$packageBlocks = [regex]::Split($registryText, '(?m)^\[\[package\]\]\s*$')
$packages = [ordered]@{}
for ($i = 1; $i -lt $packageBlocks.Count; $i++) {
    $block = $packageBlocks[$i]
    $name = Get-TomlString $block 'name'
    if ([string]::IsNullOrWhiteSpace($name)) { continue }
    if ($packages.Contains($name)) { Add-Error "Duplicate package '$name'."; continue }
    $packages[$name] = [pscustomobject]@{
        Name = $name
        Path = Get-TomlString $block 'path'
        Kind = Get-TomlString $block 'kind'
        Wave = [int](Get-TomlInt $block 'wave')
        Functions = Get-TomlString $block 'functions' $false
        ConfigSections = @(Get-TomlArray $block 'config_sections')
        Qualification = Get-TomlString $block 'qualification' $false
        Deps = @(Get-TomlArray $block 'deps')
    }
}

$declaredPackages = [int](Get-TomlInt $packageBlocks[0] 'package_count')
$declaredLibraries = [int](Get-TomlInt $packageBlocks[0] 'library_package_count')
$declaredBinaries = [int](Get-TomlInt $packageBlocks[0] 'binary_package_count')
$actualLibraries = @($packages.Values | Where-Object Kind -eq 'lib').Count
$actualBinaries = @($packages.Values | Where-Object Kind -eq 'bin').Count
if ($packages.Count -ne $declaredPackages) { Add-Error "Registry declares $declaredPackages packages but parsed $($packages.Count)." }
if ($actualLibraries -ne $declaredLibraries) { Add-Error "Registry library count $declaredLibraries differs from parsed $actualLibraries." }
if ($actualBinaries -ne $declaredBinaries) { Add-Error "Registry binary count $declaredBinaries differs from parsed $actualBinaries." }
if ($declaredPackages -ne 45 -or $declaredLibraries -ne 41 -or $declaredBinaries -ne 4) {
    Add-Error "Expected synchronized 45-package topology (41 libraries, 4 binaries); registry declares $declaredPackages/$declaredLibraries/$declaredBinaries."
}
if (-not $packages.Contains('search-config')) { Add-Error 'search-config is absent from the package registry.' }

$sectionBlocks = [regex]::Split($sectionsText, '(?m)^\[\[section\]\]\s*$')
$sections = [ordered]@{}
$allowedReload = @(
    'NOOP', 'APPLY_LIVE', 'SECURITY_BARRIER', 'RESTART_DEPENDENCY', 'DRAIN_AND_RESTART',
    'NEW_COLLECTION_GENERATION', 'REBUILD_PROJECTION', 'GATE_REQUIRED', 'REJECT'
)
$allowedSecretPolicies = @('forbid_plaintext', 'opaque_refs_only')
for ($i = 1; $i -lt $sectionBlocks.Count; $i++) {
    $block = $sectionBlocks[$i]
    $name = Get-TomlString $block 'name'
    if ([string]::IsNullOrWhiteSpace($name)) { continue }
    if ($sections.Contains($name)) { Add-Error "Duplicate configuration section '$name'."; continue }
    $owner = Get-TomlString $block 'owner'
    $firstWave = [int](Get-TomlInt $block 'first_wave')
    $reload = Get-TomlString $block 'reload'
    $secretPolicy = Get-TomlString $block 'secret_policy'
    $contract = Get-TomlString $block 'contract'
    $sections[$name] = [pscustomobject]@{
        Name = $name
        Owner = $owner
        FirstWave = $firstWave
        Reload = $reload
        SecretPolicy = $secretPolicy
        Contract = $contract
    }
    if ($allowedReload -cnotcontains $reload) { Add-Error "Section '$name' uses unknown reload class '$reload'." }
    if ($allowedSecretPolicies -cnotcontains $secretPolicy) { Add-Error "Section '$name' uses unknown secret policy '$secretPolicy'." }
    $contractText = Read-Required $contract
    $ownerToken = '- **Owner:** ' + [char]96 + $owner + [char]96
    if ($contractText -and $contractText.IndexOf($ownerToken, [StringComparison]::Ordinal) -lt 0) {
        Add-Error "Configuration packet '$contract' does not declare owner '$owner'."
    }
    foreach ($heading in @('## Fields', '## Required section API', '## Invariants', '## Required tests')) {
        if ($contractText -and $contractText.IndexOf($heading, [StringComparison]::Ordinal) -lt 0) {
            Add-Error "Configuration packet '$contract' lacks '$heading'."
        }
    }
}
if ($sections.Count -ne 20) { Add-Error "Expected 20 configuration sections; parsed $($sections.Count)." }

$registeredConfigSections = [System.Collections.Generic.List[string]]::new()
$functionPacketCount = 0
foreach ($package in $packages.Values) {
    if ($package.Functions) {
        $functionPacketCount++
        $functionText = Read-Required $package.Functions
        foreach ($pattern in @(
            '(?m)^# Function contract',
            '(?m)^#{2,3} `',
            '(?m)^## Required',
            '(?i)(cancell|deadline|timeout)',
            '(?i)(failure|error)'
        )) {
            if ($functionText -and $functionText -notmatch $pattern) {
                Add-Error "Function packet '$($package.Functions)' does not satisfy pattern '$pattern'."
            }
        }
    }
    if ($package.Qualification) { [void](Read-Required $package.Qualification) }

    foreach ($sectionName in $package.ConfigSections) {
        $registeredConfigSections.Add($sectionName)
        if (-not $sections.Contains($sectionName)) {
            Add-Error "$($package.Name) references unknown configuration section '$sectionName'."
            continue
        }
        $section = $sections[$sectionName]
        if ($section.Owner -cne $package.Name) {
            Add-Error "Configuration owner mismatch for '$sectionName': package=$($package.Name), section owner=$($section.Owner)."
        }
        if ($package.Wave -gt $section.FirstWave) {
            Add-Error "Section '$sectionName' begins at W$($section.FirstWave) before owner $($package.Name) W$($package.Wave)."
        }
        if ($package.Name -cne 'search-config' -and $package.Deps -cnotcontains 'search-config') {
            Add-Error "Configuration owner $($package.Name) does not depend on search-config."
        }
    }
}
if (-not (Same-Set $registeredConfigSections.ToArray() @($sections.Keys))) {
    Add-Error 'Package config_sections and config/sections.toml names differ.'
}

$exampleSections = @(
    [regex]::Matches($exampleText, '(?m)^\[([A-Za-z0-9_]+)\]\s*$') |
        ForEach-Object { $_.Groups[1].Value }
)
if (-not (Same-Set $exampleSections @($sections.Keys))) {
    Add-Error 'Example configuration sections differ from the central section registry.'
}
foreach ($line in ($exampleText -split '\r?\n')) {
    if ($line -match '^\s*#' -or $line -match '^\s*$') { continue }
    if ($line -match '(?i)^\s*(password|api_key|token|secret|private_key)\s*=') {
        Add-Error "Plaintext-secret-shaped key appears in example config: $line"
    }
}
if ($exampleText -match '(?m)^\s*auto_(download|upgrade)\s*=\s*true\s*$') {
    Add-Error 'Example configuration enables automatic Qdrant download/upgrade.'
}
if ($exampleText -notmatch '(?m)^profile_id\s*=\s*"UNQUALIFIED"\s*$') {
    Add-Error 'Example lexical profile must remain UNQUALIFIED before P06 acceptance.'
}
if ($exampleText -notmatch '(?ms)^\[qdrant_process\].*?^enabled\s*=\s*false\s*$') {
    Add-Error 'Example configuration must keep qdrant_process disabled.'
}

$qualificationFiles = @(
    'qualification/qdrant/README.md',
    'qualification/qdrant/W3_QUALIFICATION.md',
    'qualification/qdrant/artifact.toml',
    'qualification/qdrant/collection-schema.toml',
    'qualification/qdrant/probes.toml'
)
foreach ($file in $qualificationFiles) { [void](Read-Required $file) }
$artifactText = Read-Required 'qualification/qdrant/artifact.toml'
$probeText = Read-Required 'qualification/qdrant/probes.toml'
$collectionSchema = Read-Required 'qualification/qdrant/collection-schema.toml'
if (Get-TomlBool $artifactText 'automatic_download') { Add-Error 'Qdrant artifact contract permits automatic download.' }
if (Get-TomlBool $artifactText 'automatic_upgrade') { Add-Error 'Qdrant artifact contract permits automatic upgrade.' }
$status = Get-TomlString $artifactText 'status'
if ($status -notin @('UNQUALIFIED', 'QUALIFIED')) { Add-Error "Unknown Qdrant artifact status '$status'." }
if ($status -eq 'QUALIFIED') {
    foreach ($key in @('qualified_at', 'qualification_receipt_ref', 'reviewer_receipt_ref')) {
        if ([string]::IsNullOrWhiteSpace((Get-TomlString $artifactText $key))) { Add-Error "QUALIFIED artifact lacks '$key'." }
    }
}

$probeBlocks = [regex]::Split($probeText, '(?m)^\[\[probe\]\]\s*$')
$probeIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
for ($i = 1; $i -lt $probeBlocks.Count; $i++) {
    $id = Get-TomlString $probeBlocks[$i] 'id'
    if (-not $probeIds.Add($id)) { Add-Error "Duplicate Qdrant probe ID '$id'." }
}
$mandatoryProbeIds = @(
    'authenticated_loopback_health', 'artifact_process_identity', 'job_object_acl_shutdown',
    'strict_unindexed_retrieve_rejected', 'strict_unindexed_update_rejected',
    'schema_digest_equality', 'signed_i64_epoch_range', 'missing_valid_until_open_end',
    'sparse_idf_modifier', 'independent_idf_population_filter', 'idf_access_noninterference',
    'payload_index_completeness', 'wait_true_mutation_ack', 'strong_write_ordering',
    'exact_count_and_readback', 'uuid_point_identity', 'one_shard_topology',
    'lexical_document_query_golden', 'lexical_collision_corpus', 'point_collision_nonoverwrite',
    'publication_failpoint_matrix', 'route_epoch_pin_watermark', 'exact_reclaim_resume',
    'direct_mode_on_failure'
)
foreach ($id in $mandatoryProbeIds) {
    if (-not $probeIds.Contains($id)) { Add-Error "Missing mandatory Qdrant probe '$id'." }
}
if ($probeIds.Count -ne $mandatoryProbeIds.Count) {
    Add-Error "Expected $($mandatoryProbeIds.Count) Qdrant probes; parsed $($probeIds.Count)."
}
Require-Tokens 'qualification/qdrant/collection-schema.toml' $collectionSchema @(
    'shard_number = 1',
    '[strict_mode]',
    'enabled = true',
    'unindexed_filtering_retrieve = false',
    'unindexed_filtering_update = false',
    'name = "lex_code_v1"',
    'name = "lex_text_neutral_v1"',
    'field = "valid_from_epoch"',
    'field = "valid_until_epoch_exclusive"',
    'membership_arrays_allowed = false'
)

$launchPackages = [int](Get-TomlInt $launchText 'scaffold_package_count')
$launchLibraries = [int](Get-TomlInt $launchText 'library_package_count')
$launchBinaries = [int](Get-TomlInt $launchText 'binary_package_count')
if ($launchPackages -ne $declaredPackages -or $launchLibraries -ne $declaredLibraries -or $launchBinaries -ne $declaredBinaries) {
    Add-Error 'Launch-state package counts differ from the package registry.'
}

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    packages = $packages.Count
    configuration_sections = $sections.Count
    function_packets = $functionPacketCount
    qualification_probes = $probeIds.Count
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host 'ELIOT Search implementation-packet validation'
    Write-Host "packages=$($result.packages) config_sections=$($result.configuration_sections) function_packets=$($result.function_packets) qdrant_probes=$($result.qualification_probes)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
