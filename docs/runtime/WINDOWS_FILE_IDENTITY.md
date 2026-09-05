# Stable Windows file identity boundary

The pinned Rust 1.98.0 source marks `MetadataExt::volume_serial_number` and
`MetadataExt::file_index` as unstable `windows_by_handle`. The daemon's known
nine call sites now use one same-handle Win32 observation adapter instead.
Neither the toolchain nor the persisted digest algorithms are changed.

## Scope and contract

`bins/eliot-searchd/src/native_file.rs` exposes safe observation values to this
package's separate binary/test crate targets through its internal library target.
Only `native_file/windows.rs` contains the reviewed ABI calls; existing caller
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
file-index words as u64 big-endian. Source/revision hash domains and legacy
root-binding UTF-16 path bytes remain unchanged. Old public optional fields in
sealed-reader receipts are preserved, but successful NTFS observations now fill
them with actual `Some` values. ReFS/FAT/unsupported volumes fail closed rather
than silently reusing this legacy 64-bit representation. A new filesystem/profile
would need its own complete identity and explicit compatibility decision.

Replaced sites: primary `development.rs` and `direct_store.rs`; sealed file/root
readers, owner epochs, store and transaction readback; the retained sealed-direct
test target; and the old `service_state.rs` source. Updating that old source does
not activate it or create a second lifecycle owner. Test-only legacy targets
remain test-only. Transaction code also matches `TryLockError` variants correctly
and passes its receipt magic explicitly to `format!(concat!(...))`, preserving
exact encoded receipt bytes.

## Verification and remaining boundary

Eight library tests cover identity encoding, closed errors, actual NTFS handle
reads, distinct equal-content files, rename/hardlink identity, locator replacement,
directories and non-disk rejection. Six require native Windows on NTFS. A separate
source guard covers all nine known call sites; the retained Windows transaction
harness checks exact receipt bytes. The manual `core_tests` lane includes the new
library/source guard and retains every previous harness/process suite.

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --locked -p eliot-searchd --lib --test stable_native_identity_api
cargo +1.98.0 test --locked -p eliot-searchd --all-targets
```

Compilation, Rust tests and native execution were not available in the authoring
environment. Source changes are not a green Windows build or accepted T03 gate.
The ABI declarations include layout assertions; those still require compilation.

This does not close T07 final-handle containment/ancestor races or ACL admission,
T08 durable root ownership, T11 primary redb cutover, durable canonical preparation
or live Qdrant. Those existing architectural gaps remain open; this adapter must
not be mistaken for their implementation. File IDs can be reused after deletion,
so this observation alone is not lifetime source-identity evidence either.

## Primary references

- Rust 1.98.0: https://github.com/rust-lang/rust/blob/1.98.0/library/std/src/os/windows/fs.rs
- Win32 handle information and full NTFS index: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/ns-fileapi-by_handle_file_information
- Volume by the same file handle: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationbyhandlew
- Handle type: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfiletype

No qualification flag, launch state, architecture text or accepted receipt is
changed by this implementation increment. Writes land directly on main at the
repository owner's request; unexecuted verification is still unexecuted.
