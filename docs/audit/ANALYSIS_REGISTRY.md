# Analysis registry

Continuation memory and evidence status. It records inspected facts, selected decisions, published changes and
unrun work; it is not architecture, permission or an implementation prerequisite.

<a id="resume"></a>
## Resume

**Checkpoint:** R053, 2026-10-26 — active Windows publication route traced; Win32 wrapper rejected.  
**Branch:** `codex/refactor-preparation-20261017`, PR #610.  
**Inspected ELIOT head:** `8942d823f5c52392ee42a8d62ae5181748091060` (R052).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

Read [docs/README](../README.md), the [implementation sequence](../implementation/README.md#delivery-sequence),
then only the affected owner recipe, current callers and owning issue.

R053 changes documentation only. Product remains unimplemented. Next implementation remains Foundation, but its
first Windows code slice is now an exact native-finalization canary rather than reuse of the reachable legacy Win32
wrapper.

## R053 inspected evidence

### Upstream platform material

Inspected:

- `tempfile 3.27.0` Windows `persist_noclobber`: path-based `SetFileAttributesW` + `MoveFileExW`;
- Microsoft `FILE_RENAME_INFO` / `SetFileInformationByHandle` / `NtSetInformationFile` documentation;
- native rename information semantics for replacement-disabled relative names;
- current reports and disassembly showing the Win32 wrapper absolutizes a relative destination while retaining
  non-null `RootDirectory`, producing an invalid parameter combination;
- Qdrant Rust client/server 1.20 material already recorded by R051/R052.

### ELIOT source and active route

At R052 head inspected:

- `bins/eliot-searchd/src/owner_composition/kernel/registration_native.rs`;
- `registration_mutation.rs`;
- `secure_commands/kernel/source_registration.rs`;
- `secure_commands/kernel/entry.rs`;
- root/object owner recipes and daemon map;
- issue #360 and current programme handoff.

The source already uses direct `NtCreateFile` for relative child opens. Its `registration_native::rename` then
constructs a rename information buffer and calls the Win32 `SetFileInformationByHandle` wrapper. The legacy
`--register-source-root`/`--unregister-source-root` command dispatch reaches that mutation path.

## Findings and decisions

### F1 — the selected R049/R052 Win32 call was not a safe implementation prescription

R049/R052 correctly rejected path-based `MoveFileExW`, but still told the implementer to use
`SetFileInformationByHandle(FileRenameInfo/FileRenameInfoEx)` with a non-null retained parent and relative leaf.
That exact Win32 wrapper form is not a demonstrated handle-relative operation on current Windows.

**Decision:** remove the Win32 wrapper from selected architecture. The supported candidate is a tiny owner-private
direct native file-information call over the opened source handle and retained parent handle, with exact native
status/`IO_STATUS_BLOCK` validation and ordinary relative reopen/readback.

The exact information class/buffer is chosen only after an executed canary; documentation does not pretend that
one struct layout is already qualified.

### F2 — the repository already contains the same defective pattern

`registration_native::rename` uses the exact unsafe wrapper shape. It is not isolated test code: current secure
command dispatch exposes source-root registration/unregistration and reaches it through `CatalogCustody::publish`.

**Decision:** treat this path as a current counterexample and removal target. Do not reuse it for Foundation or
objects, and do not strengthen the old backup/phase/custody protocol merely to save one native helper. Move only
verified relative-open/native-identity techniques into the current owner, then delete the legacy route and wrapper
with callers.

### F3 — direct native reuse can remain small

The repository already has an owner-private `ntdll` FFI seam and ABI assertions for `NtCreateFile`. A current
finalizer therefore does not require a platform framework, provider service or dependency inversion layer.

**Decision:** one current root/object platform module may own:

```text
relative create/open through retained parent
native no-replace finalization of opened stage
relative reopen/readback
native identity/kind verification
```

It owns no source registration, backup phases, receipts, migration or generic file API.

### F4 — the canary must be a falsifiable support boundary

Before Windows Foundation is advertised, execute:

- ABI/offset/buffer-length checks for supported architecture;
- exact access/share flags;
- absent-destination success and existing-destination refusal;
- relative leaf only and no traversal;
- parent/path replacement resistance;
- same-volume and unsupported-filesystem behavior;
- failed-call stage/final classification;
- crash debris and exact relative reopen/readback;
- bounded native status mapping;
- truthful file-data versus namespace/sudden-power durability claims.

Failure means the platform profile is unsupported. There is no Win32-wrapper/path-only fallback.

## Preserved R052 Qdrant decisions

Stage-2 physical Text index remains explicit:

```text
vectorless one-shard collection
retrieval_text Text index
Code tokenizer; lowercase=true
ascii_folding=false; phrase_matching=false; stemmer disabled
stopwords/min/max/nested absent
BM25 scoring
enable_hnsw=false
memory=Pinned
```

Query profile remains separate: k=1.2, b=0.75, finite/range validated, no threshold. Stage 2 creates no filter
indexes; query returns no vectors and only evidence-routing payload. Stage 3A selects filter/index residency
explicitly and measures it. The repository lock remains Qdrant client 1.19; the code cut upgrades all 1.20 pieces
together.

## Current implementation sequence

| Stage | Observable result | Primary owners |
|---|---|---|
| 1 Foundation | direct-native Windows finalization canary; root/plain settings; manifest-last initialize/open; one guard; status | #333/#266/#360 |
| 2 Walking skeleton | one file → ObjectRef → retrieval text → explicit Qdrant 1.20 Text index → TextQuery canary/query → retained CLI excerpt | #264 and vertical owners |
| 3A correctness/reference | restart; inventory; explicit filter/residency profile; complete generation/view; budget matrix | #270/#271/#331/#261/#300/#303 |
| 3B conditional optimization | source-local diff/identity/mutation only where 3A budgets fail | #259/#329/#261/#263 |
| 4 Exact/session | retained-source Exact and one framed live QueryWindow | #249/#275/#280/#332 |
| 5 Maintenance/depth | stopped-root maintenance and selected product leaves | #302/#244/#281/#227/#335 |

## Anti-bureaucracy and lock rules

- Issues are owners, not dependency order.
- One manager owns each vertical cut; parallel writes are disjoint.
- No hidden server defaults, backend abstraction, alias manager, custom-shard fusion, watcher ledger, init journal,
  query scheduler/tickets, lexical qualification receipts or operation archive.
- No new native abstraction layer: one private current root/object platform module only.
- No redb writer/view mutex/window mutation borrow spans object/Qdrant/process/model/native publication I/O.
- Complete build + schema/canary + durable evidence commit precede view installation.
- Last view drop only wakes cleanup.
- Current-caller search is local to code moved/deleted.
- Minimal Rust 1.98 check/Clippy follows production cuts; broad evidence follows connected behavior.

<a id="evidence-status"></a>
## Evidence status

| Area | Status |
|---|---|
| Active legacy Win32 rename route | CHECKED reachable by static caller trace |
| Win32 wrapper suitability for relative parent handle | REJECTED as selected finalizer |
| Direct native rename candidate | SELECTED CANARY, not implemented/executed |
| Existing direct `NtCreateFile` seam | CHECKED source, not qualified for new finalizer |
| Exact Qdrant physical text profile V1 | SELECTED, not compiled/executed |
| Qdrant client/server upgrade | NOT DONE; lock remains 1.19 |
| Compiler evidence | R031 materializer + unitizer only |
| Foundation/walking skeleton/current binaries | UNRUN |
| Windows durability/live Qdrant/restart/relevance/scale/end-to-end | UNRUN |

No static donor/source inspection proves correctness.

## Immutable prior records

- R001–R031: audit/removal/dependency work through `da382b66`.
- R032–R045: canonical documentation reconciliation through `af337ec7`.
- R046: implementation-readiness correction `1eeb4191`.
- R047: active-surface hostile verification `858d1e34`.
- R048: donor/analogue synthesis `fed10c1e`.
- R049: Qdrant 1.20/platform-finalization correction `9edd2c1d`.
- R050: active Qdrant-owner synchronization `a4c23110`.
- R051: exact Qdrant query/schema reconciliation `87c84c24`.
- R052: explicit Qdrant text-index resource/query semantics `8942d823`.
- R053: active Win32 wrapper trace and direct-native finalization correction.

## Next action

Implement the smallest direct-native Windows finalization canary in the current root/object owner without reusing
the legacy source-registration publication platform. Run minimal compiler/Clippy and the focused native cases. Then
complete Foundation, upgrade the exact Qdrant client/server/schema/query profile and implement #264.
