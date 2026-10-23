# One-shot scan example — current build unverified

The source contains a narrow one-shot exact-scan route. This example was not executed on the latest refactor
HEAD. An earlier broad daemon check failed; successful checks of individual libraries do not establish that
this checkout builds or that this command currently succeeds. See the [analysis registry](docs/audit/ANALYSIS_REGISTRY.md)
for revision-specific evidence. There is no connected persistent-product quickstart yet.

## Scan one file

After the daemon builds with the pinned toolchain, the existing command syntax is query before path:

```powershell
$Source = Join-Path $env:TEMP "eliot-scan.txt"
[System.IO.File]::WriteAllText(
  $Source,
  "alpha needle omega",
  [System.Text.UTF8Encoding]::new($false)
)

cargo +1.98.0 run --locked -p eliot-searchd -- --scan-file needle $Source
```

The explicit ASCII-insensitive variant is `--scan-file-ascii-insensitive QUERY FILE`.

## Scan stdin

```powershell
Get-Content -Raw $Source |
  cargo +1.98.0 run --locked -p eliot-searchd -- --scan-stdin needle
```

A successful run would exercise only that exact-scan path over supplied bytes. It would not initialize a
persistent root, retain a revision, use Qdrant, survive source deletion or establish restart behavior.

## Persistent product blocker

The remaining daemon entrypoint still passes commands through independent registry, native-root, migration,
preparation, DIRECT-service and shell applications. The inspected registry interceptor claims search/read/service
names before their later handlers. The advertised persistent CLI client therefore is not a connected
source → retained object → Qdrant → source-backed result path. The
[daemon map](bins/eliot-searchd/FUNCTIONS.md) records the exact routing problem.

R016/R019 removed standalone snapshot/sealed applications and the daemon loopback proxy. R020 removes the empty
retention package and `wave7-lifecycle`/`full-baseline`. Remaining `wave1`–`wave6` features do not repair routing;
the default is still the empty `wave1-shell`. Do not restore old aliases or treat a feature combination as a
finished application.

Issue [#264](https://github.com/UnknownAlienHuman/eliot-search/issues/264) owns the real persistent quickstart:
initialize a current-format root, retain one file, restart, publish/query through the actual Qdrant adapter and
reopen the retained excerpt from the CLI. Until that path is implemented and executed, help text, constructor
checks, self-test flags and historical qualification are not evidence that it works.
