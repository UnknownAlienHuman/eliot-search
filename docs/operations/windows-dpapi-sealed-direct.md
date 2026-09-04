# Windows DPAPI sealed direct path

This is the first executable encrypted-at-rest direct path in the repository:

```text
same-handle file read
  → durable operation intent
  → Windows CurrentUser DPAPI protection
  → immutable no-replace publication
  → exact envelope readback
  → decrypt-and-compare reconciliation
  → terminal receipt readback
  → fresh authenticated DPAPI verification
  → exact literal search over short-lived plaintext
```

It is intentionally reported as:

```json
{
  "sealed_object_backed": true,
  "catalog_bound": false,
  "owner_epoch_bound": false,
  "scope_bound": false,
  "production_ready": false
}
```

The remaining flags cannot become true until daemon composition binds the sealed object and transaction receipt to the authoritative source catalog, current owner epoch, scope/security fence, control transaction, and startup recovery state.

## Build

```powershell
cargo build --locked -p eliot-searchd --bin eliot-search-sealed-direct
```

## Ingest a file

```powershell
.\target\debug\eliot-search-sealed-direct.exe ingest-file `
  .\.eliot-search-data `
  ingest-operation-000001 `
  source-revision-000001 `
  .\README.md
```

Properties:

- reads at most 64 MiB;
- rejects symlink/reparse final objects;
- opens the final Windows object with `FILE_FLAG_OPEN_REPARSE_POINT`;
- compares native file identity, byte length, and modification state before and after reading;
- refuses empty plaintext and existing immutable object replacement;
- writes durable operation intent before the DPAPI object effect;
- reconciles a lost acknowledgement only by decrypting and comparing every plaintext byte;
- checks the terminal transaction receipt against a new authenticated sealed-object readback;
- serializes concurrent retries of one operation ID with an OS file lock.

## Inspect transaction recovery state

```powershell
.\target\debug\eliot-search-sealed-direct.exe transaction-status `
  .\.eliot-search-data `
  ingest-operation-000001
```

Possible states:

```text
ABSENT
PREPARED
COMMITTED
COMMITTED_CLEANUP_PENDING
CONFLICTED
```

`COMMITTED_CLEANUP_PENDING` is the recoverable crash window after terminal receipt publication but before obsolete intent removal. Repeating `ingest-file` with the same operation ID, object ID, and exact file bytes verifies both records and removes the stale intent.

## Verify a sealed object

```powershell
.\target\debug\eliot-search-sealed-direct.exe verify `
  .\.eliot-search-data `
  source-revision-000001
```

## Search

Case-sensitive:

```powershell
.\target\debug\eliot-search-sealed-direct.exe search `
  .\.eliot-search-data `
  source-revision-000001 `
  source
```

ASCII case-insensitive:

```powershell
.\target\debug\eliot-search-sealed-direct.exe search-ascii-insensitive `
  .\.eliot-search-data `
  source-revision-000001 `
  SOURCE
```

Results are emitted in deterministic increasing UTF-8 byte order with:

```text
byte_start
byte_end
zero-based line
zero-based byte column
```

Search is capped at 100,000 matches and reports `complete=false` with `match_limit_reached=true` when truncated.

## Current boundary

This path proves real Windows encryption, immutable local storage, exact retry reconciliation, and exact search over authenticated decrypted bytes. It does not yet prove:

- a catalog record binds the object to a `SourceId` and `SourceRevisionId`;
- the current data-root `OwnerEpoch` admitted the mutation and read;
- source policy, scope, purge, and access generations remain current;
- one redb control transaction owns intent/receipt/catalog state;
- startup scans and reconciles every prepared or cleanup-pending operation;
- the authenticated long-running daemon exposes this path instead of standalone operational binaries.
