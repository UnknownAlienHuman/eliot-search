[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RepositoryRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Runner = Join-Path $RepositoryRoot 'tools\eliot-search-dev.ps1'
$Nonce = [Guid]::NewGuid().ToString('N')
$Sandbox = Join-Path ([IO.Path]::GetTempPath()) "Eliot Search qualification $Nonce"
$StateDir = Join-Path $Sandbox 'runtime state'
$SourceRoot = Join-Path $Sandbox 'source root with spaces'
$RootConfig = Join-Path $StateDir 'source-roots.json'
$Endpoint = Join-Path $StateDir 'endpoint.txt'
$Owner = Join-Path $StateDir 'owner.lock'

function Invoke-RunnerProcess {
    param(
        [Parameter(Mandatory)][string[]]$Arguments,
        [int]$ExpectedExitCode = 0
    )

    $outputFile = Join-Path $Sandbox ("runner-{0}.stdout" -f [Guid]::NewGuid().ToString('N'))
    $errorFile = Join-Path $Sandbox ("runner-{0}.stderr" -f [Guid]::NewGuid().ToString('N'))
    $parameters = @{
        FilePath = (Get-Command pwsh).Source
        ArgumentList = @('-NoProfile', '-File', $Runner) + $Arguments
        WorkingDirectory = $RepositoryRoot
        RedirectStandardOutput = $outputFile
        RedirectStandardError = $errorFile
        PassThru = $true
        Wait = $true
        NoNewWindow = $true
    }
    $process = Start-Process @parameters
    $stdout = if (Test-Path -LiteralPath $outputFile) {
        Get-Content -LiteralPath $outputFile -Raw
    }
    else {
        ''
    }
    $stderr = if (Test-Path -LiteralPath $errorFile) {
        Get-Content -LiteralPath $errorFile -Raw
    }
    else {
        ''
    }
    if ($process.ExitCode -ne $ExpectedExitCode) {
        throw @"
Runner exit mismatch.
Expected: $ExpectedExitCode
Actual:   $($process.ExitCode)
Arguments: $($Arguments -join ' ')
STDOUT:
$stdout
STDERR:
$stderr
"@
    }
    return [pscustomobject]@{
        ExitCode = $process.ExitCode
        Stdout = $stdout
        Stderr = $stderr
    }
}

function Stop-QualificationDaemon {
    try {
        if (Test-Path -LiteralPath $Endpoint -PathType Leaf) {
            [void](Invoke-RunnerProcess -Arguments @(
                'stop',
                '-StateDir', $StateDir,
                '-TimeoutMs', '30000'
            ))
        }
    }
    catch {
        Write-Warning "Cleanup stop failed: $_"
    }
}

try {
    New-Item -ItemType Directory -Path $SourceRoot -Force | Out-Null
    [IO.File]::WriteAllText(
        (Join-Path $SourceRoot 'fixture.txt'),
        "alpha`nneedle beta`ngamma`n",
        [Text.UTF8Encoding]::new($false)
    )

    [void](Invoke-RunnerProcess -Arguments @(
        'start',
        '-StateDir', $StateDir,
        '-SourceRoot', $SourceRoot,
        '-TimeoutMs', '30000'
    ))

    $firstSearch = Invoke-RunnerProcess -Arguments @(
        'search',
        'needle',
        '-StateDir', $StateDir,
        '-Limit', '10',
        '-TimeoutMs', '30000'
    )
    if ($firstSearch.Stdout -notmatch 'matches=1') {
        throw "First search did not return the fixture: $($firstSearch.Stdout)"
    }
    if (-not (Test-Path -LiteralPath $RootConfig -PathType Leaf)) {
        throw 'The first start did not persist source-roots.json.'
    }

    [void](Invoke-RunnerProcess -Arguments @(
        'stop',
        '-StateDir', $StateDir,
        '-TimeoutMs', '30000'
    ))
    if (Test-Path -LiteralPath $Endpoint -or Test-Path -LiteralPath $Owner) {
        throw 'Orderly stop left runtime authority markers behind.'
    }

    # No -SourceRoot: the saved, bounded configuration must be loaded and
    # revalidated before the daemon starts.
    [void](Invoke-RunnerProcess -Arguments @(
        'start',
        '-StateDir', $StateDir,
        '-TimeoutMs', '30000'
    ))
    $secondSearch = Invoke-RunnerProcess -Arguments @(
        'search',
        'needle',
        '-StateDir', $StateDir,
        '-Limit', '10',
        '-TimeoutMs', '30000'
    )
    if ($secondSearch.Stdout -notmatch 'matches=1') {
        throw "Restarted search did not reuse saved roots: $($secondSearch.Stdout)"
    }
    [void](Invoke-RunnerProcess -Arguments @(
        'stop',
        '-StateDir', $StateDir,
        '-TimeoutMs', '30000'
    ))

    # Corrupt configuration is denied before spawning a daemon.
    [IO.File]::WriteAllText(
        $RootConfig,
        '{"schema":"WRONG","roots":[]}',
        [Text.UTF8Encoding]::new($false)
    )
    $failure = Invoke-RunnerProcess -Arguments @(
        'start',
        '-StateDir', $StateDir,
        '-TimeoutMs', '30000'
    ) -ExpectedExitCode 1
    if ($failure.Stderr -notmatch 'Unsupported source-root configuration schema') {
        throw "Corrupt configuration failed for the wrong reason: $($failure.Stderr)"
    }
    if (Test-Path -LiteralPath $Endpoint -or Test-Path -LiteralPath $Owner) {
        throw 'Invalid configuration created runtime authority markers.'
    }

    Write-Output 'Windows development runner qualification: PASS'
}
finally {
    Stop-QualificationDaemon
    Remove-Item -LiteralPath $Sandbox -Recurse -Force -ErrorAction SilentlyContinue
}
