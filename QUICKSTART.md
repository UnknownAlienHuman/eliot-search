# Quickstart status

There is no verified persistent-product command sequence yet. Do not present constructors, help text, feature
flags or historical self-tests as a working installation.

## Existing diagnostic one-shot scan

The source contains a narrow exact scan over bytes supplied for one process invocation. The current daemon binary
and routes have not been compiled or run at the current head, so treat the syntax as an implementation seam:

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

Even if successful, this route would not initialize a current root, retain an immutable revision, use Qdrant,
publish a view or establish the product.

## First persistent quickstart to implement

[#264](https://github.com/UnknownAlienHuman/eliot-search/issues/264) owns the first executable path. R049 selects
Qdrant Server/Rust client 1.20.x for the code cut; the repository lock still contains client 1.19.0.

```text
start/configure one external Qdrant 1.20 server
→ initialize/open one plain current-format root
→ explicitly select one supported local file
→ retain one immutable ObjectRef
→ install one current SourceRecord
→ prepare one result
→ create one fresh never-reused physical collection
→ create scoring full-text index with explicit code/text processing
→ insert_only complete points and exact readback
→ commit evidence/current and install PublishedView
→ execute one TextQuery
→ resolve a pinned retained excerpt
→ render typed CLI output
```

The first smoke may use one process, one explicit file and one physical collection. It must persist exact
root/object/source/prepared/view references so the next cut can verify restart without replacing the path.

Foundation must first execute the selected platform publication canary. On Windows, do not use path-based
`tempfile::persist_noclobber` as the trusted root/object finalizer; use the owner-private handle-based operation and
record its exact behavior.

After the smoke works:

1. restart/reopen and authoritative startup/explicit/periodic inventory;
2. complete fresh-generation convergence after edit/removal;
3. measure build latency, throughput, peak/retained disk and reader pressure for declared corpus classes;
4. implement point-level incrementality only where those budgets fail;
5. add literal Exact and one serviced live session;
6. add stopped-root maintenance and selected optional capabilities.

Encrypted storage, watcher, regex, PDF, MCP, models, overlay, managed Qdrant, custom sharding and Qdrant Edge are
not prerequisites. Edge may be tested later as an isolated replacement candidate, never as a dual fallback.

Replace this document with executed commands only after the default CLI path works. Record exact SHA, toolchain,
platform, Qdrant client/server/profile, commands, output and unrun cases in the
[analysis registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).
