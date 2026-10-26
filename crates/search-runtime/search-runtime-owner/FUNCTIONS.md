# Root owner — one current-format data-root guard

This owner classifies, initializes and opens one current-format root. It owns the process-lifetime exclusion and
root metadata. It does not own source membership, Qdrant, query sessions, encryption policy or a lifecycle ledger.

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
owned initialization debris. Root metadata contains format/root identity, `storage_profile = plain`, and concrete
store schema versions—no source catalogue copy, owner epoch, operation receipt or secret state.

Classification remains exact:

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
→ acquire real initialization exclusion
→ retain/verify root and destination-directory handles
→ create exact owned initialization marker
→ create/flush/close/verify empty redb + object layout
→ create a unique final-manifest stage inside the root
→ write exact manifest + file-data flush
→ publish final manifest no-clobber last
→ reopen through the ordinary root reader and verify
→ remove marker/stage debris
→ return DataRootGuard
```

Foundation accepts no user source before the final manifest, so recovery is discard/retry rather than a replay
state machine.

## Platform finalization boundary

### Windows

Do not use `tempfile::persist_noclobber` as the trusted finalizer. The inspected 3.27.0 implementation changes
attributes by path and calls path-based `MoveFileExW`; it is not bound to the already-open stage handle or retained
root/destination-directory handle.

Implement one private Windows primitive inside the root/object owner:

```text
opened stage handle with required delete/rename rights
+ retained verified destination-directory handle
+ relative final leaf
→ SetFileInformationByHandle(FileRenameInfo/FileRenameInfoEx)
   with replace disabled
→ reopen final through retained root/directory
→ verify identity, bytes and manifest
```

Microsoft documents failure when destination exists and relative resolution from `RootDirectory`. Before adopting
this path, run a focused canary on the supported Windows/NTFS profile for structure layout, access/share flags,
relative-directory handling, existing-destination refusal, parent replacement, crash debris and exact readback.
If the exact handle-relative form is unsupported on a target, the profile refuses until a tested owner-private
alternative exists; do not silently fall back to a path-only rename.

`FlushFileBuffers` on the stage file supports file-data flush. Do not claim directory/sudden-power-loss durability
without an executed primitive and crash evidence.

### Unix

Use an owner-private no-replace operation relative to the retained directory when available. A link-then-unlink
fallback is acceptable only with same-filesystem validation, exact final verification and visible leftover-stage
handling. `std::fs::rename` is not a no-clobber primitive.

## Explicit discard of incomplete empty initialization

`discard-incomplete` runs only under actual exclusion and only when:

- no final manifest exists;
- the exact current-format marker is valid;
- only the recognized empty control/object layout and owner-created stages exist;
- no source/preparation/published/pending/import reference exists;
- retained/opened objects still match the inspected identities.

Unknown entries, replacement objects, conflicting bytes or uncertain deletion refuse. No baseline `resume`,
installation replay, phase journal, operation digest, owner epoch or quarantine platform exists.

## Open and shutdown

Open retains the root handle, acquires one real exclusive guard, rereads/verifies current metadata through retained
handles, then opens concrete stores. Shutdown stops admission, drains/cancels finite requests, stops publication
and Qdrant users, closes stores, then drops the guard. No `DRAINING/RELEASED` record.

No redb writer, view-manager/window lock or Qdrant/object/process I/O is nested through the guard as a per-request
mutex.

## Remove with callers

Remove owner leases/slots/succession, process/executable identities unrelated to the actual lock, health/drain
receipts, dual compatibility locks, `NativeRootRequest` argv/stage custody, initialization journal/resume and
root-level source/registry/DEK/migration/service wrappers. Preserve real path/locality/no-reparse checks, retained
handles, exact manifest classification and content-free diagnostics.

## Focused evidence

Run minimal Rust 1.98 check/Clippy for root/daemon/store consumers, then Windows and Unix canaries for absent/current/
incomplete/foreign/busy, destination-exists refusal, parent/path replacement, every initialization failure point,
strict discard, restart after crash and shutdown order. Record the exact durability profile actually achieved.
