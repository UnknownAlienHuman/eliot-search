# Current runnable smoke

The public source does **not** yet expose the persistent Search product path. The only production-dispatched
search operation is a bounded one-shot scan over current bytes. It does not retain a revision, use Qdrant,
survive source deletion or prove restart behavior.

Rust 1.98.0 is selected by the repository.

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

This is useful for checking the current bounded exact-scan path only.

## Persistent commands are not a smoke test

The production dispatcher currently routes persistent initialization, indexing, retained-revision reads,
root search, serving and source-root registration to `LEGACY_EFFECTS_UNAVAILABLE`; the old persistent
handlers are test-only or disconnected. `--help` still lists some of those commands and `--self-test`
currently hard-codes `persistent_direct_store_available=true`. Neither output is capability evidence.

Do not use these commands as a setup guide until their real handlers replace the unavailable branches:

```text
--initialize-data-root
--index-file / --index-directory
--search-root / --read-revision / --list-sources / --verify-root
--serve-data-root
--register-source-root / --sync-source-roots
```

Issue [#264](https://github.com/UnknownAlienHuman/eliot-search/issues/264) owns the first real product run:
initialize one current-format root, retain one file, restart, publish/query through Qdrant from the CLI and
reopen the exact retained excerpt. Until that succeeds, the repository has a one-shot development scan—not a
working persistent Search quickstart.
