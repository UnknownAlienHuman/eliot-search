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
function TStr([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*"([^"]*)"\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML string '$Key'." }
        return ''
    }
    $match.Groups[1].Value
}
function TInt([string]$Text, [string]$Key, [bool]$Required = $true) {
    $match = [regex]::Match($Text, ('(?m)^{0}\s*=\s*(\d+)\s*$' -f [regex]::Escape($Key)))
    if (-not $match.Success) {
        if ($Required) { Add-Error "Missing TOML integer '$Key'." }
        return [int64]0
    }
    [int64]$match.Groups[1].Value
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
function Validate-Path([string]$Owner, [string]$RelativePath, [string]$Kind) {
    if ([string]::IsNullOrWhiteSpace($RelativePath)) {
        Add-Error "$Owner has empty $Kind path."
        return
    }
    if ($RelativePath.StartsWith('docs/architecture/', [StringComparison]::Ordinal)) {
        Add-Error "$Owner declares forbidden ordinary architecture read: $RelativePath"
    }
    if (-not (Test-Path (Join-Path $Root $RelativePath) -PathType Leaf)) {
        Add-Error "$Owner references missing ${Kind}: $RelativePath"
    }
}

$registryPath = 'swarm/crates.toml'
$functionRegistryPath = 'swarm/function-packets.toml'
$launchPath = 'swarm/launch-state.toml'
$agentsPath = 'AGENTS.md'
$authorityPath = 'docs/handoff/AUTHORITY_MAP.md'
$workflowPath = '.github/workflows/function-packets.yml'

$registryText = Read-Required $registryPath
$functionText = Read-Required $functionRegistryPath
$launchText = Read-Required $launchPath
$agentsText = Read-Required $agentsPath
$authorityText = Read-Required $authorityPath
$workflowText = Read-Required $workflowPath

# Parse the package registry.
$registryBlocks = [regex]::Split($registryText, '(?m)^\[\[package\]\]\s*$')
$packages = [ordered]@{}
for ($i = 1; $i -lt $registryBlocks.Count; $i++) {
    $block = $registryBlocks[$i]
    $name = TStr $block 'name'
    if ([string]::IsNullOrWhiteSpace($name)) { continue }
    if ($packages.Contains($name)) { Add-Error "Duplicate crates registry package '$name'."; continue }
    $packages[$name] = [pscustomobject]@{
        Name = $name
        Path = TStr $block 'path'
        Wave = [int](TInt $block 'wave')
        Assignment = TStr $block 'assignment'
    }
}
if ($packages.Count -ne 45 -or (TInt $registryBlocks[0] 'package_count') -ne 45) {
    Add-Error "Expected 45 packages; parsed $($packages.Count)."
}

# Parse P00 foundation exceptions.
if ((TStr $functionText 'status') -cne 'bounded-function-packets') {
    Add-Error 'Function packet registry status is invalid.'
}
if ((TInt $functionText 'package_count') -ne 45) { Add-Error 'Function registry must declare 45 packages.' }
if ((TInt $functionText 'function_packet_count') -ne 42) { Add-Error 'Function registry must declare 42 function packets.' }
if ((TInt $functionText 'foundation_contract_package_count') -ne 3) { Add-Error 'Function registry must declare three P00 foundation packages.' }
if ((TStr $functionText 'ordinary_agent_architecture_access') -cne 'exception-only') {
    Add-Error 'Ordinary architecture access must remain exception-only.'
}

$foundationBlocks = [regex]::Split($functionText, '(?m)^\[\[foundation\]\]\s*$')
$foundation = [ordered]@{}
for ($i = 1; $i -lt $foundationBlocks.Count; $i++) {
    $block = $foundationBlocks[$i]
    $name = TStr $block 'package'
    if ($foundation.Contains($name)) { Add-Error "Duplicate foundation packet '$name'."; continue }
    $foundation[$name] = [pscustomobject]@{
        Wave = [int](TInt $block 'wave')
        Assignment = TStr $block 'assignment'
        Contract = TStr $block 'primary_contract'
        WriteScope = TStr $block 'write_scope'
    }
}
$expectedFoundation = @('search-contracts', 'search-domain', 'search-ports')
if (-not (Same-Set @($foundation.Keys) $expectedFoundation)) { Add-Error 'Foundation package set is invalid.' }
foreach ($name in $foundation.Keys) {
    if (-not $packages.Contains($name)) { Add-Error "Foundation packet references unknown package '$name'."; continue }
    $entry = $foundation[$name]
    $registered = $packages[$name]
    if ($entry.Wave -ne $registered.Wave -or $entry.Assignment -cne $registered.Assignment) {
        Add-Error "Foundation packet metadata mismatch for '$name'."
    }
    if ($entry.WriteScope -cne ($registered.Path + '/**')) { Add-Error "Foundation write scope mismatch for '$name'." }
    Validate-Path $name $entry.Assignment 'assignment'
    Validate-Path $name $entry.Contract 'primary contract'
}

# Parse all package-local function packets.
$functionBlocks = [regex]::Split($functionText, '(?m)^\[\[package\]\]\s*$')
$functionPackages = [ordered]@{}
$functionPaths = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$operationPacketCount = 0
$newW1W2Packets = [System.Collections.Generic.List[string]]::new()
for ($i = 1; $i -lt $functionBlocks.Count; $i++) {
    $block = $functionBlocks[$i]
    $name = TStr $block 'name'
    if ($functionPackages.Contains($name)) { Add-Error "Duplicate function packet package '$name'."; continue }
    if (-not $packages.Contains($name)) { Add-Error "Function packet references unknown package '$name'."; continue }

    $registered = $packages[$name]
    $wave = [int](TInt $block 'wave')
    $assignment = TStr $block 'assignment'
    $functions = TStr $block 'functions'
    $stagePacket = TStr $block 'stage_packet'
    $writeScope = TStr $block 'write_scope'

    if ($wave -ne $registered.Wave) { Add-Error "$name wave mismatch: W$wave != W$($registered.Wave)." }
    if ($assignment -cne $registered.Assignment) { Add-Error "$name assignment mismatch." }
    if ($writeScope -cne ($registered.Path + '/**')) { Add-Error "$name write scope must be '$($registered.Path)/**'." }
    if (-not $functions.StartsWith(($registered.Path + '/'), [StringComparison]::Ordinal) -or -not $functions.EndsWith('/FUNCTIONS.md', [StringComparison]::Ordinal)) {
        Add-Error "$name function path must be package-local FUNCTIONS.md: $functions"
    }
    if (-not $functionPaths.Add($functions)) { Add-Error "Duplicate function file path '$functions'." }

    Validate-Path $name $assignment 'assignment'
    Validate-Path $name $functions 'function packet'
    Validate-Path $name $stagePacket 'stage packet'

    $content = Read-Required $functions
    if ($content) {
        if ($content -notmatch '(?i)function contract') { Add-Error "$functions lacks a function-contract heading." }
        if ($content -notmatch '(?m)^#{2,3}\s+`') { Add-Error "$functions lacks operation headings." }
        if ($content -notmatch '(?i)cancellation|cancelled|deadline|timeout') { Add-Error "$functions lacks cancellation/deadline semantics." }
        if ($content -notmatch '(?i)typed failure|failure surface|typed errors|## failures') { Add-Error "$functions lacks typed failure semantics." }
        if ($content -notmatch '(?i)required tests|exit tests|qualification evidence|test seams') { Add-Error "$functions lacks required test/evidence semantics." }
        if ($content -match '(?i)todo!\(|unimplemented!\(|placeholder success') { Add-Error "$functions contains forbidden implementation placeholder language." }
        $operationPacketCount++
    }

    if ($wave -in @(1,2) -and $name -notin @('search-config', 'search-provider-protocol', 'eliot-search')) {
        $newW1W2Packets.Add($name)
    }
    $functionPackages[$name] = $functions
}

$expectedFunctionPackages = @($packages.Keys | Where-Object { $_ -notin $expectedFoundation })
if (-not (Same-Set @($functionPackages.Keys) $expectedFunctionPackages)) {
    Add-Error 'Function packet package set must equal all non-foundation packages.'
}
if ($functionPackages.Count -ne 42 -or $operationPacketCount -ne 42) {
    Add-Error "Expected 42 valid function packets; parsed $($functionPackages.Count), validated $operationPacketCount."
}

$expectedNewW1W2 = @(
    'search-runtime-owner', 'search-os-secrets', 'search-control-redb',
    'search-source-admission', 'search-source-registry', 'search-source-identity',
    'search-safe-reader', 'search-revision-store', 'search-materializer', 'search-unitizer',
    'eliot-searchd'
)
if (-not (Same-Set $newW1W2Packets.ToArray() $expectedNewW1W2)) {
    Add-Error 'W1/W2 function packet set is incomplete or unexpected.'
}

# Launch status is not changed by packet completeness.
if ((TInt $launchText 'active_wave') -ne 0 -or (TStr $launchText 'active_stage') -cne 'P00') {
    Add-Error 'Launch authority must remain P00/W0.'
}
$authorizedMatch = [regex]::Match($launchText, '(?ms)^authorized_packages\s*=\s*\[(.*?)\]')
$authorized = if ($authorizedMatch.Success) {
    @([regex]::Matches($authorizedMatch.Groups[1].Value, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
} else { @() }
if (-not (Same-Set $authorized @('search-contracts'))) { Add-Error 'Only search-contracts may be authorized.' }

# Root authority/docs must direct agents to the exact registry.
foreach ($token in @('swarm/function-packets.toml', 'swarm/crates.toml', 'swarm/launch-state.toml')) {
    if ($agentsText.IndexOf($token, [StringComparison]::Ordinal) -lt 0) { Add-Error "AGENTS.md lacks authority token '$token'." }
    if ($authorityText.IndexOf($token, [StringComparison]::Ordinal) -lt 0) { Add-Error "AUTHORITY_MAP.md lacks authority token '$token'." }
}

# Manual-only read-only workflow.
if ($workflowText.IndexOf('workflow_dispatch:', [StringComparison]::Ordinal) -lt 0) { Add-Error 'Function packet workflow lacks workflow_dispatch.' }
if ($workflowText -match '(?m)^\s*(pull_request|push|schedule):') { Add-Error 'Function packet workflow contains automatic trigger.' }
foreach ($token in @('contents: read', 'persist-credentials: false', 'validate-function-packets.ps1')) {
    if ($workflowText.IndexOf($token, [StringComparison]::Ordinal) -lt 0) { Add-Error "Function packet workflow lacks '$token'." }
}

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    packages = $packages.Count
    foundation_contracts = $foundation.Count
    function_packets = $functionPackages.Count
    validated_packets = $operationPacketCount
    w1_w2_packets = $newW1W2Packets.Count
    unique_function_paths = $functionPaths.Count
    launch_stage = TStr $launchText 'active_stage'
    launch_wave = TInt $launchText 'active_wave'
    authorized_packages = $authorized
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host 'ELIOT Search function-packet coverage validation'
    Write-Host "packages=$($result.packages) foundation=$($result.foundation_contracts) functions=$($result.function_packets) w1_w2=$($result.w1_w2_packets)"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host 'PASS' -ForegroundColor Green }
}
if (-not $result.ok) { exit 1 }
