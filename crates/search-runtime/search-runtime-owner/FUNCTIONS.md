# Root owner — one current-format data-root guard

This owner classifies, initializes and opens one current-format root. It owns process-lifetime exclusion and root
metadata. It does not own source membership, Qdrant, query sessions, encryption policy or a lifecycle ledger.

## Public operation

```text
inspect(path) -> RootInspection
initialize_plain(path, settings, RequestContext) -> DataRootGuard
open_existing(path, settings, RequestContext) -> DataRootGuard
explicit_discard_incomplete(path, RequestContext)
```

Help/version perform no root effects. `inspect` is read-only.

## Foundation layout and classification

The plain root contains only the final root manifest, minimal redb/control layout, immutable-object namespace and
owned initialization debris. Root metadata contains format/root identity, `storage_profile=plain`, and concrete
store schema versions—no source catalogue copy, owner epoch, operation receipt or secret state.

- no final manifest and no recognized owned marker/layout: `Uninitialized`;
- exact marker plus recognized empty layout, no final manifest: `IncompleteInitialization`;
- final manifest plus exact compatible layout: `Current`;
- final manifest with missing/conflicting layout: `Corrupt`;
- unsupported/foreign/pre-release layout: `Unsupported`;
- actual exclusive owner present: `Busy`.

Ordinary open/search/status never repairs, resumes, migrates or deletes.

## Final-manifest-last initialization

```text
validate selected local target
→ acquire initialization exclusion
→ retain/verify root and destination-directory handles
→ create exact owned initialization marker
→ create/flush/close/verify empty redb + object layout
→ create one unique manifest stage in the destination directory
→ write exact manifest + file-data flush
→ publish final manifest no-clobber last through the tested platform owner
→ reopen final relative to retained parent and verify current layout
→ remove marker/stage debris where safe
→ return DataRootGuard
```

Foundation accepts no user source before the final manifest, so recovery is discard/retry rather than replay.

## Windows finalization — corrected R053 boundary

Two currently available paths are explicitly **not** trusted finalizers:

1. `tempfile 3.27.0::persist_noclobber`, which uses path-based `SetFileAttributesW` and `MoveFileExW`;
2. `bins/eliot-searchd/.../registration_native.rs::rename`, which passes a relative leaf and non-null parent through
   the Win32 `SetFileInformationByHandle(FileRenameInfo)` wrapper.

The second path is not dead: `--register-source-root` and `--unregister-source-root` reach it through the current
secure-command interceptor. Its existence is a concrete removal/canary finding, not a reason to reuse or extend the
legacy source-registration platform.

The selected implementation-time candidate is one small owner-private **direct native** operation:

```text
opened stage handle with required rename/delete rights
+ retained verified destination-directory handle
+ bounded relative final leaf
→ exact native FILE_RENAME_INFORMATION/selected information buffer
   replacement disabled; RootDirectory=retained parent
→ direct native file-information call
→ require exact success status and completed IO_STATUS_BLOCK
→ reopen final relative to retained parent with the ordinary native reader
→ verify native identity, non-reparse kind, length and complete manifest bytes
```

The exact information class and buffer layout are selected by a focused code canary, not assumed from the Win32
wrapper. The canary must cover:

- ABI/offset/length calculations on supported architectures;
- required source access and share flags;
- destination absent success and destination present refusal;
- bounded relative leaf only; no absolute or parent traversal;
- retained-parent/path replacement cannot retarget the operation;
- same-volume/filesystem requirement and unsupported-volume refusal;
- source remains classifiable after a failed call;
- crash/failure debris and exact final readback;
- native status to bounded content-free error mapping.

If the native operation, deployment policy or filesystem profile cannot be demonstrated, that platform profile
refuses initialization. Do not silently fall back to `SetFileInformationByHandle`, `MoveFileExW`, path-only rename
or a generic filesystem framework.

File-data flush is separate from namespace persistence. Do not claim sudden-power-loss directory durability without
an executed primitive and crash evidence.

## Unix finalization

Use an owner-private no-replace operation relative to the retained directory when available. A link-then-unlink
fallback is acceptable only with same-filesystem validation, exact final verification and visible leftover-stage
handling. `std::fs::rename` is not a no-clobber primitive.

## Explicit discard of incomplete empty initialization

`discard-incomplete` runs only under actual exclusion and only when:

- no final manifest exists;
- the exact current-format marker is valid;
- only recognized empty control/object layout and owner-created stages exist;
- no source/preparation/published/pending/import reference exists;
- retained/opened objects still match inspected identities.

Unknown entries, replacement objects, conflicting bytes or uncertain deletion refuse. No baseline `resume`, phase
journal, operation digest, owner epoch or quarantine platform exists.

## Open and shutdown

Open retains the root handle, acquires one real exclusive guard, rereads/verifies current metadata through retained
handles, then opens concrete stores. Shutdown stops admission, drains/cancels finite requests, stops publication
and Qdrant users, closes stores, then drops the guard. No `DRAINING/RELEASED` record.

No redb writer, view/window lock or Qdrant/object/process I/O is nested through the guard as a per-request mutex.

## Removal boundary

Remove owner leases/slots/succession, process/executable identities unrelated to the lock, health/drain receipts,
dual compatibility locks, `NativeRootRequest` argv/stage custody, initialization journal/resume and root-level
source/registry/DEK/migration/service wrappers.

Specifically remove the legacy source-registration publication route, including the defective Win32 rename wrapper,
after any useful relative-open/identity helpers are moved into the tested current root/object owner. Do not retain
its backup/phase/receipt protocol merely to reuse one native call.

## Focused evidence

Run minimal Rust 1.98 check/Clippy for root/daemon/store consumers, then native canaries for absent/current/
incomplete/foreign/busy, destination-exists refusal, parent/path replacement, every initialization failure point,
strict discard, restart after crash and shutdown order. Record the exact platform and durability profile achieved.
