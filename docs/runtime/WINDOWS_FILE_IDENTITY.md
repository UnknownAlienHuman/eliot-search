# Stable Windows file identity boundary

The pinned Rust 1.98.0 source marks `MetadataExt::volume_serial_number` and
`MetadataExt::file_index` as unstable `windows_by_handle`. The daemon's eight
retained call sites use one same-handle Win32 observation adapter instead.
Neither the toolchain nor the native identity encoding is changed.

## Scope and contract

`bins/eliot-searchd/src/native_file.rs` exposes safe observation values to this
package's separate binary/test crate targets through its internal library target.
Only `native_file/windows.rs` contains the ABI calls; existing caller
`forbid`/`deny(unsafe_code)` rules are not relaxed. No native HANDLE or vendor
struct crosses that ABI module, and no shared Search port or contract is widened.
No new Cargo package, dependency, crypto implementation, owner or index is added.

The adapter borrows an already-open `File`. It does not reopen a path, read source
bytes, issue a receipt, authorize access, acquire ownership or write state.
`GetFileType` must identify a disk object; `GetVolumeInformationByHandleW` must
return NTFS and the volume serial; `GetFileInformationByHandle` must return the
same serial, full file index and non-reparse attributes. Every native failure
is checked. Missing identity is never replaced by a zero or pathname.

The local `ntfs_file` profile in `docs/contracts/p00/SOURCE_GRAPH.md` retains its
existing identity byte layout: observed volume serial u32 big-endian, then both
file-index words as u64 big-endian. Legacy root-binding UTF-16 path bytes remain
unchanged. The previously absent DIRECT multipart hash API is a separate repair;
its explicit first framing definition is in [DIRECT_HASH_FORMAT.md](DIRECT_HASH_FORMAT.md).
Old public optional fields in sealed-reader receipts are preserved, but successful
NTFS observations fill them with actual `Some` values. ReFS/FAT/unsupported volumes
fail closed rather than silently reusing this legacy 64-bit representation. A new
filesystem/profile needs its own complete identity and compatibility decision.

Retained sites: primary `development.rs` and `direct_store.rs`; sealed file/root
readers, owner epochs, store and transaction readback; the sealed-direct test target.
The ninth historical site, `service_state.rs`, was not in a Rust module closure and
had no consumer except a source-text guard. It was removed during the PR #142 repair
rather than maintaining another unused lifecycle journal. The source guard retains
all eight live/harness sites; no native or process regression suite was deleted.
Transaction code matches `TryLockError` variants and passes its receipt magic
explicitly to `format!(concat!(...))`, preserving exact encoded receipt bytes.

## Verification and remaining boundary

Eight library tests cover identity encoding, closed errors, actual NTFS handle
reads, distinct equal-content files, rename/hardlink identity, locator replacement,
directories and non-disk rejection. Six require native Windows on NTFS. A separate
source guard covers the eight retained call sites; the Windows transaction harness
checks exact receipt bytes. The manual `core_tests` lane retains these tests.

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --locked -p eliot-searchd --lib --test stable_native_identity_api
cargo +1.98.0 test --locked -p eliot-searchd --all-targets
```

Compilation and native execution were not available in the original authoring
environment. Source changes are not a green Windows build or accepted T03 gate.
The ABI layout assertions and all native fixtures still require execution.

This does not close T07 final-handle containment/ancestor races or ACL admission,
T08 durable root ownership, T11 primary redb cutover, durable preparation or live
Qdrant. File IDs can be reused after deletion: observation alone is not lifetime
source-identity evidence. No qualification or launch state is changed here.

## Primary references

- Rust 1.98.0: https://github.com/rust-lang/rust/blob/1.98.0/library/std/src/os/windows/fs.rs
- Win32 handle information: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/ns-fileapi-by_handle_file_information
- Volume by the same handle: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationbyhandlew
- Handle type: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfiletype
