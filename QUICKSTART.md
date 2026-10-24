# Diagnostic one-shot scan — not the persistent-product quickstart

The source contains a narrow exact scan over bytes supplied for one process invocation. The current R031
compiler evidence covers `search-materializer` and `search-unitizer`, not the daemon binary or these commands.
Treat the examples below as existing syntax to verify, not as a reported successful run.

## Scan one file

The existing daemon syntax places the query before the path:

```powershell
$Source = Join-Path $env:TEMP "eliot-scan.txt"
[System.IO.File]::WriteAllText(
  $Source,
  "alpha needle omega",
  [System.Text.UTF8Encoding]::new($false)
)

cargo +1.98.0 run --locked -p eliot-searchd -- --scan-file needle $Source
```

The explicit ASCII-insensitive variant is:

```powershell
cargo +1.98.0 run --locked -p eliot-searchd -- --scan-file-ascii-insensitive needle $Source
```

## Scan stdin

```powershell
Get-Content -Raw $Source |
  cargo +1.98.0 run --locked -p eliot-searchd -- --scan-stdin needle
```

A successful invocation would exercise only the one-shot exact path over supplied bytes. It would not retain a
source revision, initialize a current-format root, use Qdrant, survive restart or establish indexed Search.

## Persistent product status

There is no verified persistent quickstart yet. The default application still has overlapping legacy command
routes and disconnected publication/query composition. Remaining `wave1`–`wave6` features select old fragments;
a feature combination is not a product profile.

[#264](https://github.com/UnknownAlienHuman/eliot-search/issues/264) owns the first real quickstart:

```text
initialize/open current root
→ select and retain one file
→ restart
→ publish/query through the actual Qdrant adapter
→ reopen the pinned retained excerpt from the CLI
```

Replace this document with executed persistent steps only after that path works. Until then, help text,
constructors, self-test names and historical qualification do not prove availability. See the
[analysis registry](docs/audit/ANALYSIS_REGISTRY.md#execution-evidence) for exact revision-specific evidence.
