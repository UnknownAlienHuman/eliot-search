# Quickstart status

There is no verified persistent-product command sequence yet. Do not present constructors, help text, feature
flags or historical self-tests as a working installation.

## Existing diagnostic one-shot scan

The source contains a narrow exact scan over bytes supplied for one process invocation. The daemon binary and
these routes have not been compiled or run at the current head, so treat the syntax as an implementation seam to
verify:

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

Even if executed successfully, this route would not initialize a current root, retain an immutable revision,
use Qdrant, publish a view or establish the product.

## First persistent quickstart to implement

[#264](https://github.com/UnknownAlienHuman/eliot-search/issues/264) owns the first real executable path:

```text
initialize/open one plain current-format root
→ explicitly select one supported local file
→ retain one immutable ObjectRef
→ install one current SourceRecord
→ prepare one result
→ build one fresh Qdrant generation
→ one one-shot Search
→ resolve a pinned retained excerpt
→ render typed CLI output
```

This first smoke may use one process and one explicit file. It must persist exact root/object/source/prepared/view
references so the next convergence cut can verify restart without replacing the path.

After the smoke works, add:

1. restart/reopen and lost-wakeup reconciliation;
2. selected-root inventory and incremental publication;
3. rebuild/compaction and whole-generation cleanup;
4. literal Exact and one serviced live session for Continue/Expand;
5. stopped-root maintenance and selected optional capabilities.

Encrypted storage, watcher, regex, PDF, MCP, models, overlay, managed Qdrant and a persistent Windows pipe are
not prerequisites.

Replace this document with executed commands only after the corresponding default CLI path works. Record exact
SHA, toolchain/platform, Qdrant client/server/profile, command output and unrun cases in the
[analysis registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).
