# Quickstart status

There is no verified persistent-product command sequence yet. Constructors, help text, feature flags and
historical self-tests are not a working installation.

## Existing diagnostic one-shot scan

The source contains a narrow exact scan over bytes supplied for one process invocation. The current daemon and
these routes have not been compiled/run at R048, so treat the syntax as an implementation seam to verify:

```powershell
$Source = Join-Path $env:TEMP "eliot-scan.txt"
[System.IO.File]::WriteAllText(
  $Source,
  "alpha needle omega",
  [System.Text.UTF8Encoding]::new($false)
)

cargo +1.98.0 run --locked -p eliot-searchd -- --scan-file needle $Source
cargo +1.98.0 run --locked -p eliot-searchd -- --scan-file-ascii-insensitive needle $Source
Get-Content -Raw $Source |
  cargo +1.98.0 run --locked -p eliot-searchd -- --scan-stdin needle
```

Even if successful, this route would not initialize a current root, retain a revision, use Qdrant, publish a view
or establish the product.

## First persistent quickstart to implement

[#264](https://github.com/UnknownAlienHuman/eliot-search/issues/264) owns:

```text
initialize/open one plain current root
→ explicitly select one supported local file
→ retain one immutable ObjectRef
→ install one current SourceRecord
→ prepare one result
→ build one fresh physical Qdrant generation with explicit insert_only
→ commit matching evidence/current state
→ install one PublishedView naming that physical collection
→ one Search
→ pinned retained excerpt
→ typed CLI output
```

This smoke may use one process and one file. It persists exact root/object/source/prepared/view references so the
next cut can reopen them without replacing the architecture. No Qdrant alias is required.

## Immediately after the smoke

### 3A — correctness and restart

```text
restart/reopen
→ startup/explicit/periodic authoritative inventory
→ desired state differs from published state
→ build a complete fresh never-reused physical collection
→ commit/install the matching view
→ preserve old pinned view
→ delete retired collection only after every guard is gone
```

Dropped notifications are repaired from current desired/published state. Incomplete inventory preserves prior
served data and reports a gap.

### 3B — optional optimization

Measure complete-rebuild cost first. Only if it is materially excessive, add stable occurrence reuse,
insert-only point additions, one fixed non-creating closure and compaction. Do not implement incremental mutation
merely because an issue describes it.

Then add literal Exact, one serviced live session/QueryWindow, stopped-root maintenance and selected optional
capabilities.

Encrypted storage, watcher, regex, PDF, MCP, models, overlay, managed Qdrant, aliases and a persistent Windows
pipe are not prerequisites.

Replace this document with executed commands only after the default CLI path works. Record exact SHA,
toolchain/platform, Qdrant client/server/profile, commands/results and unrun cases in the
[analysis registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).