# Root owner — one current-format guard

This owner classifies, initializes and opens one root, holds process-lifetime exclusion, and owns the small
platform finalization seam shared with the object writer. It owns no source catalogue, query session or lifecycle
ledger. Target behavior below is not implemented/runtime evidence.

## Public operations

```text
inspect(path) -> RootInspection
initialize_plain(path, settings, ExecutionContext) -> DataRootGuard
open_existing(path, settings, ExecutionContext) -> DataRootGuard
explicit_discard_incomplete(path, ExecutionContext)
```

Use the [execution/query separation](../../../docs/implementation/README.md#minimal-cross-owner-values).
Root work needs deadline/cancellation and its own finite limits, never a query view or scoring profile.
Help/version perform no root effects; inspect is read-only.

## Classification

The plain layout contains a final root manifest, minimal control store, immutable-object namespace and recognizable
owned initialization debris. Root metadata binds format/root identity, plain profile and store schema versions.

| Observation | Classification |
|---|---|
| Absent or empty target without a final manifest/owned marker | Uninitialized |
| Exact current marker and recognized unfinished empty layout, no final manifest | IncompleteInitialization |
| Final manifest with exact compatible layout, no incomplete-import marker | Current |
| Final manifest plus recognized current-format incomplete-import marker (Stage 5) | IncompleteImport; normal open refuses before backend effects |
| Final manifest with missing/conflicting required layout | Corrupt |
| Unknown, foreign, unsafe or unsupported pre-release layout | Unsupported |
| Actual exclusive owner present | Busy |

Unknown entries are not an empty uninitialized root. A valid final manifest with leftover owned debris is not
permission to discard the committed root. Ordinary open/search/status never repairs, resumes, migrates or deletes.

## Final-manifest-last initialization

```text
validate target → acquire actual exclusion → retain/verify required path handles
→ exact owned initialization marker → create/flush/close/verify empty stores
→ unique manifest stage in final parent → write/flush exact bytes
→ platform no-clobber finalization → verified final manifest/layout
→ safe removal of owned marker/stage debris → DataRootGuard
```

No user source is admitted before the final manifest. Interruption before publication uses strict discard/retry,
not resume or phase replay. Resolve an uncertain finalization through exact final/marker/layout inspection;
never restart creation over a possibly committed root.

<a id="windows-finalization"></a>
<a id="windows-finalization--corrected-r054-boundary"></a>
## Windows finalization candidate

This is the single root/object Windows recipe. The selected candidate is documented Win32, but its ELIOT path
stability and durability remain unexecuted. [Upstream evidence and exact report](../../../docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md#plain-object-publication-donor)
are separate from support qualification.

```text
retain verified root→final-parent directory handles
  with FILE_SHARE_READ | FILE_SHARE_WRITE, without FILE_SHARE_DELETE
retain verified stage handle with DELETE access
establish a stable absolute destination namespace from the retained final parent
→ SetFileInformationByHandle(FileRenameInfo)
   RootDirectory=NULL; ReplaceIfExists=FALSE
→ reopen final through the verified parent relationship
→ compare stable identity, non-reparse kind, length and exact bytes
```

No-delete sharing constrains rename/delete access to the opened objects; it is not by itself proof that every
component of an absolute path is stable. The implementation must also account for mutable ancestors above the
data root and reparse/device-path resolution. Do not claim safety from the root→parent chain alone.

The focused canary must establish accepted absolute/extended path form, access/share flags, absent-target
success, existing-target refusal, stage/parent/ancestor replacement resistance, safe reopen, local same-volume
support, bounded errors and classifiable failure debris. Refuse an unsupported or unproven profile.

Do not use path-only MoveFileExW/tempfile publication, the legacy relative-RootDirectory wrapper, a private direct
NT finalizer or a generic filesystem framework as fallback. Reuse useful verified-open/identity code only after
tracing current callers; the legacy source-registration publication route is removed with its consumers.

File-data flush, namespace visibility, final readback and sudden-power-loss durability are separate claims.
Do not advertise the last one without executed evidence.

## Unix finalization

Use a private directory-relative no-replace operation where available. A same-filesystem link/unlink fallback
needs exact final verification and visible stage-debris handling. Ordinary overwriting rename is not no-clobber.

## Explicit discard

Discard requires actual exclusion, no final manifest, the exact current-format marker, only recognized empty
control/object layout and owned stages, no source/prepared/published/pending/import references, and matching
inspected/opened identities. Unknown entries, conflicting bytes, replacement or uncertain deletion refuse.
No baseline resume, initialization journal, operation digest, owner epoch or quarantine platform.

<a id="archive-and-import-boundary"></a>
## Archive and import boundary

[Maintenance](../search-retention/FUNCTIONS.md#backup-is-a-completed-archive-not-a-runnable-clone) owns archive
layout and completion. Neither a complete nor partial archive has a runnable root manifest. Ordinary root open
must refuse it before pending/cleanup recovery or backend access; archive readers never use normal root open.

When Stage 5 import is implemented, its empty destination is initialized normally under the exclusive guard.
Before importing data or issuing RPCs, persist one current-format incomplete-import marker bound to this root.
Normal open refuses it before backend work. Unknown/conflicting markers also refuse; absence is not enough if
layout validation fails. Explicit import retry uses the same private lock primitive, validates the marker under
exclusion, then opens destination-only owners; it does not bypass the refusal in normal open.
Clear it durably only after verified complete destination current/evidence state; an uncertain clear is resolved
by exact readback under the guard. This is not a phase journal, baseline-init resume or permission to discard
nonempty state. Add the marker and its open checks together with import, not as a Foundation prerequisite.

## Open and shutdown

Open retains required handles, acquires one actual exclusive guard, rereads/classifies current metadata, then
opens normal stores. Reject archives/incomplete imports before executing recovery or any external effect.
Shutdown stops admission, drains/cancels finite work, drops windows/views, stops publication/backend users, closes
stores, then drops the guard. No DRAINING/RELEASED record or per-request borrowing of the guard as a global mutex.

## Removal and checks

Remove owner leases/slots/succession, unrelated process/executable identities, health/drain receipts, compatibility
locks, argv/stage custody, initialization resume/journal and root-owned source/key/migration wrappers with callers.
Preserve user data and unpublished #598 overlap.

After code run minimal locked Rust 1.98 check/Clippy for root/daemon/store consumers. Focused cases cover
classification, all initialization interruption points, strict discard, exclusive ownership, documented platform
publication/replacement behavior, restart and shutdown order. Record exact platform and achieved durability.
