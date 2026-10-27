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
→ retain/verify every directory handle from root to manifest parent
→ create exact owned initialization marker
→ create/flush/close/verify empty redb + object layout
→ create one unique manifest stage in final parent
→ write exact manifest + file-data flush
→ publish final manifest no-clobber last through tested platform owner
→ reopen final relative to retained parent and verify current layout
→ remove marker/stage debris where safe
→ return DataRootGuard
```

Foundation accepts no user source before the final manifest, so recovery is discard/retry rather than replay.

## Windows finalization — corrected R054 boundary

Rejected baseline paths:

1. `tempfile::persist_noclobber` / `MoveFileExW`, because publication is path-based;
2. the reachable legacy relative-`RootDirectory` Win32 wrapper in `registration_native::rename`;
3. private user-mode `NtSetInformationFile` or equivalent direct NT calls selected prematurely by R053.

Use one documented owner-private operation:

```text
opened stage handle with DELETE access
+ retained verified directory-handle chain from data root to final parent
  (share read/write; omit FILE_SHARE_DELETE)
+ canonical absolute final path derived from retained final-parent handle
→ SetFileInformationByHandle(FileRenameInfo)
   RootDirectory = NULL
   ReplaceIfExists = FALSE
→ reopen final relative to retained parent
→ compare stable file identity; verify non-reparse kind, length and complete manifest bytes
```

Holding every directory component without delete sharing prevents rename/delete replacement of the path chain while
the documented absolute-target operation runs. The implementation canary must cover:

- accepted extended absolute path form derived from the retained parent;
- stage DELETE access and exact share modes;
- successful absent destination and refusal of existing destination;
- ancestor/final-parent rename or replacement attempts while handles are retained;
- local same-volume filesystem support and unsupported-profile refusal;
- source/stage classification after failure;
- exact relative reopen, identity and byte readback;
- bounded content-free Win32 error mapping.

Official `FILE_RENAME_INFO` documentation describes a relative `RootDirectory`, but a current supported-build report
shows the wrapper may absolutize the name while retaining that handle. Do not use the relative form until a supported
platform canary proves it. Do not bypass the wrapper with private NT APIs.

If the documented absolute-target operation cannot be demonstrated for the advertised Windows/filesystem profile,
refuse initialization. There is no `MoveFileExW`, private-NT or generic-filesystem fallback.

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

Open retains the root/path-component handles, acquires one real exclusive guard, rereads/verifies current metadata,
then opens concrete stores. Shutdown stops admission, drains/cancels finite requests, stops publication and Qdrant
users, closes stores, then drops the guard. No `DRAINING/RELEASED` record.

No redb writer, view/window lock or Qdrant/object/process I/O is nested through the guard as a per-request mutex.

## Removal boundary

Remove owner leases/slots/succession, process/executable identities unrelated to the lock, health/drain receipts,
dual compatibility locks, `NativeRootRequest` argv/stage custody, initialization journal/resume and root-level
source/registry/DEK/migration/service wrappers.

Remove the legacy source-registration publication route and defective relative-root Win32 wrapper after useful
relative-open/identity code is moved. Do not retain its backup/phase/receipt protocol or its direct-NT experiment.

## Focused evidence

Run minimal Rust 1.98 check/Clippy for root/daemon/store consumers, then canaries for absent/current/incomplete/
foreign/busy, destination-exists refusal, ancestor/parent replacement, every initialization failure point, strict
discard, restart after crash and shutdown order. Record the exact platform and durability profile achieved.
