# Primary DIRECT development smoke

This uses the primary daemon, not the old snapshot/BM25 program. It is a
source-backed development smoke, **not a production or Qdrant deployment guide**.
Rust 1.98.0 is required. Compilation and native tests must pass first.

```powershell
cargo +1.98.0 build --release --locked -p eliot-searchd -p eliot-search --bins
if ($LASTEXITCODE -ne 0) { throw "Build failed" }

# Use a fresh disposable data root. Keep source files outside it.
$DataRoot = Join-Path $env:TEMP ("eliot-smoke-" + [guid]::NewGuid().ToString("N"))
$Source = Join-Path $env:TEMP ("eliot-source-" + [guid]::NewGuid().ToString("N") + ".txt")
New-Item -ItemType Directory -Path $DataRoot -ErrorAction Stop | Out-Null
[System.IO.File]::WriteAllText($Source, "alpha needle omega", [System.Text.UTF8Encoding]::new($false))
$Daemon = ".\target\release\eliot-searchd.exe"

& $Daemon --index-file $DataRoot $Source
if ($LASTEXITCODE -ne 0) { throw "Index failed; inspect state before retrying" }
& $Daemon --search-root $DataRoot needle
if ($LASTEXITCODE -ne 0) { throw "Search failed" }

# A separate process reopens the retained revision. Live source loss does not
# silently replace that revision with current-path bytes.
Remove-Item -LiteralPath $Source -ErrorAction Stop
& $Daemon --search-root $DataRoot needle
if ($LASTEXITCODE -ne 0) { throw "Retained revision read failed" }
```

One-shot commands acquire the data-root owner themselves. Do not run them against
a root already held by `--serve-data-root` or another daemon. Live root management
through the final provider protocol remains unfinished.

For the current internal line-protocol service:

```powershell
& $Daemon --serve-data-root $DataRoot
```

Enter `version`, `health`, or `shutdown`, one per line. This is a diagnostic stdio
surface, not the final authenticated provider protocol. After a mutation failure
with `SERVICE_MUTATION_OUTCOME_UNKNOWN`, do not resend the mutation automatically.
The service exits; investigate and verify storage before restarting. No automated
repair or rollback is implied.

Legacy snapshot and sealed CLIs are no longer runnable Cargo binary/example
targets. Their regression harnesses remain available via `cargo test --test`,
including `cargo +1.98.0 test --locked -p eliot-searchd --test eliot-search-sealed-recover`.
Keep this disposable smoke directory until its results have been inspected; this
guide deliberately performs no automatic recursive data deletion.
