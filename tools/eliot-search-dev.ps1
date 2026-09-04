[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('start', 'health', 'status', 'search', 'stop')]
    [string]$Command = 'health',

    [Parameter(Position = 1)]
    [string]$Query,

    [string[]]$SourceRoot,

    [string]$StateDir = $(
        if ($env:LOCALAPPDATA) {
            Join-Path $env:LOCALAPPDATA 'EliotSearch\dev'
        }
        else {
            Join-Path ([IO.Path]::GetTempPath()) 'EliotSearch\dev'
        }
    ),

    [ValidateRange(1, 25)]
    [int]$Limit = 20,

    [ValidateRange(100, 300000)]
    [int]$TimeoutMs = 10000,

    [switch]$Rebuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RepositoryRoot = Split-Path -Parent $PSScriptRoot
$StateDir = [IO.Path]::GetFullPath($StateDir)
$TokenFile = Join-Path $StateDir 'auth.token'
$EndpointFile = Join-Path $StateDir 'endpoint.txt'
$OwnerFile = Join-Path $StateDir 'owner.lock'
$StdoutFile = Join-Path $StateDir 'daemon.stdout.log'
$StderrFile = Join-Path $StateDir 'daemon.stderr.log'
$ExecutableSuffix = if ($IsWindows -or $env:OS -eq 'Windows_NT') { '.exe' } else { '' }
$Daemon = Join-Path $RepositoryRoot "target\debug\eliot-searchd$ExecutableSuffix"
$Client = Join-Path $RepositoryRoot "target\debug\eliot-search$ExecutableSuffix"

function Assert-PlainPath {
    param([Parameter(Mandatory)][string]$Path)

    if (Test-Path -LiteralPath $Path) {
        $item = Get-Item -LiteralPath $Path -Force
        if ($item.LinkType -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Refusing reparse/symlink path: $Path"
        }
    }
}

function Ensure-Binaries {
    if ($Rebuild -or -not (Test-Path -LiteralPath $Daemon) -or -not (Test-Path -LiteralPath $Client)) {
        Push-Location $RepositoryRoot
        try {
            & cargo build --locked -p eliot-searchd -p eliot-search
            if ($LASTEXITCODE -ne 0) {
                throw "cargo build failed with exit code $LASTEXITCODE"
            }
        }
        finally {
            Pop-Location
        }
    }

    if (-not (Test-Path -LiteralPath $Daemon) -or -not (Test-Path -LiteralPath $Client)) {
        throw 'Expected daemon/client binaries were not produced.'
    }
}

function Ensure-Token {
    New-Item -ItemType Directory -Path $StateDir -Force | Out-Null
    Assert-PlainPath -Path $StateDir
    Assert-PlainPath -Path $TokenFile

    if (Test-Path -LiteralPath $TokenFile) {
        $length = (Get-Item -LiteralPath $TokenFile).Length
        if ($length -lt 32 -or $length -gt 4096) {
            throw 'Existing auth token file has an invalid bounded length.'
        }
        return
    }

    $bytes = [byte[]]::new(32)
    try {
        [Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
        $token = [Convert]::ToHexString($bytes).ToLowerInvariant()
        [IO.File]::WriteAllText(
            $TokenFile,
            $token,
            [Text.UTF8Encoding]::new($false)
        )
    }
    finally {
        [Array]::Clear($bytes, 0, $bytes.Length)
        $token = $null
    }
}

function Invoke-Client {
    param(
        [Parameter(Mandatory)][string]$ClientCommand,
        [string[]]$AdditionalArguments = @()
    )

    Ensure-Binaries
    Assert-PlainPath -Path $StateDir
    Assert-PlainPath -Path $TokenFile
    Assert-PlainPath -Path $EndpointFile

    $arguments = @(
        $ClientCommand,
        '--state-dir', $StateDir,
        '--auth-token-file', $TokenFile,
        '--timeout-ms', [string]$TimeoutMs
    ) + $AdditionalArguments

    & $Client @arguments
    return $LASTEXITCODE
}

function Wait-Endpoint {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        if (Test-Path -LiteralPath $EndpointFile) {
            return
        }
        if (Test-Path -LiteralPath $StderrFile) {
            $errorText = Get-Content -LiteralPath $StderrFile -Raw -ErrorAction SilentlyContinue
            if ($errorText) {
                throw "Daemon failed before publishing the endpoint: $errorText"
            }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)

    throw "Daemon did not publish $EndpointFile within ${TimeoutMs}ms."
}

switch ($Command) {
    'start' {
        if (-not $SourceRoot -or $SourceRoot.Count -eq 0) {
            throw 'start requires at least one -SourceRoot.'
        }
        if ($SourceRoot.Count -gt 32) {
            throw 'At most 32 source roots are supported.'
        }
        if (Test-Path -LiteralPath $OwnerFile) {
            throw "Owner marker already exists: $OwnerFile. Use status/stop; do not delete it blindly."
        }

        Ensure-Binaries
        Ensure-Token
        New-Item -ItemType Directory -Path $StateDir -Force | Out-Null

        $arguments = @(
            'serve',
            '--state-dir', $StateDir,
            '--auth-token-file', $TokenFile,
            '--bind', '127.0.0.1:0'
        )
        foreach ($root in $SourceRoot) {
            $canonicalRoot = [IO.Path]::GetFullPath($root)
            Assert-PlainPath -Path $canonicalRoot
            if (-not (Test-Path -LiteralPath $canonicalRoot -PathType Container)) {
                throw "Source root is not a directory: $canonicalRoot"
            }
            $arguments += @('--source-root', $canonicalRoot)
        }

        Remove-Item -LiteralPath $StdoutFile, $StderrFile -Force -ErrorAction SilentlyContinue
        $process = Start-Process \
            -FilePath $Daemon \
            -ArgumentList $arguments \
            -WorkingDirectory $RepositoryRoot \
            -RedirectStandardOutput $StdoutFile \
            -RedirectStandardError $StderrFile \
            -WindowStyle Hidden \
            -PassThru

        Wait-Endpoint
        $exit = Invoke-Client -ClientCommand 'health'
        if ($exit -ne 0) {
            throw "Daemon started as PID $($process.Id), but health failed with exit code $exit."
        }
        Write-Host "ELIOT Search started. PID=$($process.Id); state=$StateDir"
    }

    'health' {
        exit (Invoke-Client -ClientCommand 'health')
    }

    'status' {
        exit (Invoke-Client -ClientCommand 'status')
    }

    'search' {
        if ([string]::IsNullOrWhiteSpace($Query)) {
            throw 'search requires a non-empty positional Query.'
        }
        exit (Invoke-Client -ClientCommand 'search' -AdditionalArguments @($Query, '--limit', [string]$Limit))
    }

    'stop' {
        $exit = Invoke-Client -ClientCommand 'shutdown'
        if ($exit -ne 0) {
            exit $exit
        }
        $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
        do {
            if (-not (Test-Path -LiteralPath $EndpointFile) -and -not (Test-Path -LiteralPath $OwnerFile)) {
                Write-Host 'ELIOT Search stopped cleanly.'
                exit 0
            }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        throw 'Shutdown was acknowledged, but runtime markers were not removed before the deadline.'
    }
}
