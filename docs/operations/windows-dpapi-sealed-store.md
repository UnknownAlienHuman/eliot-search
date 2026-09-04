# Windows DPAPI sealed object store

`eliot-search-sealed-store` is the concrete Windows CurrentUser DPAPI adapter for immutable local revision or token objects.

It uses the Windows `CryptProtectData` / `CryptUnprotectData` API directly. No custom cipher, key derivation function, or persistent plaintext key is introduced.

## Build

```powershell
cargo build --locked -p eliot-searchd --bin eliot-search-sealed-store
```

The resulting binary is:

```text
target\debug\eliot-search-sealed-store.exe
```

## Store an immutable object

The data root must already exist and must not be a symlink, junction, mount-point reparse object, or another non-directory object.

```powershell
[IO.File]::ReadAllBytes("revision.bin") |
  Set-Content -AsByteStream -Path "$env:TEMP\revision-input.bin"

Get-Content -AsByteStream -Raw "$env:TEMP\revision-input.bin" |
  .\target\debug\eliot-search-sealed-store.exe put `
    .\.eliot-search-data `
    source-revision-000001
```

For arbitrary binary input, process redirection is preferable to text pipelines:

```powershell
cmd /c ".\target\debug\eliot-search-sealed-store.exe put .\.eliot-search-data source-revision-000001 < revision.bin"
```

The target is created under:

```text
DATA_ROOT\sealed-revisions\OBJECT_ID.els-dpapi
```

Creation is immutable. An existing object is never replaced.

## Verify without exposing plaintext

```powershell
.\target\debug\eliot-search-sealed-store.exe verify `
  .\.eliot-search-data `
  source-revision-000001
```

Verification performs strict envelope parsing, final-file identity/metadata readback, DPAPI authentication, identity-bound entropy verification, and plaintext-length verification. The successful JSON receipt contains counts and state only.

## Read plaintext

```powershell
cmd /c ".\target\debug\eliot-search-sealed-store.exe get .\.eliot-search-data source-revision-000001 > recovered.bin"
```

Plaintext is written only to stdout. The in-process plaintext allocation is non-cloneable, redacted in `Debug`, and explicitly overwritten before release.

## Delete

```powershell
.\target\debug\eliot-search-sealed-store.exe delete `
  .\.eliot-search-data `
  source-revision-000001
```

Deletion removes the directory entry and verifies absence. Its receipt deliberately reports:

```json
{
  "logical_delete_complete": true,
  "physical_erasure_guaranteed": false
}
```

DPAPI confidentiality does not prove physical erasure from SSD remapping, filesystem journals, snapshots, or backups.

## Security and lifecycle boundary

- Protection is bound to the current Windows user, not `LocalMachine`.
- The opaque object ID is supplied as DPAPI optional entropy. Moving ciphertext to another object ID fails authentication.
- Object IDs are bounded ASCII tokens and cannot contain path separators.
- Root, store directory, and final object reparse attributes are rejected.
- Writes use a private temporary file, `sync_all`, atomic no-replace hard-link publication, and exact post-publication readback.
- Reads are bounded to 66 MiB and compare native file identity and metadata before and after reading.
- This adapter closes local at-rest encryption for sealed objects. The daemon must still bind its revision catalog and startup recovery state to these receipts before declaring the complete retained-revision path production-ready.
