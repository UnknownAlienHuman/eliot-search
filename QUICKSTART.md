# Current runnable smoke

ELIOT Search does not yet have a connected persistent quickstart. The only simple production search route
that can be run from a fresh checkout is the bounded one-shot scan over current bytes.

## Scan one file

`--scan-file` takes the query before the path:

```powershell
$Source = Join-Path $env:TEMP "eliot-scan.txt"
[System.IO.File]::WriteAllText(
  $Source,
  "alpha needle omega",
  [System.Text.UTF8Encoding]::new($false)
)

cargo +1.98.0 run --locked -p eliot-searchd -- --scan-file needle $Source
```

Use `--scan-file-ascii-insensitive QUERY FILE` for the explicit ASCII-insensitive variant.

## Scan stdin

```powershell
Get-Content -Raw $Source |
  cargo +1.98.0 run --locked -p eliot-searchd -- --scan-stdin needle
```

This checks only the current exact-scan path. It does not initialize a product root, retain a revision, use
Qdrant, survive source deletion or prove restart behavior.

## Why there is no persistent setup command yet

The daemon entrypoint is a chain of independent command applications rather than one product dispatcher:

```text
early legacy-effect refusal
→ direct registry/journal command interceptor
→ native root initialization/inspection
→ offline migration staging
→ legacy preparation
→ optional old loopback
→ read-only DIRECT service
→ another persistent-command application
→ shell/one-shot application
```

The first direct-registry interceptor recognizes `--serve-data-root`, `--search-root`, `--list-sources`,
`--read-revision`, `--sync-source-roots` and related names before the later services. For the read-looking
commands it restores/requalifies registry state and emits a `direct_registry` metadata result; it does not
execute the named search/read/service operation. Consequently the CLI's `serve-data-root` client waits for
`data_root_ready` from a later read-only service which that argv cannot reach.

Other commands are separately reachable—native root initialize/inspect, old preparation and offline migration
staging—but they do not form one source → immutable object → Qdrant → retained excerpt application. The
production read-only service source also refuses all mutation/preparation commands, and its mutating command
implementation is test-only.

The default daemon feature is empty `wave1-shell`; Qdrant/query dependencies are behind old wave features.
`full-baseline` compiles additional obsolete query/reclaimer/overlay/retention systems but does not repair
entrypoint routing or connect the product.

Issue [#264](https://github.com/UnknownAlienHuman/eliot-search/issues/264) owns the first real quickstart:
initialize one current-format root, retain one file, restart, publish/query through Qdrant from the CLI and
reopen the exact retained excerpt. Until that succeeds, use the one-shot scan only and treat persistent help,
`--self-test` capability flags and historical qualification as non-product evidence.
