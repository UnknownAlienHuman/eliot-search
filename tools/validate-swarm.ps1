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
    @([regex]::Matches($Text, '"([^"\r\n]+)"') | ForEach-Object { $_.Groups[1].Value })
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
function Get-TomlBool([string]$Text, [string]$Key, [bool]$Default = $false) {
    $pattern = '(?m)^{0}\s*=\s*(true|false)\s*$' -f ([regex]::Escape($Key))
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) { return $Default }
    $match.Groups[1].Value -eq "true"
}
function Get-TomlArray([string]$Text, [string]$Key) {
    $pattern = '(?ms)^{0}\s*=\s*\[(.*?)\]' -f ([regex]::Escape($Key))
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) { return @() }
    Get-QuotedValues $match.Groups[1].Value
}
function Get-Section([string]$Text, [string]$Name) {
    $pattern = '(?ms)^\[{0}\]\s*(.*?)(?=^\[|\z)' -f ([regex]::Escape($Name))
    $match = [regex]::Match($Text, $pattern)
    if ($match.Success) { return $match.Groups[1].Value }
    ""
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

$rootCargoPath = Join-Path $Root "Cargo.toml"
$registryPath = Join-Path $Root "swarm/crates.toml"
$launchPath = Join-Path $Root "swarm/launch-state.toml"
foreach ($required in @($rootCargoPath, $registryPath, $launchPath)) {
    if (-not (Test-Path $required -PathType Leaf)) { Add-Error "Missing required file: $required" }
}
if ($errors.Count -gt 0) { throw ($errors -join [Environment]::NewLine) }

$rootCargo = [IO.File]::ReadAllText($rootCargoPath)
$registry = [IO.File]::ReadAllText($registryPath)
$launch = [IO.File]::ReadAllText($launchPath)

$workspaceSection = Get-Section $rootCargo "workspace"
$membersMatch = [regex]::Match($workspaceSection, '(?ms)^members\s*=\s*\[(.*?)\]')
$workspaceMembers = if ($membersMatch.Success) { @(Get-QuotedValues $membersMatch.Groups[1].Value) } else { @() }
if ($workspaceMembers.Count -eq 0) { Add-Error "Root Cargo workspace has no members." }

$blocks = [regex]::Split($registry, '(?m)^\[\[package\]\]\s*$')
$preamble = $blocks[0]
$packages = [ordered]@{}
for ($i = 1; $i -lt $blocks.Count; $i++) {
    $block = $blocks[$i]
    $name = Get-TomlString $block "name"
    if ([string]::IsNullOrWhiteSpace($name)) { continue }
    if ($packages.Contains($name)) { Add-Error "Duplicate registry package: $name"; continue }
    $packages[$name] = [pscustomobject]@{
        Name = $name
        Path = Get-TomlString $block "path"
        Kind = Get-TomlString $block "kind"
        Wave = [int](Get-TomlInt $block "wave")
        Optional = Get-TomlBool $block "optional"
        Target = [int](Get-TomlInt $block "soft_src_line_target")
        Assignment = Get-TomlString $block "assignment"
        Deps = @(Get-TomlArray $block "deps")
        Progressive = Get-TomlBool $block "progressive_composition"
    }
}

$declaredPackageCount = [int](Get-TomlInt $preamble "package_count")
$declaredLibraryCount = [int](Get-TomlInt $preamble "library_package_count")
$declaredBinaryCount = [int](Get-TomlInt $preamble "binary_package_count")
$hardLimit = [int](Get-TomlInt $preamble "hard_handwritten_rust_line_limit")

if ($packages.Count -ne $declaredPackageCount) { Add-Error "Registry package_count=$declaredPackageCount but parsed $($packages.Count)." }
$actualLibraries = @($packages.Values | Where-Object Kind -eq "lib").Count
$actualBinaries = @($packages.Values | Where-Object Kind -eq "bin").Count
if ($actualLibraries -ne $declaredLibraryCount) { Add-Error "Registry library count mismatch." }
if ($actualBinaries -ne $declaredBinaryCount) { Add-Error "Registry binary count mismatch." }

$registryPaths = @($packages.Values | ForEach-Object Path)
if (-not (Same-Set $workspaceMembers $registryPaths)) { Add-Error "Cargo workspace members differ from registry package paths." }

$internalNames = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$packages.Keys | ForEach-Object { [void]$internalNames.Add($_) }

foreach ($package in $packages.Values) {
    foreach ($dep in $package.Deps) {
        if (-not $packages.Contains($dep)) { Add-Error "$($package.Name) references unknown dependency $dep." }
    }

    $packageDir = Join-Path $Root $package.Path
    $manifestPath = Join-Path $packageDir "Cargo.toml"
    foreach ($path in @($packageDir, $manifestPath, (Join-Path $packageDir "AGENTS.md"), (Join-Path $packageDir "README.md"), (Join-Path $packageDir "src"))) {
        if (-not (Test-Path $path)) { Add-Error "Missing package artifact: $path" }
    }
    $assignmentPath = Join-Path $Root $package.Assignment
    if (-not (Test-Path $assignmentPath -PathType Leaf)) { Add-Error "Missing assignment for $($package.Name): $($package.Assignment)" }
    if (-not (Test-Path $manifestPath -PathType Leaf)) { continue }

    $manifest = [IO.File]::ReadAllText($manifestPath)
    $manifestName = Get-TomlString (Get-Section $manifest "package") "name"
    if ($manifestName -cne $package.Name) { Add-Error "Manifest name mismatch at $manifestPath: '$manifestName' != '$($package.Name)'." }

    $dependencySection = Get-Section $manifest "dependencies"
    $manifestInternalDeps = [System.Collections.Generic.List[string]]::new()
    foreach ($line in ($dependencySection -split '\r?\n')) {
        $match = [regex]::Match($line, '^\s*([A-Za-z0-9_-]+)(?:\.workspace\s*=\s*true|\s*=\s*\{[^}]*\bworkspace\s*=\s*true[^}]*\})')
        if ($match.Success -and $internalNames.Contains($match.Groups[1].Value)) { $manifestInternalDeps.Add($match.Groups[1].Value) }
    }
    if (-not (Same-Set $package.Deps $manifestInternalDeps.ToArray())) {
        Add-Error "$($package.Name) manifest dependencies differ from registry. Registry=[$($package.Deps -join ', ')] Cargo=[$($manifestInternalDeps -join ', ')]."
    }

    $rustFiles = @(Get-ChildItem (Join-Path $packageDir "src") -Filter "*.rs" -Recurse -File -ErrorAction SilentlyContinue)
    $lineCount = 0
    foreach ($file in $rustFiles) {
        $lineCount += @([IO.File]::ReadAllLines($file.FullName)).Count
        $content = [IO.File]::ReadAllText($file.FullName)
        if ($content -match '\b(todo!|unimplemented!)\s*\(') { Add-Error "Forbidden placeholder macro in $($file.FullName)." }
    }
    if ($lineCount -gt $hardLimit) { Add-Error "$($package.Name) exceeds hard Rust line limit: $lineCount > $hardLimit." }
    elseif ($lineCount -gt $package.Target) { Add-Warning "$($package.Name) exceeds soft src target: $lineCount > $($package.Target)." }
}

$visiting = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$visited = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
function Visit-Package([string]$Name, [string[]]$Stack) {
    if ($script:visited.Contains($Name)) { return }
    if ($script:visiting.Contains($Name)) {
        Add-Error "Dependency cycle: $(($Stack + $Name) -join ' -> ')"
        return
    }
    [void]$script:visiting.Add($Name)
    $package = $script:packages[$Name]
    foreach ($dep in $package.Deps) {
        if ($script:packages.Contains($dep)) {
            Visit-Package $dep ($Stack + $Name)
            if (-not $package.Progressive -and $script:packages[$dep].Wave -gt $package.Wave) {
                Add-Error "$Name (W$($package.Wave)) depends on later $dep (W$($script:packages[$dep].Wave))."
            }
        }
    }
    [void]$script:visiting.Remove($Name)
    [void]$script:visited.Add($Name)
}
$packages.Keys | ForEach-Object { Visit-Package $_ @() }

$assignmentDir = Join-Path $Root "swarm/assignments"
$assignmentFiles = @(Get-ChildItem $assignmentDir -Filter "*.md" -File | Where-Object Name -ne "README.md" | ForEach-Object { "swarm/assignments/$($_.Name)" })
$registryAssignments = @($packages.Values | ForEach-Object Assignment)
if (-not (Same-Set $assignmentFiles $registryAssignments)) { Add-Error "Assignment files differ from registry assignments." }

$launchPackageCount = [int](Get-TomlInt $launch "scaffold_package_count")
$launchLibraryCount = [int](Get-TomlInt $launch "library_package_count")
$launchBinaryCount = [int](Get-TomlInt $launch "binary_package_count")
if ($launchPackageCount -ne $packages.Count -or $launchLibraryCount -ne $actualLibraries -or $launchBinaryCount -ne $actualBinaries) {
    Add-Error "Launch-state package counts differ from registry."
}
$activeWave = [int](Get-TomlInt $launch "active_wave")
$authorized = @(Get-TomlArray $launch "authorized_packages")
$conditional = @(Get-TomlArray $launch "conditionally_authorized_packages")
foreach ($name in @($authorized + $conditional)) {
    if (-not $packages.Contains($name)) { Add-Error "Launch state names unknown package $name."; continue }
    if ($packages[$name].Wave -ne $activeWave) { Add-Error "Launch-authorized $name is W$($packages[$name].Wave), active wave is W$activeWave." }
}

$p00Files = @("README.md", "CANONICAL_TYPES.md", "CONTRACT_CHALLENGES.md", "SOURCE_GRAPH.md", "RECIPES.md", "QUERY_AND_RESULTS.md", "PROTOCOL_AND_LIFECYCLE.md", "REASON_CODES.md", "PORT_OPERATIONS.md")
foreach ($file in $p00Files) {
    if (-not (Test-Path (Join-Path $Root "docs/contracts/p00/$file") -PathType Leaf) { Add-Error "Missing P00 contract file: $file" }
}

$result = [ordered]@{
    ok = ($errors.Count -eq 0)
    packages = $packages.Count
    libraries = $actualLibraries
    binaries = $actualBinaries
    assignments = $assignmentFiles.Count
    active_wave = $activeWave
    authorized = $authorized
    conditional = $conditional
    warnings = @($warnings)
    errors = @($errors)
}

if ($Json) { $result | ConvertTo-Json -Depth 8 }
else {
    Write-Host "ELIOT Search swarm validation"
    Write-Host "packages=$($result.packages) libraries=$($result.libraries) binaries=$($result.binaries) assignments=$($result.assignments)"
    Write-Host "active_wave=W$activeWave authorized=[$($authorized -join ', ')] conditional=[$($conditional -join ', ')]"
    foreach ($warning in $warnings) { Write-Warning $warning }
    foreach ($error in $errors) { Write-Host "ERROR: $error" -ForegroundColor Red }
    if ($result.ok) { Write-Host "PASS" -ForegroundColor Green }
}

if (-not $result.ok) { exit 1 }
