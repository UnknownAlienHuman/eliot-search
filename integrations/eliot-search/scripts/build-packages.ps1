#requires -Version 7.0
[CmdletBinding(SupportsShouldProcess, ConfirmImpact = 'Low')]
param(
    [Parameter(Mandatory)][string]$BinaryDirectory,
    [Parameter(Mandatory)][string]$DataRoot,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$QdrantUrl
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-AbsolutePath([string]$Value, [string]$Label) {
    if ([string]::IsNullOrWhiteSpace($Value) -or -not [IO.Path]::IsPathFullyQualified($Value)) {
        throw "$Label must be an absolute path."
    }
    $full = [IO.Path]::GetFullPath($Value)
    if ($full -eq [IO.Path]::GetPathRoot($full)) { return $full }
    return $full.TrimEnd([char[]]@('\', '/'))
}

function Test-Within([string]$Candidate, [string]$Directory) {
    $prefix = $Directory.TrimEnd([char[]]@('\', '/')) + [IO.Path]::DirectorySeparatorChar
    return $Candidate.Equals($Directory, [StringComparison]::OrdinalIgnoreCase) -or
        $Candidate.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)
}

function New-PackageDirectory {
    [CmdletBinding(SupportsShouldProcess, ConfirmImpact = 'Low')]
    param([string]$Path)
    if (-not $PSCmdlet.ShouldProcess($Path, 'Create fresh package directory')) { throw 'Packaging declined.' }
    $parent = [IO.Path]::GetDirectoryName($Path)
    $leaf = [IO.Path]::GetFileName($Path)
    if (Test-Path -LiteralPath $Path) { throw "Output already exists: $Path" }
    $item = Get-Item -LiteralPath $parent
    if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Output parent must be an existing non-reparse directory: $parent"
    }
    [void](New-Item -Path $parent -Name $leaf -ItemType Directory -ErrorAction Stop)
}

function Write-NewJson([string]$Path, [System.Collections.IDictionary]$Value) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try {
        $bytes = [Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 30) + [Environment]::NewLine)
        $stream.Write($bytes, 0, $bytes.Length)
    } finally { $stream.Dispose() }
}

function Get-StreamHash([IO.Stream]$Stream) {
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($algorithm.ComputeHash($Stream)).Replace('-', '') }
    finally { $algorithm.Dispose() }
}

function Copy-Verified([string]$Source, [string]$Destination, [string]$ExpectedHash) {
    [IO.File]::Copy($Source, $Destination, $false)
    if ((Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash -ne $ExpectedHash) {
        throw "Copied bytes did not match: $Destination"
    }
}

function Get-BinaryVersion([string]$Path, [string]$Name) {
    # The real binaries handle this metadata argv before root/server setup.
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Path
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    [void]$start.ArgumentList.Add('--version')
    $probe = [Diagnostics.Process]::new()
    $probe.StartInfo = $start
    try {
        if (-not $probe.Start()) { throw "Version probe could not start: $Name" }
        $probe.StandardInput.Close()
        $stdout = $probe.StandardOutput.ReadToEndAsync()
        $stderr = $probe.StandardError.ReadToEndAsync()
        if (-not $probe.WaitForExit(10000)) {
            $probe.Kill($true)
            $probe.WaitForExit()
            throw "Version probe timed out: $Name"
        }
        if (-not [Threading.Tasks.Task]::WhenAll([Threading.Tasks.Task[]]@($stdout, $stderr)).Wait(10000)) {
            throw "Version probe pipes did not close: $Name"
        }
        if ($probe.ExitCode -ne 0) { throw "Version probe failed: $Name" }
        $outText = $stdout.GetAwaiter().GetResult().Trim()
        $errText = $stderr.GetAwaiter().GetResult().Trim()
        if ($Name -eq 'eliot-search-mcp') {
            $text = ($outText + $errText).Trim()
            if ($text -notmatch '^eliot-search-mcp ([0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?)$') {
                throw 'Unexpected MCP version response.'
            }
            return $Matches[1]
        }
        $value = ConvertFrom-Json -InputObject $outText -AsHashtable
        if ($value.binary -ne $Name -or $errText -ne '' -or
            $value.version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$') {
            throw "Unexpected version response: $Name"
        }
        return [string]$value.version
    } finally { $probe.Dispose() }
}

function New-VerifiedArchive {
    [CmdletBinding(SupportsShouldProcess, ConfirmImpact = 'Low')]
    param([string]$Path, [System.Collections.IDictionary]$Entries)
    if (-not $PSCmdlet.ShouldProcess($Path, 'Create new hash-verified package archive')) { throw 'Packaging declined.' }
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    $archive = $null
    try {
        $archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create, $false)
        foreach ($pair in $Entries.GetEnumerator()) {
            $entry = $archive.CreateEntry([string]$pair.Key, [IO.Compression.CompressionLevel]::Optimal)
            $inputStream = [IO.File]::OpenRead([string]$pair.Value)
            $outputStream = $entry.Open()
            try { $inputStream.CopyTo($outputStream) }
            finally { $outputStream.Dispose(); $inputStream.Dispose() }
        }
    } finally {
        if ($null -ne $archive) { $archive.Dispose() } else { $stream.Dispose() }
    }
    $readStream = [IO.File]::OpenRead($Path)
    $readArchive = $null
    try {
        $readArchive = [IO.Compression.ZipArchive]::new($readStream, [IO.Compression.ZipArchiveMode]::Read, $false)
        if ($readArchive.Entries.Count -ne $Entries.Count) { throw "Wrong archive member count: $Path" }
        foreach ($pair in $Entries.GetEnumerator()) {
            $entry = $readArchive.GetEntry([string]$pair.Key)
            if ($null -eq $entry) { throw "Missing archive member: $($pair.Key)" }
            $content = $entry.Open()
            try { $actual = Get-StreamHash $content } finally { $content.Dispose() }
            if ($actual -ne (Get-FileHash -LiteralPath ([string]$pair.Value) -Algorithm SHA256).Hash) {
                throw "Archive member bytes did not match: $($pair.Key)"
            }
        }
    } finally {
        if ($null -ne $readArchive) { $readArchive.Dispose() } else { $readStream.Dispose() }
    }
}

if (-not $IsWindows) { throw 'This builder packages the Windows Release pair only.' }
$binaryPath = Get-AbsolutePath $BinaryDirectory 'BinaryDirectory'
$dataRootPath = Get-AbsolutePath $DataRoot 'DataRoot' # Strings only: never open/stat the dataset.
$outputPath = Get-AbsolutePath $OutputDirectory 'OutputDirectory'
$packageSource = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$repositoryPath = Get-AbsolutePath ([IO.Path]::GetFullPath((Join-Path $packageSource '../..'))) 'Repository'
foreach ($protectedPath in @($dataRootPath, $binaryPath, $repositoryPath)) {
    if ((Test-Within $outputPath $protectedPath) -or (Test-Within $protectedPath $outputPath)) {
        throw 'OutputDirectory must not overlap the dataset, input binaries or source checkout.'
    }
}
if (Test-Path -LiteralPath $outputPath) { throw 'OutputDirectory must be fresh; nothing is overwritten.' }
if ($PSBoundParameters.ContainsKey('QdrantUrl')) {
    [Uri]$endpoint = $null
    if ([string]::IsNullOrWhiteSpace($QdrantUrl) -or
        -not [Uri]::TryCreate($QdrantUrl, [UriKind]::Absolute, [ref]$endpoint) -or
        $endpoint.Scheme -notin @('http', 'https')) {
        throw 'QdrantUrl must be a nonempty absolute HTTP(S) URL when supplied.'
    }
}
$macroStart = '$' + '{'
foreach ($literal in @($dataRootPath, $outputPath, $QdrantUrl)) {
    if ($null -ne $literal -and $literal.Contains($macroStart)) {
        throw 'Dataset/output/URL must not contain client macro syntax; literal escaping is not assumed.'
    }
}
$binaryNames = @('eliot-search-mcp.exe', 'eliot-searchd.exe', 'eliot-search.exe')
$inputHashes = [ordered]@{}
foreach ($name in $binaryNames) {
    $path = Join-Path $binaryPath $name
    $item = Get-Item -LiteralPath $path
    if ($item.PSIsContainer -or $item.Length -eq 0 -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Expected a real nonempty binary file: $path"
    }
    $inputHashes[$name] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
}
$plugin = Get-Content -LiteralPath (Join-Path $packageSource 'plugin.json') -Raw | ConvertFrom-Json -AsHashtable
$portable = Get-Content -LiteralPath (Join-Path $packageSource 'mcp.template.json') -Raw | ConvertFrom-Json -AsHashtable
$desktop = Get-Content -LiteralPath (Join-Path $packageSource 'desktop/manifest.template.json') -Raw | ConvertFrom-Json -AsHashtable
if (-not $PSCmdlet.ShouldProcess($outputPath, 'Verify matching Release binaries and create bound ELIOT Search artifacts')) { return }
$mcpVersion = Get-BinaryVersion (Join-Path $binaryPath $binaryNames[0]) 'eliot-search-mcp'
$daemonVersion = Get-BinaryVersion (Join-Path $binaryPath $binaryNames[1]) 'eliot-searchd'
$operatorVersion = Get-BinaryVersion (Join-Path $binaryPath $binaryNames[2]) 'eliot-search'
if ($mcpVersion -ne $daemonVersion -or $mcpVersion -ne $operatorVersion -or
    $plugin.version -ne $mcpVersion -or $desktop.version -ne $mcpVersion) {
    throw 'All three actual binary versions and both package manifests must match; update metadata explicitly.'
}
$skillSource = Join-Path $packageSource 'skills/eliot-search/SKILL.md'
$skillHash = (Get-FileHash -LiteralPath $skillSource -Algorithm SHA256).Hash
$packagePath = Join-Path $outputPath 'plugin'
$binPath = Join-Path $packagePath 'bin'
$skillPath = Join-Path $packagePath 'skills/eliot-search'
$desktopPath = Join-Path $outputPath 'desktop'
$launchArgs = @('--data-root', $dataRootPath)
if ($PSBoundParameters.ContainsKey('QdrantUrl')) { $launchArgs += @('--qdrant-url', $QdrantUrl) }
# Portable schema, machine-bound connection: no invented plugin-root macro.
$portable.mcpServers['eliot-search'].command = Join-Path $binPath 'eliot-search-mcp.exe'
$portable.mcpServers['eliot-search'].args = $launchArgs
# Only the documented MCPB extension-directory substitution is retained.
$desktop.server.mcp_config.command = '${__dirname}/server/eliot-search-mcp.exe'
$desktop.server.mcp_config.args = $launchArgs

# Preserve failure outputs; no recursive cleanup or rollback deletion.
New-PackageDirectory $outputPath -Confirm:$false
foreach ($path in @($packagePath, $binPath, (Join-Path $packagePath 'skills'), $skillPath, $desktopPath)) {
    New-PackageDirectory $path -Confirm:$false
}
Write-NewJson (Join-Path $packagePath 'plugin.json') $plugin
Write-NewJson (Join-Path $packagePath 'mcp.json') $portable
Write-NewJson (Join-Path $desktopPath 'manifest.json') $desktop
foreach ($name in $binaryNames) {
    Copy-Verified (Join-Path $binaryPath $name) (Join-Path $binPath $name) $inputHashes[$name]
}
Copy-Verified $skillSource (Join-Path $skillPath 'SKILL.md') $skillHash
$desktopBundle = Join-Path $outputPath 'eliot-search.mcpb'
New-VerifiedArchive $desktopBundle ([ordered]@{
    'manifest.json' = (Join-Path $desktopPath 'manifest.json')
    'server/eliot-search-mcp.exe' = (Join-Path $binPath 'eliot-search-mcp.exe')
    'server/eliot-searchd.exe' = (Join-Path $binPath 'eliot-searchd.exe')
}) -Confirm:$false
$skillArchive = Join-Path $outputPath 'eliot-search-skill.zip'
New-VerifiedArchive $skillArchive ([ordered]@{ 'eliot-search/SKILL.md' = (Join-Path $skillPath 'SKILL.md') }) -Confirm:$false
foreach ($name in $binaryNames) {
    foreach ($path in @((Join-Path $binaryPath $name), (Join-Path $binPath $name))) {
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $inputHashes[$name]) {
            throw "Binary bytes changed during packaging: $path"
        }
    }
}
foreach ($path in @($skillSource, (Join-Path $skillPath 'SKILL.md'))) {
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $skillHash) {
        throw 'Skill bytes changed during packaging.'
    }
}
[pscustomobject]@{
    Version = $mcpVersion
    MachineBoundPluginDirectory = $packagePath
    DesktopBundle = $desktopBundle
    SkillArchive = $skillArchive
    BinarySHA256 = $inputHashes
    SkillSHA256 = $skillHash
    InstalledOrRegistered = $false
}
