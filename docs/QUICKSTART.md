# ELIOT Search — development quick start

The current executable provides a bounded authenticated **DIRECT development search** over explicitly supplied local source roots. Indexed/Qdrant retrieval is not yet advertised as ready.

## Windows

Requirements:

- Rust `1.98.0` with the MSVC target;
- PowerShell 7 or Windows PowerShell with .NET cryptographic RNG support;
- a local directory to search.

From the repository root:

```powershell
# Build, create a cryptographically random local token and start the daemon.
./tools/eliot-search-dev.ps1 start -SourceRoot 'C:\path\to\documents'

# Verify the authenticated endpoint.
./tools/eliot-search-dev.ps1 health
./tools/eliot-search-dev.ps1 status

# Bounded DIRECT search. Limit is 1..25.
./tools/eliot-search-dev.ps1 search 'needle' -Limit 20

# Graceful endpoint shutdown and owner-marker cleanup.
./tools/eliot-search-dev.ps1 stop
```

Multiple roots are explicit:

```powershell
./tools/eliot-search-dev.ps1 start -SourceRoot @(
  'C:\work\project-a',
  'D:\notes'
)
```

The default runtime directory is:

```text
%LOCALAPPDATA%\EliotSearch\dev
```

It contains:

- `auth.token` — local bearer token; never printed by the runner;
- `endpoint.txt` — token-free loopback endpoint descriptor;
- `owner.lock` — exclusive live-owner marker;
- bounded daemon stdout/stderr logs.

Do not delete `owner.lock` while a daemon may be alive. The current development shell fails closed on a stale marker instead of guessing that ownership is safe to steal.

## Direct binaries

Build:

```powershell
cargo build --locked -p eliot-searchd -p eliot-search
```

Create a secure token file:

```powershell
$bytes = [byte[]]::new(32)
[Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
$token = [Convert]::ToHexString($bytes).ToLowerInvariant()
New-Item -ItemType Directory -Force .eliot-search | Out-Null
[IO.File]::WriteAllText(
  (Resolve-Path .eliot-search).Path + '\auth.token',
  $token,
  [Text.UTF8Encoding]::new($false)
)
[Array]::Clear($bytes, 0, $bytes.Length)
$token = $null
```

Start:

```powershell
./target/debug/eliot-searchd.exe serve `
  --state-dir .eliot-search `
  --auth-token-file .eliot-search/auth.token `
  --source-root 'C:\path\to\documents'
```

In another terminal:

```powershell
./target/debug/eliot-search.exe health `
  --state-dir .eliot-search `
  --auth-token-file .eliot-search/auth.token

./target/debug/eliot-search.exe search 'needle' --limit 20 `
  --state-dir .eliot-search `
  --auth-token-file .eliot-search/auth.token

./target/debug/eliot-search.exe shutdown `
  --state-dir .eliot-search `
  --auth-token-file .eliot-search/auth.token
```

## Current DIRECT limits

The daemon currently enforces:

- loopback transport only;
- at most 32 source roots;
- at most 32 directory levels;
- at most 100,000 files per request;
- at most 8 MiB per file;
- at most 512 MiB read per request;
- UTF-8 text files only;
- at most 4 KiB per query;
- at most 25 returned results;
- no symlink traversal;
- explicit `complete` or `partial` coverage with a gap count.

This mode performs a bounded scan and does not claim indexed, semantic, document-provider or Qdrant readiness.
